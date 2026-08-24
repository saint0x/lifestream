#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKEND_DIR="$ROOT_DIR/backend"
FRONTEND_DIR="$ROOT_DIR/frontend"
RUN_DIR="${VANTA_DEV_RUN_DIR:-$ROOT_DIR/.dev}"
BACKEND_PID="$RUN_DIR/backend.pid"
FRONTEND_PID="$RUN_DIR/frontend.pid"
BACKEND_LOG="$RUN_DIR/backend.log"
FRONTEND_LOG="$RUN_DIR/frontend.log"
VANTA_DEV_DB="${VANTA_DEV_DB:-$BACKEND_DIR/vanta.db}"
BACKEND_URL="${VANTA_BACKEND_URL:-http://127.0.0.1:8080}"
FRONTEND_URL="${VANTA_FRONTEND_URL:-http://127.0.0.1:5173}"

usage() {
  cat <<EOF
Usage: ./dev.sh <command>

Commands:
  start       Start backend and frontend dev servers
  stop        Stop dev servers started by this script
  restart     Stop then start both servers
  status      Show local server status and configured paths
  logs        Tail backend and frontend logs
  db          Open the dev SQLite database with sqlite3
  db-path     Print the current dev database path
  migrate     Run backend migrations by starting the backend briefly
  test        Run backend tests and frontend typecheck/build
  smoke       Probe local health and search endpoints
  clean       Stop servers and remove local dev logs

Environment:
  VANTA_DEV_DB=$VANTA_DEV_DB
  VANTA_BACKEND_URL=$BACKEND_URL
  VANTA_FRONTEND_URL=$FRONTEND_URL
EOF
}

ensure_run_dir() {
  mkdir -p "$RUN_DIR"
}

is_running() {
  local pid_file="$1"
  [[ -f "$pid_file" ]] && kill -0 "$(cat "$pid_file")" 2>/dev/null
}

start_backend() {
  ensure_run_dir
  if is_running "$BACKEND_PID"; then
    echo "backend already running: pid $(cat "$BACKEND_PID")"
    return
  fi
  (
    cd "$BACKEND_DIR"
    env -u CARGO_BUILD_TARGET -u TARGET -u CFLAGS VANTA_DATABASE_URL="sqlite://$VANTA_DEV_DB?mode=rwc" cargo run
  ) >"$BACKEND_LOG" 2>&1 &
  echo $! >"$BACKEND_PID"
  echo "backend: $BACKEND_URL (pid $(cat "$BACKEND_PID"))"
}

start_frontend() {
  ensure_run_dir
  if is_running "$FRONTEND_PID"; then
    echo "frontend already running: pid $(cat "$FRONTEND_PID")"
    return
  fi
  (
    cd "$FRONTEND_DIR"
    VITE_VANTA_API_BASE_URL="$BACKEND_URL" bun run dev -- --host 127.0.0.1
  ) >"$FRONTEND_LOG" 2>&1 &
  echo $! >"$FRONTEND_PID"
  echo "frontend: $FRONTEND_URL (pid $(cat "$FRONTEND_PID"))"
}

stop_one() {
  local name="$1"
  local pid_file="$2"
  if is_running "$pid_file"; then
    kill "$(cat "$pid_file")"
    echo "stopped $name"
  else
    echo "$name not running"
  fi
  rm -f "$pid_file"
}

wait_for_url() {
  local url="$1"
  for _ in {1..60}; do
    if curl -fsS "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.5
  done
  echo "timed out waiting for $url" >&2
  return 1
}

case "${1:-}" in
  start)
    start_backend
    wait_for_url "$BACKEND_URL/health"
    start_frontend
    wait_for_url "$FRONTEND_URL"
    ;;
  stop)
    stop_one frontend "$FRONTEND_PID"
    stop_one backend "$BACKEND_PID"
    ;;
  restart)
    "$0" stop
    "$0" start
    ;;
  status)
    echo "backend:  $(is_running "$BACKEND_PID" && echo "running pid $(cat "$BACKEND_PID")" || echo "stopped")"
    echo "frontend: $(is_running "$FRONTEND_PID" && echo "running pid $(cat "$FRONTEND_PID")" || echo "stopped")"
    echo "db:       $VANTA_DEV_DB"
    echo "logs:     $BACKEND_LOG"
    echo "          $FRONTEND_LOG"
    ;;
  logs)
    ensure_run_dir
    tail -f "$BACKEND_LOG" "$FRONTEND_LOG"
    ;;
  db)
    sqlite3 "$VANTA_DEV_DB"
    ;;
  db-path)
    echo "$VANTA_DEV_DB"
    ;;
  migrate)
    start_backend
    wait_for_url "$BACKEND_URL/health"
    ;;
  test)
    (cd "$BACKEND_DIR" && env -u CARGO_BUILD_TARGET -u TARGET -u CFLAGS cargo test)
    (cd "$FRONTEND_DIR" && bun run typecheck && bun run build)
    ;;
  smoke)
    curl -fsS "$BACKEND_URL/health" | python3 -m json.tool
    python3 - <<PY
import json, urllib.parse, urllib.request
base = "$BACKEND_URL"
for query in ["northlight", "Mara Vale", "cinematic tech"]:
    url = base + "/api/v1/search?" + urllib.parse.urlencode({"q": query, "limit": 5})
    data = json.load(urllib.request.urlopen(url))
    print(query, "=>", [(item["kind"], item["title"]) for item in data["items"][:5]])
PY
    ;;
  clean)
    "$0" stop
    rm -f "$BACKEND_LOG" "$FRONTEND_LOG"
    ;;
  *)
    usage
    exit 1
    ;;
esac
