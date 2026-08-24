import json
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
HEADERS = {"Authorization": "Bearer vanta-local-dev-token"}


def req(path, method="GET", body=None):
    headers = dict(HEADERS)
    data = None
    if body is not None:
        headers["Content-Type"] = "application/json"
        data = json.dumps(body).encode()
    request = urllib.request.Request(BASE + path, headers=headers, data=data, method=method)
    try:
        with urllib.request.urlopen(request) as response:
            raw = response.read().decode()
            return response.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        return exc.code, json.loads(raw) if raw else None


live_control = req("/api/v1/creator/me/live/control")
assert live_control[0] == 200, live_control
payload = live_control[1]
assert payload["settings"]["activeSceneId"], payload
assert any(
    scene["id"] == payload["settings"]["activeSceneId"]
    for scene in payload["settings"]["scenes"]
), payload
assert payload["bitrateHistory"][-1] == payload["health"]["samples"][-1]["bitrateKbps"], payload
assert payload["viewerHistory"][-1] == payload["health"]["samples"][-1]["viewers"], payload
assert payload["currentViewers"] == payload["viewerHistory"][-1], payload
if payload["snapshot"]["ingestSession"] is not None:
    assert (
        payload["health"]["currentBitrateKbps"]
        == payload["snapshot"]["ingestSession"]["bitrateKbps"]
    ), payload
else:
    assert payload["snapshot"]["currentBroadcast"] is None, payload
assert payload["snapshot"]["profile"]["subscribers"] == sum(
    tier["subscriberCount"] for tier in payload["subscriberTiers"]
), payload

content = req("/api/v1/creator/me/content?kind=all&status=all&sort=uploaded")
assert content[0] == 200, content
summary = content[1]["summary"]
uploads = content[1]["uploads"]
assert summary["totalUploads"] == len(uploads), summary
assert summary["filteredCount"] == len(uploads), summary
assert summary["publishedUploads"] == sum(1 for item in uploads if item["status"] == "published"), summary
assert summary["scheduledUploads"] == sum(1 for item in uploads if item["status"] == "scheduled"), summary
assert summary["processingUploads"] == sum(1 for item in uploads if item["status"] == "processing"), summary
assert summary["draftUploads"] == sum(1 for item in uploads if item["status"] == "draft"), summary
assert summary["archivedUploads"] == sum(1 for item in uploads if item["status"] == "archived"), summary

sorted_views = req("/api/v1/creator/me/content?kind=all&status=all&sort=views")
assert sorted_views[0] == 200, sorted_views
view_counts = [item["views"] for item in sorted_views[1]["uploads"]]
assert view_counts == sorted(view_counts, reverse=True), view_counts

episode_only = req("/api/v1/creator/me/content?kind=episode&status=all&sort=uploaded")
assert episode_only[0] == 200, episode_only
assert all(item["kind"] == "episode" for item in episode_only[1]["uploads"]), episode_only

bulk_delete_published = req(
    "/api/v1/creator/me/uploads/bulk",
    "POST",
    {"uploadIds": ["up-vod-rust-queue"], "action": "delete"},
)
assert (
    bulk_delete_published[0] == 400
    and "only draft, archived, or taken-down uploads can be deleted"
    in bulk_delete_published[1]["error"]
), bulk_delete_published

bulk_archive_processing = req(
    "/api/v1/creator/me/uploads/bulk",
    "POST",
    {"uploadIds": ["up-halcyon-s1e2"], "action": "archive"},
)
assert (
    bulk_archive_processing[0] == 400
    and "processing uploads cannot be archived" in bulk_archive_processing[1]["error"]
), bulk_archive_processing

print("creator-control-content-pass")
