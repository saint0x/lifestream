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
DB = "/Users/deepsaint/Desktop/vanta/backend/vanta.db"
AUTH = "Bearer vanta-local-dev-token"


def req(path, method="GET", body=None, headers=None):
    request_headers = {"Authorization": AUTH}
    if headers:
        request_headers.update(headers)
    data = None
    if body is not None:
        request_headers["Content-Type"] = "application/json"
        data = json.dumps(body).encode()
    request = urllib.request.Request(
        BASE + path,
        headers=request_headers,
        data=data,
        method=method,
    )
    try:
        with urllib.request.urlopen(request) as response:
            raw = response.read().decode()
            return response.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        return exc.code, json.loads(raw) if raw else None


def db_exec(query, params=()):
    conn = sqlite3.connect(DB)
    conn.execute(query, params)
    conn.commit()
    conn.close()


def upload_payload_job(title, storage_key, payload, mime_type):
    create = req(
        "/api/v1/creator/me/upload-jobs",
        "POST",
        {
            "kind": "film",
            "sourceType": "resumable-upload",
            "title": title,
            "intendedVisibility": "private",
            "bytesExpected": len(payload),
            "storageKey": storage_key,
            "mimeType": mime_type,
        },
    )
    assert create[0] == 200, create
    job = create[1]

    ticket = req(f"/api/v1/creator/me/upload-jobs/{job['id']}/ingest", "POST")
    assert ticket[0] == 200, ticket
    upload_token = ticket[1]["uploadToken"]

    request = urllib.request.Request(
        BASE + f"/api/v1/creator/me/upload-jobs/{job['id']}/ingest/chunk?offset=0",
        data=payload,
        headers={"Authorization": AUTH, "x-upload-token": upload_token},
        method="PUT",
    )
    with urllib.request.urlopen(request) as response:
        chunk_payload = json.loads(response.read().decode())
    assert chunk_payload["bytesReceived"] == len(payload), chunk_payload

    complete = req(
        f"/api/v1/creator/me/upload-jobs/{job['id']}/ingest/complete",
        "POST",
        headers={"x-upload-token": upload_token},
    )
    assert complete[0] == 200, complete
    return job


def wait_for_job(job_id, target_status, attempts=30, interval=2):
    last = None
    for _ in range(attempts):
        time.sleep(interval)
        jobs = req("/api/v1/creator/me/upload-jobs")
        assert jobs[0] == 200, jobs
        last = next(item for item in jobs[1] if item["id"] == job_id)
        if last["status"] == target_status:
            return last
    raise AssertionError(last)


invalid_suffix = uuid.uuid4().hex
invalid_payload = b"invalid media for admin retry control path"
failed_job = upload_payload_job(
    "Admin failed job retry",
    f"uploads/creator/deepsaint/features/admin-failed-job-{invalid_suffix}.mp4",
    invalid_payload,
    "application/octet-stream",
)
failed_state = wait_for_job(failed_job["id"], "failed")
initial_failed_at = failed_state["lastFailedAt"]

admin_failed = req(
    "/api/v1/admin/media/upload-jobs?status=failed&creatorId=crt-deepsaint&limit=50"
)
assert admin_failed[0] == 200, admin_failed
failed_rows = {item["uploadJob"]["id"]: item for item in admin_failed[1]}
assert failed_job["id"] in failed_rows, failed_rows
assert failed_rows[failed_job["id"]]["repairRequired"] is True, failed_rows[failed_job["id"]]

failed_inspected = req(f"/api/v1/admin/media/upload-jobs/{failed_job['id']}")
assert failed_inspected[0] == 200, failed_inspected
assert failed_inspected[1]["uploadJob"]["id"] == failed_job["id"], failed_inspected
assert failed_inspected[1]["uploadJob"]["status"] == "failed", failed_inspected

retried = req(f"/api/v1/admin/media/upload-jobs/{failed_job['id']}/retry", "POST")
assert retried[0] == 200, retried
assert retried[1]["uploadJob"]["status"] in ("uploaded", "processing"), retried

failed_again = None
for _ in range(30):
    time.sleep(2)
    jobs = req("/api/v1/creator/me/upload-jobs")
    assert jobs[0] == 200, jobs
    candidate = next(item for item in jobs[1] if item["id"] == failed_job["id"])
    if candidate["status"] == "failed" and candidate["lastFailedAt"] != initial_failed_at:
        failed_again = candidate
        break
assert failed_again is not None, failed_again

fd, path = tempfile.mkstemp(suffix=".mp4")
os.close(fd)
subprocess.run(
    [
        "ffmpeg",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=320x240:rate=24:duration=3",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=1000:sample_rate=48000:duration=3",
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
valid_payload = open(path, "rb").read()
valid_suffix = uuid.uuid4().hex
valid_job = upload_payload_job(
    "Admin stale processing recovery",
    f"uploads/creator/deepsaint/features/admin-stale-job-{valid_suffix}.mp4",
    valid_payload,
    "video/mp4",
)
ready_job = wait_for_job(valid_job["id"], "ready", attempts=30, interval=2)
assert ready_job["status"] == "ready", ready_job

db_exec(
    "UPDATE upload_jobs SET status = 'processing', updated_at = ?, last_processing_error = NULL WHERE id = ?",
    ("2026-08-17T00:00:00+00:00", valid_job["id"]),
)
db_exec(
    "UPDATE media_assets SET status = 'processing', updated_at = ? WHERE upload_job_id = ?",
    ("2026-08-17T00:00:00+00:00", valid_job["id"]),
)

processing_jobs = req(
    "/api/v1/admin/media/upload-jobs?status=processing&creatorId=crt-deepsaint&limit=50"
)
assert processing_jobs[0] == 200, processing_jobs
processing_rows = {item["uploadJob"]["id"]: item for item in processing_jobs[1]}
assert valid_job["id"] in processing_rows, processing_rows
assert processing_rows[valid_job["id"]]["staleProcessing"] is True, processing_rows[valid_job["id"]]
assert processing_rows[valid_job["id"]]["repairRequired"] is True, processing_rows[valid_job["id"]]

stale_inspected = req(f"/api/v1/admin/media/upload-jobs/{valid_job['id']}")
assert stale_inspected[0] == 200, stale_inspected
assert stale_inspected[1]["uploadJob"]["id"] == valid_job["id"], stale_inspected
assert stale_inspected[1]["staleProcessing"] is True, stale_inspected

reconciled = req(
    f"/api/v1/admin/media/upload-jobs/{valid_job['id']}/reconcile",
    "POST",
)
assert reconciled[0] == 200, reconciled
assert reconciled[1]["jobId"] == valid_job["id"], reconciled
assert any(
    action["actionType"] == "job_reconciled"
    and action["previousStatus"] == "processing"
    and action["nextStatus"] == "uploaded"
    for action in reconciled[1]["actions"]
), reconciled
assert reconciled[1]["record"]["uploadJob"]["status"] == "uploaded", reconciled
assert reconciled[1]["record"]["assetStatus"] == "uploaded", reconciled

recovered_job = wait_for_job(valid_job["id"], "ready", attempts=12, interval=2)
assert recovered_job["processingAttemptCount"] >= 2, recovered_job

asset = req(f"/api/v1/creator/me/upload-jobs/{valid_job['id']}/media-asset")
assert asset[0] == 200 and asset[1]["status"] == "ready", asset

os.remove(path)
print("admin-media|inspect|retry|stale-recover")
