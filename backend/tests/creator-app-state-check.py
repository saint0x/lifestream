import json
import urllib.request

BASE = "http://127.0.0.1:8080"
HOST = "Bearer vanta-local-dev-token"


def req(path, token=None):
    headers = {}
    if token:
        headers["Authorization"] = token
    request = urllib.request.Request(BASE + path, headers=headers)
    with urllib.request.urlopen(request) as response:
        raw = response.read().decode()
        return response.status, json.loads(raw) if raw else None


state = req("/api/v1/creator/me/state", token=HOST)
assert state[0] == 200, state
assert state[1]["dashboard"]["profile"]["handle"] == "deepsaint", state
assert (
    "liveControl" in state[1]
    and "liveRuntime" in state[1]
    and "content" in state[1]
    and "uploadOperations" in state[1]
), state
assert (
    state[1]["content"]["summary"]["filteredCount"]
    >= len(state[1]["content"]["uploads"])
), state
assert (
    state[1]["uploadOperations"]["summary"]["totalJobs"]
    >= len(state[1]["uploadOperations"]["records"])
), state
assert (
    state[1]["uploadOperations"]["summary"]["totalBytesExpected"]
    >= sum(item["uploadJob"]["bytesExpected"] for item in state[1]["uploadOperations"]["records"])
), state
assert (
    state[1]["uploadOperations"]["summary"]["totalBytesReceived"]
    >= sum(item["uploadJob"]["bytesReceived"] for item in state[1]["uploadOperations"]["records"])
), state
assert (
    state[1]["liveControl"]["snapshot"]["profile"]["id"]
    == state[1]["dashboard"]["profile"]["id"]
), state
assert state[1]["dashboard"]["profile"]["liveStatus"] != "ready", state
assert state[1]["liveControl"]["snapshot"]["profile"]["liveStatus"] != "ready", state
assert state[1]["liveRuntime"]["snapshot"]["profile"]["liveStatus"] != "ready", state
for broadcast in [state[1]["dashboard"]["currentBroadcast"], *state[1]["dashboard"]["scheduledBroadcasts"]]:
    if broadcast is not None:
        assert broadcast["status"] != "ready", broadcast
for key in ("currentBroadcast", "pendingBroadcast"):
    broadcast = state[1]["liveControl"]["snapshot"][key]
    if broadcast is not None:
        assert broadcast["status"] != "ready", broadcast
    runtime_broadcast = state[1]["liveRuntime"]["snapshot"][key]
    if runtime_broadcast is not None:
        assert runtime_broadcast["status"] != "ready", runtime_broadcast
assert (
    state[1]["liveRuntime"]["snapshot"]["profile"]["id"]
    == state[1]["dashboard"]["profile"]["id"]
), state
assert "collaboration" in state[1]["liveControl"], state
assert "collaboration" in state[1]["liveRuntime"], state
assert (
    state[1]["liveControl"]["collaboration"]["activeSessionCount"]
    == state[1]["liveRuntime"]["collaboration"]["activeSessionCount"]
), state

bootstrap = req("/api/v1/bootstrap", token=HOST)
assert bootstrap[0] == 200, bootstrap
assert bootstrap[1]["creator"]["profile"]["id"] == state[1]["dashboard"]["profile"]["id"], bootstrap
assert bootstrap[1]["creator"]["profile"]["liveStatus"] != "ready", bootstrap
assert (
    bootstrap[1]["creatorState"]["dashboard"]["profile"]["id"]
    == state[1]["dashboard"]["profile"]["id"]
), bootstrap
assert (
    bootstrap[1]["creatorState"]["uploadOperations"]["summary"]["totalJobs"]
    == state[1]["uploadOperations"]["summary"]["totalJobs"]
), bootstrap

upload_operations = req("/api/v1/creator/me/upload-operations", token=HOST)
assert upload_operations[0] == 200, upload_operations
assert (
    upload_operations[1]["summary"]["totalJobs"]
    == state[1]["uploadOperations"]["summary"]["totalJobs"]
), upload_operations
assert (
    upload_operations[1]["summary"]["readyAssets"]
    == sum(
        1
        for item in upload_operations[1]["records"]
        if item["mediaAsset"] is not None and item["mediaAsset"]["status"] == "ready"
    )
), upload_operations

upload_jobs = req("/api/v1/creator/me/upload-jobs", token=HOST)
assert upload_jobs[0] == 200, upload_jobs
media_assets = req("/api/v1/creator/me/media-assets", token=HOST)
assert media_assets[0] == 200, media_assets
assert len(upload_jobs[1]) >= upload_operations[1]["summary"]["totalJobs"], upload_jobs
assert len(media_assets[1]) >= upload_operations[1]["summary"]["readyAssets"], media_assets

print("creator-app-state|bootstrap|consistent")
