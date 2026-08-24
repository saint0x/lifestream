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


profile = req("/api/v1/me/profile", token=HOST)
assert profile[0] == 200, profile
settings = req("/api/v1/me/settings", token=HOST)
assert settings[0] == 200, settings
plan = req("/api/v1/me/plan", token=HOST)
assert plan[0] == 200 and plan[1]["planName"] == "VANTA Premium", plan

original_display_name = profile[1]["user"]["displayName"]
original_email = profile[1]["email"]
original_mature = profile[1]["matureContentAllowed"]
original_audio = profile[1]["defaultAudio"]
original_preset = profile[1]["subtitlePreset"]
original_filter = profile[1]["liveChatFilter"]

original_playback = settings[1]["playback"]
original_notifications = settings[1]["notifications"]
original_privacy = settings[1]["privacy"]
original_parental = settings[1]["parental"]
original_downloads = settings[1]["downloads"]
original_language = settings[1]["language"]

bad_profile = req(
    "/api/v1/me/profile",
    "PATCH",
    HOST,
    {
        "defaultAudio": "Surround 11.1",
    },
)
assert (
    bad_profile[0] == 400 and "defaultAudio contains an unsupported value" in bad_profile[1]["error"]
), bad_profile

bad_settings = req(
    "/api/v1/me/settings",
    "PATCH",
    HOST,
    {
        "notifications": {
            "seriesReleases": original_notifications["seriesReleases"],
            "liveStreams": original_notifications["liveStreams"],
            "originals": original_notifications["originals"],
            "watchlistUpdates": original_notifications["watchlistUpdates"],
            "creatorUpdates": original_notifications["creatorUpdates"],
            "securityAlerts": {
                "label": original_notifications["securityAlerts"]["label"],
                "push": False,
                "email": False,
                "lock": True,
            },
        }
    },
)
assert (
    bad_settings[0] == 400
    and "securityAlerts must keep push and email enabled" in bad_settings[1]["error"]
), bad_settings

bad_downloads = req(
    "/api/v1/me/settings",
    "PATCH",
    HOST,
    {
        "downloads": {
            "videoQuality": "Ultra (4K)",
            "wifiOnly": True,
            "smartDownloads": True,
            "storageUsedGb": 60.0,
            "storageLimitGb": 50.0,
            "deviceLimit": 4,
            "activeDevices": 2,
        }
    },
)
assert (
    bad_downloads[0] == 400
    and "downloads.storageUsedGb cannot exceed storageLimitGb" in bad_downloads[1]["error"]
), bad_downloads

updated_profile = req(
    "/api/v1/me/profile",
    "PATCH",
    HOST,
    {
        "displayName": "Deep Saint Control",
        "email": "deepsaint.control@vanta.tv",
        "matureContentAllowed": False,
        "defaultAudio": "Original language",
        "subtitlePreset": "English · Large",
        "autoplayTrailers": True,
        "liveChatFilter": "Strict",
    },
)
assert updated_profile[0] == 200, updated_profile
assert updated_profile[1]["user"]["displayName"] == "Deep Saint Control", updated_profile
assert updated_profile[1]["email"] == "deepsaint.control@vanta.tv", updated_profile
assert updated_profile[1]["defaultAudio"] == "Original language", updated_profile
assert updated_profile[1]["liveChatFilter"] == "Strict", updated_profile

updated_settings = req(
    "/api/v1/me/settings",
    "PATCH",
    HOST,
    {
        "playback": {
            "defaultQuality": "1080p",
            "audioLanguage": "Original language",
            "subtitleLanguage": "Japanese",
            "subtitleStyle": "English · High contrast",
            "autoplayNextEpisode": False,
            "autoplayTrailers": True,
            "reducedMotion": True,
            "preferDubbed": True,
            "playbackSpeed": "1.25×",
        },
        "privacy": {
            "showFriendActivity": True,
            "improveRecommendations": False,
            "personalizedAds": False,
            "abTests": False,
            "dataExportSizeMb": 24.5,
            "deleteCooldownDays": 14,
        },
        "parental": {
            "maxRating": "TV-14",
            "requirePinForMature": True,
            "hideLiveChatForKids": True,
            "blockMatureLiveStreams": True,
            "pinSet": True,
        },
        "downloads": {
            "videoQuality": "Ultra (4K)",
            "wifiOnly": True,
            "smartDownloads": False,
            "storageUsedGb": 8.5,
            "storageLimitGb": 64.0,
            "deviceLimit": 4,
            "activeDevices": 3,
        },
        "language": {
            "interfaceLanguage": "English (UK)",
            "subtitleLanguage": "German",
            "catalogRegion": "Canada",
            "dateFormat": "YYYY-MM-DD",
            "clockFormat": "24 hour",
        },
    },
)
assert updated_settings[0] == 200, updated_settings
assert updated_settings[1]["playback"]["defaultQuality"] == "1080p", updated_settings
assert updated_settings[1]["downloads"]["videoQuality"] == "Ultra (4K)", updated_settings
assert updated_settings[1]["language"]["catalogRegion"] == "Canada", updated_settings

# restore original state so later tests still see the seeded defaults
restore_profile = req(
    "/api/v1/me/profile",
    "PATCH",
    HOST,
    {
        "displayName": original_display_name,
        "email": original_email,
        "matureContentAllowed": original_mature,
        "defaultAudio": original_audio,
        "subtitlePreset": original_preset,
        "autoplayTrailers": profile[1]["autoplayTrailers"],
        "liveChatFilter": original_filter,
    },
)
assert restore_profile[0] == 200, restore_profile

restore_settings = req(
    "/api/v1/me/settings",
    "PATCH",
    HOST,
    {
        "playback": original_playback,
        "notifications": original_notifications,
        "privacy": original_privacy,
        "parental": original_parental,
        "downloads": original_downloads,
        "language": original_language,
    },
)
assert restore_settings[0] == 200, restore_settings

print("account-settings|validated|persisted")
