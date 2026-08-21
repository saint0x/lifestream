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
- Replaced the creator app-state live embeds with a compact control-plane shell so `/api/v1/creator/me/state` no longer hydrates full operator runtime history, targets, telemetry, or collaboration control state.
- Fixed collaboration runtime topology planning so co-stream guests that both publish to host and mirror to their own channel always receive a guest program/fanout path, eliminating the `compiled collaboration runtime bundle missing output fanout` background-worker failure.
- Fixed stale live moderation authority so moderation routes now use the same freshness rules as public live discovery instead of trusting a stale `live_streams` row after ingest heartbeat loss.

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

## Isolated creator shell spot checks

These checks were rerun on Friday, August 21, 2026 after the compact creator app-state refactor.

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/creator/me/state` before compact shell | 1.98 | timeout | timeout | active live creator shell at roughly `274 KB`; 16 timeouts in 8s |
| `GET /api/v1/creator/me/state` after compact shell | 39.53 | 385.61 ms | 500.20 ms | active live creator shell at roughly `43 KB`; no timeouts in the isolated run |

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

After the compact creator shell refactor and collaboration fanout repair on a healthy active co-stream fixture:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 145.80 | 105.95 ms | 295.90 ms | public listing remained healthy on the active fixture |
| `GET /api/v1/live/streams/deepsaint-live` | 338.20 | 44.32 ms | 127.17 ms | public detail remained healthy |
| `GET /api/v1/bootstrap` | 17.48 | 896.73 ms | 1.02 s | heavier than public reads but stable |
| `GET /api/v1/me/state` | 17.83 | 854.86 ms | 1.00 s | stable under mixed control-plane pressure |
| `GET /api/v1/creator/me/state` | 3.96 | timeout | timeout | isolated fix landed, but half the requests still timed out under the full shared sweep |
| `GET /api/v1/creator/me/live/control` | 17.84 | 792.61 ms | 1.11 s | stable after the runtime fanout repair |
| `GET /api/v1/creator/me/live/runtime` | 7.93 | 1.77 s | 1.89 s | still the heaviest live operator route; 5 timeouts remained |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 23.79 | 587.47 ms | 897.01 ms | collaboration control remained stable |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 25.76 | 589.18 ms | 746.25 ms | collaboration runtime remained stable |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 27.74 | 534.04 ms | 761.11 ms | playback issuance remained stable |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 471.29 | 30.07 ms | 533.93 ms | non-2xxs were still expected chat limiter responses |

After the compact creator dashboard cut on the same Friday, August 21, 2026 active live/co-stream fixture:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/creator/me/state` | 9.91 | 1.53 s | 1.89 s | no timeout cliff; still slower than desired under concurrent operator pressure |
| `GET /api/v1/creator/me/live/control` | 40.77 | 341.43 ms | 661.34 ms | materially healthier under the targeted mixed sweep |
| `GET /api/v1/creator/me/live/runtime` | 17.85 | 836.12 ms | 1.18 s | still the heaviest runtime read, but stable in the targeted mixed sweep |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 59.62 | 232.37 ms | 574.66 ms | collaboration runtime remained the strongest of the operator paths |

After the session-scoped runtime read refactor on the same Friday, August 21, 2026 active live/co-stream fixture:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/creator/me/state` | 9.42 | 1.57 s | 1.76 s | creator shell remained the slowest path, but stayed off the timeout cliff |
| `GET /api/v1/creator/me/live/control` | 41.55 | 340.66 ms | 633.13 ms | effectively flat from the prior sweep, which confirms the runtime refactor did not regress control |
| `GET /api/v1/creator/me/live/runtime` | 23.67 | 629.49 ms | 907.81 ms | materially better after reusing the active session, scoping active telemetry to the live session, and removing duplicate current-session lookups |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 59.92 | 241.64 ms | 465.62 ms | collaboration runtime stayed strong while the creator runtime tail came down |

After the stale moderation authority repair and creator app-state shared-read refactor on the same Friday, August 21, 2026 active live/co-stream fixture:

Focused operator slice:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/creator/me/state` | 11.71 | 1.29 s | 1.47 s | improved from `9.42 req/s`; no timeout cliff in the focused slice |
| `GET /api/v1/creator/me/live/control` | 43.29 | 315.85 ms | 538.12 ms | creator live control stayed healthy |
| `GET /api/v1/creator/me/live/runtime` | 25.03 | 577.94 ms | 751.30 ms | best focused runtime result of the pass so far |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 63.25 | 214.16 ms | 471.38 ms | collaboration runtime remained the strongest operator path |

Full mixed sweep after the same refactor on the same active fixture:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 347.11 | 42.62 ms | 134.18 ms | public listing remained healthy |
| `GET /api/v1/live/streams/deepsaint-live` | 347.73 | 42.17 ms | 134.94 ms | public detail remained healthy |
| `GET /api/v1/bootstrap` | 19.68 | 747.25 ms | 1.03 s | slightly better than the prior active-fixture mixed run |
| `GET /api/v1/me/state` | 18.79 | 762.67 ms | 1.04 s | stable under shared pressure |
| `GET /api/v1/creator/me/state` | 3.99 | timeout | timeout | creator shell still collapses under the full 11-lane mixed sweep |
| `GET /api/v1/creator/me/live/control` | 17.94 | 764.61 ms | 1.06 s | similar to the earlier full mixed band |
| `GET /api/v1/creator/me/live/runtime` | 9.96 | 1.44 s | 1.57 s | improved over the earlier `1.77 s` / `1.89 s` full mixed result, but still heavy |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 23.67 | 589.53 ms | 931.91 ms | collaboration control stayed bounded |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 25.94 | 557.84 ms | 896.33 ms | collaboration runtime remained stable |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 31.90 | 457.24 ms | 766.46 ms | playback issuance remained healthy |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 467.89 | 29.78 ms | 640.69 ms | non-2xxs were still expected chat limiter responses |

After removing upload-release reconciliation from the hot creator shell read path and replacing `uploadOperations` record hydration with summary-only aggregates on the same Friday, August 21, 2026 active live/co-stream fixture:

Focused operator slice:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/creator/me/state` | 60.72 | 259.06 ms | 346.40 ms | major improvement after cutting the full upload/media record graph out of the shell |
| `GET /api/v1/creator/me/live/control` | 24.69 | 615.09 ms | 730.97 ms | slower than the previous focused control-only sweep, but still bounded |
| `GET /api/v1/creator/me/live/runtime` | 13.95 | 1.06 s | 1.22 s | focused runtime regressed once the creator shell probe was made dramatically cheaper, suggesting renewed endpoint contention elsewhere |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 33.91 | 439.43 ms | 577.09 ms | collaboration runtime remained healthy, though not as strong as the prior focused slice |

Full mixed sweep on the same newest summary-only creator shell build:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 259.29 | 59.39 ms | 119.91 ms | public listing remained healthy under the latest mixed sweep |
| `GET /api/v1/live/streams/deepsaint-live` | 253.66 | 59.90 ms | 121.75 ms | public detail remained healthy |
| `GET /api/v1/bootstrap` | 13.94 | 1.04 s | 1.22 s | bootstrap is now one of the heavier non-creator reads |
| `GET /api/v1/me/state` | 13.96 | 1.10 s | 1.18 s | stable but heavier than before |
| `GET /api/v1/creator/me/state` | 35.38 | 436.02 ms | 579.45 ms | the old full-mixed timeout cliff is gone on this build |
| `GET /api/v1/creator/me/live/control` | 13.96 | 1.12 s | 1.28 s | materially heavier than the creator shell after the latest refactor |
| `GET /api/v1/creator/me/live/runtime` | 7.98 | 1.87 s | 1.98 s | still the heaviest runtime path under full mixed load; 16 timeouts remained |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 17.94 | 837.24 ms | 1.03 s | collaboration control remained bounded but heavy |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 17.94 | 836.93 ms | 1.01 s | collaboration runtime remained bounded but heavy |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 21.93 | 685.16 ms | 811.52 ms | playback issuance remained available but slowed with the full operator stack active |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 374.29 | 41.06 ms | 550.02 ms | non-2xxs were still expected chat limiter responses |

After collapsing collaboration runtime hydration into a single preloaded graph, removing duplicate grants/pickups reads, and reducing host-summary/count fanout on the same Friday, August 21, 2026 active live/co-stream fixture:

Isolated endpoint spot checks:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/creator/me/live/control` | 104.66 | 295.54 ms | 351.54 ms | strongest focused control result of the pass so far |
| `GET /api/v1/creator/me/live/runtime` | 55.13 | 506.78 ms | 613.43 ms | best isolated runtime result of the pass so far |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 131.47 | 224.59 ms | 439.92 ms | host collaboration control materially improved after the shared runtime-context build |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 132.67 | 228.78 ms | 422.65 ms | host collaboration runtime now sits back in the healthy low-hundreds req/s band |

