import json
import os
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
CREATOR_HEADERS = {"Authorization": "Bearer lifestream-local-dev-token"}
SUFFIX = str(int(time.time() * 1000))


def req(path, method="GET", body=None, headers=None):
    request_headers = dict(headers or {})
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
            raw_bytes = response.read()
            raw = raw_bytes.decode()
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
        "sine=frequency=1000:sample_rate=48000:duration=2",
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
valid_job = req(
    "/api/v1/creator/me/upload-jobs",
    "POST",
    {
        "kind": "film",
        "sourceType": "resumable-upload",
        "title": f"Playback invalidation validation {SUFFIX}",
        "intendedVisibility": "public",
        "bytesExpected": len(payload),
        "storageKey": f"uploads/creator/deepsaint/features/playback-invalidation-validation-{SUFFIX}.mp4",
        "mimeType": "video/mp4",
    },
    CREATOR_HEADERS,
)
assert valid_job[0] == 200, valid_job
valid_job_id = valid_job[1]["id"]
ticket = req(
    f"/api/v1/creator/me/upload-jobs/{valid_job_id}/ingest",
    "POST",
    headers=CREATOR_HEADERS,
)
assert ticket[0] == 200, ticket
upload_headers = {
    "Authorization": CREATOR_HEADERS["Authorization"],
    "x-upload-token": ticket[1]["uploadToken"],
}
chunk = req(
    f"/api/v1/creator/me/upload-jobs/{valid_job_id}/ingest/chunk?offset=0",
    "PUT",
    payload,
    upload_headers,
)
assert chunk[0] == 200, chunk
complete = req(
    f"/api/v1/creator/me/upload-jobs/{valid_job_id}/ingest/complete",
    "POST",
    headers=upload_headers,
)
assert complete[0] == 200, complete

asset = None
for _ in range(30):
    time.sleep(1)
    asset = req(
        f"/api/v1/creator/me/upload-jobs/{valid_job_id}/media-asset",
        headers=CREATOR_HEADERS,
    )
    assert asset[0] == 200, asset
    if asset[1]["status"] == "ready":
        break
assert asset[1]["status"] == "ready", asset

publish = req(
    f"/api/v1/creator/me/upload-jobs/{valid_job_id}/publish",
    "POST",
    {
        "description": "playback invalidation publish",
        "visibility": "public",
        "accessPolicy": "free",
    },
    CREATOR_HEADERS,
)
assert publish[0] == 200, publish
upload_id = publish[1]["id"]

playback = req(f"/api/v1/playback/uploads/{upload_id}/session", "POST")
assert playback[0] == 200, playback
session_id = playback[1]["session"]["id"]
playback_token = playback[1]["playbackToken"]
manifest = req(
    f"/api/v1/playback/sessions/{session_id}/manifest?playbackToken={playback_token}"
)
assert manifest[0] == 200, manifest

takedown = req(
    f"/api/v1/creator/me/uploads/{upload_id}/takedown",
    "POST",
    headers=CREATOR_HEADERS,
)
assert takedown[0] == 200 and takedown[1]["status"] == "taken_down", takedown

session_after = req(
    f"/api/v1/playback/sessions/{session_id}?playbackToken={playback_token}"
)
assert session_after[0] == 401, session_after
manifest_after = req(
    f"/api/v1/playback/sessions/{session_id}/manifest?playbackToken={playback_token}"
)
assert manifest_after[0] == 401, manifest_after

invalid_payload = b"retry me through the failed media processing control plane"
invalid_job = req(
    "/api/v1/creator/me/upload-jobs",
    "POST",
    {
        "kind": "film",
        "sourceType": "resumable-upload",
        "title": f"Playback retry validation {SUFFIX}",
        "intendedVisibility": "private",
        "bytesExpected": len(invalid_payload),
        "storageKey": f"uploads/creator/deepsaint/features/playback-retry-validation-{SUFFIX}.mp4",
        "mimeType": "application/octet-stream",
    },
    CREATOR_HEADERS,
)
assert invalid_job[0] == 200, invalid_job
invalid_job_id = invalid_job[1]["id"]
invalid_ticket = req(
    f"/api/v1/creator/me/upload-jobs/{invalid_job_id}/ingest",
    "POST",
    headers=CREATOR_HEADERS,
)
assert invalid_ticket[0] == 200, invalid_ticket
invalid_headers = {
    "Authorization": CREATOR_HEADERS["Authorization"],
    "x-upload-token": invalid_ticket[1]["uploadToken"],
}
invalid_chunk = req(
    f"/api/v1/creator/me/upload-jobs/{invalid_job_id}/ingest/chunk?offset=0",
    "PUT",
    invalid_payload,
    invalid_headers,
)
assert invalid_chunk[0] == 200, invalid_chunk
invalid_complete = req(
    f"/api/v1/creator/me/upload-jobs/{invalid_job_id}/ingest/complete",
    "POST",
    headers=invalid_headers,
)
assert invalid_complete[0] == 200, invalid_complete

first_failure = None
for _ in range(24):
    time.sleep(2)
    jobs = req("/api/v1/creator/me/upload-jobs", headers=CREATOR_HEADERS)
    assert jobs[0] == 200, jobs
    first_failure = next(item for item in jobs[1] if item["id"] == invalid_job_id)
    if first_failure["status"] == "failed":
        break
assert first_failure["status"] == "failed", first_failure
assert first_failure["processingAttemptCount"] == 3, first_failure
initial_failed_at = first_failure["lastFailedAt"]

retry = req(
    f"/api/v1/creator/me/upload-jobs/{invalid_job_id}/retry",
    "POST",
    headers=CREATOR_HEADERS,
)
assert retry[0] == 200, retry

second_failure = None
for _ in range(24):
    time.sleep(2)
    jobs = req("/api/v1/creator/me/upload-jobs", headers=CREATOR_HEADERS)
    assert jobs[0] == 200, jobs
    second_failure = next(item for item in jobs[1] if item["id"] == invalid_job_id)
    if (
        second_failure["status"] == "failed"
        and second_failure["lastFailedAt"] is not None
        and second_failure["lastFailedAt"] != initial_failed_at
    ):
        break
assert second_failure["status"] == "failed", second_failure
assert second_failure["processingAttemptCount"] == 4, second_failure
assert second_failure["lastFailedAt"] != initial_failed_at, second_failure

os.remove(path)
print("invalidated|retried|failed")
