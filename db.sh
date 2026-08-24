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
  query <sql>       Run SQL against production Postgres
  tables            List production Postgres tables
  search-docs       Show production search document counts by kind
  vars              Show safe database/storage variable presence

SQLite commands:
  local             Open the local SQLite dev database
  local-query <sql> Run SQL against the local SQLite dev database

R2 commands:
  r2-buckets        List Cloudflare R2 buckets with wrangler
  r2-get <key>      Fetch an object from the configured R2 bucket
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

run_query() {
  local sql="$1"
  require_cmd railway
  require_cmd psql
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
  if [[ -n "${R2_ENDPOINT_URL:-}" ]]; then
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
    railway_api_run sh -c 'for name in VANTA_DATABASE_KIND VANTA_DATABASE_URL VANTA_STORAGE_KIND VANTA_OBJECT_STORAGE_BUCKET VANTA_OBJECT_STORAGE_CDN_BASE_URL VANTA_CDN_COOKIE_DOMAIN; do if printenv "$name" >/dev/null; then printf "%s=present\n" "$name"; else printf "%s=missing\n" "$name"; fi; done'
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
    wrangler r2 object get "$bucket/${2}"
    ;;
  r2-put)
    require_cmd wrangler
    bucket="$(r2_bucket)"
    [[ -n "$bucket" ]] || { echo "VANTA_OBJECT_STORAGE_BUCKET is not configured" >&2; exit 1; }
    [[ -n "${2:-}" && -n "${3:-}" ]] || { echo "usage: ./db.sh r2-put <key> <file>" >&2; exit 1; }
    wrangler r2 object put "$bucket/${2}" --file "$3"
    ;;
  r2-delete)
    require_cmd wrangler
    bucket="$(r2_bucket)"
    [[ -n "$bucket" ]] || { echo "VANTA_OBJECT_STORAGE_BUCKET is not configured" >&2; exit 1; }
    [[ -n "${2:-}" ]] || { echo "missing R2 key" >&2; exit 1; }
    wrangler r2 object delete "$bucket/${2}"
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
