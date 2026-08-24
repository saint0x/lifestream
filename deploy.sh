#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$ROOT_DIR/frontend"
API_URL="${VANTA_PRODUCTION_API_URL:-https://api-production-4becb.up.railway.app}"
WEB_URL="${VANTA_PRODUCTION_WEB_URL:-https://streamvanta.tv}"
RAILWAY_SERVICE="${RAILWAY_SERVICE:-api}"

usage() {
  cat <<EOF
Usage: ./deploy.sh <command>

Commands:
  backend       Deploy the Railway API service
  frontend      Deploy the Vercel frontend to production
  all           Deploy backend, then frontend
  status        Show Railway deployment status
  smoke         Run production API smoke checks plus search probes

Environment:
  RAILWAY_SERVICE=$RAILWAY_SERVICE
  VANTA_PRODUCTION_API_URL=$API_URL
  VANTA_PRODUCTION_WEB_URL=$WEB_URL
EOF
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required command: $1" >&2
    exit 1
  }
}

deploy_backend() {
  require_cmd railway
  railway up --service "$RAILWAY_SERVICE" --detach
}

deploy_frontend() {
  require_cmd vercel
  (
    cd "$FRONTEND_DIR"
    VITE_VANTA_API_BASE_URL="$API_URL" vercel deploy --prod --yes
  )
}

smoke_search() {
  python3 - <<PY
import json, urllib.parse, urllib.request
base = "$API_URL"
for query in ["northlight", "Mara Vale", "cinematic tech"]:
    url = base + "/api/v1/search?" + urllib.parse.urlencode({"q": query, "limit": 5})
    data = json.load(urllib.request.urlopen(url))
    assert data["items"], (query, data)
    print(query, "=>", [(item["kind"], item["title"]) for item in data["items"][:5]])
PY
}

case "${1:-}" in
  backend)
    deploy_backend
    ;;
  frontend)
    deploy_frontend
    ;;
  all)
    deploy_backend
    deploy_frontend
    ;;
  status)
    require_cmd railway
    railway deployment list --service "$RAILWAY_SERVICE" --json
    ;;
  smoke)
    node "$ROOT_DIR/scripts/smoke-production-api.mjs"
    smoke_search
    echo "$WEB_URL"
    ;;
  *)
    usage
    exit 1
    ;;
esac
