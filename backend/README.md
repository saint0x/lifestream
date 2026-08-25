# VANTA Backend

Production-grade Rust backend for the VANTA frontend contract.

## Stack

- `axum` for HTTP and WebSocket delivery
- `sqlx` with SQLite for local development and Postgres for production persistence
- local filesystem storage for development and Cloudflare R2-backed object storage for production media
- API and worker containers that own catalog, user state, creator control plane, media jobs, and live chat

## Run

```bash
cargo run
```

The service listens on `127.0.0.1:8080` by default and creates `vanta.db` in the backend directory.

## Production Configuration

Production is Railway/Cloudflare-first and cost-conscious:

- Railway runs the API/runtime container.
- Railway Postgres is the production database.
- Cloudflare R2 owns persistent media artifacts.
- R2 public/custom-domain CDN delivery serves playback manifests, segments, images, and captions.
- SQLite and local media files are development-only providers.

Required production env:

```bash
VANTA_ENV=production
VANTA_DATABASE_KIND=postgres
VANTA_DATABASE_URL=postgres://...
VANTA_STORAGE_KIND=object
VANTA_OBJECT_STORAGE_BUCKET=vanta-media
VANTA_OBJECT_STORAGE_ENDPOINT_URL=https://<cloudflare-account-id>.r2.cloudflarestorage.com
VANTA_OBJECT_STORAGE_ACCESS_KEY_ID=<r2-access-key-id>
VANTA_OBJECT_STORAGE_SECRET_ACCESS_KEY=<r2-secret-access-key>
VANTA_OBJECT_STORAGE_REGION=auto
VANTA_OBJECT_STORAGE_CDN_BASE_URL=https://pub-4cffb671265940d19168dde582d31087.r2.dev
VANTA_CDN_COOKIE_DOMAIN=.streamvanta.tv
VANTA_ALLOWED_ORIGINS=https://streamvanta.tv,https://www.streamvanta.tv
VANTA_TOKEN_HASH_SECRET=at-least-32-characters
VANTA_ADMIN_API_ENABLED=false
```

When `VANTA_STORAGE_KIND=object`, uploaded source files and generated playback
artifacts are written to Cloudflare R2 before the database advertises their CDN
paths as ready. Processing retries restore missing scratch files from R2, so
Railway ephemeral disk is not the durable media source.

## Auth

Protected routes require `Authorization: Bearer <token>`.

Auth sessions are persisted in the active database provider. Provision users, creator profiles, and bearer sessions explicitly through the backend runtime commands:

```bash
cargo run -- provision-user \
  --user-id user_demo \
  --handle demo \
  --display-name "Demo User"

cargo run -- provision-creator \
  --creator-id creator_demo \
  --user-id user_demo \
  --handle demo-live \
  --display-name "Demo Live"

cargo run -- issue-session \
  --user-id user_demo \
  --label local-dev \
  --scopes viewer,creator,admin
```

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
- `GET /api/v1/creator/me/ad-hub`
- `POST /api/v1/creator/me/ad-offers/:offer_id/accept`
- `POST /api/v1/creator/me/ad-offers/:offer_id/decline`
- `POST /api/v1/creator/me/ad-offers/:offer_id/submissions`
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
- `/health/live` is process liveness only. `/health/ready` verifies the active database provider readiness.
- Catalog detail reads return persisted playback progress when the caller is authenticated.
- Search is backed by the active database provider instead of in-memory scans, so title/tag/streamer lookups stay fast as the catalog grows.

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
