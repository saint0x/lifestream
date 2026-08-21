#!/usr/bin/env python3

import argparse
import hashlib
import json
import os
import signal
import sqlite3
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[3]
BACKEND_ROOT = ROOT / "backend"
DB_PATH = BACKEND_ROOT / "lifestream.db"
BASE_URL = os.environ.get("LIFESTREAM_BASE_URL", "http://127.0.0.1:8080")
AUTH = "Bearer lifestream-local-dev-token"
HEADERS = {"Authorization": AUTH, "Content-Type": "application/json"}
PLAYBACK_WRK = BACKEND_ROOT / "tests" / "load" / "playback-session.lua"
CHAT_WRK = BACKEND_ROOT / "tests" / "load" / "chat-message.lua"


@dataclass
class Fixture:
    broadcast_id: str
    session_id: str
    ingest_token: str
    collab_session_id: str


def req(path: str, method: str = "GET", body: Any | None = None, headers: dict[str, str] | None = None):
    request_headers = dict(headers or {})
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
    try:
        with urllib.request.urlopen(request) as response:
            raw = response.read().decode()
            return response.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        return exc.code, json.loads(raw) if raw else None


def ensure_auth_session() -> None:
    now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    with sqlite3.connect(DB_PATH) as conn:
        conn.execute(
            """
            INSERT OR REPLACE INTO auth_sessions (
                id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
            ) VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, NULL)
            """,
            (
                "sess-load-owner",
                "usr-1",
                "load-owner",
                hashlib.sha256("lifestream-local-dev-token".encode()).hexdigest(),
                json.dumps(["user", "creator", "creator:write", "admin"]),
                now,
            ),
        )
        conn.commit()


def build_fixture() -> Fixture:
    ensure_auth_session()
    live_status, live = req("/api/v1/creator/me/live", headers={"Authorization": AUTH})
    if live_status != 200:
        raise RuntimeError(f"failed to fetch creator live state: {live_status} {live}")

    for key in ("currentBroadcast", "pendingBroadcast"):
        broadcast = live.get(key)
        if broadcast is not None:
            ended_status, ended = req(
                f"/api/v1/creator/me/broadcasts/{broadcast['id']}/end",
                "POST",
                headers={"Authorization": AUTH},
            )
            if ended_status != 200:
                raise RuntimeError(f"failed to end existing broadcast: {ended_status} {ended}")

    suffix = str(int(time.time() * 1000))
    started_status, started = req(
        "/api/v1/creator/me/broadcasts/start",
        "POST",
        {
            "title": f"Load sweep fixture {suffix}",
            "category": "Tech",
            "tags": ["bench", "runtime", "live"],
            "isMature": False,
            "notifyFollowers": False,
        },
        HEADERS,
    )
    if started_status != 200:
        raise RuntimeError(f"failed to start broadcast: {started_status} {started}")

    live_status, live = req("/api/v1/creator/me/live", headers={"Authorization": AUTH})
    if live_status != 200:
        raise RuntimeError(f"failed to re-fetch creator live state: {live_status} {live}")

    connect_status, connected = req(
        "/api/v1/ingest/live/connect",
        "POST",
        {
            "streamKey": live["profile"]["streamKey"],
            "protocol": "rtmp",
            "ingestServer": "rtmp-us-east-1-primary",
            "broadcastId": started["id"],
        },
        {"Content-Type": "application/json"},
    )
    if connect_status != 200:
        raise RuntimeError(f"failed to connect ingest session: {connect_status} {connected}")

    session = connected["session"]
    ingest_token = connected["ingestToken"]
    heartbeat_status, heartbeat = req(
        f"/api/v1/ingest/live/{session['id']}/heartbeat",
        "POST",
        {
            "bitrateKbps": 7200,
            "viewers": 2115,
            "droppedFrames": 3,
            "cpuPercent": 42,
            "freeDiskGb": 602.5,
        },
        {"Content-Type": "application/json", "x-ingest-token": ingest_token},
    )
    if heartbeat_status != 200:
        raise RuntimeError(f"failed to heartbeat ingest session: {heartbeat_status} {heartbeat}")

    runtime_status, runtime = req(
        f"/api/v1/ingest/live/{session['id']}/runtime",
        "POST",
        {
            "runtimeState": "healthy",
            "packagingStatus": "ready",
            "archiveStatus": "not_started",
            "manifestRelativePath": (
                f"live/{live['profile']['id']}/{started['id']}/{session['id']}/master.m3u8"
            ),
            "archiveRelativePath": None,
            "lastError": None,
        },
        {"Content-Type": "application/json", "x-ingest-token": ingest_token},
    )
    if runtime_status != 200:
        raise RuntimeError(f"failed to report runtime output: {runtime_status} {runtime}")

    collab_status, collab = req(
        "/api/v1/creator/me/live/collabs/sessions",
        "POST",
        {
            "broadcastId": started["id"],
            "title": f"Load sweep collab {suffix}",
            "chatMode": "shared",
            "recordingPolicy": "host_archive",
        },
        HEADERS,
    )
    if collab_status != 200:
        raise RuntimeError(f"failed to create collaboration session: {collab_status} {collab}")

    return Fixture(
        broadcast_id=started["id"],
        session_id=session["id"],
        ingest_token=ingest_token,
        collab_session_id=collab["id"],
    )


