#!/usr/bin/env bash
set -euo pipefail

tmp_dir="$(mktemp -d)"
port="${VANTA_EDITOR_SMOKE_PORT:-42117}"
base="http://127.0.0.1:${port}"
editor_db="${tmp_dir}/editor.db"
pipeline_db="${tmp_dir}/pipeline.db"
media_root="${tmp_dir}/storage"
ad_hub_outbox="${tmp_dir}/ad-hub"
sample="${tmp_dir}/sample.mp4"
server_pid=""

cleanup() {
  if [[ -n "${server_pid}" ]]; then
    kill "${server_pid}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

VANTA_EDITOR_DATABASE="${editor_db}" \
VANTA_EDITOR_MEDIA_ROOT="${media_root}" \
VANTA_MEDIA_PIPELINE_DATABASE="${pipeline_db}" \
VANTA_AD_HUB_OUTBOX="${ad_hub_outbox}" \
VANTA_EDITOR_BIND_ADDR="127.0.0.1:${port}" \
cargo run >/tmp/vanta-editor-integration-smoke.log 2>&1 &
server_pid="$!"

for _ in $(seq 1 60); do
  if curl -fsS "${base}/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.25
done
curl -fsS "${base}/health" >/dev/null

ffmpeg -y \
  -f lavfi -i testsrc=size=320x180:rate=24 \
  -f lavfi -i sine=frequency=440:sample_rate=48000 \
  -t 1 -c:v libx264 -pix_fmt yuv420p -c:a aac \
  "${sample}" >/dev/null 2>&1

project_id="$(curl -fsS "${base}/api/v1/editor/me/projects" | ruby -rjson -e 'puts JSON.parse(STDIN.read).fetch(0).fetch("id")')"

forbidden_status="$(curl -sS -o /dev/null -w "%{http_code}" -X POST \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/projects/${project_id}/campaign-requirements" \
  -d '{"title":"Forbidden campaign requirement"}')"
[[ "${forbidden_status}" == "403" ]]

crud_project_id="$(curl -fsS -X POST \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/projects" \
  -d '{"title":"CRUD smoke project","source_kind":"imported_raw","campaign_id":"campaign_crud"}' \
  | ruby -rjson -e 'puts JSON.parse(STDIN.read).fetch("id")')"

curl -fsS -X PATCH \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/projects/${crud_project_id}" \
  -d '{"status":"editing"}' >/dev/null

crud_asset_id="$(curl -fsS -X POST \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/projects/${crud_project_id}/import-media-asset" \
  -d '{"media_asset_id":"media_crud_existing","role":"b_roll"}' \
  | ruby -rjson -e 'puts JSON.parse(STDIN.read).fetch("id")')"
curl -fsS -X PATCH \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/assets/${crud_asset_id}" \
  -d '{"display_name":"CRUD asset","processing_status":"ready","rights_status":"cleared","duration_seconds":12.0,"metadata_json":{"checked":true}}' >/dev/null

crud_track_id="$(curl -fsS -X POST \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/projects/${crud_project_id}/tracks" \
  -d '{"kind":"video","name":"CRUD V1"}' \
  | ruby -rjson -e 'puts JSON.parse(STDIN.read).fetch("id")')"
curl -fsS -X PATCH \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/tracks/${crud_track_id}" \
  -d '{"name":"CRUD V1 renamed","visible":true,"muted":false}' >/dev/null

crud_clip_id="$(curl -fsS -X POST \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/projects/${crud_project_id}/clips" \
  -d "{\"track_id\":\"${crud_track_id}\",\"media_asset_id\":\"media_crud_existing\",\"label\":\"CRUD clip\",\"source_in_seconds\":0,\"source_out_seconds\":10,\"timeline_in_seconds\":0,\"timeline_out_seconds\":10}" \
  | ruby -rjson -e 'puts JSON.parse(STDIN.read).fetch("id")')"
curl -fsS -X PATCH \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/clips/${crud_clip_id}" \
  -d '{"label":"CRUD clip trimmed","timeline_in_seconds":1,"timeline_out_seconds":8}' >/dev/null

crud_slot_id="$(curl -fsS -X POST \
  -H 'X-Vanta-User-Id: user_ad_ops' \
  -H 'X-Vanta-Role: vanta_ad_ops' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/projects/${crud_project_id}/ad-slots" \
  -d '{"label":"CRUD sponsor slot","placement_type":"mid-roll","timeline_in_seconds":2,"timeline_out_seconds":7,"required_duration_seconds":5}' \
  | ruby -rjson -e 'puts JSON.parse(STDIN.read).fetch("id")')"
curl -fsS -X POST "${base}/api/v1/editor/me/ad-slots/${crud_slot_id}/validate" >/dev/null
curl -fsS -X PATCH \
  -H 'X-Vanta-User-Id: user_ad_ops' \
  -H 'X-Vanta-Role: vanta_ad_ops' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/ad-slots/${crud_slot_id}" \
  -d '{"status":"draft","review_status":"needs_changes","measurement_key":"crud-measurement"}' >/dev/null

