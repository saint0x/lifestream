import json
import time
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
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


payload = b"this is not valid media but should still exercise the retry state machine"
create = req(
    "/api/v1/creator/me/upload-jobs",
    "POST",
    {
        "kind": "film",
        "sourceType": "resumable-upload",
        "title": "Invalid media retry validation",
        "intendedVisibility": "private",
        "bytesExpected": len(payload),
        "storageKey": "uploads/creator/deepsaint/features/invalid-media-retry-validation.mp4",
        "mimeType": "application/octet-stream",
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

failed_job = None
asset = None
for _ in range(24):
    time.sleep(2)
    jobs = req("/api/v1/creator/me/upload-jobs")
    assert jobs[0] == 200, jobs
    failed_job = next(item for item in jobs[1] if item["id"] == job["id"])
    asset = req(f"/api/v1/creator/me/upload-jobs/{job['id']}/media-asset")
    assert asset[0] == 200, asset
    if failed_job["status"] == "failed":
        break

assert failed_job is not None, "job not found after processing"
assert failed_job["status"] == "failed", failed_job
assert failed_job["processingAttemptCount"] == 3, failed_job
assert failed_job["lastProcessingError"], failed_job
assert failed_job["lastFailedAt"], failed_job
assert asset[1]["status"] == "failed", asset
assert any(run["status"] == "failed" for run in asset[1]["processingRuns"]), asset
assert any(run["stage"] == "job_failure" for run in asset[1]["processingRuns"]), asset

print(f"{failed_job['status']}|{failed_job['processingAttemptCount']}|{asset[1]['status']}")
