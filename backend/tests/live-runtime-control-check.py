import json
import hashlib
import os
import sqlite3
import time
import urllib.error
import urllib.request
from pathlib import Path

BASE = os.environ.get("VANTA_BASE_URL", "http://127.0.0.1:8080")
DB = os.environ.get(
    "VANTA_DB_PATH",
    str(Path(__file__).resolve().parents[1] / "vanta.db"),
)
AUTH = "Bearer vanta-local-dev-token"
HEADERS = {"Authorization": AUTH, "Content-Type": "application/json"}
SUFFIX = str(int(time.time() * 1000))


def req(path, method="GET", body=None, headers=None):
    request_headers = dict(headers or {})
    data = None
    if body is not None:
        data = json.dumps(body).encode()
        request_headers.setdefault("Content-Type", "application/json")
    request = urllib.request.Request(BASE + path, data=data, headers=request_headers, method=method)
    try:
        with urllib.request.urlopen(request) as response:
            raw = response.read().decode()
            return response.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        return exc.code, json.loads(raw) if raw else None


conn = sqlite3.connect(DB)
now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
conn.execute(
    """
    INSERT OR REPLACE INTO auth_sessions (
        id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
    ) VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, NULL)
    """,
    (
        "sess-runtime-control-owner",
        "usr-1",
        "runtime-control-owner",
        hashlib.sha256("vanta-local-dev-token".encode()).hexdigest(),
        json.dumps(["user", "creator", "creator:write", "admin"]),
        now,
    ),
)
conn.commit()
conn.close()


live_before = req("/api/v1/creator/me/live", headers={"Authorization": AUTH})
assert live_before[0] == 200, live_before
for broadcast_key in ("currentBroadcast", "pendingBroadcast"):
    broadcast = live_before[1].get(broadcast_key)
    if broadcast is not None:
        ended = req(
            f"/api/v1/creator/me/broadcasts/{broadcast['id']}/end",
            "POST",
            headers={"Authorization": AUTH},
        )
        assert ended[0] == 200, ended


start = req(
    "/api/v1/creator/me/broadcasts/start",
    "POST",
    {
        "title": f"Runtime authority validation {SUFFIX}",
        "category": "Tech",
        "tags": ["runtime", "control", "live"],
        "isMature": False,
        "notifyFollowers": False,
    },
    HEADERS,
)
assert start[0] == 200, start
broadcast = start[1]

live = req("/api/v1/creator/me/live", headers={"Authorization": AUTH})
assert live[0] == 200, live
stream_key = live[1]["profile"]["streamKey"]

connect = req(
    "/api/v1/ingest/live/connect",
    "POST",
    {
        "streamKey": stream_key,
        "protocol": "rtmp",
        "ingestServer": "rtmp-us-east-1-primary",
        "broadcastId": broadcast["id"],
    },
    {"Content-Type": "application/json"},
)
assert connect[0] == 200, connect
session = connect[1]["session"]
ingest_token = connect[1]["ingestToken"]

heartbeat = req(
    f"/api/v1/ingest/live/{session['id']}/heartbeat",
    "POST",
    {
        "bitrateKbps": 7200,
        "viewers": 1888,
        "droppedFrames": 4,
        "cpuPercent": 44,
        "freeDiskGb": 602.5,
    },
    {"Content-Type": "application/json", "x-ingest-token": ingest_token},
)
assert heartbeat[0] == 200, heartbeat

runtime_report = req(
    f"/api/v1/ingest/live/{session['id']}/runtime",
    "POST",
    {
        "runtimeState": "healthy",
        "packagingStatus": "ready",
        "archiveStatus": "not_started",
        "manifestRelativePath": f"live/{live[1]['profile']['id']}/{broadcast['id']}/{session['id']}/master.m3u8",
        "archiveRelativePath": None,
        "lastError": None,
    },
    {"Content-Type": "application/json", "x-ingest-token": ingest_token},
)
assert runtime_report[0] == 200, runtime_report
assert runtime_report[1]["runtimeState"] == "healthy", runtime_report
assert runtime_report[1]["packagingStatus"] == "ready", runtime_report

