# Vanta Video Editor

Standalone Vite + Rust workspace for Vanta's internal video editor. It reuses the Vanta frontend UI primitives and player chrome while keeping media processing, rendering, validation, proof links, and publish state on the Rust backend.

## Structure

- `backend/` - Axum API, SQLite store, editor domain rules, ffmpeg media processing, HLS packaging, and Fozzy scenarios.
- `frontend/` - Vite React app using copied Vanta UI/player components and editor-specific composition.

## Run

Backend:

```sh
cd backend
VANTA_EDITOR_DATABASE=./vanta-editor.db \
VANTA_EDITOR_MEDIA_ROOT=./storage \
VANTA_MEDIA_PIPELINE_DATABASE=./vanta-media-pipeline.db \
VANTA_AD_HUB_OUTBOX=./storage/ad-hub \
cargo run
```

Frontend:

```sh
cd frontend
bun install
bun run dev -- --host 127.0.0.1 --port 5178 --strictPort
```

The frontend defaults to `http://127.0.0.1:4117` for the editor API. Override with `VITE_VANTA_EDITOR_API_BASE_URL`.

## Test

```sh
cd backend
cargo fmt --check
cargo test
bash tests/editor-integration-smoke.sh
fozzy doctor --deep --scenario tests/editor-backend.fozzy.json --runs 5 --seed 424242 --json
fozzy test --det --strict-verify tests/editor-backend.fozzy.json --json
fozzy run tests/editor-backend.fozzy.json --det --strict-verify --record tests/editor-backend.trace.fozzy --json
fozzy trace verify tests/editor-backend.trace.fozzy --strict --json
fozzy replay tests/editor-backend.trace.fozzy --json
fozzy ci tests/editor-backend.trace.fozzy --strict --json

cd ../frontend
bun run lint
bun run build
```

With the backend, frontend, and Aegis server running:

```sh
cd backend
python3 tests/editor-aegis-e2e.py
```

## Operations

- Requires `ffmpeg` and `ffprobe` on `PATH` for upload derivatives and HLS packaging.
- Auth is represented by Vanta role headers in this standalone service: `X-Vanta-User-Id` and `X-Vanta-Role`.
- Generated media lives under `VANTA_EDITOR_MEDIA_ROOT`; editor SQLite lives at `VANTA_EDITOR_DATABASE`.
- `VANTA_MEDIA_PIPELINE_DATABASE` enables root-compatible Vanta media pipeline writes into `upload_jobs`, `media_assets`, `media_asset_variants`, and advertiser marketplace submissions. If omitted, editor publish still returns the intended pipeline identifiers but does not mutate an external pipeline database.
- `VANTA_AD_HUB_OUTBOX` receives structured review-room JSON files for the External Ad Hub sync path.
- Advertiser proof links resolve to `https://streamvanta.tv/ad-hub/proofs/{token}` and are stored with review request records plus Ad Hub outbox records.
