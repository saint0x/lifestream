#!/usr/bin/env python3

import argparse
import hashlib
import json
import os
import sqlite3
import statistics
import threading
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[3]
BACKEND_ROOT = ROOT / "backend"
DB_PATH = BACKEND_ROOT / "vanta.db"
BASE_URL = os.environ.get("VANTA_BASE_URL", "http://127.0.0.1:8080")
SCOPE_SET = ["user", "creator", "creator:write", "admin"]


@dataclass(frozen=True)
class CreatorConfig:
    creator_id: str
    user_id: str
    handle: str
    token: str


@dataclass
class Sample:
    stage: str
    latency_ms: float
    status: int


@dataclass
class WorkerResult:
    creator_handle: str
    iterations: int = 0
    failures: list[str] = field(default_factory=list)
    samples: list[Sample] = field(default_factory=list)


CREATORS = [
    CreatorConfig(
        creator_id="crt-deepsaint",
        user_id="usr-1",
        handle="deepsaint",
        token="vanta-local-dev-token",
    ),
    CreatorConfig(
        creator_id="crt-atlas",
        user_id="usr-2",
        handle="atlas_codes",
        token="vanta-local-atlas-token",
    ),
]

PRINT_LOCK = threading.Lock()


def log(message: str) -> None:
    with PRINT_LOCK:
        print(message, flush=True)


def req(
    path: str,
    bearer_token: str | None = None,
    method: str = "GET",
    body: Any | None = None,
    headers: dict[str, str] | None = None,
) -> tuple[int, Any, float]:
    request_headers = dict(headers or {})
    if bearer_token is not None:
        request_headers["Authorization"] = f"Bearer {bearer_token}"
    data = None
    if body is not None:
        data = json.dumps(body).encode()
        request_headers.setdefault("Content-Type", "application/json")
    request = urllib.request.Request(
        BASE_URL + path,
        data=data,
        headers=request_headers,
        method=method,
    )
    started_at = time.perf_counter()
    try:
        with urllib.request.urlopen(request) as response:
            raw = response.read().decode()
            latency_ms = (time.perf_counter() - started_at) * 1000.0
            return response.status, json.loads(raw) if raw else None, latency_ms
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        latency_ms = (time.perf_counter() - started_at) * 1000.0
        return exc.code, json.loads(raw) if raw else None, latency_ms


def ensure_auth_sessions() -> None:
    now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    with sqlite3.connect(DB_PATH) as conn:
        for creator in CREATORS:
            conn.execute(
                """
                INSERT OR REPLACE INTO auth_sessions (
                    id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
                ) VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, ?)
                """,
                (
                    f"sess-load-{creator.handle}",
                    creator.user_id,
                    f"load-{creator.handle}",
                    hashlib.sha256(creator.token.encode()).hexdigest(),
                    json.dumps(SCOPE_SET),
                    now,
                    now,
                ),
            )
        conn.commit()


def repair_creator_state(creator: CreatorConfig) -> None:
    timestamp = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    with sqlite3.connect(DB_PATH) as conn:
        conn.execute(
            """
            UPDATE broadcasts
            SET status = 'ended',
                ended_at = COALESCE(ended_at, ?),
                duration_sec = COALESCE(duration_sec, 0)
            WHERE creator_id = ?
              AND status IN ('ready', 'live')
            """,
            (timestamp, creator.creator_id),
        )
        conn.execute(
            """
            UPDATE live_ingest_sessions
            SET status = CASE
                    WHEN status = 'terminated' THEN status
                    ELSE 'ended'
                END,
                contribution_state = 'disconnected',
                disconnected_at = COALESCE(disconnected_at, ?),
                last_heartbeat_at = COALESCE(last_heartbeat_at, ?)
            WHERE creator_id = ?
              AND status IN ('connected', 'stale', 'ready', 'live', 'ended')
            """,
            (timestamp, timestamp, creator.creator_id),
        )
        conn.execute(
            """
            UPDATE creator_profiles
            SET live_status = 'offline',
                current_broadcast_id = NULL
            WHERE id = ?
            """,
            (creator.creator_id,),
        )
        conn.commit()


