# Vanta OBS

Standalone Vanta OBS-style live production app with a Rust API and React control-room UI.

## Structure

- `backend/src/obs`: OBS scene, source, audio, cue, replay, recording, runtime, preflight, bridge, and post-show API.
- `backend/src/native`: versioned native helper protocol, session supervision, health, command, and event persistence.
- `backend/src/media`: capture-session and encode-job orchestration that prepares native helpers and stores production media intent.
- `frontend/src`: Vanta-native React UI built from the shared design primitives and player styling.
- `docs/obs`: OBS source audit, value filter, and compatibility decision log.
- `backend/tests`: Fozzy deterministic scenario and trace assets.

## OBS Scope

OBS is a reference and compatibility target, not the default product shell. Use [docs/obs/decision-log.md](docs/obs/decision-log.md) before adding OBS-derived functionality. Keep features that directly improve Vanta live production, sponsor proof, guests, recording, replay, archive, moderation, publishing, or live ops recovery; reject parity-only plugin/filter/settings sprawl.

The OBS bridge uses a typed, limited WebSocket v5 surface: connection profiles, sync snapshots, compatibility warnings, scene switching, stream start/stop, recording start/stop, replay save, and event history. It intentionally does not expose raw OBS request passthrough.

OBS scene collection import parses external OBS JSON into Vanta-owned scene, source, and instance records. Unsupported sources, filters, blend modes, and scene items are reported explicitly in durable import reports; partial imports must be requested explicitly.

Source definitions are contract-driven. `backend/src/obs/source.rs` owns the accepted Vanta source kinds, required schema fields, renderer class, permission kind, local sync transport, and OBS-compatible source kind. Source rows expose enriched contract, validation, permission, and sync metadata to the studio UI, where `frontend/src/engine/sourceSync.ts` renders compact rail and inspector states.

Source filters are durable, value-filtered contracts. The backend accepts only production-useful filter kinds such as color correction, chroma key, crop/pad, scale/aspect, and sharpness; each filter persists order, enabled state, settings, validation, and OBS mapping metadata. Source rows expose `filters_chain_json` to the Inspector. Applying those filters inside the real renderer remains part of source renderer implementation work.

Audio mixing is graph-driven. `backend/src/obs/audio.rs` derives bus membership, route state, filter state, deterministic meter telemetry, and warnings for each channel, while `/api/v1/obs/me/audio/channels/:channel_id` persists gain, mute, solo, monitor, delay, filter, and routing changes. The frontend mixer renders those backend graph fields rather than inventing local meter values.

Native media work is split between `native` and `media`: native owns helper process protocol and health, while media owns capture and encode session intent. Media encode jobs can render validated H.264/AAC fragmented MP4 and H.264/Opus MKV artifacts through FFmpeg and FFprobe, detect H.265, AV1, Opus, and local hardware encoder family availability, select hardware encoders when available with software fallback, apply Vanta latency profiles, write through recoverable partial artifacts before atomic promotion, persist selected/attempted encoder and muxer recovery metadata, and package playable jobs into HLS/CMAF VOD bundles. macOS display preview frames are captured as validated PNG artifacts, and display-backed capture sessions can write real FFmpeg/AVFoundation H.264 MP4 segment artifacts with frame-coverage and SHA-256 validation. Runtime latency adaptation and long-running/live capture execution must land behind that boundary rather than in the OBS compatibility layer.

Native capture inventory is exposed through `/api/v1/media/capture/devices`. On macOS it uses FFmpeg AVFoundation to enumerate real camera, microphone, and display devices, returns explicit support flags for unsupported window and application-audio capture, and attaches source-health metadata to capture preparation and persisted capture-session health. The Media panel shows a compact native-device count beside the player.

Display-backed capture sessions can create low-latency preview frames through `/api/v1/media/capture/sessions/:session_id/preview-frame`; frames are written under `VANTA_OBS_MEDIA_DIR`, validated for PNG dimensions and SHA-256 integrity, and listed from `/api/v1/media/capture/sessions/:session_id/frames`.

Display-backed capture sessions can also create continuous native capture segments through `/api/v1/media/capture/sessions/:session_id/segment`; artifacts are written under `VANTA_OBS_MEDIA_DIR`, validated with FFprobe for MP4 playability, duration, frame coverage, and SHA-256 integrity, persisted as capture-artifact rows, and listed from `/api/v1/media/capture/sessions/:session_id/artifacts`.