runtime = req("/api/v1/creator/me/live/runtime", headers={"Authorization": AUTH})
assert runtime[0] == 200, runtime
payload = runtime[1]
assert payload["activeSession"]["id"] == session["id"], payload
assert payload["activeRuntimeOutput"]["sessionId"] == session["id"], payload
assert payload["activeRuntimeOutput"]["runtimeState"] == "healthy", payload
assert payload["activeRuntimeOutput"]["packagingStatus"] == "ready", payload
assert payload["telemetrySummary"]["totalSamples"] >= 2, payload
assert payload["telemetrySummary"]["lastRuntimeState"] == "healthy", payload
assert payload["recentRuntimeOutputs"][0]["sessionId"] == session["id"], payload
assert payload["recentTelemetry"][0]["sampleKind"] == "runtime_report", payload
assert payload["recentTelemetry"][0]["runtimeState"] == "healthy", payload
assert payload["snapshot"]["currentBroadcast"]["status"] == "live", payload
assert payload["health"]["samples"][-1]["viewers"] == 1888, payload
assert payload["collaboration"]["activeSession"] is None, payload
assert payload["collaboration"]["activeSessionCount"] == 0, payload
assert any(event["eventType"] == "connected" for event in payload["recentEvents"]), payload
assert any(
    event["eventType"] == "heartbeat_recorded" and event["payload"]["viewers"] == 1888
    for event in payload["recentEvents"]
), payload

repair = req(
    f"/api/v1/creator/me/live/ingest/{session['id']}/runtime/repair",
    "POST",
    {
        "reason": "runtime control verification",
        "runtimeState": "healthy",
        "packagingStatus": "ready",
        "archiveStatus": "finalizing",
        "archiveRelativePath": f"archive/{live[1]['profile']['id']}/{broadcast['id']}/{session['id']}/final.mp4",
        "clearLastError": True,
    },
    HEADERS,
)
assert repair[0] == 200, repair
assert repair[1]["actorScope"] == "creator", repair
assert repair[1]["record"]["runtimeOutput"]["archiveStatus"] == "finalizing", repair
assert repair[1]["record"]["recentTelemetry"][0]["sampleKind"] == "runtime_repair", repair
assert any(action["field"] == "archiveStatus" for action in repair[1]["actions"]), repair

runtime_after_repair = req("/api/v1/creator/me/live/runtime", headers={"Authorization": AUTH})
assert runtime_after_repair[0] == 200, runtime_after_repair
assert runtime_after_repair[1]["activeRuntimeOutput"]["archiveStatus"] == "finalizing", runtime_after_repair
assert runtime_after_repair[1]["recentTelemetry"][0]["sampleKind"] == "runtime_repair", runtime_after_repair

overview = req("/api/v1/admin/live/ingest/overview", headers={"Authorization": AUTH})
assert overview[0] == 200, overview
assert overview[1]["activeSessions"] >= 1, overview
assert overview[1]["readyOutputs"] >= 1, overview
assert overview[1]["archiveFinalizingOutputs"] >= 1, overview
assert any(
    item["creatorId"] == live[1]["profile"]["id"] and item["activeSessions"] >= 1
    for item in overview[1]["creatorBreakdown"]
), overview

metrics_body = urllib.request.urlopen(BASE + "/metrics").read().decode()
assert "vanta_live_ingest_active_sessions" in metrics_body, metrics_body
assert "vanta_live_ingest_ready_outputs" in metrics_body, metrics_body

created = req(
    "/api/v1/creator/me/live/collabs/sessions",
    "POST",
    {
        "broadcastId": broadcast["id"],
        "title": f"Runtime collab summary {SUFFIX}",
        "chatMode": "shared",
        "recordingPolicy": "host_archive",
    },
    HEADERS,
)
assert created[0] == 200, created
collab_session = created[1]

control = req("/api/v1/creator/me/live/control", headers={"Authorization": AUTH})
assert control[0] == 200, control
control_payload = control[1]
assert control_payload["collaboration"]["activeSession"]["id"] == collab_session["id"], control_payload
assert control_payload["collaboration"]["activeControl"]["runtime"]["session"]["id"] == collab_session["id"], control_payload
assert control_payload["collaboration"]["activeSessionCount"] >= 1, control_payload
assert control_payload["collaboration"]["totalSessions"] >= 1, control_payload

