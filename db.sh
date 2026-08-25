#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
API_SERVICE="${RAILWAY_SERVICE:-api}"
POSTGRES_SERVICE="${RAILWAY_POSTGRES_SERVICE:-Postgres}"
ENVIRONMENT="${RAILWAY_ENVIRONMENT:-production}"
LOCAL_DB="${VANTA_DEV_DB:-$ROOT_DIR/backend/vanta.db}"
TUNNEL_PORT="${VANTA_DB_TUNNEL_PORT:-}"
RUN_DIR="${VANTA_DEV_RUN_DIR:-$ROOT_DIR/.dev}"

usage() {
  cat <<EOF
Usage: ./db.sh <command> [args...]

Postgres commands:
  psql              Open production Railway Postgres through Railway's database shell
  tunnel            Open a local SSH tunnel to production Postgres
  proxy             Show Railway TCP proxy status for production Postgres
  ensure-proxy      Create a Railway TCP proxy for production Postgres when missing
  query <sql>       Run SQL against production Postgres
  tables            List production Postgres tables
  search-docs       Show production search document counts by kind
  vars              Show safe database/storage variable presence

SQLite commands:
  local             Open the local SQLite dev database
  local-query <sql> Run SQL against the local SQLite dev database

R2 commands:
  r2-buckets        List Cloudflare R2 buckets with wrangler
  r2-get <key> [file] Fetch an object from the configured R2 bucket
  r2-put <key> <file> Put an object into the configured R2 bucket
  r2-delete <key>   Delete an object from the configured R2 bucket
  r2-ls [prefix]    List objects via AWS CLI when R2 S3 credentials are exported

Environment:
  RAILWAY_SERVICE=$API_SERVICE
  RAILWAY_POSTGRES_SERVICE=$POSTGRES_SERVICE
  RAILWAY_ENVIRONMENT=$ENVIRONMENT
  VANTA_DEV_DB=$LOCAL_DB
  VANTA_DB_TUNNEL_PORT=${TUNNEL_PORT:-auto}
EOF
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required command: $1" >&2
    exit 1
  }
}

railway_api_run() {
  require_cmd railway
  railway run --service "$API_SERVICE" --environment "$ENVIRONMENT" -- "$@"
}

postgres_proxy_url() {
  require_cmd railway
  require_cmd jq
  require_cmd python3

  local vars_json proxies_json endpoint
  vars_json="$(railway variables --service "$POSTGRES_SERVICE" --environment "$ENVIRONMENT" --json)"
  proxies_json="$(railway tcp-proxy list --service "$POSTGRES_SERVICE" --environment "$ENVIRONMENT" --json)"
  endpoint="$(jq -r '.proxies[]? | select(.syncStatus == "ACTIVE") | .endpoint' <<<"$proxies_json" | head -1)"
  [[ -n "$endpoint" ]] || return 1

  POSTGRES_VARS_JSON="$vars_json" POSTGRES_PROXY_ENDPOINT="$endpoint" python3 - <<'PY'
import json
import os
import urllib.parse

vars_json = json.loads(os.environ["POSTGRES_VARS_JSON"])
endpoint = os.environ["POSTGRES_PROXY_ENDPOINT"]
host, port = endpoint.rsplit(":", 1)

raw_url = vars_json.get("DATABASE_URL") or ""
parsed = urllib.parse.urlparse(raw_url)
user = parsed.username or vars_json.get("PGUSER") or vars_json.get("POSTGRES_USER") or "postgres"
password = parsed.password or vars_json.get("PGPASSWORD") or vars_json.get("POSTGRES_PASSWORD") or ""
database = (parsed.path or "").lstrip("/") or vars_json.get("PGDATABASE") or vars_json.get("POSTGRES_DB") or "railway"

print(
    "postgresql://"
    + urllib.parse.quote(user, safe="")
    + ":"
    + urllib.parse.quote(password, safe="")
    + "@"
    + host
    + ":"
    + port
    + "/"
    + urllib.parse.quote(database, safe="")
)
PY
}

run_query() {
  local sql="$1"
  require_cmd railway
  require_cmd psql
  local proxy_url=""
  if proxy_url="$(postgres_proxy_url)"; then
    psql "$proxy_url" -v ON_ERROR_STOP=1 -c "$sql"
    return
  fi

  local log_file="$RUN_DIR/db-tunnel.log"
  mkdir -p "$RUN_DIR"
  local tunnel_args=(connect "$POSTGRES_SERVICE" --environment "$ENVIRONMENT" --tunnel-only)
  if [[ -n "$TUNNEL_PORT" ]]; then
    tunnel_args+=(--port "$TUNNEL_PORT")
  fi
  railway "${tunnel_args[@]}" >"$log_file" 2>&1 &
  local tunnel_pid=$!
  trap 'kill "$tunnel_pid" 2>/dev/null || true' RETURN

  local url=""
  for _ in {1..80}; do
    url="$(grep -Eo 'postgresql://[^[:space:]]+' "$log_file" | tail -1 || true)"
    [[ -n "$url" ]] && break
    if ! kill -0 "$tunnel_pid" 2>/dev/null; then
      cat "$log_file" >&2
      exit 1
    fi
    sleep 0.25
  done
  [[ -n "$url" ]] || { cat "$log_file" >&2; echo "timed out waiting for database tunnel" >&2; exit 1; }
  psql "$url" -v ON_ERROR_STOP=1 -c "$sql"
}

r2_bucket() {
  railway_api_run sh -c 'printf "%s" "${VANTA_OBJECT_STORAGE_BUCKET:-}"'
}