Microphone capture sessions use the same segment route to create real AAC/M4A live-input artifacts; FFprobe validates audio duration, sample rate, channel count, and SHA-256 integrity, with isolated-audio metadata persisted on the artifact.

Media-file sources can ingest isolated audio through `/api/v1/media/sources/audio`. Inputs must be absolute paths under `VANTA_OBS_MEDIA_DIR`; FFmpeg writes AAC/M4A artifacts, FFprobe validates playability, duration, sample rate, channel count, SHA-256 integrity, and audio/video drift metadata, and source-artifact rows keep the result attached to the source for studio inspection.

Desktop audio is modeled as a separate `desktop_audio` capture kind, not as a microphone. On macOS, the AVFoundation inventory classifies real loopback devices such as BlackHole, Loopback, Soundflower, Background Music, or OBS Virtual Audio as desktop audio and exposes an explicit unsupported/no-device state when none is installed. Native no-loopback system audio uses a distinct `system_audio` capture kind backed by ScreenCaptureKit, advertises `loopback_device_required: false`, records AAC/M4A artifacts through the native helper boundary when Screen Recording permission is granted, and remains separate from loopback device capture. Application audio uses `application_audio`, enumerates captureable running applications from ScreenCaptureKit, captures app-filtered AAC/M4A artifacts without a loopback device, and keeps the same permission and validation metadata as system audio. Audio artifacts use FFmpeg `aresample=async=1000:first_pts=0` where FFmpeg owns the capture/ingest path so captured/source audio is actively corrected during artifact creation.

Native helper packaging is command-driven. `cargo run --bin vanta-native-package -- build` fans out the built Rust helper into the macOS capture, encode, replay, and audio helper slots, applies the matching entitlement profile, verifies `codesign`, builds `.pkg` installers, and writes `build-manifest.json` next to each installer. `cargo run --bin vanta-native-package -- build-all` also stages the Windows helper installers and signs them with `signtool` when `VANTA_WINDOWS_SIGNING_CERT` is configured on a Windows signing host. `cargo run --bin vanta-native-package -- release-readiness` prints the same production release blockers exposed by `/api/v1/release/readiness` for CI and distribution checks. `cargo run --bin vanta-native-package -- verify-distribution` independently rechecks helper hashes, production signatures, installer signatures, macOS notarization staples, and the system-audio validation artifact against the built distribution. Add `--strict` to either release command to keep JSON output while exiting nonzero when blocked. Without production signing, notarization credentials, and `VANTA_SYSTEM_AUDIO_VALIDATION_ARTIFACT` for a permission-granted ScreenCaptureKit audio capture produced by the signed audio helper, the status remains explicitly blocked instead of reporting false production readiness.

Replay saves create validated local clip draft artifacts with sponsor-proof metadata, relative timeline data, pressure metadata, and instant local Vanta asset promotion. The studio top bar supports compact 15s, 30s, 60s, and custom 5s-300s save controls plus sponsor-proof mode. Saves resolve the latest usable native/live Vanta media source from packaged program recordings or recording assets, cut the requested replay with FFmpeg, persist the selected source in the replay manifest and Vanta media asset manifest, and use a clearly marked generated runtime fallback only when no live media source exists. The durable replay ledger in `obs_replay_buffer_segments` exposes selected source paths, hashes, retention policy, and memory pressure, while the Runtime panel surfaces replay source, buffer, asset, and memory state.

Vanta runtime start/end is persisted as Vanta-owned live state: ingest sessions, runtime targets, runtime outputs, telemetry samples, playback readiness grants, masked stream-key hints, protocol negotiation from latency profile, reconnect policy metadata, and compact top-bar/runtime readiness display. The studio subscribes to a creator runtime WebSocket for dashboard snapshots, and runtime errors are ingested as telemetry, events, incidents, degraded runtime/output state, and compact health/safety indicators. Runtime telemetry samples derive backend-owned bandwidth estimates, stream health thresholds, reconnecting/degraded state, and adaptive bitrate/resolution decisions that the Runtime panel displays without local policy logic. Runtime rows include an operator status summary for reconnect policy/count, packaging, archive integrity, source validation, target, output, and playback state. Live Ops overrides are backend-owned actions for safe-mode hold, forced stream end, and incident clearing with runtime/output/ingest mutation and audit events. Real media transport and retry execution remain separate streaming work.