def cleanup_active_broadcast(creator: CreatorConfig) -> None:
    status, live, _ = req("/api/v1/creator/me/live", bearer_token=creator.token)
    if status != 200:
        raise RuntimeError(f"{creator.handle}: failed to fetch live snapshot: {status} {live}")
    current = live.get("currentBroadcast") or live.get("pendingBroadcast")
    if current is not None:
        ended_status, ended, _ = req(
            f"/api/v1/creator/me/broadcasts/{current['id']}/end",
            bearer_token=creator.token,
            method="POST",
        )
        if ended_status != 200:
            repair_creator_state(creator)
    else:
        repair_creator_state(creator)


def start_broadcast(creator: CreatorConfig, iteration: int) -> tuple[dict[str, Any], Sample]:
    status, payload, latency_ms = req(
        "/api/v1/creator/me/broadcasts/start",
        bearer_token=creator.token,
        method="POST",
        body={
            "title": f"Ingest stress {creator.handle} {iteration}",
            "category": "Tech",
            "tags": ["stress", "ingest", creator.handle],
            "isMature": False,
            "notifyFollowers": False,
        },
    )
    if status == 400 and payload == {"error": "bad request: an active or pending broadcast already exists"}:
        cleanup_active_broadcast(creator)
        status, payload, latency_ms = req(
            "/api/v1/creator/me/broadcasts/start",
            bearer_token=creator.token,
            method="POST",
            body={
                "title": f"Ingest stress {creator.handle} {iteration}",
                "category": "Tech",
                "tags": ["stress", "ingest", creator.handle],
                "isMature": False,
                "notifyFollowers": False,
            },
        )
    if status != 200:
        raise RuntimeError(f"{creator.handle}: start broadcast failed: {status} {payload}")
    return payload, Sample("start_broadcast", latency_ms, status)


def worker_run(
    creator: CreatorConfig,
    iterations: int,
    heartbeats_per_iteration: int,
    heartbeat_sleep_ms: int,
) -> WorkerResult:
    result = WorkerResult(creator_handle=creator.handle)
    for iteration in range(1, iterations + 1):
        try:
            cleanup_active_broadcast(creator)
            broadcast, sample = start_broadcast(creator, iteration)
            result.samples.append(sample)

            status, live, latency_ms = req(
                "/api/v1/creator/me/live",
                bearer_token=creator.token,
            )
            result.samples.append(Sample("creator_live_snapshot", latency_ms, status))
            if status != 200:
                raise RuntimeError(f"{creator.handle}: live snapshot failed: {status} {live}")

            status, connected, latency_ms = req(
                "/api/v1/ingest/live/connect",
                method="POST",
                body={
                    "streamKey": live["profile"]["streamKey"],
                    "protocol": "rtmp",
                    "ingestServer": f"rtmp-{creator.handle}-stress",
                    "broadcastId": broadcast["id"],
                },
            )
            result.samples.append(Sample("connect", latency_ms, status))
            if status != 200:
                raise RuntimeError(f"{creator.handle}: connect failed: {status} {connected}")

            session = connected["session"]
            ingest_token = connected["ingestToken"]
            runtime_manifest = (
                f"live/{creator.creator_id}/{broadcast['id']}/{session['id']}/master.m3u8"
            )

            for heartbeat_index in range(heartbeats_per_iteration):
                status, payload, latency_ms = req(
                    f"/api/v1/ingest/live/{session['id']}/heartbeat",
                    method="POST",
                    body={
                        "bitrateKbps": 5400 + heartbeat_index * 100,
                        "viewers": 100 + heartbeat_index,
                        "droppedFrames": heartbeat_index % 3,
                        "cpuPercent": 30 + heartbeat_index,
                        "freeDiskGb": 512.0 - heartbeat_index,
                        "ingestLatencyMs": 90 + heartbeat_index,
                        "sourceProbe": {
                            "containerFormat": "mpegts",
                            "videoCodec": "h264",
                            "audioCodec": "aac",
                            "width": 1920,
                            "height": 1080,
                            "frameRate": 59.94,
                            "audioSampleRateHz": 48000,
                            "audioChannels": 2,
                        },
                    },
                    headers={"x-ingest-token": ingest_token},
                )
                result.samples.append(Sample("heartbeat", latency_ms, status))
                if status != 200:
                    raise RuntimeError(f"{creator.handle}: heartbeat failed: {status} {payload}")
                if heartbeat_sleep_ms > 0:
                    time.sleep(heartbeat_sleep_ms / 1000.0)

            status, payload, latency_ms = req(
                f"/api/v1/ingest/live/{session['id']}/runtime",
                method="POST",
                body={
                    "runtimeState": "healthy",
                    "packagingStatus": "ready",
                    "archiveStatus": "not_started",
                    "manifestRelativePath": runtime_manifest,
                    "archiveRelativePath": None,
                    "lastError": None,
                },
                headers={"x-ingest-token": ingest_token},
            )
            result.samples.append(Sample("runtime_report", latency_ms, status))
            if status != 200:
                raise RuntimeError(f"{creator.handle}: runtime report failed: {status} {payload}")

            status, payload, latency_ms = req(
                f"/api/v1/ingest/live/{session['id']}/disconnect",
                method="POST",
                headers={"x-ingest-token": ingest_token},
            )
            result.samples.append(Sample("disconnect", latency_ms, status))
            if status != 200:
                raise RuntimeError(f"{creator.handle}: disconnect failed: {status} {payload}")

            status, payload, latency_ms = req(
                f"/api/v1/creator/me/broadcasts/{broadcast['id']}/end",
                bearer_token=creator.token,
                method="POST",
            )
            result.samples.append(Sample("end_broadcast", latency_ms, status))
            if status == 400 and payload == {
                "error": "bad request: broadcast is not the creator's active or pending broadcast"
            }:
                status, live_after_disconnect, latency_ms = req(
                    "/api/v1/creator/me/live",
                    bearer_token=creator.token,
                )
                result.samples.append(Sample("creator_live_post_disconnect", latency_ms, status))
                if status != 200:
                    raise RuntimeError(
                        f"{creator.handle}: post-disconnect live snapshot failed: {status} {live_after_disconnect}"
                    )
                if live_after_disconnect["profile"]["liveStatus"] != "offline":
                    raise RuntimeError(
                        f"{creator.handle}: broadcast cleanup incomplete after disconnect: {live_after_disconnect}"
                    )
            elif status != 200:
                raise RuntimeError(f"{creator.handle}: end broadcast failed: {status} {payload}")

            result.iterations += 1
            log(f"{creator.handle}: completed ingest iteration {iteration}/{iterations}")
        except Exception as error:
            result.failures.append(str(error))
            repair_creator_state(creator)
            log(f"{creator.handle}: failure on iteration {iteration}: {error}")
    return result