def heartbeat_loop(fixture: Fixture) -> subprocess.Popen[bytes]:
    payload = (
        '{"bitrateKbps":7200,"viewers":2115,"droppedFrames":3,'
        '"cpuPercent":42,"freeDiskGb":602.5}'
    )
    cmd = (
        "while true; do "
        f"curl -s -X POST -H 'Content-Type: application/json' "
        f"-H 'x-ingest-token: {fixture.ingest_token}' "
        f"-d '{payload}' "
        f"{BASE_URL}/api/v1/ingest/live/{fixture.session_id}/heartbeat >/dev/null; "
        "sleep 3; "
        "done"
    )
    return subprocess.Popen(
        ["bash", "-lc", cmd],
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        preexec_fn=os.setsid,
    )


def parse_wrk_output(text: str, exit_code: int) -> dict[str, Any]:
    result: dict[str, Any] = {"exit_code": exit_code}
    if exit_code != 0:
        result["error"] = text.strip()[:500]

    lines = text.splitlines()
    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("Requests/sec:"):
            result["req_per_sec"] = float(stripped.split()[-1])
        elif stripped.startswith("Non-2xx or 3xx responses:"):
            result["non_2xx"] = int(stripped.split()[-1])
        elif stripped.startswith("Socket errors:"):
            result["socket_errors"] = stripped.split(":", 1)[1].strip()
        elif stripped.startswith("50%"):
            result["p50"] = stripped.split(None, 1)[1]
        elif stripped.startswith("99%"):
            result["p99"] = stripped.split(None, 1)[1]
        elif index == 0:
            result["headline"] = stripped
    return result


def run_lane(name: str, command: str) -> dict[str, Any]:
    with tempfile.NamedTemporaryFile(delete=False) as temp:
        temp_path = temp.name
    try:
        with open(temp_path, "w") as output:
            proc = subprocess.Popen(
                ["bash", "-lc", command],
                cwd=ROOT,
                stdout=output,
                stderr=subprocess.STDOUT,
            )
            exit_code = proc.wait()
        with open(temp_path) as output:
            text = output.read()
        return parse_wrk_output(text, exit_code)
    finally:
        try:
            os.unlink(temp_path)
        except FileNotFoundError:
            pass


