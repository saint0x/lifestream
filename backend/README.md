# LIFESTREAM Backend

Production-grade Rust backend for the LIFESTREAM frontend contract.

## Stack

- `axum` for HTTP and WebSocket delivery
- `sqlx` + SQLite (WAL mode) for persistence
- one process that owns catalog, user state, creator control plane, and live chat

## Run

```bash
cargo run
```

The service listens on `127.0.0.1:8080` by default and creates `lifestream.db` in the backend directory.

## Auth

Protected routes require `Authorization: Bearer <token>`.

For local development, the seed process creates a local bearer token on first boot:

```text
lifestream-local-dev-token
```

Set `LIFESTREAM_LOCAL_SEED_TOKEN` before first boot to override that local token.

## Main API Surface

- `GET /health`
- `GET /health/live`
- `GET /health/ready`
- `GET /metrics`
- `GET /api/v1/bootstrap`
- `GET /api/v1/home`
- `GET /api/v1/catalog/series`
- `GET /api/v1/catalog/series/:slug`
- `GET /api/v1/catalog/films`
- `GET /api/v1/catalog/films/:slug`
- `GET /api/v1/catalog/content/:id`
- `GET /api/v1/live/streams`
- `GET /api/v1/live/streams/:slug`
- `GET /api/v1/live/streams/:stream_id/chat`
- `POST /api/v1/live/streams/:stream_id/chat/messages`
- `GET /api/v1/categories`
- `GET /api/v1/categories/:slug`
- `GET /api/v1/search?q=rust`
- `GET /api/v1/me`
- `GET|POST /api/v1/me/sessions`
- `DELETE /api/v1/me/sessions/:id`
- `POST|DELETE /api/v1/me/watchlist/:content_id`
- `POST|DELETE /api/v1/me/following/:streamer_id`
- `PUT /api/v1/me/progress`
- `DELETE /api/v1/me/progress/:content_id`
- `GET /api/v1/creator/me/dashboard`
- `GET|PATCH /api/v1/creator/me/live`
- `POST /api/v1/creator/me/broadcasts/start`
- `POST /api/v1/creator/me/broadcasts/:id/end`
- `POST /api/v1/creator/me/stream-key/rotate`
- `GET /api/v1/creator/me/uploads`
- `PATCH /api/v1/creator/me/uploads/:id`
- `POST /api/v1/creator/me/uploads/bulk`
- `GET /api/v1/creator/me/analytics`
- `GET /api/v1/creator/me/revenue`
- `GET /api/v1/creator/me/notifications`
- `GET /ws/live/:stream_id`

Public routes:
- `/health`
- `/health/live`
- `/health/ready`
- `/metrics`
- `/api/v1/home`
- `/api/v1/bootstrap`
- catalog, categories, search, and live stream reads

Authenticated routes:
- `/api/v1/me/*`
- `/api/v1/creator/me/*`
- `POST /api/v1/live/streams/:stream_id/chat/messages`

## Operational Notes

- Every HTTP response includes `x-request-id`. Supply one on ingress if you want to preserve an upstream trace ID; otherwise the backend generates one.
- `/metrics` exposes a Prometheus-style plaintext surface for HTTP totals, response codes, rate-limit counts, uptime, DB pool state, and websocket connection counts.
- `/health/live` is process liveness only. `/health/ready` verifies SQLite readiness.
- Catalog detail reads return persisted playback progress when the caller is authenticated.
- Search is backed by SQLite FTS5 instead of in-memory scans, so title/tag/streamer lookups stay fast as the catalog grows.

## Session Management

- `GET /api/v1/me/sessions` lists the caller's sessions and marks the current one with `isCurrent`.
- `POST /api/v1/me/sessions` creates a new bearer session and returns the plaintext `accessToken` once.
- `DELETE /api/v1/me/sessions/:id` revokes a session immediately.

## Realtime Protocol

`/ws/live/:stream_id` sends JSON messages tagged by `type`:

- `chatHistory`
- `chatMessage`
- `viewerCount`

Clients can connect anonymously to watch. To send chat over websocket, connect with `?access_token=<token>` and send:

```json
{
  "color": "#fafafa",
  "body": "hello chat"
}
```