crud_requirement_id="$(curl -fsS -X POST \
  -H 'X-Vanta-User-Id: user_ad_ops' \
  -H 'X-Vanta-Role: vanta_ad_ops' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/projects/${crud_project_id}/campaign-requirements" \
  -d '{"campaign_id":"campaign_crud","title":"CRUD requirement","requirement_kind":"sponsor_deliverable","status":"draft","body_json":{"copy":"approved claims only"}}' \
  | ruby -rjson -e 'puts JSON.parse(STDIN.read).fetch("id")')"
curl -fsS -X PATCH \
  -H 'X-Vanta-User-Id: user_ad_ops' \
  -H 'X-Vanta-Role: vanta_ad_ops' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/campaign-requirements/${crud_requirement_id}" \
  -d '{"status":"approved","body_json":{"copy":"approved claims only","legal":true}}' >/dev/null

crud_transcript_id="$(curl -fsS -X POST \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/projects/${crud_project_id}/transcript" \
  -d '{"start_seconds":0,"end_seconds":4,"speaker":"Host","text":"CRUD transcript","flags_json":{"ad_read":false}}' \
  | ruby -rjson -e 'puts JSON.parse(STDIN.read).fetch("id")')"
curl -fsS -X PATCH \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/transcript/${crud_transcript_id}" \
  -d '{"text":"CRUD transcript updated","flags_json":{"ad_read":true}}' >/dev/null

crud_comment_id="$(curl -fsS -X POST \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/projects/${crud_project_id}/comments" \
  -d '{"timeline_seconds":3,"body":"CRUD comment","visibility":"creator_team"}' \
  | ruby -rjson -e 'puts JSON.parse(STDIN.read).fetch("id")')"
curl -fsS -X PATCH \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/comments/${crud_comment_id}" \
  -d '{"body":"CRUD comment updated","resolved":false}' >/dev/null
curl -fsS -X POST \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  "${base}/api/v1/editor/me/comments/${crud_comment_id}/resolve" >/dev/null

curl -fsS -X POST \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  "${base}/api/v1/editor/me/projects/${crud_project_id}/timeline/versions" >/dev/null
crud_review_id="$(curl -fsS -X POST \
  -H 'X-Vanta-User-Id: user_ad_ops' \
  -H 'X-Vanta-Role: vanta_ad_ops' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/projects/${crud_project_id}/review-requests" \
  -d '{"review_kind":"advertiser","due_at":"2026-08-31T00:00:00Z"}' \
  | ruby -rjson -e 'puts JSON.parse(STDIN.read).fetch("id")')"
curl -fsS -X PATCH \
  -H 'X-Vanta-User-Id: user_ad_ops' \
  -H 'X-Vanta-Role: vanta_ad_ops' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/review-requests/${crud_review_id}" \
  -d '{"status":"approved"}' >/dev/null

curl -fsS "${base}/api/v1/editor/me/projects/${crud_project_id}" >/dev/null
curl -fsS "${base}/api/v1/editor/me/projects/${crud_project_id}/assets" >/dev/null
curl -fsS "${base}/api/v1/editor/me/projects/${crud_project_id}/tracks" >/dev/null
curl -fsS "${base}/api/v1/editor/me/projects/${crud_project_id}/clips" >/dev/null
curl -fsS "${base}/api/v1/editor/me/projects/${crud_project_id}/ad-slots" >/dev/null
curl -fsS "${base}/api/v1/editor/me/projects/${crud_project_id}/campaign-requirements" >/dev/null
curl -fsS "${base}/api/v1/editor/me/projects/${crud_project_id}/transcript" >/dev/null
curl -fsS "${base}/api/v1/editor/me/projects/${crud_project_id}/comments" >/dev/null
curl -fsS "${base}/api/v1/editor/me/projects/${crud_project_id}/review-requests" >/dev/null

curl -fsS -X DELETE -H 'X-Vanta-User-Id: user_ad_ops' -H 'X-Vanta-Role: vanta_ad_ops' "${base}/api/v1/editor/me/review-requests/${crud_review_id}" >/dev/null
curl -fsS -X DELETE -H 'X-Vanta-User-Id: user_creator_owner' -H 'X-Vanta-Role: creator_owner' "${base}/api/v1/editor/me/comments/${crud_comment_id}" >/dev/null
curl -fsS -X DELETE -H 'X-Vanta-User-Id: user_creator_owner' -H 'X-Vanta-Role: creator_owner' "${base}/api/v1/editor/me/transcript/${crud_transcript_id}" >/dev/null
curl -fsS -X DELETE -H 'X-Vanta-User-Id: user_ad_ops' -H 'X-Vanta-Role: vanta_ad_ops' "${base}/api/v1/editor/me/campaign-requirements/${crud_requirement_id}" >/dev/null
curl -fsS -X DELETE -H 'X-Vanta-User-Id: user_ad_ops' -H 'X-Vanta-Role: vanta_ad_ops' "${base}/api/v1/editor/me/ad-slots/${crud_slot_id}" >/dev/null
curl -fsS -X DELETE -H 'X-Vanta-User-Id: user_creator_owner' -H 'X-Vanta-Role: creator_owner' "${base}/api/v1/editor/me/clips/${crud_clip_id}" >/dev/null
curl -fsS -X DELETE -H 'X-Vanta-User-Id: user_creator_owner' -H 'X-Vanta-Role: creator_owner' "${base}/api/v1/editor/me/tracks/${crud_track_id}" >/dev/null
curl -fsS -X DELETE -H 'X-Vanta-User-Id: user_creator_owner' -H 'X-Vanta-Role: creator_owner' "${base}/api/v1/editor/me/assets/${crud_asset_id}" >/dev/null
curl -fsS -X DELETE -H 'X-Vanta-User-Id: user_creator_owner' -H 'X-Vanta-Role: creator_owner' "${base}/api/v1/editor/me/projects/${crud_project_id}" >/dev/null