def sweep_lanes(fixture: Fixture) -> list[tuple[str, str]]:
    return [
        ("public_list", f"/usr/local/bin/wrk -t2 -c32 -d4s --latency {BASE_URL}/api/v1/live/streams"),
        (
            "public_detail",
            f"/usr/local/bin/wrk -t2 -c32 -d4s --latency {BASE_URL}/api/v1/live/streams/deepsaint-live",
        ),
        (
            "bootstrap",
            f"/usr/local/bin/wrk -t2 -c32 -d4s --latency -H 'Authorization: {AUTH}' {BASE_URL}/api/v1/bootstrap",
        ),
        (
            "me_state",
            f"/usr/local/bin/wrk -t2 -c32 -d4s --latency -H 'Authorization: {AUTH}' {BASE_URL}/api/v1/me/state",
        ),
        (
            "creator_state",
            f"/usr/local/bin/wrk -t2 -c32 -d4s --latency -H 'Authorization: {AUTH}' {BASE_URL}/api/v1/creator/me/state",
        ),
        (
            "creator_live_control",
            f"/usr/local/bin/wrk -t2 -c32 -d4s --latency -H 'Authorization: {AUTH}' {BASE_URL}/api/v1/creator/me/live/control",
        ),
        (
            "creator_live_runtime",
            f"/usr/local/bin/wrk -t2 -c32 -d4s --latency -H 'Authorization: {AUTH}' {BASE_URL}/api/v1/creator/me/live/runtime",
        ),
        (
            "collab_control",
            f"/usr/local/bin/wrk -t2 -c32 -d4s --latency -H 'Authorization: {AUTH}' "
            f"{BASE_URL}/api/v1/creator/me/live/collabs/sessions/{fixture.collab_session_id}/control",
        ),
        (
            "collab_runtime",
            f"/usr/local/bin/wrk -t2 -c32 -d4s --latency -H 'Authorization: {AUTH}' "
            f"{BASE_URL}/api/v1/creator/me/live/collabs/sessions/{fixture.collab_session_id}/runtime",
        ),
        (
            "playback_post",
            f"/usr/local/bin/wrk -t2 -c32 -d4s --latency -s {PLAYBACK_WRK} {BASE_URL}",
        ),
        (
            "chat_post",
            f"/usr/local/bin/wrk -t2 -c32 -d4s --latency -s {CHAT_WRK} {BASE_URL}",
        ),
    ]


def run_sweep(fixture: Fixture, repeats: int) -> list[dict[str, Any]]:
    results: list[dict[str, Any]] = []
    for iteration in range(1, repeats + 1):
        procs: list[tuple[str, subprocess.Popen[bytes], str]] = []
        try:
            for name, command in sweep_lanes(fixture):
                temp = tempfile.NamedTemporaryFile(delete=False)
                temp.close()
                output = open(temp.name, "w")
                proc = subprocess.Popen(
                    ["bash", "-lc", command],
                    cwd=ROOT,
                    stdout=output,
                    stderr=subprocess.STDOUT,
                )
                output.close()
                procs.append((name, proc, temp.name))

            iteration_result = {"iteration": iteration, "lanes": {}}
            for name, proc, output_path in procs:
                exit_code = proc.wait()
                with open(output_path) as output:
                    text = output.read()
                iteration_result["lanes"][name] = parse_wrk_output(text, exit_code)
                os.unlink(output_path)
            results.append(iteration_result)
            time.sleep(1)
        finally:
            for _, proc, output_path in procs:
                if proc.poll() is None:
                    proc.kill()
                    proc.wait()
                if os.path.exists(output_path):
                    os.unlink(output_path)
    return results


def main() -> int:
    parser = argparse.ArgumentParser(description="Run the full mixed load sweep against a fresh live fixture.")
    parser.add_argument("--repeats", type=int, default=3, help="Number of repeated mixed sweeps to run.")
    args = parser.parse_args()

    health_status, health = req("/health")
    if health_status != 200 or health.get("status") != "ok":
        raise RuntimeError(f"server is not healthy: {health_status} {health}")

    fixture = build_fixture()
    heartbeat = heartbeat_loop(fixture)
    try:
        results = {
            "fixture": {
                "broadcast_id": fixture.broadcast_id,
                "session_id": fixture.session_id,
                "collab_session_id": fixture.collab_session_id,
            },
            "repeats": args.repeats,
            "results": run_sweep(fixture, args.repeats),
        }
    finally:
        try:
            os.killpg(os.getpgid(heartbeat.pid), signal.SIGINT)
        except ProcessLookupError:
            pass

    print(json.dumps(results, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
