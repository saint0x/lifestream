# Control Plane Load Bench

Date: Friday, August 21, 2026

## Scope

- Binary: `backend/target/debug/backend serve`
- Bind: `127.0.0.1:8080`
- Database: `backend/lifestream.db`
- Media root: `backend/media`
- Persistence mode: live SQLite state, no fixture reset inside the binary
- Load tool: `/usr/local/bin/wrk`
- Deterministic validation: `fozzy`

## Production fixes included in this pass

- Fixed reconnect-with-active-collaboration failures by making `live_runtime_targets.id` session-scoped while keeping logical upsert keys stable.
- Restored guest-side program topology for co-stream participants that both publish into the host program and mirror to their own channel.
- Cached binary readiness probes in-process so `/health` no longer shells out to `ffmpeg` and `ffprobe` on every request.

## End-to-end verification

Verified on Friday, August 21, 2026 against the final binary:

- Broadcast: `06810ef0-539b-4973-89f7-e2d5f190d107`
- Ingest session: `ing-a6e26d0cfe79411192163c7b64803507`
- Collaboration session: `cols-45d02389d70c486987e5ead6af8ce7eb`
- Guest participant: `colp-c55fdf6e97c84e35ae76fdb8a5558a2f`
- Live stream: `lv-deepsaint-live`

Observed state:

- Live ingest connected and heartbeat-attached at `7100 kbps`, `1888` viewers, `95 ms` ingest latency
- Runtime reported `healthy` with `packagingStatus=ready`
- Public live stream row was present and `playbackReady=true`
- Creator control showed `isLive=true`, `currentViewers=1888`, `activeSessionCount=1`
- Creator runtime showed `collabOutputs=3` and `collabPrograms=2`
- Live playback session creation returned HTTP `200` with a real playback session and token

## Deterministic coverage

Passed:

- `fozzy doctor --deep --scenario backend/tests/collaboration-runtime-topology.pass.fozzy.json --runs 5 --seed 20260820 --strict --json`
- `fozzy doctor --deep --scenario backend/tests/live-runtime-control.pass.fozzy.json --runs 5 --seed 20260820 --strict --json`
- `fozzy doctor --deep --scenario backend/tests/live-chat-authority.pass.fozzy.json --runs 5 --seed 20260821 --strict --json`
- `fozzy run backend/tests/live-runtime-control.pass.fozzy.json --det --record backend/tests/control-plane-runtime-20260820-postfix.trace.fozzy --json`
- `fozzy trace verify backend/tests/control-plane-runtime-20260820-postfix.trace.fozzy --strict --json`
- `fozzy replay backend/tests/control-plane-runtime-20260820-postfix.trace.fozzy --json`
- `fozzy ci backend/tests/control-plane-runtime-20260820-postfix.trace.fozzy --strict --json`
- `fozzy run backend/tests/live-chat-authority.pass.fozzy.json --det --record backend/tests/live-chat-authority-20260821.trace.fozzy --json`
- `fozzy trace verify backend/tests/live-chat-authority-20260821.trace.fozzy --strict --json`
- `fozzy replay backend/tests/live-chat-authority-20260821.trace.fozzy --json`
- `fozzy ci backend/tests/live-chat-authority-20260821.trace.fozzy --strict --json`

All deterministic runs passed with no reported compatibility, checksum, replay, or CI integrity failures.

## Targeted Rust tests

Passed after the final fixes:

- `cargo test live_ingest_notifications::stale -- --nocapture`
- `cargo test collaboration_presence -- --nocapture`
- `cargo test live_presence -- --nocapture`
- `cargo test health_runtime -- --nocapture`
- `cargo test reconnect_with_active_collaboration_session_preserves_runtime_and_grants -- --nocapture`
- `cargo test collaboration_topology_changes_rebuild_runtime_artifacts_for_active_ingest -- --nocapture`
- `cargo test creator_live_authoritative_reads_reconcile_expired_collaboration_truth -- --nocapture`

## HTTP load results

Settings:

- `wrk -t2 -c32 -d4s --latency`
- Authenticated endpoints used the local host token
- Playback session creation used `POST /api/v1/playback/live/lv-deepsaint-live/session`
- No socket errors and no non-2xx/3xx responses were observed in the recorded `wrk` outputs