def summarize(results: list[WorkerResult], started_at: float) -> dict[str, Any]:
    samples = [sample for result in results for sample in result.samples]
    by_stage: dict[str, list[Sample]] = {}
    for sample in samples:
        by_stage.setdefault(sample.stage, []).append(sample)

    stage_summary = {}
    for stage, stage_samples in sorted(by_stage.items()):
        latencies = [sample.latency_ms for sample in stage_samples]
        stage_summary[stage] = {
            "count": len(stage_samples),
            "statusCodes": sorted({sample.status for sample in stage_samples}),
            "avgMs": round(sum(latencies) / len(latencies), 2),
            "p50Ms": round(statistics.median(latencies), 2),
            "maxMs": round(max(latencies), 2),
        }

    return {
        "baseUrl": BASE_URL,
        "elapsedSec": round(time.perf_counter() - started_at, 2),
        "workerCount": len(results),
        "successfulIterations": sum(result.iterations for result in results),
        "failedIterations": sum(len(result.failures) for result in results),
        "failures": {
            result.creator_handle: result.failures
            for result in results
            if result.failures
        },
        "stageSummary": stage_summary,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--iterations", type=int, default=4)
    parser.add_argument("--heartbeats", type=int, default=3)
    parser.add_argument("--heartbeat-sleep-ms", type=int, default=100)
    args = parser.parse_args()

    ensure_auth_sessions()
    selected_creators = CREATORS[:]
    started_at = time.perf_counter()
    with ThreadPoolExecutor(max_workers=len(selected_creators)) as executor:
        futures = [
            executor.submit(
                worker_run,
                creator,
                args.iterations,
                args.heartbeats,
                args.heartbeat_sleep_ms,
            )
            for creator in selected_creators
        ]
        results = [future.result() for future in as_completed(futures)]

    summary = summarize(results, started_at)
    print(json.dumps(summary, indent=2))
    return 1 if summary["failedIterations"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
