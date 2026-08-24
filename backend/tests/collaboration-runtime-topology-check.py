import json
import time
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
HOST = "Bearer vanta-local-dev-token"
COLLAB = "Bearer vanta-local-collaborator-token"
SUFFIX = str(int(time.time() * 1000))


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


def get_member(payload, role=None, participant_id=None):
    members = payload["topology"]["members"]
    for member in members:
        if role is not None and member["role"] == role:
            return member
        if participant_id is not None and member["participantId"] == participant_id:
            return member
    raise AssertionError((role, participant_id, members))


live_status, live = req("/api/v1/creator/me/live", token=HOST)
assert live_status == 200, (live_status, live)
for broadcast_key in ("currentBroadcast", "pendingBroadcast"):
    broadcast = live.get(broadcast_key)
    if broadcast is not None:
        ended = req(f"/api/v1/creator/me/broadcasts/{broadcast['id']}/end", "POST", HOST)
        assert ended[0] == 200, ended

start = req(
    "/api/v1/creator/me/broadcasts/start",
    "POST",
    HOST,
    {
        "title": f"Collaboration topology validation {SUFFIX}",
        "category": "Tech",
        "tags": ["collaboration", "runtime", "topology"],
        "isMature": False,
        "notifyFollowers": False,
    },
)
assert start[0] == 200, start
broadcast = start[1]

created = req(
    "/api/v1/creator/me/live/collabs/sessions",
    "POST",
    HOST,
    {
        "broadcastId": broadcast["id"],
        "title": f"Topology flow {SUFFIX}",
        "chatMode": "shared",
        "recordingPolicy": "host_archive",
    },
)
assert created[0] == 200, created
session = created[1]
session_id = session["id"]

host_runtime = req(
    f"/api/v1/creator/me/live/collabs/sessions/{session_id}/runtime",
    token=HOST,
)
assert host_runtime[0] == 200, host_runtime
host_payload = host_runtime[1]
host_member = get_member(host_payload, role="host")
assert host_payload["session"]["participant"]["id"] == host_member["participantId"], host_payload
assert host_payload["topology"]["sharedChat"] is True, host_payload
assert host_payload["topology"]["recordingOwnerCreatorId"] == host_payload["session"]["hostCreatorId"], host_payload
assert host_payload["topology"]["liveParticipantIds"] == [host_member["participantId"]], host_payload
assert host_payload["topology"]["hostOutputParticipantIds"] == [host_member["participantId"]], host_payload
assert host_member["hostOutputState"] == "host", host_payload
assert host_member["mirrorPickupState"] == "host", host_payload

invite = req(
    f"/api/v1/creator/me/live/collabs/sessions/{session_id}/invites",
    "POST",
    HOST,
    {
        "inviteeUserId": "usr-2",
        "role": "co_streamer",
        "mirrorToGuestChannel": True,
        "message": "topology invite",
        "expiresInMinutes": 30,
    },
)
assert invite[0] == 200, invite

accepted = req(f"/api/v1/live/collabs/invites/{invite[1]['id']}/accept", "POST", COLLAB)
assert accepted[0] == 200 and accepted[1]["state"] == "backstage", accepted
guest_participant_id = accepted[1]["id"]

guest_runtime_backstage = req(
    f"/api/v1/me/live/collabs/sessions/{session_id}/runtime",
    token=COLLAB,
)
assert guest_runtime_backstage[0] == 200, guest_runtime_backstage
guest_backstage_payload = guest_runtime_backstage[1]
guest_member = get_member(guest_backstage_payload, participant_id=guest_participant_id)
assert guest_backstage_payload["session"]["participant"]["id"] == guest_participant_id, guest_backstage_payload
assert guest_participant_id in guest_backstage_payload["topology"]["backstageParticipantIds"], guest_backstage_payload
assert guest_member["hostOutputState"] == "backstage", guest_backstage_payload
assert guest_member["mirrorPickupState"] == "eligible", guest_backstage_payload

live_guest = req(
    f"/api/v1/creator/me/live/collabs/sessions/{session_id}/participants/{guest_participant_id}",
    "PATCH",
    HOST,
    {
        "state": "live",
        "mirrorToGuestChannel": True,
        "publishToHost": True,
        "canSpeakInChat": True,
    },
)
assert live_guest[0] == 200 and live_guest[1]["state"] == "live", live_guest

grant = req(
    f"/api/v1/creator/me/live/collabs/sessions/{session_id}/participants/{guest_participant_id}/grants/mirror",
    "POST",
    HOST,
)
assert grant[0] == 200 and grant[1]["state"] == "issued", grant

creator_runtime = req(
    f"/api/v1/creator/me/live/collabs/sessions/{session_id}/runtime",
    token=HOST,
)
assert creator_runtime[0] == 200, creator_runtime
creator_payload = creator_runtime[1]
guest_member_issued = get_member(creator_payload, participant_id=guest_participant_id)
assert guest_participant_id in creator_payload["topology"]["liveParticipantIds"], creator_payload
assert guest_participant_id in creator_payload["topology"]["hostOutputParticipantIds"], creator_payload
assert guest_member_issued["hostOutputState"] == "live", creator_payload
assert guest_member_issued["mirrorPickupState"] == "issued", creator_payload
assert creator_payload["topology"]["mirroredCreatorIds"] == [grant[1]["guestCreatorId"]], creator_payload
assert any(item["id"] == grant[1]["id"] and item["state"] == "issued" for item in creator_payload["grants"]), creator_payload
assert any(event["eventType"] == "mirror_grant_issued" for event in creator_payload["recentEvents"]), creator_payload

redeemed = req(f"/api/v1/live/collabs/grants/{grant[1]['id']}/redeem", "POST", COLLAB)
assert redeemed[0] == 200 and redeemed[1]["state"] == "active", redeemed

guest_runtime_live = req(
    f"/api/v1/me/live/collabs/sessions/{session_id}/runtime",
    token=COLLAB,
)
assert guest_runtime_live[0] == 200, guest_runtime_live
guest_live_payload = guest_runtime_live[1]
guest_member_active = get_member(guest_live_payload, participant_id=guest_participant_id)
assert guest_member_active["mirrorPickupState"] == "active", guest_live_payload
assert guest_member_active["canSpeakInChat"] is True, guest_live_payload
assert guest_live_payload["topology"]["connectedParticipants"] == 0, guest_live_payload
assert any(item["id"] == redeemed[1]["id"] and item["state"] == "active" for item in guest_live_payload["grants"]), guest_live_payload
assert any(event["eventType"] == "mirror_grant_redeemed" for event in guest_live_payload["recentEvents"]), guest_live_payload

ended = req(f"/api/v1/creator/me/live/collabs/sessions/{session_id}/end", "POST", HOST)
assert ended[0] == 200 and ended[1]["status"] == "ended", ended

print("collab-runtime|backstage|live|mirrored|ended")