Focused shared operator slice:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/creator/me/state` | 135.64 | 224.34 ms | 409.34 ms | creator shell stayed excellent while sharing pressure with the hot live endpoints |
| `GET /api/v1/creator/me/live/control` | 86.69 | 346.76 ms | 503.96 ms | materially better than the prior focused `615 ms` / `731 ms` slice |
| `GET /api/v1/creator/me/live/runtime` | 47.23 | 603.27 ms | 778.24 ms | dominant live runtime read materially improved from `13.95 req/s`, `1.06 s` p50, `1.22 s` p99 |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 110.34 | 268.11 ms | 475.94 ms | collaboration runtime recovered to the stronger sub-300 ms p50 band under shared pressure |

Deterministic validation on the same Friday, August 21, 2026 current build:

- `fozzy doctor --deep --scenario tests/live-runtime-control.pass.fozzy.json --runs 5 --seed 20260821 --strict --host-backends --json`
  Result: `consistent=true` across all 5 runs with the same signature `eab0be67a620d3e3e572e395a98174ced5812b1059fada98bf6b41ecdee7dd68`
- `fozzy test tests/live-runtime-control.pass.fozzy.json --det --strict-verify --seed 20260821 --host-backends --json`
  Result: pass
- `fozzy test tests/collaboration-session-graph.pass.fozzy.json --det --strict-verify --seed 20260821 --host-backends --json`
  Result: pass
- `fozzy test tests/live-chat-replay.pass.fozzy.json --det --strict-verify --seed 20260821 --host-backends --json`
  Result: pass
- `fozzy test tests/collaboration-websocket-control.pass.fozzy.json --det --strict-verify --seed 20260821 --host-backends --json`
  Result: pass
- `fozzy test tests/stale-live-read-consistency.pass.fozzy.json --det --strict-verify --seed 20260821 --host-backends --json`
  Result: pass
- `fozzy run tests/live-runtime-control.pass.fozzy.json --det --strict-verify --seed 20260821 --record tests/live-runtime-control-20260821-current.trace.fozzy --proc-backend host --fs-backend host --http-backend host --json`

After removing read-time creator profile normalization writes, switching live control/runtime hot paths to persisted profile reads, eliminating `creator_live_settings` write-on-read behavior, and compacting bootstrap’s creator shell on Friday, August 21, 2026:

Deterministic validation on the newest build:

- `fozzy doctor --deep --scenario tests/live-runtime-control.pass.fozzy.json --runs 5 --seed 20260821 --json`
  Result: `consistent=true` with the same signature `eab0be67a620d3e3e572e395a98174ced5812b1059fada98bf6b41ecdee7dd68` across all five runs
- `fozzy test --det --strict-verify tests/live-runtime-control.pass.fozzy.json tests/stale-live-read-consistency.pass.fozzy.json --json`
  Result: pass
- `fozzy run tests/live-runtime-control.pass.fozzy.json --det --record tests/live-runtime-control-20260821-current.trace.1.fozzy --json`
  Result: pass
- `fozzy trace verify tests/live-runtime-control-20260821-current.trace.1.fozzy --strict --json`
  Result: pass
- `fozzy replay tests/live-runtime-control-20260821-current.trace.1.fozzy --json`
  Result: pass
- `fozzy ci tests/live-runtime-control-20260821-current.trace.1.fozzy --json`
  Result: pass

Latest full mixed sweep on the newest build:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 292.33 | 100.59 ms | 450.49 ms | public listing recovered versus the prior write-contended run |
| `GET /api/v1/live/streams/deepsaint-live` | 280.69 | 106.74 ms | 446.30 ms | public detail recovered as well |
| `GET /api/v1/bootstrap` | 14.27 | 1.11 s | 1.31 s | nearly doubled from `7.93 req/s` after replacing the full creator dashboard bootstrap hydrate with the compact shell |
| `GET /api/v1/me/state` | 7.88 | timeout | timeout | still collapses under the 11-lane shared sweep and remains a top remaining control-plane problem |
| `GET /api/v1/creator/me/state` | 23.60 | 1.19 s | 1.24 s | materially healthier than the old timeout cliff, though still heavy |
| `GET /api/v1/creator/me/live/control` | 15.75 | 1.55 s | 2.00 s | still high-latency under shared write pressure |
| `GET /api/v1/creator/me/live/runtime` | 7.88 | timeout | timeout | still the heaviest live read path in the full mixed sweep |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 15.73 | 1.69 s | 1.79 s | stable but too heavy for the target bar |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 15.75 | 1.69 s | 1.79 s | stable but still dominated by shared SQLite pressure |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 15.72 | 1.69 s | 1.93 s | remained available throughout the sweep |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 204.60 | 109.45 ms | 1.81 s | non-2xxs were expected chat limiter responses |

Interpretation:

- The creator shell is no longer the main collapse point; the worst remaining production pain is shared mixed-load contention between viewer state reads, creator runtime reads, and write-heavy chat/playback traffic on the same SQLite control-plane store.
- The bootstrap path is materially better after removing the full dashboard hydrate from bootstrap.
- The remaining work is no longer “placeholder cleanup”; it is core concurrency architecture work around the viewer shell and the live runtime/control fanout under mixed read-write pressure.

After batching watchlist hydration, adding fast no-op guards to viewer entitlement reconciliation, stale creator-live socket reconciliation, and host collaboration expiry reconciliation on Friday, August 21, 2026:

Latest full mixed sweep on the updated build:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 410.71 | 72.67 ms | 448.03 ms | strongest public listing result of the day so far |
| `GET /api/v1/live/streams/deepsaint-live` | 376.88 | 76.48 ms | 477.09 ms | public detail improved with the same viewer-side contention cuts |
| `GET /api/v1/bootstrap` | 7.93 | timeout | timeout | still starved when the full operator stack is active |
| `GET /api/v1/me/state` | 10.90 | 239.13 ms | 967.08 ms | improved from `7.88 req/s` on the prior newest-build sweep; timeout pressure remains but the viewer shell is materially healthier |
| `GET /api/v1/creator/me/state` | 23.78 | 1.12 s | 1.50 s | broadly flat versus the prior sweep |
| `GET /api/v1/creator/me/live/control` | 15.85 | 1.62 s | 1.99 s | still heavy under shared operator load |
| `GET /api/v1/creator/me/live/runtime` | 7.93 | timeout | timeout | still the dominant remaining control-plane bottleneck |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 15.87 | 1.52 s | 1.66 s | stable but heavy |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 15.86 | 1.56 s | 1.65 s | stable but still bounded by collaboration/runtime hydration cost |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 22.55 | 1.36 s | 1.78 s | playback issuance improved materially from `15.72 req/s` |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 209.12 | 122.31 ms | 1.68 s | non-2xxs were expected chat limiter responses |

What moved:

- The viewer-side cuts helped real mixed-load behavior: `me/state` climbed from `7.88 req/s` to `10.90 req/s`, and public live lanes moved sharply upward.
- The remaining collapse is no longer primarily the viewer shell. It is the creator live runtime/control and collaboration-runtime graph under the full mixed read/write operator stack.
- The next highest-value work is to reduce authoritative creator runtime hydration cost directly, especially the embedded collaboration control/runtime payload and recent-runtime history reads.

After splitting creator-wide collaboration summaries onto a compact control projection, then reducing creator live runtime recent-history windows to the minimum exercised by current checks on Friday, August 21, 2026:

First mixed sweep on the compact collaboration-summary build:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | lane reset | n/a | n/a | two public lanes saw transient `Connection reset by peer`, but `/health` stayed green immediately after, so this looked like overload rather than a process crash |
| `GET /api/v1/creator/me/live/runtime` | 7.93 | 1.88 s | 1.98 s | no longer pure timeout floor in this run, but still far too heavy |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 23.82 | 1.10 s | 1.56 s | materially better than the prior `15.86 req/s` band |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 226.22 | 114.08 ms | 1.20 s | expected limiter responses remained the non-2xx source |

Repeat mixed sweep on the same compact collaboration-summary build:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 256.61 | 104.44 ms | 472.19 ms | public lane reset did not reproduce on the immediate rerun |
| `GET /api/v1/live/streams/deepsaint-live` | 244.22 | 112.84 ms | 472.43 ms | stable on rerun |
| `GET /api/v1/bootstrap` | 7.93 | 1.52 s | 1.99 s | still heavily starved |
| `GET /api/v1/me/state` | 7.93 | timeout | timeout | viewer shell regressed on this rerun |
| `GET /api/v1/creator/me/state` | 23.79 | 1.23 s | 1.43 s | stable |
| `GET /api/v1/creator/me/live/control` | 15.87 | 1.39 s | 1.99 s | slightly healthier p50 than the prior pass |
| `GET /api/v1/creator/me/live/runtime` | 7.93 | timeout | timeout | runtime still collapsed under the full shared sweep |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 15.86 | 1.10 s | 1.17 s | meaningful p50 improvement on the session-specific control lane |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 15.86 | 1.06 s | 1.98 s | somewhat healthier session-specific runtime lane |

Latest mixed sweep after trimming creator live runtime recent-session/output/target/telemetry/event windows to `1`:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 430.36 | 64.93 ms | 396.46 ms | best public listing result of the day so far |
| `GET /api/v1/live/streams/deepsaint-live` | 370.17 | 76.27 ms | 401.86 ms | public detail stayed strong |
| `GET /api/v1/bootstrap` | 7.99 | timeout | timeout | bootstrap still starved under the full sweep |
| `GET /api/v1/me/state` | 7.93 | 1.98 s | 1.98 s | viewer shell regressed back toward timeout behavior on this build |
| `GET /api/v1/creator/me/state` | 23.80 | 1.10 s | 1.38 s | creator shell remained stable |
| `GET /api/v1/creator/me/live/control` | 15.87 | 1.73 s | 2.00 s | control did not materially improve from the history trim |
| `GET /api/v1/creator/me/live/runtime` | 7.86 | timeout | timeout | the history trim alone was not enough to move the dominant runtime bottleneck |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 16.62 | 1.44 s | 1.57 s | slight improvement in the session-specific control lane |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 15.85 | 1.45 s | 1.53 s | stable, but still heavy |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 15.85 | 1.47 s | 1.61 s | playback remained available |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 221.77 | 113.99 ms | 1.53 s | expected limiter responses remained the non-2xx source |

Current interpretation:

- The compact collaboration-summary split materially helped the session-specific collaboration control/runtime lanes and did not break the runtime smoke coverage.
- The creator-wide `live/runtime` endpoint remains the single biggest unresolved control-plane bottleneck under the full mixed sweep.
- The latest history trim improved public lanes further but was not sufficient to move `creator/me/live/runtime` off the timeout floor or keep `me/state` consistently healthy under the full operator stack.
- The next pass should target deeper runtime-read architecture, not more shallow payload trimming: most likely the remaining authoritative runtime advisory/output/telemetry assembly and any shared SQLite write contention those reads still trigger indirectly.

After removing deep runtime artifact reconciliation/inspection from the hot creator runtime read path, then swapping the creator runtime hot path onto a compact session telemetry summary on Friday, August 21, 2026:

Mixed sweep after removing deep artifact reconciliation/inspection from `GET /api/v1/creator/me/live/runtime`:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 310.89 | 86.67 ms | 461.16 ms | public reads remained healthy |
| `GET /api/v1/live/streams/deepsaint-live` | 296.71 | 95.00 ms | 470.27 ms | public detail remained healthy |
| `GET /api/v1/bootstrap` | 12.78 | 1.13 s | 1.86 s | materially healthier than the `7.99 req/s` timeout-floor run |
| `GET /api/v1/me/state` | 7.88 | timeout | timeout | viewer shell still collapsed under the full operator mix |
| `GET /api/v1/creator/me/state` | 23.63 | 1.16 s | 1.27 s | creator shell stayed stable |
| `GET /api/v1/creator/me/live/control` | 15.75 | 1.75 s | 1.97 s | still heavy |
| `GET /api/v1/creator/me/live/runtime` | 7.87 | timeout | timeout | hot-path artifact inspection removal alone did not move the runtime lane |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 15.73 | 1.68 s | 1.72 s | stable |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 15.74 | 1.68 s | 1.79 s | stable |

Mixed sweep after replacing the hot runtime path’s full session telemetry summary with a compact aggregate plus latest-sample derivation:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 177.20 | 164.65 ms | 524.81 ms | public reads stayed available but slowed under the latest build’s shared sweep |
| `GET /api/v1/live/streams/deepsaint-live` | 173.71 | 156.96 ms | 529.49 ms | similar story on public detail |
| `GET /api/v1/bootstrap` | 7.86 | timeout | timeout | regressed back to the timeout floor under this sweep |
| `GET /api/v1/me/state` | 7.88 | timeout | timeout | unchanged from the worst viewer-shell mixed-load band |
| `GET /api/v1/creator/me/state` | 23.64 | 1.12 s | 1.30 s | creator shell still stable |
| `GET /api/v1/creator/me/live/control` | 15.74 | 1.63 s | 1.90 s | slightly healthier p50 than the prior no-inspection-only sweep |
| `GET /api/v1/creator/me/live/runtime` | 7.88 | timeout | timeout | the compact telemetry summary still did not move the creator runtime lane off the timeout floor |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 15.72 | 1.48 s | 1.55 s | stable |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 15.72 | 1.48 s | 1.55 s | stable |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 222.33 | 120.43 ms | 1.24 s | expected limiter responses remained the non-2xx source |

Updated interpretation:

- Removing deep artifact inspection from the hot creator runtime path was architecturally correct and runtime-safe, but it was not the dominant bottleneck.
- Replacing the full telemetry summary with a compact hot-path summary also did not materially improve `GET /api/v1/creator/me/live/runtime` under the full shared sweep.
- The creator runtime timeout floor now appears to be dominated by broader shared read/write pressure and/or the remaining live-runtime assembly fanout rather than any single already-trimmed artifact or telemetry query.
- The next meaningful pass should likely go after shared SQLite concurrency architecture directly or collapse more of the creator live runtime assembly into precomputed/runtime-owned state instead of request-time composition.
  Result: pass; fresh trace recorded at `backend/tests/live-runtime-control-20260821-current.trace.fozzy`
- `fozzy trace verify tests/live-runtime-control-20260821-current.trace.fozzy --strict --json`
  Result: pass
- `fozzy replay tests/live-runtime-control-20260821-current.trace.fozzy --json`
  Result: pass
- `fozzy ci tests/live-runtime-control-20260821-current.trace.fozzy --strict --json`
  Result: pass

Full mixed sweep on the fresh active live + collaboration fixture after the same collaboration read-graph collapse:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 592.56 | 57.73 ms | 99.77 ms | public listing remained healthy |
| `GET /api/v1/live/streams/deepsaint-live` | 542.48 | 59.59 ms | 106.38 ms | public detail remained healthy on the slug route |
| `GET /api/v1/bootstrap` | 31.72 | 975.63 ms | 1.08 s | bootstrap stayed heavy under the full mixed stack |
| `GET /api/v1/me/state` | 23.79 | 1.13 s | 1.21 s | viewer shell remained slower than the creator shell |
| `GET /api/v1/creator/me/state` | 71.42 | 432.71 ms | 547.13 ms | creator shell stayed materially healthier than the earlier full mixed baselines |
| `GET /api/v1/creator/me/live/control` | 39.68 | 681.96 ms | 918.56 ms | improved over the earlier `1.12 s` / `1.28 s` full mixed band |
| `GET /api/v1/creator/me/live/runtime` | 23.80 | 1.15 s | 1.44 s | materially better than the earlier `1.87 s` / `1.98 s` full mixed band, but still the slowest operator lane |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 55.52 | 510.56 ms | 806.56 ms | collaboration control improved substantially over the earlier `837 ms` / `1.03 s` band |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 55.56 | 508.60 ms | 804.20 ms | collaboration runtime improved substantially over the earlier `837 ms` / `1.01 s` band |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 627.64 | 42.37 ms | 733.90 ms | non-2xx responses were still expected limiter enforcement responses |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | n/a | n/a | n/a | this lane hit `Connection reset by peer` during the 11-lane sweep and needs a broader mixed-pressure retest |

Playback cross-test after the same mixed-sweep fault isolation:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/creator/me/live/runtime` | 65.41 | 447.85 ms | 568.95 ms | improved further when paired only with control + playback |
| `GET /api/v1/creator/me/live/control` | 119.69 | 257.43 ms | 359.94 ms | strong three-lane result |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 150.71 | 206.91 ms | 328.70 ms | playback stayed stable when paired directly with the heavy creator live endpoints |

