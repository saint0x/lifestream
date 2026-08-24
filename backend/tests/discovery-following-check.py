import json
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
HOST = "Bearer vanta-local-dev-token"


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


streamers = req("/api/v1/streamers")
assert streamers[0] == 200 and len(streamers[1]) > 0, streamers
target_streamer_id = streamers[1][0]["id"]

followed = req(f"/api/v1/me/following/{target_streamer_id}", "POST", HOST)
assert followed[0] == 200 and target_streamer_id in followed[1]["following"], followed

feed = req("/api/v1/me/following", token=HOST)
assert feed[0] == 200, feed
feed_streamer_ids = [item["id"] for item in feed[1]["followedStreamers"]]
assert target_streamer_id in feed_streamer_ids, feed
for stream in feed[1]["liveStreams"]:
    assert stream["streamer"]["id"] in feed_streamer_ids, stream

discovery = req("/api/v1/live/discovery?category=Tech&sort=newest&limit=3")
assert discovery[0] == 200, discovery
assert discovery[1]["activeCategory"] == "Tech", discovery
assert discovery[1]["activeSort"] == "newest", discovery
assert discovery[1]["totalChannels"] >= len(discovery[1]["streams"]), discovery
assert discovery[1]["totalViewers"] >= sum(item["viewers"] for item in discovery[1]["streams"]), discovery
for stream in discovery[1]["streams"]:
    assert stream["category"] == "Tech", stream
started_ats = [item["startedAt"] for item in discovery[1]["streams"]]
assert started_ats == sorted(started_ats, reverse=True), started_ats

bad_discovery = req("/api/v1/live/discovery?category=Unknown")
assert bad_discovery[0] == 400 and "unknown live category filter" in bad_discovery[1]["error"], bad_discovery

categories = req("/api/v1/categories")
assert categories[0] == 200 and len(categories[1]) > 0, categories
category_slug = categories[1][0]["slug"]
category_name = categories[1][0]["name"]

browse = req(f"/api/v1/categories/{category_slug}/browse")
assert browse[0] == 200, browse
assert browse[1]["category"]["slug"] == category_slug, browse
assert browse[1]["category"]["name"] == category_name, browse
assert browse[1]["totalVodTitles"] == len(browse[1]["series"]) + len(browse[1]["films"]), browse
for stream in browse[1]["liveStreams"]:
    assert stream["category"] == category_name, stream
for series in browse[1]["series"]:
    assert category_name in series["genres"], series
for film in browse[1]["films"]:
    assert category_name in film["genres"], film

print("discovery-following-pass")
