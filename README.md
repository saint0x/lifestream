# lifestream

Lifestream is a streaming platform backend and frontend workspace for a product that combines creator-led live broadcasting with premium on-demand media delivery.

## What it covers

- multi-user live channels with collaboration and guest routing
- creator control-plane APIs for channels, uploads, publishing, moderation, and operations
- audience-facing playback, presence, catalog, and realtime surfaces
- SQLite-backed persistence for control-plane state
- media pipeline orchestration for upload processing, packaging, and delivery policy

## Workspace

- `backend/`: Rust control plane and media runtime orchestration
- `frontend/`: product UI and creator surfaces
- `CODECS.md`: remaining media-runtime and codec-system design work

## Development

Backend:

```bash
cd backend
cargo check --tests
```

Frontend:

```bash
cd frontend
npm install
npm run dev
```

## Status

The control plane is live and under active hardening. Remaining work is concentrated in the deeper media runtime path, especially ingest termination, collaborative media routing, and mirrored output wiring.