Playback broader cross-test on the same Friday, August 21, 2026 build with `bootstrap + me/state + creator live control/runtime + playback`:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/bootstrap` | 55.35 | 517.10 ms | 618.42 ms | materially healthier than the full 11-lane mixed sweep |
| `GET /api/v1/me/state` | 47.31 | 636.84 ms | 784.01 ms | viewer shell stayed stable |
| `GET /api/v1/creator/me/live/control` | 78.75 | 371.22 ms | 487.44 ms | strong under the five-lane playback cross-test |
| `GET /api/v1/creator/me/live/runtime` | 71.04 | 392.64 ms | 760.45 ms | best broader shared-pressure runtime result of the pass so far |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 2475.59 | 11.97 ms | 166.88 ms | no reset reproduced; lane stayed fast even with the heavier shell/runtime reads active |

Repeated full 11-lane mixed sweep on the same Friday, August 21, 2026 fresh active live + collaboration fixture:

Iteration 1:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 479.16 | 60.72 ms | 228.66 ms | listing stayed up despite some socket read/write noise |
| `GET /api/v1/live/streams/deepsaint-live` | 573.53 | 57.49 ms | 96.03 ms | public detail stayed healthy |
| `GET /api/v1/bootstrap` | 24.13 | 989.16 ms | 1.21 s | heavy under broad matrix pressure |
| `GET /api/v1/me/state` | 31.02 | 1.15 s | 1.21 s | viewer state stayed up |
| `GET /api/v1/creator/me/state` | 64.24 | 419.69 ms | 645.46 ms | creator shell stayed materially healthier than the older mixed baselines |
| `GET /api/v1/creator/me/live/control` | reset | n/a | n/a | one lane hit `Connection reset by peer` |
| `GET /api/v1/creator/me/live/runtime` | 23.58 | 1.13 s | 1.22 s | runtime stayed up |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 67.72 | 494.07 ms | 530.51 ms | strong collab control result |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 55.25 | 507.45 ms | 726.45 ms | stable despite socket noise |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 51.76 | 528.26 ms | 742.22 ms | playback did not reset |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 630.67 | 41.87 ms | 664.51 ms | limiter-driven non-2xx responses remained expected |

Iteration 2:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 518.12 | 61.67 ms | 110.25 ms | healthy |
| `GET /api/v1/live/streams/deepsaint-live` | 499.68 | 63.81 ms | 110.78 ms | healthy |
| `GET /api/v1/bootstrap` | 23.90 | 1.06 s | 1.14 s | still one of the heaviest non-creator reads |
| `GET /api/v1/me/state` | 23.64 | 1.27 s | 1.31 s | stable |
| `GET /api/v1/creator/me/state` | 62.98 | 465.10 ms | 528.61 ms | stable |
| `GET /api/v1/creator/me/live/control` | 39.43 | 721.66 ms | 788.29 ms | recovered cleanly after the prior reset |
| `GET /api/v1/creator/me/live/runtime` | 23.61 | 1.24 s | 1.34 s | similar to iteration 1 |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 53.13 | 566.78 ms | 639.62 ms | stable |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 51.01 | 561.42 ms | 642.42 ms | stable |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 47.28 | 586.38 ms | 651.86 ms | still stable |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 669.10 | 44.81 ms | 101.91 ms | limiter-driven non-2xx responses remained expected |

Iteration 3:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 553.03 | 61.33 ms | 121.65 ms | healthy |
| `GET /api/v1/live/streams/deepsaint-live` | 534.57 | 62.06 ms | 121.50 ms | healthy |
| `GET /api/v1/bootstrap` | 31.49 | 1.11 s | 1.16 s | stable |
| `GET /api/v1/me/state` | 23.66 | 1.31 s | 1.36 s | stable |
| `GET /api/v1/creator/me/state` | 63.13 | 483.57 ms | 509.69 ms | stable |
| `GET /api/v1/creator/me/live/control` | 38.38 | 754.82 ms | 1.02 s | stayed up; no reset repeated |
| `GET /api/v1/creator/me/live/runtime` | 22.18 | 1.31 s | 1.42 s | stayed up |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 47.34 | 578.52 ms | 890.94 ms | stayed up |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 47.35 | 579.42 ms | 894.53 ms | stayed up |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 42.57 | 604.46 ms | 922.46 ms | still no reset reproduced |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 538.35 | 49.43 ms | 747.79 ms | limiter-driven non-2xx responses remained expected |

## Read of the numbers

- Public discovery and stream detail reads are fast and stable.
- Collaboration control/runtime reads are healthy for the current single-node SQLite control plane.
- Playback session issuance is solid and stayed in a good latency band.
- Creator live control and creator live runtime were the dominant bottlenecks under mixed load; the latest trimming pass removed the timeout-only failure mode and materially improved both endpoints.
- Creator live runtime is still the heaviest operator-panel path and remains the first place to optimize if we need more concurrency headroom.
- The compact creator shell refactor materially improved `/api/v1/creator/me/state` in isolation, proving the endpoint no longer needs a large embedded live payload to represent source-of-truth state.
- Even after the shell refactor, `/api/v1/creator/me/state` still degrades under the full mixed sweep, which means the remaining issue is shared control-plane contention rather than response size alone.
- The follow-up compact dashboard cut improved `/api/v1/creator/me/state` under targeted concurrent operator pressure from a timeout-dominant path into a slower but functioning path, which is real progress even though it still needs more headroom.
- The session-scoped runtime read refactor produced the cleanest improvement on `/api/v1/creator/me/live/runtime` so far, cutting p50 from `836 ms` to `629 ms` and p99 from `1.18 s` to `908 ms` on the same active co-stream fixture.
- The stale moderation authority repair closed a real correctness hole: stale streams now reject moderation writes the same way they already rejected public detail, playback, and chat reads.
- The creator app-state shared-read refactor improved the focused operator slice again, but the full mixed sweep still shows `/api/v1/creator/me/state` as the dominant remaining bottleneck.
- The follow-up shell refactor that removed scheduled-upload release reconciliation from reads and replaced upload-operation record hydration with summary-only aggregates finally eliminated the old `/api/v1/creator/me/state` full-mixed timeout cliff.
- After that creator-shell win, the next dominant bottleneck clearly shifted to `/api/v1/creator/me/live/runtime` and the collaboration control/runtime reads under the same shared mixed pressure.
- `/health` improved dramatically after probe caching, but it still shows a long tail on cold probe paths. The hot path is fast; the p99 reflects cache miss and dependency-check variance.
- Host chat flood non-2xxs were expected `429`-style enforcement behavior from the per-user chat limiter, not corruption or authority failure.
- The collaboration runtime fanout repair restored a healthy background worker and removed the `compiled collaboration runtime bundle missing output fanout` failure mode from `/health`.
- The collaboration read-graph collapse materially improved both creator live runtime and the host collaboration endpoints in the same pass, which confirms the remaining latency was coming from duplicated collaboration hydration rather than the runtime artifact reads alone.
- The fresh Fozzy run/trace/verify/replay/ci chain on the August 21, 2026 build stayed deterministic and clean on the current runtime-control path, which gives us replayable evidence for the core live-control surface rather than only ad hoc HTTP checks.
- Playback issuance is now well-supported by evidence: it passed the dedicated Fozzy playback scenarios, stayed healthy in isolation, stayed healthy in the direct three-lane cross-test with creator live control/runtime, and stayed healthy again in the broader five-lane cross-test with bootstrap and viewer state. The one `Connection reset by peer` observed in the earlier 11-lane sweep now looks more like a worst-case broad-matrix saturation artifact than a playback-specific defect.
- The repeated three-iteration 11-lane sweep further narrowed the anomaly: playback stayed up in all three repeated runs, while `creator/me/live/control` reset once in iteration 1 and then recovered cleanly in iterations 2 and 3. At this point the evidence supports a conclusion of broad saturation variance under the full matrix, not a reproducible route-specific playback defect.

Startup path correction on the same Friday, August 21, 2026:

- A real configuration footgun was uncovered during the load pass: when the binary was launched from the repo root without `LIFESTREAM_DATABASE_URL`, it silently opened `/Users/deepsaint/Desktop/lifestream/lifestream.db` instead of `/Users/deepsaint/Desktop/lifestream/backend/lifestream.db`.
- This was corrected in `backend/src/config.rs` by resolving the default database and media paths against the detected backend workspace root rather than the shell working directory.
- Verified after rebuilding the binary by launching from the repo root and inspecting live file handles:
  - DB path: `/Users/deepsaint/Desktop/lifestream/backend/lifestream.db`
  - Media root: `/Users/deepsaint/Desktop/lifestream/backend/media`
  - `/health` returned `ready=true`

Post-fix mixed sweep using the authoritative backend DB on the same Friday, August 21, 2026 build after trimming creator live history windows:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 393.33 to 407.39 | 75.05 to 76.93 ms | 343.08 to 383.50 ms | public listing stayed healthy |
| `GET /api/v1/live/streams/deepsaint-live` | 284.78 to 384.73 | 79.76 to 104.97 ms | 202.67 to 383.33 ms | public detail stayed healthy |
| `GET /api/v1/bootstrap` | 7.93 to 13.88 | 1.72 to 1.96 s | 1.99 s | bootstrap remained very heavy on the authoritative DB |
| `GET /api/v1/me/state` | 7.93 to 15.87 | timeout to 1.46 s | timeout to 1.79 s | viewer state is now clearly a top-tier bottleneck on the real dataset |
| `GET /api/v1/creator/me/state` | 23.80 to 26.26 | 959.27 ms to 1.00 s | 1.16 to 1.18 s | materially heavier than the earlier root-run measurements |
| `GET /api/v1/creator/me/live/control` | 15.86 to 15.94 | 1.62 to 1.70 s | 1.83 to 1.85 s | still up, but much heavier on the authoritative DB |
| `GET /api/v1/creator/me/live/runtime` | 7.93 to 7.94 | timeout | timeout | still one of the dominant remaining operator bottlenecks on the real dataset |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 23.04 to 23.79 | 1.11 to 1.15 s | 1.53 to 1.69 s | collaboration control is materially heavier on the authoritative DB |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 23.84 to 23.90 | 1.12 to 1.14 s | 1.52 to 1.63 s | collaboration runtime is materially heavier on the authoritative DB |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 23.74 to 23.78 | 1.26 to 1.27 s | 1.46 to 1.58 s | playback stayed stable, but broad-matrix pressure on the real dataset is still high |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 243.17 to 335.39 | 78.01 to 101.29 ms | 296.18 ms to 1.44 s | limiter-driven non-2xx responses remained expected |

## Remaining limits

- Friday, August 21, 2026 viewer/bootstrap refactor pass:
  - `me/state` now shares the followed-feed assembly path, batches effective live-viewer hydration, and no longer double-reads entitlements when building the viewer shell.
  - `home/bootstrap` now reuses the already-fetched live stream set when computing category live totals, and `bootstrap.me` was removed because the frontend hydrate path already sources the current user from `/api/v1/me/state`.
  - Verification stayed green on the updated binary:
    - `python3 tests/creator-app-state-check.py` -> `creator-app-state|bootstrap|consistent`
    - `python3 tests/live-runtime-control-check.py` -> `runtime|socket-inspect|connected|terminated`
    - `fozzy test --det --strict-verify tests/live-runtime-control.pass.fozzy.json tests/stale-live-read-consistency.pass.fozzy.json --json` -> `pass`

Post-viewer/bootstrap-refactor mixed sweep on the same Friday, August 21, 2026 authoritative backend DB:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 187.50 to reset | 142.63 ms | 428.75 ms | first rerun stayed up but read-heavy; second rerun hit one `Connection reset by peer` on the public-list lane while the server stayed healthy |
| `GET /api/v1/live/streams/deepsaint-live` | 208.78 to 254.01 | 115.78 to 136.49 ms | 268.86 to 371.95 ms | public detail remained stable |
| `GET /api/v1/bootstrap` | 7.92 to 7.93 | timeout | timeout | still timeout-bound under the full 11-lane matrix |
| `GET /api/v1/me/state` | 7.93 to 11.14 | timeout to 1.41 s | timeout to 1.61 s | viewer state improved versus the prior all-timeout floor, but still saturates badly under mixed pressure |
| `GET /api/v1/creator/me/state` | 23.80 to 31.73 | 915.56 ms to 1.03 s | 1.11 to 1.33 s | meaningful improvement on the creator shell |
| `GET /api/v1/creator/me/live/control` | 15.87 | 1.63 to 1.67 s | 1.85 to 1.91 s | essentially flat |
| `GET /api/v1/creator/me/live/runtime` | 7.93 | timeout to 1.58 s | timeout to 1.58 s | slightly better than pure timeout-only behavior, but still one of the dominant bottlenecks |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 19.85 to 47.85 | 277.00 ms to 1.30 s | 1.31 to 1.43 s | improved in one rerun, regressed in the other; shared-load variance remains high |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 21.06 to 33.19 | 961.44 ms to 1.26 s | 1.31 to 1.44 s | same story as collab control |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 16.86 to 24.00 | 1.16 to 1.36 s | 1.49 to 1.63 s | playback issuance stayed healthy and improved again |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 236.42 to 259.09 | 99.21 to 109.58 ms | 1.26 to 1.30 s | limiter-driven non-2xx responses remained expected |

- Read of this pass:
  - The viewer-side refactor produced a real gain on `/api/v1/me/state`, but not enough to move it out of the saturation tier under the full mixed matrix.
  - `/api/v1/creator/me/state` improved materially again, which confirms the control-plane shell is moving in the right direction.
  - The remaining dominant mixed-load bottlenecks are still `/api/v1/bootstrap`, `/api/v1/me/state`, and `/api/v1/creator/me/live/runtime`.
  - The public-list reset did not correspond to a process crash; `/health` remained `ready=true` immediately after the sweep, so the current evidence points to overload/reset behavior rather than a fatal server fault.

- Friday, August 21, 2026 watchlist/settings parallelization pass:
  - `me/state` now builds the shared `User` once from a base user row plus already-fetched watchlist/following payloads, so profile hydration no longer re-fetches the full user shell.
  - `/api/v1/bootstrap` now pulls continue-watching directly instead of reconstructing a whole `User` object.
  - `fetch_watchlist_response` now resolves saved titles concurrently instead of serially.
  - `fetch_user_settings_bundle` now reads the six user settings tables concurrently instead of one-by-one.
  - Verification stayed green on the updated binary:
    - `python3 tests/creator-app-state-check.py` -> `creator-app-state|bootstrap|consistent`
    - `python3 tests/live-runtime-control-check.py` -> `runtime|socket-inspect|connected|terminated`
    - `/health` still returned `ready=true` immediately after the full mixed sweep

Post-watchlist/settings-parallelization mixed sweep on the same Friday, August 21, 2026 authoritative backend DB:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 291.30 to 425.04 | 74.69 to 116.09 ms | 317.98 to 390.56 ms | public listing stayed broadly healthy across reruns |
| `GET /api/v1/live/streams/deepsaint-live` | 389.62 to reset | 79.67 ms | 389.39 ms | one rerun stayed strong; one rerun hit `Connection reset by peer` while the server remained healthy |
| `GET /api/v1/bootstrap` | 7.87 to 8.93 | 1.32 s to timeout | 1.99 s to timeout | still timeout-bound under the full matrix, but one rerun did get partial timed responses instead of an all-timeout cliff |
| `GET /api/v1/me/state` | 7.88 to 7.93 | timeout | timeout | viewer state remains the clearest remaining viewer-side bottleneck on the real dataset |
| `GET /api/v1/creator/me/state` | 23.80 to 28.09 | 1.04 to 1.07 s | 1.16 to 1.22 s | creator shell stayed somewhat healthier again |
| `GET /api/v1/creator/me/live/control` | 15.78 to 15.86 | 1.64 to 1.74 s | 1.75 to 1.99 s | roughly flat |
| `GET /api/v1/creator/me/live/runtime` | 7.88 to 7.93 | 1.91 s to timeout | 1.97 s to timeout | still in the dominant bottleneck tier |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 23.64 to 23.80 | 1.23 to 1.25 s | 1.42 to 1.58 s | stable |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 19.07 to 23.64 | 1.25 to 1.30 s | 1.43 to 1.56 s | slightly better one rerun, broadly still heavy |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 15.86 to 23.62 | 1.28 to 1.57 s | 1.47 to 1.71 s | stayed healthy; no playback-specific collapse reproduced |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 224.20 to 290.18 | 91.55 to 100.42 ms | 1.00 to 1.61 s | limiter-driven non-2xx responses remained expected |

- Read of this pass:
  - The additional viewer-shell refactor did remove real duplicate work, but it did not materially lift `/api/v1/me/state` under the full 11-lane matrix on the authoritative dataset.
  - The public lanes stayed healthier overall than in the immediately preceding rerun, even though one public-detail lane still reset once under worst-case shared pressure.
  - The remaining dominant bottlenecks are now even more clearly concentrated in `/api/v1/bootstrap`, `/api/v1/me/state`, and `/api/v1/creator/me/live/runtime`.
  - Because `/health` stayed `ready=true` immediately after the latest sweep, the current evidence still supports overload/reset behavior rather than fatal process instability.

- Friday, August 21, 2026 bootstrap/watchlist preview pass:
  - `/api/v1/bootstrap` no longer builds the unused `home` payload during the frontend hydrate path; it now returns `home: null` while still serving the creator bootstrap surface the frontend actually consumes.
  - Watchlist series hydration now uses series previews with season stubs instead of full episode trees, which removes a large chunk of viewer-shell work from `/api/v1/me/state`.
  - Verification stayed green on the updated binary:
    - `python3 tests/creator-app-state-check.py` -> `creator-app-state|bootstrap|consistent`
    - `python3 tests/live-runtime-control-check.py` -> `runtime|socket-inspect|connected|terminated`
    - `fozzy test --det --strict-verify tests/live-runtime-control.pass.fozzy.json tests/stale-live-read-consistency.pass.fozzy.json --json` -> `pass`
    - `/health` -> `ready=true` on the repo-root launched binary backed by `/Users/deepsaint/Desktop/lifestream/backend/lifestream.db`

Post-bootstrap/watchlist-preview mixed sweep on the same Friday, August 21, 2026 authoritative backend DB:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 247.53 | 113.08 ms | 438.55 ms | public listing stayed up cleanly |
| `GET /api/v1/live/streams/deepsaint-live` | 237.56 | 120.52 ms | 440.72 ms | public detail stayed up cleanly in this rerun |
| `GET /api/v1/bootstrap` | 7.87 | 1.93 s | 1.99 s | still timeout-bound under the full matrix even after removing the unused home build |
| `GET /api/v1/me/state` | 7.88 | timeout | timeout | viewer state remains the clearest remaining viewer-side bottleneck |
| `GET /api/v1/creator/me/state` | 24.36 | 949.16 ms | 1.39 s | creator shell remained functional and slightly healthier than earlier authoritative baselines |
| `GET /api/v1/creator/me/live/control` | 15.73 | 1.78 s | 1.88 s | broadly flat |
| `GET /api/v1/creator/me/live/runtime` | 7.87 | timeout | timeout | still in the dominant bottleneck tier |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 22.13 | 1.32 s | 1.59 s | stable |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 20.87 | 1.29 s | 1.60 s | stable |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 15.75 | 1.50 s | 1.73 s | stayed healthy |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 254.52 | 105.87 ms | 1.50 s | limiter-driven non-2xx responses remained expected |

- Read of this pass:
  - The bootstrap cut removed real dead work from the request path, but the mixed sweep shows that the creator bootstrap payload itself is still heavy enough to keep `/api/v1/bootstrap` in the saturation tier.
  - The watchlist preview cut reduced viewer-shell over-hydration, but `/api/v1/me/state` is still timing out under the full 11-lane matrix, which means there is still a deeper dominant path inside viewer-state assembly.
  - The public lanes were cleaner in this rerun, which supports the conclusion that the latest changes did remove some broad shared-pressure work even though the three heaviest lanes remain the same.
  - The next highest-value optimization target is still the deepest remaining `me/state` path, especially notification reconciliation and any remaining content hydration that is not required for the frontend shell.

- Friday, August 21, 2026 bounded-read and reconciliation-fast-path pass:
  - `me/state` now fetches bounded notification, session, history, and continue-watching windows directly instead of loading full collections and truncating them afterward.
  - Notification read reconciliation now exits early when no due pending/retrying deliveries exist for the current read scope, avoiding unnecessary dispatch scans on the hot viewer path.
  - Verification stayed green on the updated binary:
    - `cargo check` -> passed
    - `cargo test config::tests -- --nocapture` -> passed
    - `python3 tests/creator-app-state-check.py` -> `creator-app-state|bootstrap|consistent`
    - `python3 tests/live-runtime-control-check.py` -> `runtime|socket-inspect|connected|terminated`
    - `fozzy test --det --strict-verify tests/live-runtime-control.pass.fozzy.json tests/stale-live-read-consistency.pass.fozzy.json --json` -> `pass`
    - `/health` remained `ready=true` immediately after the mixed sweep

Post-bounded-read mixed sweep on the same Friday, August 21, 2026 authoritative backend DB:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 176.54 | 153.20 ms | 494.16 ms | public listing stayed up, but broad read pressure remained high |
| `GET /api/v1/live/streams/deepsaint-live` | 199.77 | 152.14 ms | 285.02 ms | public detail stayed up cleanly |
| `GET /api/v1/bootstrap` | 7.94 | timeout | timeout | still pinned in the saturation tier |
| `GET /api/v1/me/state` | 7.93 | timeout | timeout | the viewer shell is still the clearest unresolved high-load bottleneck |
| `GET /api/v1/creator/me/state` | 23.80 | 1.09 s | 1.40 s | creator shell stayed functional but not materially improved in this rerun |
| `GET /api/v1/creator/me/live/control` | 15.61 | 1.75 s | 1.91 s | flat |
| `GET /api/v1/creator/me/live/runtime` | 7.93 | 1.82 s | 1.97 s | still one of the dominant bottlenecks |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 16.12 | 1.36 s | 1.57 s | stable but heavy |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 16.36 | 1.36 s | 1.57 s | stable but heavy |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 15.86 | 1.51 s | 1.60 s | playback stayed healthy |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 211.22 | 118.10 ms | 1.53 s | limiter-driven non-2xx responses remained expected |

- Read of this pass:
  - The bounded-read cut removed real wasted work from `me/state`, but the full mixed sweep shows the dominant remaining latency is now deeper than simple collection overfetch.
  - Because `/health` remained `ready=true` immediately after the sweep and the server stayed up, the evidence still points to saturation rather than crash instability.
  - The next highest-value move is to isolate and slim the remaining expensive subgraphs inside `me/state`, especially the watchlist/following/profile/settings bundle as a combined shell rather than separate payload families.

- Friday, August 21, 2026 joined account-bundle pass:
  - `me/state` now hydrates profile, settings, and billing from one joined one-row account bundle instead of three separate read families.
  - Connected accounts remain separate, but the dominant one-to-one viewer tables are now collapsed into a single account-bundle read.
  - Verification stayed green on the updated binary:
    - `cargo check` -> passed
    - `cargo test config::tests -- --nocapture` -> passed
    - `python3 tests/creator-app-state-check.py` -> `creator-app-state|bootstrap|consistent`
    - `python3 tests/live-runtime-control-check.py` -> `runtime|socket-inspect|connected|terminated`
    - `fozzy test --det --strict-verify tests/live-runtime-control.pass.fozzy.json tests/stale-live-read-consistency.pass.fozzy.json --json` -> `pass`
    - `/health` remained `ready=true` immediately after the mixed sweep

Post-joined-account-bundle mixed sweep on the same Friday, August 21, 2026 authoritative backend DB:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 243.55 | 113.72 ms | 428.83 ms | public listing stayed up cleanly |
| `GET /api/v1/live/streams/deepsaint-live` | 297.48 | 99.16 ms | 265.23 ms | public detail materially improved in this rerun |
| `GET /api/v1/bootstrap` | 15.85 | 1.71 s | 1.79 s | finally moved materially off the old timeout floor |
| `GET /api/v1/me/state` | 8.18 | 1.92 s | 1.92 s | also moved off the pure-timeout floor, though still far too heavy |
| `GET /api/v1/creator/me/state` | reset | n/a | n/a | one creator-shell lane hit `Connection reset by peer` while the server stayed healthy |
| `GET /api/v1/creator/me/live/control` | 23.78 | 1.26 s | 1.40 s | meaningful improvement in this rerun |
| `GET /api/v1/creator/me/live/runtime` | 11.39 | 1.95 s | 2.00 s | higher throughput than the prior timeout-bound floor, but still in the dominant bottleneck tier |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 31.71 | 1.02 s | 1.20 s | materially improved |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 25.52 | 1.07 s | 1.54 s | materially improved |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 23.80 | 1.17 s | 1.59 s | playback stayed healthy and improved again |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 313.86 | 75.77 ms | 1.37 s | limiter-driven non-2xx responses remained expected |

- Read of this pass:
  - This is the first authoritative full-matrix pass where both `/api/v1/bootstrap` and `/api/v1/me/state` clearly moved off their earlier timeout-only floor at the same time.
  - The viewer shell is still too heavy for the target concurrency band, but the joined account-bundle cut proved that the remaining latency was not purely in creator/runtime pressure; part of it was still viewer-table fanout.
  - The creator/control/runtime and collaboration lanes also improved materially in the same rerun, which suggests the removed viewer-side query fanout was contributing to broader shared-pressure contention.
  - The remaining highest-value bottlenecks are now `/api/v1/me/state`, `/api/v1/creator/me/live/runtime`, and whichever creator-shell path still reset once under the full matrix.

- WebSocket throughput was validated through deterministic scenarios and runtime tests, not through a separate socket flood harness.
- The backend is now healthy and consistent end-to-end for the verified control-plane scope, but `/api/v1/creator/me/state` and `/api/v1/creator/me/live/runtime` still need deeper shared-load optimization before this can be called fully dusted for high operator concurrency.
- A remaining likely next move is to trim or decouple the upload-heavy portions of `creator/me/state` under operator-panel mixed load, because the shell still saturates before the rest of the control plane does.
- The next likely highest-value move is to cut repeated collaboration/runtime hydration from `creator/me/live/runtime` and the host collaboration endpoints, because after the latest shell fix those are now the dominant heavy paths in the mixed sweep.
- Media-plane codec, transcoding, and player-side runtime work remain outside this document. This report is strictly for the Rust control plane.

- Friday, August 21, 2026 creator-live runtime and collaboration hot-path pass:
  - Locked the SQLite default pool size to `8` after a fresh rebuilt bakeoff across `2`, `4`, `8`, and `12`, where `8` gave the best overall mixed-lane balance without the playback regressions seen at `4` and `12`.
  - Removed unconditional collaboration-session reconciliation from the direct read path by adding a session-scoped no-op eligibility guard before full reconciliation runs.
  - Stopped creator live snapshot reads from loading full broadcast history when the hot shell only needs the current `live` or `ready` broadcast window.
  - Parallelized collaboration session invite and participant hydration so host/session reads stop serializing independent subqueries.
  - Fixed collaboration topology `connectedParticipants` to count distinct active participants instead of socket tabs.
  - Fixed creator live runtime `recentEvents` to read from the active session window so the authoritative runtime feed surfaces `connected`, `heartbeat_recorded`, and `runtime_reported` together.

- Verification on the fresh rebuilt binary from Friday, August 21, 2026:
  - `cargo build` -> passed
  - `python3 backend/tests/live-runtime-control-check.py` -> `runtime|socket-inspect|connected|terminated`
  - `python3 backend/tests/creator-app-state-check.py` -> `creator-app-state|bootstrap|consistent`
  - `fozzy doctor --deep --scenario tests/live-runtime-control.pass.fozzy.json --runs 5 --seed 20260821 --json` -> `ok=true`, `consistent=true`
  - `fozzy test --det --strict-verify tests/live-runtime-control.pass.fozzy.json --json` -> `pass`
  - `fozzy run tests/live-runtime-control.pass.fozzy.json --det --record tests/live-runtime-control-20260821-current.trace.2.fozzy --proc-backend host --fs-backend host --http-backend host --json` -> `pass`
  - `fozzy trace verify tests/live-runtime-control-20260821-current.trace.2.fozzy --strict --json` -> `ok=true`
  - `fozzy replay tests/live-runtime-control-20260821-current.trace.2.fozzy --json` -> `pass`
  - `fozzy ci tests/live-runtime-control-20260821-current.trace.2.fozzy --json` -> `ok=true`

Final same-binary mixed sweep on Friday, August 21, 2026:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 187.73 | 159.85 ms | 389.24 ms | public listing stayed healthy |
| `GET /api/v1/live/streams/deepsaint-live` | 183.84 | 159.73 ms | 385.51 ms | public detail stayed healthy |
| `GET /api/v1/bootstrap` | 55.23 | 486.34 ms | 749.97 ms | still materially ahead of the earlier timeout-bound floor |
| `GET /api/v1/me/state` | 55.41 | 555.26 ms | 872.73 ms | viewer shell stayed off the old timeout floor |
| `GET /api/v1/creator/me/state` | 23.61 | 1.15 s | 1.25 s | still heavy but stable |
| `GET /api/v1/creator/me/live/control` | 19.67 | 1.43 s | 1.85 s | improved over the old `~15.7 req/s` floor |
| `GET /api/v1/creator/me/live/runtime` | 15.73 | 1.66 s | 1.81 s | event-window fix kept correctness green, but this lane remains a top bottleneck |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 23.61 | 1.01 s | 1.10 s | materially improved from the older `~15.7 req/s` band |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 25.35 | 1.01 s | 1.09 s | materially improved and now the clearest win of this pass |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 15.74 | 1.86 s | 1.91 s | still stable but remains write-pressure sensitive under full overlap |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 137.41 | 185.91 ms | 1.82 s | limiter-driven non-2xx responses remained expected |

- Read of this pass:
  - The creator live and collaboration read-path cuts were real: the direct collaboration host endpoints moved from the old `~15.7 req/s` saturation floor into the low-to-mid `20s req/s` range on the same rebuilt binary.
  - The viewer shell and bootstrap shell both remained healthy on the same binary while the authority and deterministic runtime checks stayed green, which is the best evidence so far that the backend is no longer hiding stale-binary wins.
  - `/api/v1/creator/me/live/runtime` is still the main unresolved control-plane hotspot. It is now contract-correct and deterministic, but it still does enough runtime hydration that it remains expensive under the full mixed matrix.
  - `/api/v1/playback/live/:stream/session` remains functionally healthy, but the mixed matrix still shows that write-path contention under concurrent creator/runtime load is the next major production sensitivity after creator live runtime.

- Friday, August 21, 2026 playback grant hot-path pass:
  - Removed the immediate post-insert playback session re-read and now build the issued playback session directly from the authoritative insert payload.
  - Collapsed playback user settings hydration into one query and built grant-side preference + preview inputs in parallel before assembling the final tracks.
  - Verification on the freshly rebuilt foreground binary:
    - `cargo build` -> passed
    - `python3 backend/tests/creator-app-state-check.py` -> `creator-app-state|bootstrap|consistent`
    - `python3 backend/tests/live-runtime-control-check.py` -> `runtime|socket-inspect|connected|terminated`
    - `/health` -> `ready=true` before and after the mixed sweep

Post-playback-grant-pass mixed sweep on the same Friday, August 21, 2026 fresh rebuilt binary:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 224.96 | 123.19 ms | 358.23 ms | public list stayed healthy |
| `GET /api/v1/live/streams/deepsaint-live` | 209.30 | 134.54 ms | 359.59 ms | public detail stayed healthy |
| `GET /api/v1/bootstrap` | 60.60 | 493.48 ms | 739.91 ms | bootstrap remained in the healthy post-refactor band |
| `GET /api/v1/me/state` | 47.29 | 766.21 ms | 924.95 ms | viewer shell stayed materially below the old timeout floor |
| `GET /api/v1/creator/me/state` | 23.60 | 1.19 s | 1.25 s | creator shell remained stable but heavy |
| `GET /api/v1/creator/me/live/control` | 15.76 | 1.46 s | 2.00 s | still load-sensitive with a few timeout-bound sockets |
| `GET /api/v1/creator/me/live/runtime` | 15.73 | 1.75 s | 1.90 s | still one of the dominant control-plane hotspots |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 23.63 | 1.00 s | 1.15 s | collaboration control stayed in the improved band |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 23.59 | 1.03 s | 1.15 s | collaboration runtime stayed stable despite some socket read/write noise |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 7.87 | 0.00 us | 0.00 us | still timed out across all `32` sockets under the full mixed overlap, so the next real move must be deeper than the removed re-read |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 136.54 | 172.39 ms | 1.87 s | limiter-driven non-2xx responses remained expected |

- Read of this pass:
  - The playback grant cleanup removed real redundant work, but the full mixed sweep says the dominant cost of `POST /api/v1/playback/live/:stream/session` is still inside live playback target/runtime readiness validation and shared write contention, not the old session re-fetch.
  - The broader shell and collaboration lanes held their post-refactor health band on the same fresh binary, so this pass did not regress the stronger control-plane reads.
  - The highest-value remaining control-plane bottlenecks are still `GET /api/v1/creator/me/live/runtime` and `POST /api/v1/playback/live/:stream/session` under fully overlapped operator traffic.

- Friday, August 21, 2026 shared-contention and hot-path isolation pass:
  - Stopped `creator/me/live/runtime` from re-querying recent active-session state it already had in hand when the creator already has an active ingest session.
  - Narrowed live playback issuance readiness checks to authoritative manifest readiness instead of re-running full archive and collaboration artifact inspection on every issued playback session.
  - Split live snapshot broadcast reads into direct indexed `live` and `ready` reads instead of a mixed-status temp-sort path.
  - Added explicit mixed-load SQLite tuning and ingest-session hot-path indexes:
    - `temp_store = MEMORY`
    - `cache_size = -32768`
    - `mmap_size = 268435456`
    - `idx_live_ingest_sessions_creator_connected_at`
    - `idx_live_ingest_sessions_creator_status_heartbeat`
    - `idx_live_ingest_sessions_broadcast_status_heartbeat`
  - Verification on the freshly rebuilt foreground binary:
    - `cargo build` -> passed
    - `python3 backend/tests/creator-app-state-check.py` -> `creator-app-state|bootstrap|consistent`
    - `python3 backend/tests/live-runtime-control-check.py` -> `runtime|socket-inspect|connected|terminated`
    - `/health` -> `ready=true` before and after the mixed sweep

Isolated hot-path probes on the same Friday, August 21, 2026 fresh binary:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/creator/me/live/runtime` | 283.96 | 107.76 ms | 149.29 ms | isolated runtime endpoint is healthy on its own |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 214.77 | 139.20 ms | 222.05 ms | isolated playback issuance is also healthy on its own |