r2_endpoint() {
  if [[ -n "${VANTA_OBJECT_STORAGE_ENDPOINT_URL:-}" ]]; then
    echo "$VANTA_OBJECT_STORAGE_ENDPOINT_URL"
  elif [[ -n "${R2_ENDPOINT_URL:-}" ]]; then
    echo "$R2_ENDPOINT_URL"
  elif [[ -n "${CLOUDFLARE_ACCOUNT_ID:-}" ]]; then
    echo "https://$CLOUDFLARE_ACCOUNT_ID.r2.cloudflarestorage.com"
  else
    echo ""
  fi
}

case "${1:-}" in
  psql)
    require_cmd railway
    railway connect "$POSTGRES_SERVICE" --environment "$ENVIRONMENT"
    ;;
  tunnel)
    require_cmd railway
    if [[ -n "$TUNNEL_PORT" ]]; then
      railway connect "$POSTGRES_SERVICE" --environment "$ENVIRONMENT" --tunnel-only --port "$TUNNEL_PORT"
    else
      railway connect "$POSTGRES_SERVICE" --environment "$ENVIRONMENT" --tunnel-only
    fi
    ;;
  proxy)
    require_cmd railway
    railway tcp-proxy list --service "$POSTGRES_SERVICE" --environment "$ENVIRONMENT" --json
    ;;
  ensure-proxy)
    require_cmd railway
    if railway tcp-proxy list --service "$POSTGRES_SERVICE" --environment "$ENVIRONMENT" --json | jq -e '.proxies[]? | select(.syncStatus == "ACTIVE")' >/dev/null; then
      railway tcp-proxy list --service "$POSTGRES_SERVICE" --environment "$ENVIRONMENT" --json
    else
      railway tcp-proxy create --port 5432 --service "$POSTGRES_SERVICE" --environment "$ENVIRONMENT" --json
    fi
    ;;
  query)
    shift
    [[ $# -gt 0 ]] || { echo "missing SQL" >&2; exit 1; }
    run_query "$*"
    ;;
  tables)
    run_query "SELECT schemaname, tablename FROM pg_catalog.pg_tables WHERE schemaname NOT IN ('pg_catalog', 'information_schema') ORDER BY schemaname, tablename"
    ;;
  search-docs)
    run_query "SELECT kind, count(*) AS documents FROM search_documents GROUP BY kind ORDER BY kind"
    ;;
  vars)
    railway_api_run sh -c 'for name in VANTA_DATABASE_KIND VANTA_DATABASE_URL VANTA_STORAGE_KIND VANTA_OBJECT_STORAGE_BUCKET VANTA_OBJECT_STORAGE_CDN_BASE_URL VANTA_CDN_COOKIE_DOMAIN VANTA_OBJECT_STORAGE_ENDPOINT_URL VANTA_OBJECT_STORAGE_ACCESS_KEY_ID VANTA_OBJECT_STORAGE_SECRET_ACCESS_KEY VANTA_OBJECT_STORAGE_REGION AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY CLOUDFLARE_ACCOUNT_ID R2_ENDPOINT_URL; do if printenv "$name" >/dev/null; then printf "%s=present\n" "$name"; else printf "%s=missing\n" "$name"; fi; done'
    ;;
  local)
    require_cmd sqlite3
    sqlite3 "$LOCAL_DB"
    ;;
  local-query)
    shift
    [[ $# -gt 0 ]] || { echo "missing SQL" >&2; exit 1; }
    require_cmd sqlite3
    sqlite3 -header -column "$LOCAL_DB" "$*"
    ;;
  r2-buckets)
    require_cmd wrangler
    wrangler r2 bucket list
    ;;
  r2-get)
    require_cmd wrangler
    bucket="$(r2_bucket)"
    [[ -n "$bucket" ]] || { echo "VANTA_OBJECT_STORAGE_BUCKET is not configured" >&2; exit 1; }
    [[ -n "${2:-}" ]] || { echo "missing R2 key" >&2; exit 1; }
    if [[ -n "${3:-}" ]]; then
      wrangler r2 object get "$bucket/${2}" --file "$3" --remote
    else
      wrangler r2 object get "$bucket/${2}" --remote
    fi
    ;;
  r2-put)
    require_cmd wrangler
    bucket="$(r2_bucket)"
    [[ -n "$bucket" ]] || { echo "VANTA_OBJECT_STORAGE_BUCKET is not configured" >&2; exit 1; }
    [[ -n "${2:-}" && -n "${3:-}" ]] || { echo "usage: ./db.sh r2-put <key> <file>" >&2; exit 1; }
    wrangler r2 object put "$bucket/${2}" --file "$3" --remote
    ;;
  r2-delete)
    require_cmd wrangler
    bucket="$(r2_bucket)"
    [[ -n "$bucket" ]] || { echo "VANTA_OBJECT_STORAGE_BUCKET is not configured" >&2; exit 1; }
    [[ -n "${2:-}" ]] || { echo "missing R2 key" >&2; exit 1; }
    wrangler r2 object delete "$bucket/${2}" --remote
    ;;
  r2-ls)
    require_cmd aws
    bucket="$(r2_bucket)"
    endpoint="$(r2_endpoint)"
    [[ -n "$bucket" ]] || { echo "VANTA_OBJECT_STORAGE_BUCKET is not configured" >&2; exit 1; }
    [[ -n "$endpoint" ]] || { echo "set R2_ENDPOINT_URL or CLOUDFLARE_ACCOUNT_ID for S3-compatible listing" >&2; exit 1; }
    aws --endpoint-url "$endpoint" s3 ls "s3://$bucket/${2:-}"
    ;;
  *)
    usage
    exit 1
    ;;
esac