Channel metadata is part of runtime state. The broadcast profile owns title, category, tags, mature flag, language, schedule, visibility, follower notifications, and chat mode; `/api/v1/obs/me/broadcasts/:broadcast_id` patches those fields with validation, emits channel update events, and mirrors the current channel into `runtime_status_json.channel`. The Channel panel keeps edits compact beside the player.

Audience moderation is a durable platform contract. Broadcasts own moderator roles, blocked terms, moderation queue items, and pinned chat messages through typed APIs. The dashboard exposes compact moderation state for pending queue count, active pin, blocked terms, and moderator roles, and the Moderation panel lets operators queue, approve/hide, pin/unpin, and add moderation controls without leaving the player-focused studio.

Audience telemetry is a Vanta platform contract. Broadcasts persist viewer, chat, tip, subscription, revenue, and discovery snapshots through `/api/v1/obs/me/broadcasts/:broadcast_id/audience/telemetry`; the backend derives current, peak, average, uptime, revenue, and discovery state for the compact Audience panel.

Engagement management is backend-owned. Broadcasts persist schedule slots, live polls, predictions, votes, and alert events; the dashboard exposes next schedule, active poll, vote percentages, and ready alerts for the compact Engagement panel beside the player.

Sponsor inventory is a Vanta execution contract. Campaign attachment updates the broadcast, accepted creative kinds create source and cue rows, and required/prohibited claims persist with each inventory item. Proof capture now writes real FFmpeg-backed proof clips, thumbnail frames, and manifests under `VANTA_OBS_MEDIA_DIR`, prefers the latest replay/recording media as source material, persists `sponsor_proof` rows in `vanta_media_assets`, enriches dashboard proofs with asset state, and moves those proofs through ad-ops review and performance handoff metadata in the compact Sponsor panel.

Recording jobs write local Vanta recording packages under `VANTA_OBS_MEDIA_DIR`: a pending manifest on start, safe pause/resume timeline ranges, FFmpeg-generated program, clean-feed, and isolated-audio media segments on stop, FFprobe playability validation, SHA-256 integrity metadata, recoverable partial-file cleanup, atomic promotion metadata, and instant local Vanta asset promotion through `vanta_media_assets`. Discarding a recording requires the exact `DISCARD RECORDING` confirmation, removes local package and asset directories, tombstones the recording and media asset rows, and audits a warning event. The Runtime panel stays collapsed by default but exposes recording integrity, asset state, and discard controls when expanded. Native/live long-session recording and true captured program sourcing remain behind the native runtime pipeline.

Scene program changes now create durable transition execution rows with previous and next scene ids, applied transition kind and duration, replace-running interruption policy, renderer preview metadata, and broadcast-scoped audit events. Cut, fade, dip to black, swipe, and stinger transitions share a Vanta-owned planner that returns explicit renderer ids, phase timing, stinger cut-point metadata, reduced-motion fallbacks, and a non-mutating preview API shown in the compact Transition panel. Scene rows expose derived `scene_validation_json` for visible sources, source readiness, layout bounds, opacity, video presence, and runtime role; preflight uses that derived state. Scene deletion and reordering are backend-owned mutations: deletes are blocked for locked, active, preview, program, or last remaining scenes, and reorder requests must include every scene exactly once. Scene templates are durable Vanta-authored blueprints for dual stream, screen share/game view, and sponsor read workflows; creating one instantiates a normal scene with real source instances. Scene groups are backend-created `scene_group` sources with same-collection and cycle guards, retargeting, compact controls, and nested rendering through the browser compositor.

Studio hotkeys are persisted backend controls, not browser-only shortcuts. Seeded bindings cover scene program changes, replay saves, recording start, go-live, and emergency hold; patch and trigger APIs dispatch through the same canonical studio actions as the UI. The frontend matcher ignores editable fields and the compact Hotkeys panel exposes run/toggle state beside the player.

Safety is backend-enforced. Stream start re-evaluates preflight and returns a conflict when blockers such as missing camera permission remain. High-risk actions require allowed operator roles, exact confirmation phrases, and sponsor-campaign recording acknowledgement when relevant; blocked attempts create warning events and incidents. Emergency disconnect stops active runtime output, routes program to the Emergency Holding scene, marks the runtime safe-mode state, persists an incident, and exposes the result in the Safety panel. Support bundles persist runtime, health, preflight, source, audio, event, and incident diagnostics for operator recovery.