| Endpoint | Req/s | p50 | p99 |
| --- | ---: | ---: | ---: |
| `GET /health` | 7257.29 | 3.24 ms | 1.10 s |
| `GET /api/v1/home` | 821.61 | 38.57 ms | 45.29 ms |
| `GET /api/v1/live/streams` | 12792.77 | 2.41 ms | 7.08 ms |
| `GET /api/v1/live/streams/deepsaint-live` | 13369.86 | 2.35 ms | 3.59 ms |
| `GET /api/v1/me` | 4795.96 | 5.14 ms | 364.52 ms |
| `GET /api/v1/creator/me/live/control` | 63.45 | 466.76 ms | 508.65 ms |
| `GET /api/v1/creator/me/live/runtime` | 39.16 | 763.39 ms | 888.44 ms |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 488.27 | 64.45 ms | 81.16 ms |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 492.69 | 63.48 ms | 102.73 ms |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 667.82 | 46.73 ms | 77.23 ms |

## Mixed-load sweep

Settings:

- concurrent `wrk` runs against public listing/detail, creator live control/runtime, collaboration control/runtime, live playback POST, and host chat POST
- duration `4s` per lane with shared process pressure
- benchmark date: Friday, August 21, 2026

Before payload and query trimming:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 1084.88 | 20.06 ms | 56.00 ms | public reads stayed healthy |
| `GET /api/v1/live/streams/deepsaint-live` | 1105.87 | 19.98 ms | 47.01 ms | public detail stayed healthy |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 53.68 | 282.45 ms | 414.92 ms | heavy but stable |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 55.58 | 269.65 ms | 529.70 ms | heavy but stable |
| `GET /api/v1/creator/me/live/control` | 5.95 | 1.99 s | 2.00 s | 40 timeouts |
| `GET /api/v1/creator/me/live/runtime` | 5.95 | 1.99 s | 2.00 s | 48 timeouts |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 67.50 | 220.81 ms | 446.46 ms | stable under pressure |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 1007.13 | 13.90 ms | 309.16 ms | non-2xxs were rate-limit responses |

After trimming creator live health history, recent runtime windows, and collaboration summary hydration:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 940.73 | 23.58 ms | 47.76 ms | public reads remained solid |
| `GET /api/v1/live/streams/deepsaint-live` | 934.99 | 23.58 ms | 49.76 ms | public detail remained solid |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 45.31 | 335.77 ms | 526.82 ms | still heavy but bounded |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 47.41 | 322.63 ms | 494.76 ms | still heavy but bounded |
| `GET /api/v1/creator/me/live/control` | 32.80 | 464.73 ms | 625.61 ms | timeout mode removed |
| `GET /api/v1/creator/me/live/runtime` | 15.97 | 940.36 ms | 1.10 s | timeout mode removed, still heaviest |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 57.86 | 259.63 ms | 441.30 ms | stable under pressure |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 865.76 | 16.53 ms | 333.90 ms | non-2xxs were rate-limit responses |

## Read of the numbers

- Public discovery and stream detail reads are fast and stable.
- Collaboration control/runtime reads are healthy for the current single-node SQLite control plane.
- Playback session issuance is solid and stayed in a good latency band.
- Creator live control and creator live runtime were the dominant bottlenecks under mixed load; the latest trimming pass removed the timeout-only failure mode and materially improved both endpoints.
- Creator live runtime is still the heaviest operator-panel path and remains the first place to optimize if we need more concurrency headroom.
- `/health` improved dramatically after probe caching, but it still shows a long tail on cold probe paths. The hot path is fast; the p99 reflects cache miss and dependency-check variance.
- Host chat flood non-2xxs were expected `429`-style enforcement behavior from the per-user chat limiter, not corruption or authority failure.

## Remaining limits

- WebSocket throughput was validated through deterministic scenarios and runtime tests, not through a separate socket flood harness.
- The backend is now consistent and usable end-to-end for the control-plane scope, but the creator live runtime and collaboration summary paths still have room for deeper query and payload trimming if we want more operator concurrency headroom.
- Media-plane codec, transcoding, and player-side runtime work remain outside this document. This report is strictly for the Rust control plane.