Post-shared-contention-pass mixed sweep on the same Friday, August 21, 2026 fresh rebuilt binary:

| Endpoint | Req/s | p50 | p99 | Notes |
| --- | ---: | ---: | ---: | --- |
| `GET /api/v1/live/streams` | 190.63 | 126.44 ms | 329.21 ms | public list stayed healthy, though noisier under overlap |
| `GET /api/v1/live/streams/deepsaint-live` | 300.48 | 108.07 ms | 322.02 ms | public detail improved materially in this rerun |
| `GET /api/v1/bootstrap` | 55.73 | 482.38 ms | 807.95 ms | bootstrap held its healthy post-refactor band |
| `GET /api/v1/me/state` | 63.46 | 497.37 ms | 892.94 ms | materially improved under the full mixed matrix |
| `GET /api/v1/creator/me/state` | 23.81 | 1.24 s | 1.31 s | creator shell remained stable but heavy |
| `GET /api/v1/creator/me/live/control` | 15.87 | 1.69 s | 1.96 s | still heavily load-sensitive under overlap |
| `GET /api/v1/creator/me/live/runtime` | 15.88 | 1.66 s | 1.89 s | essentially unchanged under the full mixed matrix despite strong isolated health |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/control` | 35.46 | 961.80 ms | 1.11 s | clear improvement from the prior low-20 req/s band |
| `GET /api/v1/creator/me/live/collabs/sessions/:id/runtime` | 31.73 | 953.81 ms | 1.09 s | clear improvement from the prior low-20 req/s band |
| `POST /api/v1/playback/live/lv-deepsaint-live/session` | 7.93 | 0.00 us | 0.00 us | still timeout-bound under full overlap even though isolated playback issuance is healthy |
| `POST /api/v1/live/streams/lv-deepsaint-live/chat/messages` | 141.05 | 135.83 ms | 1.77 s | limiter-driven non-2xx responses remained expected |

- Read of this pass:
  - The isolated probes prove the remaining problem is not raw endpoint implementation speed. Both `creator/me/live/runtime` and live playback issuance are healthy on their own.
  - The mixed sweep still pins those same lanes when the whole platform is active, which means the current ceiling is shared concurrency pressure across the control plane rather than local endpoint logic alone.
  - The shared SQLite/snapshot pass did produce real wins anyway: `me/state` improved materially and both collaboration host endpoints moved from the low-20 req/s band into the low-to-mid 30s.
  - The next highest-value work is now at the contention architecture layer: reducing shared database pressure between heavy read shells and playback session writes, or splitting the hottest live reads away from the write-sensitive issuance path.
