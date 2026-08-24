import json
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
HOST = "Bearer vanta-local-dev-token"
COLLAB = "Bearer vanta-local-collaborator-token"


def req(path, method="GET", token=None, body=None):
    headers = {}
    if token:
        headers["Authorization"] = token
    data = None
    if body is not None:
        data = json.dumps(body).encode()
        headers["Content-Type"] = "application/json"
    request = urllib.request.Request(BASE + path, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(request) as response:
            raw = response.read().decode()
            return response.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        return exc.code, json.loads(raw) if raw else None


live_status, live = req("/api/v1/creator/me/live", token=HOST)
assert live_status == 200, (live_status, live)
broadcast = live["currentBroadcast"] or live["pendingBroadcast"]
if broadcast is None:
    start_status, started = req(
        "/api/v1/creator/me/broadcasts/start",
        "POST",
        HOST,
        {
            "title": "collaboration authority validation",
            "category": "Tech",
            "tags": ["collaboration", "authority"],
            "isMature": False,
            "notifyFollowers": False,
        },
    )
    assert start_status == 200, (start_status, started)
    broadcast = started

collabs_status, collabs = req("/api/v1/creator/me/live/collabs", token=HOST)
assert collabs_status == 200, (collabs_status, collabs)
for session in collabs:
    if session["sourceBroadcastId"] == broadcast["id"] and session["status"] in ("pending", "active"):
        end_status, ended = req(
            f"/api/v1/creator/me/live/collabs/sessions/{session['id']}/end",
            "POST",
            HOST,
        )
        assert end_status == 200, (end_status, ended)

created = req(
    "/api/v1/creator/me/live/collabs/sessions",
    "POST",
    HOST,
    {
        "broadcastId": broadcast["id"],
        "title": "authority flow",
        "chatMode": "shared",
        "recordingPolicy": "host_archive",
    },
)
assert created[0] == 200, created
sid = created[1]["id"]

host_view = req(f"/api/v1/creator/me/live/collabs/sessions/{sid}", token=HOST)
assert host_view[0] == 200, host_view
host_pid = next(item["id"] for item in host_view[1]["participants"] if item["role"] == "host")

host_mut = req(
    f"/api/v1/creator/me/live/collabs/sessions/{sid}/participants/{host_pid}",
    "PATCH",
    HOST,
    {"state": "backstage"},
)
assert host_mut[0] == 400 and "host participant" in host_mut[1]["error"], host_mut

invite = req(
    f"/api/v1/creator/me/live/collabs/sessions/{sid}/invites",
    "POST",
    HOST,
    {
        "inviteeUserId": "usr-2",
        "role": "co_streamer",
        "mirrorToGuestChannel": True,
        "message": "authority invite",
        "expiresInMinutes": 30,
    },
)
assert invite[0] == 200, invite

accepted = req(f"/api/v1/live/collabs/invites/{invite[1]['id']}/accept", "POST", COLLAB)
assert accepted[0] == 200 and accepted[1]["state"] == "backstage", accepted
pid = accepted[1]["id"]

grant_before = req(
    f"/api/v1/creator/me/live/collabs/sessions/{sid}/participants/{pid}/grants/mirror",
    "POST",
    HOST,
)
assert grant_before[0] == 400 and "live participants" in grant_before[1]["error"], grant_before

live = req(
    f"/api/v1/creator/me/live/collabs/sessions/{sid}/participants/{pid}",
    "PATCH",
    HOST,
    {
        "state": "live",
        "mirrorToGuestChannel": True,
        "publishToHost": True,
        "canSpeakInChat": True,
    },
)
assert live[0] == 200 and live[1]["state"] == "live", live

grant = req(
    f"/api/v1/creator/me/live/collabs/sessions/{sid}/participants/{pid}/grants/mirror",
    "POST",
    HOST,
)
assert grant[0] == 200 and grant[1]["state"] == "issued", grant

left = req(f"/api/v1/me/live/collabs/sessions/{sid}/leave", "POST", COLLAB)
assert left[0] == 200 and left[1]["state"] == "left", left

left_session = req(f"/api/v1/me/live/collabs/sessions/{sid}", token=COLLAB)
assert left_session[0] == 403, left_session

left_runtime = req(f"/api/v1/me/live/collabs/sessions/{sid}/runtime", token=COLLAB)
assert left_runtime[0] == 403, left_runtime

left_grants = req(f"/api/v1/me/live/collabs/sessions/{sid}/grants", token=COLLAB)
assert left_grants[0] == 403, left_grants

left_sessions = req("/api/v1/me/live/collabs/sessions", token=COLLAB)
assert left_sessions[0] == 200 and all(item["id"] != sid for item in left_sessions[1]), left_sessions

invalid = req(
    f"/api/v1/creator/me/live/collabs/sessions/{sid}/participants/{pid}",
    "PATCH",
    HOST,
    {"state": "live"},
)
assert (
    invalid[0] == 400 and "illegal collaboration participant transition" in invalid[1]["error"]
), invalid

reinvite = req(
    f"/api/v1/creator/me/live/collabs/sessions/{sid}/invites",
    "POST",
    HOST,
    {
        "inviteeUserId": "usr-2",
        "role": "co_streamer",
        "mirrorToGuestChannel": True,
        "message": "come back",
        "expiresInMinutes": 30,
    },
)
assert reinvite[0] == 200, reinvite

rejoined = req(f"/api/v1/live/collabs/invites/{reinvite[1]['id']}/accept", "POST", COLLAB)
assert (
    rejoined[0] == 200
    and rejoined[1]["id"] == pid
    and rejoined[1]["state"] == "backstage"
    and rejoined[1]["leftAt"] is None
), rejoined

rejoin_events = req(f"/api/v1/creator/me/live/collabs/sessions/{sid}/events", token=HOST)
assert rejoin_events[0] == 200 and any(
    event["eventType"] == "participant_rejoined" for event in rejoin_events[1]
), rejoin_events

removed = req(
    f"/api/v1/creator/me/live/collabs/sessions/{sid}/participants/{pid}/remove",
    "POST",
    HOST,
)
assert removed[0] == 200 and removed[1]["state"] == "removed", removed

removed_session = req(f"/api/v1/me/live/collabs/sessions/{sid}", token=COLLAB)
assert removed_session[0] == 403, removed_session

removed_runtime = req(f"/api/v1/me/live/collabs/sessions/{sid}/runtime", token=COLLAB)
assert removed_runtime[0] == 403, removed_runtime

removed_grants = req(f"/api/v1/me/live/collabs/sessions/{sid}/grants", token=COLLAB)
assert removed_grants[0] == 403, removed_grants

removed_sessions = req("/api/v1/me/live/collabs/sessions", token=COLLAB)
assert removed_sessions[0] == 200 and all(item["id"] != sid for item in removed_sessions[1]), removed_sessions

removed_live = req(
    f"/api/v1/creator/me/live/collabs/sessions/{sid}/participants/{pid}",
    "PATCH",
    HOST,
    {"state": "live"},
)
assert (
    removed_live[0] == 400
    and "illegal collaboration participant transition" in removed_live[1]["error"]
), removed_live

ended = req(f"/api/v1/creator/me/live/collabs/sessions/{sid}/end", "POST", HOST)
assert ended[0] == 200 and ended[1]["status"] == "ended", ended

print("collaboration-authority-pass")