socket_id = f"cls-test-{SUFFIX}"
stale_seen_at = "2026-08-18T03:55:00Z"
conn = sqlite3.connect(DB)
conn.execute(
    """
    INSERT INTO creator_live_socket_sessions (
        id, creator_id, user_id, session_token_hash, connected_at, last_seen_at, disconnected_at
    ) VALUES (?, ?, ?, ?, ?, ?, NULL)
    """,
    (
        socket_id,
        live[1]["profile"]["id"],
        "usr-1",
        hashlib.sha256(f"creator-live-stale-{SUFFIX}".encode()).hexdigest(),
        stale_seen_at,
        stale_seen_at,
    ),
)
conn.commit()
conn.close()

socket_inspected = req(
    f"/api/v1/creator/me/live/socket-sessions/{socket_id}",
    headers={"Authorization": AUTH},
)
assert socket_inspected[0] == 200, socket_inspected
assert socket_inspected[1]["id"] == socket_id, socket_inspected
assert socket_inspected[1]["isStale"] is True, socket_inspected
assert socket_inspected[1]["disconnectedAt"] is None, socket_inspected

socket_reconciled = req(
    f"/api/v1/creator/me/live/socket-sessions/{socket_id}/reconcile",
    "POST",
    headers={"Authorization": AUTH},
)
assert socket_reconciled[0] == 200, socket_reconciled
assert socket_reconciled[1]["creatorId"] == live[1]["profile"]["id"], socket_reconciled
assert socket_reconciled[1]["socketSessionId"] == socket_id, socket_reconciled
assert socket_reconciled[1]["socketSession"]["id"] == socket_id, socket_reconciled
assert socket_reconciled[1]["socketSession"]["disconnectedAt"] is not None, socket_reconciled
assert len(socket_reconciled[1]["actions"]) == 1, socket_reconciled
assert socket_reconciled[1]["actions"][0]["actionType"] == "socket_disconnected", socket_reconciled
assert socket_reconciled[1]["actions"][0]["previousState"] == "connected", socket_reconciled
assert socket_reconciled[1]["actions"][0]["nextState"] == "disconnected", socket_reconciled

runtime_with_collab = req("/api/v1/creator/me/live/runtime", headers={"Authorization": AUTH})
assert runtime_with_collab[0] == 200, runtime_with_collab
runtime_collab_payload = runtime_with_collab[1]
assert runtime_collab_payload["collaboration"]["activeSession"]["id"] == collab_session["id"], runtime_collab_payload
assert runtime_collab_payload["collaboration"]["activeControl"]["runtime"]["topology"]["sharedChat"] is True, runtime_collab_payload
assert runtime_collab_payload["collaboration"]["recentSessions"][0]["id"] == collab_session["id"], runtime_collab_payload

ended_collab = req(
    f"/api/v1/creator/me/live/collabs/sessions/{collab_session['id']}/end",
    "POST",
    headers={"Authorization": AUTH},
)
assert ended_collab[0] == 200 and ended_collab[1]["status"] == "ended", ended_collab

terminated = req(
    f"/api/v1/creator/me/live/ingest/{session['id']}/terminate",
    "POST",
    {"reason": "operator cleanup validation"},
    HEADERS,
)
assert terminated[0] == 200, terminated
assert terminated[1]["status"] == "terminated", terminated

runtime_after = req("/api/v1/creator/me/live/runtime", headers={"Authorization": AUTH})
assert runtime_after[0] == 200, runtime_after
after = runtime_after[1]
assert after["activeSession"] is None, after
assert after["activeRuntimeOutput"] is None, after
assert after["snapshot"]["profile"]["liveStatus"] == "offline", after
assert after["collaboration"]["activeSession"] is None, after
assert after["recentSessions"][0]["id"] == session["id"], after
assert after["recentSessions"][0]["status"] == "terminated", after
assert after["recentRuntimeOutputs"][0]["sessionId"] == session["id"], after
assert after["recentRuntimeOutputs"][0]["runtimeState"] == "disconnected", after
assert after["recentTelemetry"][0]["sampleKind"] == "session_state", after
assert after["recentTelemetry"][0]["runtimeState"] == "disconnected", after
assert any(event["eventType"] == "creator_terminated" for event in after["recentEvents"]), after

print("runtime|socket-inspect|connected|terminated")
