# VANTA Pre-Deployment TODO

This document is a source-level audit of what still needs to be done before deploying the current app as a real production system.

Scope of this pass:
- Backend production readiness
- Database and storage modularity
- Streaming/media delivery concerns
- Frontend/backend coverage and UI flow gaps
- Sensible baseline functionality only

Out of scope for this pass:
- Manual QA
- Pixel-level design review
- Fixing the items below

## Do Now

These are implementation tasks that are clearly required before deployment.

### P0 Deployment Blockers

- [x] Introduce a real provider boundary for the database layer instead of binding the entire app to `SqlitePool`.
  Evidence:
  - [backend/src/state.rs](/Users/deepsaint/Desktop/vanta/backend/src/state.rs)
  - [backend/src/main.rs](/Users/deepsaint/Desktop/vanta/backend/src/main.rs)
  - large portions of `backend/src/**` take `&SqlitePool` directly.
  Required outcome:
  - handlers stop depending on SQLite types directly
  - provider selection is driven by env/config
  - SQLite can remain supported as a local/dev provider behind a flag
  - Postgres can be added as a first-class production provider

- [x] Remove SQLite-specific assumptions from the runtime bootstrap path before claiming “switchable DB providers”.
  Evidence:
  - SQLite PRAGMAs are hardcoded in [backend/src/main.rs](/Users/deepsaint/Desktop/vanta/backend/src/main.rs)
  - app state stores `SqlitePool` in [backend/src/state.rs](/Users/deepsaint/Desktop/vanta/backend/src/state.rs)
  - migrations include SQLite-specific features such as FTS5 in [backend/migrations/0003_search_fts.sql](/Users/deepsaint/Desktop/vanta/backend/migrations/0003_search_fts.sql)
  - telemetry/search queries use SQLite JSON/strftime behavior in multiple `backend/src/api/control/**` modules
  Required outcome:
  - provider-neutral interfaces
  - provider-specific query implementations where needed
  - clear compatibility plan for search, JSON querying, time math, and conflict handling

- [x] Add `database.kind` config support, not just a single `VANTA_DATABASE_URL`.
  Evidence:
  - [backend/src/config.rs](/Users/deepsaint/Desktop/vanta/backend/src/config.rs)
  Required outcome:
  - env-driven provider selection
  - config validation for the active provider
  - startup failures that clearly explain missing env vars

- [x] Create repository/service interfaces for high-level domains before adding Postgres.
  Suggested domain boundaries:
  - auth/sessions
  - catalog/discovery
  - user profile/settings/watchlist/following
  - creator live/control plane
  - uploads/media pipeline
  - playback grants/sessions
  - notifications
  - moderation
  Reason:
  - the code currently mixes HTTP handlers, domain logic, and SQL shape heavily inside route modules

- [x] Audit and isolate SQLite-only query/row model usage before adding Postgres support.
  Evidence:
  - widespread `sqlx::sqlite::SqliteRow`
  - widespread `&SqlitePool`
  - SQLite FTS/JSON/time expressions across discovery/control/admin code
  Required outcome:
  - list provider-specific SQL
  - define Postgres equivalents
  - keep provider-specific code out of HTTP handler layers

- [x] Move DB connection setup behind provider-specific initializers.
  Evidence:
  - [backend/src/main.rs](/Users/deepsaint/Desktop/vanta/backend/src/main.rs)
  Required outcome:
  - SQLite init can keep PRAGMA behavior
  - Postgres init gets its own pool/config path
  - application code stops assuming one concrete pool type

- [x] Replace task-local media storage assumptions with a storage abstraction.
  Evidence:
  - media root is local filesystem state in [backend/src/config.rs](/Users/deepsaint/Desktop/vanta/backend/src/config.rs)
  - media paths are resolved by joining `media_root` in [backend/src/api/media/access/filesystem.rs](/Users/deepsaint/Desktop/vanta/backend/src/api/media/access/filesystem.rs)
  - HLS output is written directly to local disk in [backend/src/api/media/pipeline/packaging/hls/generate.rs](/Users/deepsaint/Desktop/vanta/backend/src/api/media/pipeline/packaging/hls/generate.rs)
  Required outcome:
  - storage provider interface
  - local filesystem provider for dev
  - object storage provider for production
  - no handler/business logic should care which provider is active

