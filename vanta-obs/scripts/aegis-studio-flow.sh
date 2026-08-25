#!/usr/bin/env bash
set -euo pipefail

AEGIS_ADDR="${AEGIS_ADDR:-127.0.0.1:7878}"
APP_URL="${APP_URL:-http://localhost:5184/}"
API_BASE="${API_BASE:-http://127.0.0.1:4127}"
USER_ID="${VANTA_USER_ID:-user_creator_owner}"
ROLE="${VANTA_ROLE:-creator_owner}"

require_bin() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required binary: $1" >&2
    exit 127
  }
}

api() {
  local method="$1"
  local path="$2"
  local body="${3:-}"
  if [[ -n "$body" ]]; then
    curl -fsS \
      -X "$method" \
      -H "Accept: application/json" \
      -H "Content-Type: application/json" \
      -H "X-Vanta-User-Id: $USER_ID" \
      -H "X-Vanta-Role: $ROLE" \
      -d "$body" \
      "$API_BASE$path"
  else
    curl -fsS \
      -X "$method" \
      -H "Accept: application/json" \
      -H "X-Vanta-User-Id: $USER_ID" \
      -H "X-Vanta-Role: $ROLE" \
      "$API_BASE$path"
  fi
}

inspect_page() {
  aegis --server-addr "$AEGIS_ADDR" page inspect
}

assert_json_contains() {
  local json="$1"
  local jq_path="$2"
  local needle="$3"
  if ! jq -e --arg needle "$needle" "$jq_path | contains(\$needle)" >/dev/null <<<"$json"; then
    echo "expected page snapshot $jq_path to contain: $needle" >&2
    exit 1
  fi
}

require_bin aegis
require_bin curl
require_bin jq

dashboard="$(api GET /api/v1/obs/me/dashboard)"
broadcast_id="$(jq -r '.broadcast.id' <<<"$dashboard")"
initial_replays="$(jq -r '.replays | length' <<<"$dashboard")"
flow_title="Aegis Flow $(date +%H%M%S)"

aegis --server-addr "$AEGIS_ADDR" navigate "$APP_URL" >/dev/null
initial_page="$(inspect_page)"
assert_json_contains "$initial_page" '.content_scopes.controls_text' "Expand Scenes"
assert_json_contains "$initial_page" '.content_scopes.controls_text' "Expand Channel"
assert_json_contains "$initial_page" '.content_scopes.controls_text' "Expand Runtime"
assert_json_contains "$initial_page" '.content_scopes.main_text' "PROGRAM"
assert_json_contains "$initial_page" '.content_scopes.main_text' "PREVIEW"

api PATCH "/api/v1/obs/me/broadcasts/$broadcast_id" "$(jq -nc --arg title "$flow_title" '{
  title: $title,
  category: "Technology",
  latency_profile: "low"
}')" >/dev/null
api POST "/api/v1/obs/me/broadcasts/$broadcast_id/audience/telemetry" '{
  "viewer_count": 1337,
  "chat_messages_per_minute": 144,
  "tips_cents": 1200,
  "subscriptions": 2,
  "revenue_cents": 3400,
  "discovery_source": "aegis_flow",
  "discovery_score": 91.2,
  "details_json": { "surface": "aegis" }
}' >/dev/null
api POST "/api/v1/obs/me/broadcasts/$broadcast_id/replay-buffer/save" '{
  "duration_seconds": 5,
  "label": "Aegis browser replay",
  "sponsor_proof": true
}' >/dev/null

aegis --server-addr "$AEGIS_ADDR" navigate "$APP_URL" >/dev/null
updated_page="$(inspect_page)"
updated_dashboard="$(api GET /api/v1/obs/me/dashboard)"
updated_replays="$(jq -r '.replays | length' <<<"$updated_dashboard")"

assert_json_contains "$updated_page" '.content_scopes.main_text' "$flow_title"
assert_json_contains "$updated_page" '.content_scopes.main_text' "1,337"
if [[ "$updated_replays" -le "$initial_replays" ]]; then
  echo "expected replay count to increase: before=$initial_replays after=$updated_replays" >&2
  exit 1
fi

jq -nc \
  --arg title "$flow_title" \
  --argjson initial_replays "$initial_replays" \
  --argjson updated_replays "$updated_replays" \
  '{
    ok: true,
    flow: "aegis-studio-operation",
    title: $title,
    replay_count_before: $initial_replays,
    replay_count_after: $updated_replays,
    verified: [
      "collapsed_expand_controls",
      "program_preview_player_window",
      "channel_patch_visible",
      "audience_telemetry_visible",
      "replay_save_visible"
    ]
  }'