Guest collaboration is a durable Vanta control-room contract. The backend owns guest rooms, backstage participants, invite links, promotion into scenes, guest source creation, mute/solo/safety-disable controls, selective-forwarding and mix-minus return-feed metadata, connection health, degrade policy, and isolated-recording intent. The Guests panel keeps those controls compact beside the player. Real participant media transport, active-speaker layouts, shared game/screen delivery, and per-participant recorded media remain separate runtime/native work.

Guest RTP contribution now persists the latency-critical decoded media path: packet ingress, jitter-buffered worker frames, H.264/Opus decoded artifacts, program/return/archive route frames, route-level A/V sync pairs, software-fallback 1920x1080 program compositor PNG artifacts, and a durable software playout ledger with sequence, pacing, jitter, and dropped-frame metadata. Continuous GPU/runtime playout remains the next media-engine layer.

Post-show packaging writes real local artifacts under `VANTA_OBS_MEDIA_DIR`: archive manifest, clip pack, sponsor proof export, caption VTT, transcript text, editor handoff manifest, FFmpeg-extracted replay thumbnails, archive asset manifests, and highlight publish manifests. Replay clips are marked against an encoded timeline, tagged for social promotion, attached to the broadcast/archive, promoted into `vanta_media_assets`, and surfaced as compact archive/highlight state in the collapsed Runtime panel.

## Run

```bash
cd backend
VANTA_OBS_BIND_ADDR=127.0.0.1:4127 cargo run

cd ../frontend
VITE_VANTA_OBS_API_BASE_URL=http://127.0.0.1:4127 npm run dev -- --port 5178
```

## Test

```bash
cd backend && cargo test
cd frontend && npm run build
cd .. && fozzy test backend/tests/obs-editor.fozzy.json --det --strict-verify --proc-backend host --fs-backend host --http-backend host --json
cd .. && fozzy test backend/tests/obs-editor-audience.fozzy.json --det --strict-verify --json
cd .. && fozzy test backend/tests/obs-editor-engagement.fozzy.json --det --strict-verify --json
cd .. && fozzy test backend/tests/obs-editor-sponsor.fozzy.json --det --strict-verify --json
cd .. && fozzy test backend/tests/obs-editor-recording.fozzy.json --det --strict-verify --json
```

## Browser Validation

Use Aegis for browser checks:

```bash
aegis --mode headless serve --detach --addr 127.0.0.1:7878
aegis --server-addr 127.0.0.1:7878 navigate http://127.0.0.1:5178/
aegis --server-addr 127.0.0.1:7878 page inspect
```

## Configuration

- `VANTA_OBS_BIND_ADDR`: backend bind address, default `127.0.0.1:4127`.
- `VANTA_OBS_DATABASE`: SQLite database path, default `vanta-obs.db`.
- `VANTA_OBS_MEDIA_DIR`: rendered media artifact directory, default system temp `vanta-obs-media`.
- `VITE_VANTA_OBS_API_BASE_URL`: frontend API base URL.
- `VANTA_NATIVE_HELPER_BINARY`: optional source helper binary for `vanta-native-package build`.
- `VANTA_MACOS_DEVELOPER_ID`: Developer ID Installer/Application signing identity for production macOS helper packages.
- `VANTA_MACOS_NOTARY_PROFILE`: keychain profile used by `xcrun notarytool submit --wait` before stapling and validating macOS helper installers.
- `VANTA_MACOS_NOTARY_APPLE_ID`, `VANTA_MACOS_NOTARY_PASSWORD`, `VANTA_MACOS_NOTARY_TEAM_ID`: direct notarytool credentials when a keychain profile is not used.
- `VANTA_WINDOWS_SIGNING_CERT`: Authenticode certificate path used by `signtool` for Windows helper binaries and installer artifacts.
- `VANTA_WINDOWS_SIGNING_CERT_PASSWORD`: optional password for `VANTA_WINDOWS_SIGNING_CERT`.
- `VANTA_WINDOWS_TIMESTAMP_URL`: optional RFC 3161 timestamp URL for Windows Authenticode signing.
- `VANTA_WINDOWS_SIGNTOOL`: optional path to `signtool`, default `signtool`.
- `VANTA_SYSTEM_AUDIO_VALIDATION_ARTIFACT`: permission-granted ScreenCaptureKit system-audio M4A produced by the signed macOS audio helper before that helper can report production-ready.