- [x] Split local processing from persistent asset ownership.
  Current issue:
  - the system writes processed assets directly into a local durable tree
  Required outcome:
  - transient local scratch space for ffmpeg jobs
  - persistent artifact store in object storage
  - job completion step that publishes assets to the active storage provider

- [x] Stop relying on full frontend bootstrap hydration before the app can render.
  Evidence:
  - [frontend/src/main.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/main.tsx)
  - [frontend/src/lib/repository.ts](/Users/deepsaint/Desktop/vanta/frontend/src/lib/repository.ts)
  Current issue:
  - the app blocks render on `await repository.hydrate()`
  - if any required request fails, the app can fail into a blank/black screen
  Required outcome:
  - render shell first
  - show explicit loading/error states
  - allow partial recovery where possible

- [x] Remove the baked-in dev auth token path from the frontend before deployment.
  Evidence:
  - [frontend/src/lib/api.ts](/Users/deepsaint/Desktop/vanta/frontend/src/lib/api.ts)
  Current issue:
  - `DEV_ACCESS_TOKEN` is implicitly used on localhost
  Required outcome:
  - real auth/session bootstrap
  - explicit sign-in/sign-out handling
  - no hidden token fallback in production code paths

### Storage, Media, and Delivery

- [x] Move from backend-hosted file origin assumptions to object storage + CDN assumptions.
  Current issue:
  - media URLs are application URLs (`/api/v1/media/...`) and are tied to local filesystem reads
  Required outcome:
  - clear origin strategy for VOD and live artifacts
  - cacheable asset paths
  - CDN-aware auth strategy

- [x] Redesign playback/media authorization so CDN caching is viable.
  Evidence:
  - manifest and asset URLs append `playbackToken` in:
    - [backend/src/api/playback/grants/build.rs](/Users/deepsaint/Desktop/vanta/backend/src/api/playback/grants/build.rs)
    - [backend/src/api/media/access/request.rs](/Users/deepsaint/Desktop/vanta/backend/src/api/media/access/request.rs)
  Current issue:
  - per-session tokenized asset URLs reduce cacheability
  Required outcome:
  - manifest auth strategy
  - segment/caption/image auth strategy
  - CDN-friendly signing/cookie story

- [x] Stop reading media files fully into memory for regular media responses.
  Evidence:
  - `tokio::fs::read` in [backend/src/api/media/access/request.rs](/Users/deepsaint/Desktop/vanta/backend/src/api/media/access/request.rs)
  Current issue:
  - not acceptable for large segments, images, or media assets at production scale
  Required outcome:
  - streaming responses
  - support for range/partial content where appropriate
  - avoid full-buffer reads for large files

- [x] Add correct cache headers and content delivery behavior for media responses.
  Evidence:
  - current media responses set content type but not a real cache strategy
  Required outcome:
  - explicit `Cache-Control` policy
  - origin/CDN behavior by asset class
  - separate policies for manifests, live playlists, segments, images, captions, and admin-only artifacts

- [x] Add a proper worker/process model for media processing instead of treating the app process as the long-term place to own all packaging responsibilities.
  Evidence:
  - media jobs and runtime commands exist, but deployment boundaries are not yet production-grade
  Required outcome:
  - background processing design
  - failure/retry ownership
  - worker scalability plan