curl -fsS -X POST \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  "${base}/api/v1/editor/me/projects/${project_id}/assets/upload" \
  -F role=raw_video \
  -F display_name=integration-smoke.mp4 \
  -F file=@"${sample}" \
  | ruby -rjson -e 'asset=JSON.parse(STDIN.read); abort "missing source derivative" unless asset.dig("metadata_json", "source_path")'

render_job_id="$(curl -fsS -X POST \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/projects/${project_id}/render-jobs" \
  -d '{"export_kind":"final_vanta_master","target":"hls-master"}' \
  | ruby -rjson -e 'job=JSON.parse(STDIN.read); abort "render did not complete" unless job["status"] == "completed" && job.dig("render_plan_json", "package", "manifest_path"); puts job.fetch("id")')"

export_id="$(curl -fsS "${base}/api/v1/editor/me/projects/${project_id}/exports" | ruby -rjson -e 'items=JSON.parse(STDIN.read); ready=items.reverse.find { |item| item["status"] == "ready" }; abort "no ready export" unless ready; puts ready.fetch("id")')"

curl -fsS -X PATCH \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/exports/${export_id}" \
  -d '{"status":"ready"}' >/dev/null

proof_link_id="$(curl -fsS -X POST \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/exports/${export_id}/proof-link" \
  | ruby -rjson -e 'proof=JSON.parse(STDIN.read); abort "missing proof" unless proof["url"].to_s.include?("/ad-hub/proofs/"); puts proof.fetch("id")')"

curl -fsS -X PATCH \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/proof-links/${proof_link_id}" \
  -d '{"status":"revoked"}' >/dev/null

review_request_id="$(curl -fsS -X POST \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/exports/${export_id}/submit-advertiser-review" \
  | ruby -rjson -e 'review=JSON.parse(STDIN.read); abort "missing ad hub room" unless review.dig("external_room", "mode") == "sqlite-submission-and-outbox"; puts review.fetch("review_request").fetch("id")')"

curl -fsS -X POST \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  -H 'Content-Type: application/json' \
  "${base}/api/v1/editor/me/exports/${export_id}/publish" \
  | ruby -rjson -e 'published=JSON.parse(STDIN.read); abort "missing media pipeline publish" unless published.dig("media_pipeline", "mode") == "sqlite-upsert"'

PIPELINE_DB="${pipeline_db}" AD_HUB_OUTBOX="${ad_hub_outbox}" python3 - <<'PY'
import json
import os
import pathlib
import sqlite3

pipeline_db = pathlib.Path(os.environ["PIPELINE_DB"])
outbox = pathlib.Path(os.environ["AD_HUB_OUTBOX"])
con = sqlite3.connect(pipeline_db)
asset_count = con.execute(
    "select count(*) from media_assets where status='published' and playback_relative_path like '%master.m3u8'"
).fetchone()[0]
submission_count = con.execute(
    "select count(*) from ad_marketplace_submissions where status='review_pending'"
).fetchone()[0]
rooms = list(outbox.glob("room-*.json"))
assert asset_count >= 1, asset_count
assert submission_count >= 1, submission_count
assert rooms, "missing ad hub room"
room = json.loads(rooms[-1].read_text())
assert room["system"] == "vanta-ad-hub"
assert room["submissionUrl"].startswith("https://streamvanta.tv/ad-hub/proofs/")
print("editor-integration-smoke=ok")
PY

curl -fsS -X DELETE \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  "${base}/api/v1/editor/me/proof-links/${proof_link_id}" >/dev/null
curl -fsS -X DELETE \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  "${base}/api/v1/editor/me/review-requests/${review_request_id}" >/dev/null
curl -fsS -X DELETE \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  "${base}/api/v1/editor/me/exports/${export_id}" >/dev/null
curl -fsS -X DELETE \
  -H 'X-Vanta-User-Id: user_creator_owner' \
  -H 'X-Vanta-Role: creator_owner' \
  "${base}/api/v1/editor/me/render-jobs/${render_job_id}" >/dev/null
