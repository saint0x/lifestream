import json
import os
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import uuid

BASE = "http://127.0.0.1:8080"
DB = "/Users/deepsaint/Desktop/lifestream/backend/lifestream.db"
AUTH = "Bearer lifestream-local-dev-token"


def req(path, method="GET", body=None, headers=None):
    request_headers = {}
    if headers:
        request_headers.update(headers)
    data = None
    if body is not None and not isinstance(body, (bytes, bytearray)):
        request_headers["Content-Type"] = "application/json"
        data = json.dumps(body).encode()
    elif body is not None:
        data = body
    request = urllib.request.Request(
        BASE + path,
        headers=request_headers,
        data=data,
        method=method,
    )
    try:
        with urllib.request.urlopen(request) as response:
            raw = response.read().decode()
            content_type = response.headers.get("Content-Type", "")
            if "application/json" in content_type:
                return response.status, json.loads(raw) if raw else None
            return response.status, raw
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        content_type = exc.headers.get("Content-Type", "")
        if "application/json" in content_type:
            return exc.code, json.loads(raw) if raw else None
        return exc.code, raw


def db_exec(query, params=()):
    conn = sqlite3.connect(DB)
    conn.execute(query, params)
    conn.commit()
    conn.close()


def upload_video(title, storage_key, payload, slug):
    create = req(
        "/api/v1/creator/me/upload-jobs",
        "POST",
        {
            "kind": "film",
            "sourceType": "resumable-upload",
            "title": title,
            "intendedVisibility": "public",
            "bytesExpected": len(payload),
            "storageKey": storage_key,
            "mimeType": "video/mp4",
        },
        {"Authorization": AUTH},
    )
    assert create[0] == 200, create
    job_id = create[1]["id"]

    ingest = req(
        f"/api/v1/creator/me/upload-jobs/{job_id}/ingest",
        "POST",
        headers={"Authorization": AUTH},
    )
    assert ingest[0] == 200, ingest
    upload_token = ingest[1]["uploadToken"]

    chunk = req(
        f"/api/v1/creator/me/upload-jobs/{job_id}/ingest/chunk?offset=0",
        "PUT",
        payload,
        {"Authorization": AUTH, "x-upload-token": upload_token},
    )
    assert chunk[0] == 200, chunk

    complete = req(
        f"/api/v1/creator/me/upload-jobs/{job_id}/ingest/complete",
        "POST",
        headers={"Authorization": AUTH, "x-upload-token": upload_token},
    )
    assert complete[0] == 200, complete

    asset = None
    for _ in range(30):
        time.sleep(1)
        asset = req(
            f"/api/v1/creator/me/upload-jobs/{job_id}/media-asset",
            headers={"Authorization": AUTH},
        )
        assert asset[0] == 200, asset
        if asset[1]["status"] == "ready":
            break
    assert asset[1]["status"] == "ready", asset

    publish = req(
        f"/api/v1/creator/me/upload-jobs/{job_id}/publish",
        "POST",
        {
            "description": "admin playback operator validation",
            "visibility": "public",
            "accessPolicy": "free",
            "slug": slug,
        },
        {"Authorization": AUTH},
    )
    assert publish[0] == 200, publish
    return publish[1]["id"]


fd, path = tempfile.mkstemp(suffix=".mp4")
os.close(fd)
subprocess.run(
    [
        "ffmpeg",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=320x240:rate=24:duration=2",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=800:sample_rate=48000:duration=2",
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "aac",
        "-shortest",
        path,
    ],
    check=True,
    stdout=subprocess.DEVNULL,
    stderr=subprocess.DEVNULL,
)
payload = open(path, "rb").read()
suffix = uuid.uuid4().hex
upload_id = upload_video(
    "Admin playback operator validation",
    f"uploads/creator/deepsaint/features/admin-playback-validation-{suffix}.mp4",
    payload,
    f"admin-playback-operator-validation-{suffix}",
)

first_playback = req(f"/api/v1/playback/uploads/{upload_id}/session", "POST")
assert first_playback[0] == 200, first_playback
first_session_id = first_playback[1]["session"]["id"]
first_token = first_playback[1]["playbackToken"]

inspected = req(
    f"/api/v1/admin/playback/sessions/{first_session_id}",
    headers={"Authorization": AUTH},
)
assert inspected[0] == 200, inspected
assert inspected[1]["session"]["id"] == first_session_id, inspected
assert inspected[1]["creatorId"] == "crt-deepsaint", inspected
assert inspected[1]["active"] is True, inspected
assert inspected[1]["validAccess"] is True, inspected

db_exec("UPDATE uploads SET visibility = 'private' WHERE id = ?", (upload_id,))

reconciled = req(
    f"/api/v1/admin/playback/sessions/{first_session_id}/reconcile",
    "POST",
    headers={"Authorization": AUTH},
)
assert reconciled[0] == 200, reconciled
assert reconciled[1]["sessionId"] == first_session_id, reconciled
assert any(
    action["actionType"] == "session_invalidated"
    and action["previousState"] == "active"
    and action["nextState"] == "invalid"
    for action in reconciled[1]["actions"]
), reconciled
assert reconciled[1]["record"]["session"]["id"] == first_session_id, reconciled
assert reconciled[1]["record"]["active"] is False, reconciled
assert reconciled[1]["record"]["validAccess"] is False, reconciled

invalid_only = req(
    f"/api/v1/admin/playback/sessions?contentId={upload_id}&state=invalid&limit=50",
    headers={"Authorization": AUTH},
)
assert invalid_only[0] == 200, invalid_only
invalid_rows = {item["session"]["id"]: item for item in invalid_only[1]}
assert first_session_id in invalid_rows, invalid_rows

expired_session = req(
    f"/api/v1/playback/sessions/{first_session_id}?playbackToken={first_token}"
)
assert expired_session[0] == 401, expired_session

db_exec("UPDATE uploads SET visibility = 'public' WHERE id = ?", (upload_id,))

second_playback = req(
    f"/api/v1/playback/uploads/{upload_id}/session",
    "POST",
    headers={"Authorization": AUTH},
)
assert second_playback[0] == 200, second_playback
second_session_id = second_playback[1]["session"]["id"]
second_token = second_playback[1]["playbackToken"]

second_manifest = req(
    f"/api/v1/playback/sessions/{second_session_id}/manifest?playbackToken={second_token}"
)
assert second_manifest[0] == 200, second_manifest

revoked = req(
    f"/api/v1/admin/playback/sessions/{second_session_id}/revoke",
    "POST",
    headers={"Authorization": AUTH},
)
assert revoked[0] == 200, revoked
assert revoked[1]["session"]["id"] == second_session_id, revoked
assert revoked[1]["active"] is False, revoked

revoked_session = req(
    f"/api/v1/playback/sessions/{second_session_id}?playbackToken={second_token}"
)
assert revoked_session[0] == 401, revoked_session

os.remove(path)
print("admin-playback|inspect|invalidate|revoke")
