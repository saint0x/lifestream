import json
import os
import subprocess
import tempfile
import time
import urllib.request
import uuid

BASE = "http://127.0.0.1:8080"
AUTH = "Bearer vanta-local-dev-token"


def open_json(request):
    with urllib.request.urlopen(request) as response:
        raw = response.read().decode()
        return json.loads(raw) if raw else None


def open_creator_media(path):
    request = urllib.request.Request(
        BASE + path,
        headers={"Authorization": AUTH},
    )
    return urllib.request.urlopen(request)


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

payload = open(path, "rb").read()
size = len(payload)
run_id = uuid.uuid4().hex
title = f"Fozzy pipeline validation {run_id}"
storage_key = f"uploads/creator/deepsaint/features/fozzy-pipeline-validation-{run_id}.mp4"

create = urllib.request.Request(
    BASE + "/api/v1/creator/me/upload-jobs",
    data=json.dumps(
        {
            "kind": "film",
            "sourceType": "resumable-upload",
            "title": title,
            "intendedVisibility": "public",
            "bytesExpected": size,
            "storageKey": storage_key,
            "mimeType": "video/mp4",
        }
    ).encode(),
    headers={
        "content-type": "application/json",
        "Authorization": AUTH,
    },
    method="POST",
)
job = open_json(create)

start = urllib.request.Request(
    BASE + f"/api/v1/creator/me/upload-jobs/{job['id']}/ingest",
    headers={"Authorization": AUTH},
    method="POST",
)
ticket = open_json(start)
token = ticket["uploadToken"]

chunk = urllib.request.Request(
    BASE + f"/api/v1/creator/me/upload-jobs/{job['id']}/ingest/chunk?offset=0",
    data=payload,
    headers={
        "Authorization": AUTH,
        "x-upload-token": token,
    },
    method="PUT",
)
open_json(chunk)

complete = urllib.request.Request(
    BASE + f"/api/v1/creator/me/upload-jobs/{job['id']}/ingest/complete",
    headers={
        "Authorization": AUTH,
        "x-upload-token": token,
    },
    method="POST",
)
open_json(complete)

asset = None
for _ in range(30):
    time.sleep(1)
    request = urllib.request.Request(
        BASE + f"/api/v1/creator/me/upload-jobs/{job['id']}/media-asset",
        headers={"Authorization": AUTH},
    )
    asset = open_json(request)
    if asset["status"] == "ready":
        break

assert asset["status"] == "ready", asset

playback = open_creator_media(asset["playbackUrl"])
poster = open_creator_media(asset["posterUrl"])

publish = urllib.request.Request(
    BASE + f"/api/v1/creator/me/upload-jobs/{job['id']}/publish",
    data=json.dumps({"description": "fozzy validation publish"}).encode(),
    headers={
        "Authorization": AUTH,
        "content-type": "application/json",
    },
    method="POST",
)
uploaded = open_json(publish)

print(
    f"{asset['status']}|{len(asset['variants'])}|{playback.status}|{poster.status}|{uploaded['status']}|{uploaded['resolution']}"
)

os.remove(path)