- [x] Define the persistent storage layout for live runtime artifacts, archives, and mirrored collaboration outputs.
  Evidence:
  - control/runtime artifact code under [backend/src/api/control/**](/Users/deepsaint/Desktop/vanta/backend/src/api/control)
  Required outcome:
  - clear storage prefixes
  - retention rules
  - cleanup/archive behavior

### Backend Runtime Hardening

- [x] Add a real auth lifecycle to the app, not just bearer-token assumptions.
  Evidence:
  - frontend reads local storage tokens in [frontend/src/lib/api.ts](/Users/deepsaint/Desktop/vanta/frontend/src/lib/api.ts)
  - many protected endpoints exist but no real sign-in flow is present in the frontend

- [x] Add explicit startup/config checks for production-required env.
  Required outcome:
  - DB provider config
  - storage provider config
  - allowed origins
  - secret material
  - external service dependencies if introduced

- [x] Harden CORS/origin config for production domains.
  Evidence:
  - local defaults only in [backend/src/config.rs](/Users/deepsaint/Desktop/vanta/backend/src/config.rs)

- [x] Add API-level request timeouts/retry strategy where sensible on the frontend.
  Evidence:
  - [frontend/src/lib/api.ts](/Users/deepsaint/Desktop/vanta/frontend/src/lib/api.ts)
  Current issue:
  - no timeout
  - no abort signal support
  - no standardized error translation

- [x] Add structured error handling and UX-facing error surfaces for important flows.
  Current issue:
  - many actions either silently revert or show raw error strings
  Evidence:
  - optimistic store actions in [frontend/src/lib/store.ts](/Users/deepsaint/Desktop/vanta/frontend/src/lib/store.ts)

- [x] Add response compression policy for text responses.
  Evidence:
  - router currently uses `CorsLayer` and `TraceLayer` only in [backend/src/api/api_surface.rs](/Users/deepsaint/Desktop/vanta/backend/src/api/api_surface.rs)
  Required outcome:
  - compress JSON/manifests/VTT where appropriate
  - do not treat this as the main streaming optimization

- [x] Add production logging and observability boundaries for background workers, media jobs, websocket sessions, and playback failures.
  Evidence:
  - metrics support and request IDs already exist
  - remaining work is system coverage and deploy-grade dashboards/alerts

### Frontend Data Flow and Route Gaps

- [x] Replace the “hydrate everything into one in-memory repository” pattern with route/feature-level data loading.
  Evidence:
  - [frontend/src/lib/repository.ts](/Users/deepsaint/Desktop/vanta/frontend/src/lib/repository.ts)
  Current issue:
  - all core content is fetched upfront
  - search/filter/detail flows mostly operate on an in-memory snapshot
  - this does not scale as the catalog grows

- [x] Stop using in-memory local search for the actual search experience.
  Evidence:
  - header search and search page use `repository.search(...)`
  - backend exposes `/api/v1/search`
  Files:
  - [frontend/src/components/layout/Header.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/components/layout/Header.tsx)
  - [frontend/src/pages/SearchPage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/SearchPage.tsx)
  Required outcome:
  - frontend search hits backend search
  - result loading/error states
  - debounced querying or submit-based query flow

- [x] Audit repository-derived screens that should become endpoint-driven rather than snapshot-driven.
  Examples:
  - home
  - browse/live discovery
  - search
  - creator overview/analytics/revenue
  - live recommendations/sidebar followed-live lists

- [x] Add route-level loading/error/empty states consistently instead of mixed ad hoc behavior.

- [x] Fix the broken route target for home page originals.
  Resolution:
  - home links to `/originals`
  - `/originals` resolves to the originals catalog route

- [x] Wire the header settings button to a real route/action.
  Evidence:
  - no handler on the settings icon in [frontend/src/components/layout/Header.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/components/layout/Header.tsx)

- [x] Wire the header account menu actions for `Preferences` and `Sign out`.
  Evidence:
  - no handlers in [frontend/src/components/layout/Header.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/components/layout/Header.tsx)

### Viewer Account/Profile/Settings

- [x] Implement the `Edit profile` flow instead of leaving profile/settings buttons inert.
  Evidence:
  - no-op buttons in:
    - [frontend/src/pages/ProfilePage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/ProfilePage.tsx)
    - [frontend/src/pages/SettingsPage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/SettingsPage.tsx)
  Backend support already exists:
  - `/api/v1/me/profile`

- [x] Implement sign-out.
  Evidence:
  - no-op sign-out buttons in:
    - [frontend/src/pages/ProfilePage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/ProfilePage.tsx)
    - [frontend/src/pages/SettingsPage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/SettingsPage.tsx)
    - [frontend/src/components/layout/Header.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/components/layout/Header.tsx)

- [x] Persist settings changes to the backend instead of holding them in local component state only.
  Evidence:
  - `SettingsPage` uses local `useState` in helper controls and never calls the API
  - backend exposes `/api/v1/me/settings`
  Files:
  - [frontend/src/pages/SettingsPage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/SettingsPage.tsx)
  Required outcome:
  - real save/reset flow
  - error states
  - optimistic or confirmed updates

- [x] Add UI for session management if `/api/v1/me/sessions` is intended to be user-facing.
  Evidence:
  - backend exposes session list/create/revoke
  - no page or control exists in the frontend

- [x] Add UI for notification read state if `/api/v1/me/notifications/:notification_id/read` is intended to be used.
  Evidence:
  - header renders notifications but does not mark them read

- [x] Implement billing/plan controls or remove the inactive controls from the production launch.
  Evidence:
  - no-op buttons such as `Manage plan`, `Downgrade`, `Pause membership`, `Cancel subscription`
  Files:
  - [frontend/src/pages/ProfilePage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/ProfilePage.tsx)
  - [frontend/src/pages/SettingsPage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/SettingsPage.tsx)

### Viewer Content and Live Experience

- [x] Implement share actions or remove the buttons for launch.
  Evidence:
  - no handlers on share buttons in:
    - [frontend/src/pages/FilmPage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/FilmPage.tsx)
    - [frontend/src/pages/SeriesPage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/SeriesPage.tsx)
    - [frontend/src/pages/LiveWatchPage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/LiveWatchPage.tsx)

- [x] Replace raw backend error text with normalized UI messages in viewer actions.
  Examples:
  - notify
  - clip
  - report
  - playback start
  - watchlist/follow toggles

- [x] Decide whether chat settings and emotes are launch features; if not, remove or disable the controls.
  Evidence:
  - no handlers on chat settings/emote buttons in [frontend/src/components/chat/LiveChat.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/components/chat/LiveChat.tsx)

- [x] Normalize live chat reconnect and offline UX.
  Current state:
  - reconnect exists
  - UX is still basic and not productized
  Files:
  - [frontend/src/components/chat/LiveChat.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/components/chat/LiveChat.tsx)

### Creator and Studio UX

- [x] Stop presenting creator analytics/revenue/overview as live operational surfaces if they only read static bootstrap data once.
  Evidence:
  - these pages read from `repository` instead of fetching dedicated live data
  Files:
  - [frontend/src/pages/creator/CreatorOverviewPage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/creator/CreatorOverviewPage.tsx)
  - [frontend/src/pages/creator/CreatorAnalyticsPage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/creator/CreatorAnalyticsPage.tsx)
  - [frontend/src/pages/creator/CreatorRevenuePage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/creator/CreatorRevenuePage.tsx)
  Current issue:
  - copy implies live/refreshing operational data
  - UI is actually reading a single hydrated snapshot

- [x] Implement or remove inactive creator overview controls.
  Evidence:
  - scheduled broadcast `Edit` button is inert in [frontend/src/pages/creator/CreatorOverviewPage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/creator/CreatorOverviewPage.tsx)

- [x] Implement or remove inactive creator revenue controls.
  Evidence:
  - `Statements` and `Payout settings` are inert in [frontend/src/pages/creator/CreatorRevenuePage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/creator/CreatorRevenuePage.tsx)

- [x] Implement or remove inactive creator content controls.
  Evidence:
  - `Export CSV` and `Upload` are inert in [frontend/src/pages/creator/CreatorContentPage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/creator/CreatorContentPage.tsx)

- [x] Add the missing creator surfaces for backend features that already exist.
  Backend features without equivalent obvious UI pages:
  - subscriber tier management
  - creator series/project management
  - creator notification inbox/read flow
  - creator upload job ingest workflow
  - creator upload operations view
  - creator live health
  Relevant backend files:
  - [backend/src/api/creator/business/mod.rs](/Users/deepsaint/Desktop/vanta/backend/src/api/creator/business/mod.rs)
  - [backend/src/api/creator/core/mod.rs](/Users/deepsaint/Desktop/vanta/backend/src/api/creator/core/mod.rs)
  - [backend/src/api/media/jobs/mod.rs](/Users/deepsaint/Desktop/vanta/backend/src/api/media/jobs/mod.rs)
  - [backend/src/api/creator/live/handlers.rs](/Users/deepsaint/Desktop/vanta/backend/src/api/creator/live/handlers.rs)

- [x] Add the missing collaborator/member UX if collaboration is considered a launch feature.
  Evidence:
  - backend has full member/invite/grant endpoints under [backend/src/api/collabs/mod.rs](/Users/deepsaint/Desktop/vanta/backend/src/api/collabs/mod.rs)
  Current issue:
  - there is host-side collaboration UI in creator live
  - there is no equivalent viewer/member-facing collaboration page set in the frontend routes

### Backend Features Exposed Without Matching Frontend Coverage

- [x] Decide whether admin APIs are launch-facing or internal-only, then either build admin tooling or move them out of the public app surface.
  Evidence:
  - admin notification/media/playback/ingest/enforcement routes exist
  Files:
  - [backend/src/api/admin_ops.rs](/Users/deepsaint/Desktop/vanta/backend/src/api/admin_ops.rs)
  - [backend/src/api/ingest/mod.rs](/Users/deepsaint/Desktop/vanta/backend/src/api/ingest/mod.rs)
  - [backend/src/api/playback/mod.rs](/Users/deepsaint/Desktop/vanta/backend/src/api/playback/mod.rs)
  - [backend/src/api/creator/core/mod.rs](/Users/deepsaint/Desktop/vanta/backend/src/api/creator/core/mod.rs)

- [x] Decide whether user entitlement reconciliation endpoints need UI or should remain operational-only.
  Evidence:
  - `/api/v1/me/entitlements/**` exists
  - no UI path exists

- [x] Decide whether upload-job ingest APIs are internal/studio-only or need an actual upload workflow page.
  Evidence:
  - endpoints exist under [backend/src/api/media/jobs/mod.rs](/Users/deepsaint/Desktop/vanta/backend/src/api/media/jobs/mod.rs)
  - creator content page does not implement upload-job creation or chunked ingest

### Consistency and Cleanup

- [x] Add pagination/incremental fetching to catalog/discovery screens where dataset growth would break the current preload model.
  Affected areas:
  - home
  - browse/live
  - search
  - category pages
  - related content rows

- [x] Stop assuming the entire viewer catalog is cheaply preloaded into memory on every app boot.
  Evidence:
  - [frontend/src/lib/repository.ts](/Users/deepsaint/Desktop/vanta/frontend/src/lib/repository.ts)

- [x] Remove or replace demo-style labels/copy that overstate backend behavior.
  Examples:
  - creator overview claims metrics refresh every 60 seconds
  - creator dashboards look operationally live while reading a one-time bootstrap snapshot

- [x] Audit all “button exists but feature does not exist” cases and either implement or remove them before launch.
  Confirmed examples from source:
  - header settings
  - header preferences
  - header sign out
  - profile edit/sign out/manage plan
  - settings sign out and billing actions
  - series/film/live share
  - live subscribe
  - creator overview edit
  - creator revenue statements/payout settings
  - creator content export/upload
  - chat settings/emotes

## Decide On

These are decisions that should be made before deployment because they change the implementation path.

- [x] Decide whether dual database provider support is a real product requirement or just a migration requirement.
  Practical baseline:
  - keep SQLite for local/dev only
  - make Postgres the production/default deploy target
  Reason:
  - supporting both long term is materially more expensive than migrating once
  Decision:
  - SQLite remains local/development only; Postgres is the production provider.

- [x] Decide whether `/browse` should be distinct from `/live` or whether one route should be removed.
  Evidence:
  - both currently resolve to `BrowseLivePage` in [frontend/src/router.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/router.tsx)
  Decision:
  - `/live` is canonical; `/browse` is removed as a launch route.

- [x] Decide whether live delivery will stay standard HLS first or actually target low-latency HLS as a launch requirement.
  Reason:
  - the codebase already has live runtime complexity
  - pushing LL-HLS too early increases deployment and ops complexity
  Decision:
  - launch with standard HLS first; defer LL-HLS.

- [x] Decide and document the VOD encoding policy.
  Evidence:
  - static ladder in [backend/src/api/media/pipeline/packaging/hls/generate.rs](/Users/deepsaint/Desktop/vanta/backend/src/api/media/pipeline/packaging/hls/generate.rs)
  Needed decisions:
  - codec defaults
  - audio defaults
  - segment duration
  - bitrate ladder policy
  Decision:
  - H.264 video, AAC stereo audio at 48 kHz, 6 second standard HLS segments, adaptive ladder capped at source resolution.

- [x] Decide whether live playback is public, authenticated, or tier-gated.
  Evidence:
  - live playback session creation is called with `auth: false` in [frontend/src/pages/LiveWatchPage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/LiveWatchPage.tsx)
  Decision:
  - public live playback for launch.

- [x] Decide whether the live subscribe flow is in scope for launch.
  Evidence:
  - UI button exists in [frontend/src/pages/LiveWatchPage.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/pages/LiveWatchPage.tsx)
  - creator subscription backend exists
  Needed decision:
  - implement now or remove from launch UI
  Decision:
  - not a launch UI feature.

- [x] Decide whether chat settings and emotes are real launch features or should be removed from the UI.
  Evidence:
  - controls exist without behavior in [frontend/src/components/chat/LiveChat.tsx](/Users/deepsaint/Desktop/vanta/frontend/src/components/chat/LiveChat.tsx)
  Decision:
  - not launch features.

- [x] Decide whether collaboration is a launch feature for guests/members, not just for hosts.
  Evidence:
  - backend has full member/invite/grant endpoints
  - frontend only exposes host-side collaboration UI in creator live
  Decision:
  - host-side studio controls can remain; guest/member collaboration UX is deferred from launch.

- [x] Decide whether admin APIs are meant to stay inside this app surface or be moved to separate internal tooling.
  Evidence:
  - admin routes already exist for notifications, media, playback, ingest, and enforcement
  Decision:
  - admin APIs stay internal/operational and are disabled in production app surface by default.

- [x] Decide whether user entitlement reconciliation and upload-job ingest flows need frontend surfaces or remain operational/internal only.
  Decision:
  - entitlement reconciliation and upload-job ingest controls remain operational/internal for launch.

- [x] Decide the final production provider model explicitly.
  The unresolved architecture question is:
  - SQLite locally?
  - Postgres in production?
  - local filesystem locally?
  - S3/object storage in production?
  This needs to be made explicit in config and code boundaries before deployment work continues.
  Decision:
  - AWS ECS, Postgres, S3, and CloudFront in production; SQLite and local filesystem in development.

## Suggested Execution Order

- [x] 1. Make the `Decide On` calls that affect architecture.
- [ ] 2. Introduce database and storage provider boundaries.
- [ ] 3. Make Postgres and object storage the production implementations.
- [ ] 4. Redesign media delivery/auth around CDN-compatible asset access.
- [ ] 5. Fix frontend startup/data-loading architecture.
- [ ] 6. Remove or implement all inert user-facing controls.
- [ ] 7. Close major backend/frontend surface mismatches.
- [ ] 8. Harden media serving, caching, and streaming behavior.
- [ ] 9. Add deploy-grade auth/session/config handling.

## Notes

- The backend already has more capability than the frontend exposes. The main pre-deploy problem is not missing backend ideas. The main problems are:
  - local DB assumptions
  - local media assumptions
  - snapshot-heavy frontend data flow
  - several no-op controls
  - provider portability not yet designed into the core
