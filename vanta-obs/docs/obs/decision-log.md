# OBS Source Audit And Vanta Decision Log

Audit date: 2026-08-25

Upstream sources:

- OBS Studio: `obsproject/obs-studio` at `bcd53e2914c68a62b2a9387a7e8ee3b59d1fd1df`
- obs-websocket: `obsproject/obs-websocket` at `1ef34bf48110c2a18184e50e41cd0b1a855e2147`

OBS Studio describes itself as capture, compositing, encoding, recording, and streaming software. It is GPL-2.0-or-later. Vanta OBS can study its architecture, interoperate with it, and reimplement compatible concepts, but copied or linked OBS/libobs code requires an explicit licensing and distribution decision before it enters the product.

## Decision Rules

Use this log before adding any OBS-derived feature.

- Keep features that directly improve Vanta live creation, preview/program production, source reliability, audio quality, sponsor execution, proof capture, recording, replay, clips, archive, guest production, moderation, publishing, or live ops recovery.
- Adapt OBS concepts into Vanta-native Rust/domain contracts instead of exposing generic OBS settings.
- Reject parity-only work, novelty filters, broad plugin hosting, and advanced knobs that Vanta can choose automatically from quality or latency profiles.
- Prefer WebSocket bridge and scene import/export before any vendored OBS/libobs implementation.
- Keep original OBS metadata for diagnostics and fallback, but never make OBS scene JSON the internal source of truth.

## Module Map

| OBS Area | Upstream Evidence | Vanta Decision | Rationale |
| --- | --- | --- | --- |
| `libobs/obs-source.*`, `libobs/obs-scene.*`, `libobs/obs-video.*`, `libobs/graphics`, `libobs/media-io` | Core scene/source/video/media primitives | Reimplement as Vanta-native scene graph, renderer, capture, audio, and media contracts | These are essential, but direct linkage creates GPL/product coupling. |
| `libobs/obs-output.*`, `libobs/obs-encoder.*`, `obs-ffmpeg`, `obs-outputs`, `obs-x264`, `obs-nvenc`, `obs-qsv11`, `mac-videotoolbox`, `coreaudio-encoder` | Output, encoder, muxing, and hardware paths | Adapt concepts through native helper and media modules | Vanta needs production output, but the UI should expose quality/latency profiles rather than raw encoder sprawl. |
| `mac-avcapture`, `mac-capture`, `linux-v4l2`, `linux-pipewire`, `linux-capture`, `win-dshow`, `win-capture`, `win-wasapi` | Platform capture plugins | Reimplement behind `native/capture` and `native/audio` helpers | Valuable for real devices and long sessions; keep platform complexity outside frontend. |
| `obs-browser`, `image-source`, `obs-text`, `text-freetype2`, `vlc-video` | Common creator sources | Implement compatible Vanta source types and OBS import/export mapping | These support Vanta overlays, sponsor cards, chat, media assets, images, and text. |
| `obs-transitions` | Transition implementations | Keep cut, fade, dip to black, swipe, stinger | These are production-useful; reject broad transition/filter novelty until a Vanta use case exists. |
| `obs-filters`, `nv-filters`, `obs-vst` | Filters, GPU filters, VST audio | Defer most filters; implement only audio chain and brand/sponsor-safe visual controls | Generic filter sprawl is not valuable unless it improves stream quality, audio safety, sponsor compliance, or accessibility. |
| `obs-websocket` protocol | Requests/events for scenes, inputs, outputs, recording, replay, and subscriptions | First-class bridge adapter with mockable protocol boundary | Best path for existing power users without forking OBS. |
| `frontend`, `frontend-tools` | Qt desktop UI, docks, profiles, hotkeys | Reference workflows only; do not copy UI | Vanta UI must stay player-first and use existing frontend design primitives. |
| `rtmp-services`, `obs-webrtc` | Streaming services and WebRTC | Adapt output-target concepts into Vanta runtime negotiation | Valuable when mapped to Vanta ingest; reject generic service lists. |
| `aja`, `decklink`, `aja-output-ui`, `decklink-output-ui` | Broadcast hardware integrations | Defer until customer demand proves value | Useful for high-end studios, but not required for Vanta's first Twitch-like creator path. |
| `frontend` profiles, global plugin settings, generic property editors | Operator configuration surface | Reject as direct UI surface | Vanta should own simple, safe workflows instead of exposing OBS configuration machinery. |
| Third-party plugin ecosystem | Broad extension surface | Defer; build a small Vanta OBS plugin only after bridge value is proven | Broad plugin compatibility increases support load without direct Vanta value. |

## Accepted Vanta Contracts

The following OBS-derived concepts become canonical Vanta contracts:

- scene collection;
- scene;
- source;
- source instance;
- source health;
- source permission;
- transform, crop, opacity, visibility, lock, z-order;
- preview scene;
- program scene;
- transition kind and duration;
- audio channel;
- program bus;
- monitor bus;
- mix-minus bus;
- stream output;
- recording output;
- native helper session;
- media capture session;
- media encode job;
- validated media output artifact;
- replay marker;
- runtime event;
- post-show package;
- OBS bridge connection;
- OBS import report;
- OBS export job;
- compatibility warning.

## Accepted Source Types

Keep these source types because they directly serve Vanta creator workflows:

- camera;
- microphone;
- display capture;
- window capture;
- browser capture;
- media file;
- image;
- text;
- lower third;
- sponsor card;
- countdown timer;
- chat overlay;
- alert overlay;
- guest feed;
- remote contribution;
- Vanta video asset;
- Vanta clip;
- color matte;
- safe-area guide;
- scene/group source.

Reject or defer unsupported OBS source/filter kinds unless a product requirement maps them to the value filter. Import should preserve their original metadata and report them as unsupported instead of silently dropping them.

## WebSocket Bridge Surface

The first bridge implementation should cover only:

- connection profile storage;
- authentication status;
- scene and scene collection reads;
- input/source reads;
- transition reads;
- audio input reads;
- current program scene;
- current preview scene;
- stream state;
- recording state;
- replay buffer state;
- scene switching;
- stream start/stop when permitted;
- recording start/stop when permitted;
- replay save when supported;
- events for scene, input, scene item, output, recording, replay, and websocket session state.

Do not expose arbitrary vendor requests, custom plugin commands, filter editing, or raw request passthrough until a Vanta workflow requires them.

## Import And Export Rules

OBS import must:

- parse scene collection JSON into Vanta contracts;
- preserve order, transforms, crop, opacity, visibility, locked state, nested scenes, groups, and transition preferences where representable;
- retain original OBS metadata for diagnostics and fallback;
- produce a migration report with warnings and omissions;
- allow partial import only when omissions are explicit.

OBS export must:

- generate OBS-compatible scene collection JSON for representable Vanta scenes;
- export Vanta overlays as browser sources where possible;
- include asset bundle references;
- include warnings for Vanta-native runtime features that OBS cannot represent.

## Vendoring Gate

Vendored OBS/libobs is not on the default path. Revisit only if native helper and WebSocket/import/export paths cannot meet performance or parity targets.

Before any vendored OBS code lands, the project needs:

- legal approval for GPL obligations;
- explicit distribution model;
- build isolation for C/C++/Qt/libobs dependencies;
- patch and security update policy;
- reproducible macOS and Windows builds;
- clear boundaries between Vanta-native code and vendored code;
- removal plan if licensing or maintenance cost blocks the product.

## Media Output Decision

Media encoding follows OBS' hardware-first output lesson without exposing OBS' raw encoder setting surface. Vanta chooses from local FFmpeg capability data, prefers platform hardware encoders when they are available, falls back to software encoders, applies product latency profiles, writes muxed output to per-job partial artifacts, validates with FFprobe before atomic promotion, and persists selected/attempted encoder plus muxer recovery metadata in media job health. Keep future encoder work behind Vanta quality, latency, reliability, and platform-output profiles unless a creator workflow proves that a specific low-level knob is necessary.

## Source Contract Decision

Source kinds are Vanta contracts first and OBS mappings second. Every accepted source kind must define its renderer class, permission kind, local sync transport, OBS-compatible kind, required payload fields, and persisted validation state before it appears in the studio UI. Keep source readiness and diagnostics in the backend contract, then let the frontend source-sync engine render compact operator state from that metadata. Do not add UI-only source types or generic OBS source passthrough without a Vanta workflow that passes the value filter.

## Source Filter Decision

Source filters are accepted only when they support production quality, keying, layout, or sponsor-safe output. Vanta persists filter kind, label, order, enabled state, settings, validation, and OBS mapping metadata, and exposes the selected source's filter chain in the compact Inspector. Keep novelty filters, broad plugin filters, and raw OBS filter passthrough out of the product until they pass the value filter. Future renderer work should apply this filter chain in preview/program composition without changing the persistence contract.

## Audio Graph Decision

Audio state is owned by the backend graph, not by the mixer component. Channels persist gain, mute, solo, monitor, program, delay, filters, and route JSON; the backend derives program, monitor, mix-minus, isolated bus state, meters, and warnings for preflight and UI display. Keep future live-sample capture, drift correction, isolated recordings, and participant audio on this graph boundary so the product can degrade and recover with clear operator state.

## Replay Clip Draft Decision

Replay saves produce Vanta-owned local clip draft artifacts, not generic OBS replay buffer files. The backend renders through FFmpeg, validates playable audio/video with FFprobe before publishing the artifact path, stores sponsor-proof and relative timeline metadata, and exposes a deferred local upload queue state to the studio Runtime panel. The studio top bar keeps replay operation compact with 15s, 30s, 60s, custom 5s-300s save lengths, and sponsor-proof mode. Future replay work should connect this draft contract to a true rolling encoded buffer, native/live media sourcing, pressure-based retention, and direct Vanta upload without exposing OBS replay configuration sprawl.

## Vanta Runtime Decision

The Go Live and End controls bind OBS workspace state to Vanta-owned runtime rows instead of pretending local OBS state is the product runtime. Starting a broadcast issues a masked stream-key hint, selects RTMP/SRT/WebRTC-style protocol from the latency profile, creates an ingest session, runtime target, program output, playback readiness grant, telemetry sample, and reconnect policy metadata, then reflects target/output/playback state in the compact studio chrome. Ending the broadcast closes ingest/output/readiness rows, including degraded outputs, and emits end-confirmation telemetry. The creator studio subscribes to a Vanta runtime WebSocket that sends dashboard snapshots, while runtime error ingestion records telemetry, severity-aware events, incidents, degraded runtime/output state, and last-error health for operator recovery. Runtime rows expose a Vanta-owned status summary for reconnect policy/count/ingest state, packaging and recording state, archive integrity, source validation, target, output, and playback readiness so operators do not need to infer health from scattered tables. Live Ops overrides are backend-owned recovery actions for safe-mode hold, forced stream end, and incident clearing; each mutates runtime/output/ingest/playback state where appropriate and writes audit events. Future runtime work should attach real media transport and retry execution to this boundary.

Runtime telemetry policy is backend-owned. Samples include bitrate, upload bandwidth, ingest latency, dropped frames, CPU pressure, reconnect count, and optional details; the backend persists the sample, derives green/yellow/red thresholds, writes output/runtime state, and chooses adaptive bitrate/resolution targets that prioritize continuity under pressure. The frontend displays `stream_health` and `adaptation` from the runtime summary only. Future transport work should execute reconnect and encoder changes from this contract instead of reintroducing UI-side policy.

Channel metadata is a Vanta platform contract, not OBS profile text. Broadcast rows own title, category, tags, mature flag, language, schedule, visibility, follower notifications, and accepted chat modes, while runtime status mirrors the current channel state for live snapshots and websocket subscribers. The studio exposes a compact Channel panel for metadata and chat mode updates beside the player. Future moderation, discovery, and monetization work should extend this Vanta platform layer instead of adding generic OBS settings.

Audience moderation is backend-owned Vanta platform state. Moderator roles, blocked terms, moderation queue items, and pinned chat messages persist against the broadcast and emit runtime events when changed. The studio only renders compact operator controls for queue resolution, pinning, blocked terms, and moderator assignment. Future chat transport should feed into this queue and blocked-term contract rather than inventing a separate moderation layer.

Audience telemetry is backend-owned Vanta platform state. Viewer count, chat velocity, tips, subscriptions, revenue, and discovery metadata persist as broadcast snapshots, and the backend derives the current, peak, average, uptime, and monetization rollups shown in the compact Audience panel. Future runtime and payments integrations should write into this contract instead of making the UI compute live platform truth.

Engagement management is a Vanta platform contract, not generic OBS UI state. Schedule slots, live polls, predictions, votes, and alerts persist against the broadcast with backend validation and derived rollups for next schedule, active poll, vote percentages, and ready alerts. Future chat, payments, and discovery integrations should feed this contract rather than adding separate transient overlays.

Sponsor inventory belongs to Vanta's ad execution model, not a generic OBS overlay bucket. Campaign attachment, creative source/cue generation, required/prohibited claims, runtime-clock scheduling, missed-inventory warnings, proof artifact review, and performance handoff metadata are backend-owned contracts shown in a compact Sponsor panel. Future real-media proof extraction should write captured artifacts into this inventory/proof model.

## Scene Transition Decision

Send-to-program creates a Vanta-owned transition execution row instead of being only a runtime pointer update. Cut, fade, dip to black, swipe, and stinger are the accepted production set because they support normal live show operation without novelty-effect sprawl. The planner returns explicit renderer ids, phase timing, stinger cut-point metadata, reduced-motion fallbacks, and a non-mutating preview contract; execution stores the same plan with previous scene, target scene, duration, replace-running interruption policy, completed status, and broadcast-scoped audit events shown in the compact studio panels. Future renderer work should attach GPU/compositor execution to this contract without exposing generic OBS transition/plugin sprawl.

## Scene Mutation Decision

Scene deletion and reordering are server-authoritative show-control mutations, not loose frontend list edits. Reorder requests must include every scene in the collection exactly once so source instance and hotkey semantics do not drift. Deletion refuses locked, collection-active, runtime preview, runtime program, and last remaining scenes; when deletion is allowed, scene-local source instances are removed, guest and cue scene references are detached, scene hotkeys are cleared, order indexes are normalized, and a broadcast event is emitted for operator audit. The scene rail exposes only compact move, duplicate, and delete icon controls around the player.

## Scene Validation Decision

Scene readiness is derived from the current Vanta graph instead of trusted as a stored label. Each scene row includes `scene_validation_json` with runtime role, visible/video instance counts, source ids, errors, and warnings derived from source instance visibility, dimensions, opacity, canvas position, missing sources, source validation, permission, sync, and health state. Preflight consumes this derived state, and the scene rail shows only compact ready/warning/blocked badges so operators see production risk without reading a configuration page.

## Scene Template Decision

Scene templates are Vanta-authored production blueprints, not generic OBS preset sprawl. The seeded set is deliberately small: dual stream, screen share/game view, and sponsor read. Each template persists layout JSON, source-kind requirements, transition defaults, and value metadata; creation verifies required source kinds, creates a normal scene, instantiates real source instances, marks the scene ready after layout creation, and emits a broadcast event. The scene rail exposes a compact picker and add control beside the player.

## Scene Group Decision

Scene groups are Vanta-owned nested scene references expressed through the existing `scene_group` source contract. Creating a group verifies both scenes are in the same collection, rejects self references and indirect cycles, creates a normal source and source instance in the target scene, and marks the target ready once the nested reference exists. Retargeting uses the same cycle guard. The browser compositor flattens nested scene graphs into the parent frame so preview/program render the child scene instead of a placeholder, and the studio exposes only compact add/retarget controls.

## Safety Decision

Safety gates are enforced by the backend, not trusted to the operator UI. Stream start re-evaluates current preflight state and blocks with a conflict when required capture, audio, scene, runtime, or sponsor checks fail. Emergency disconnect is a first-class runtime action that closes active output, routes program to the Emergency Holding scene, marks safe mode, persists a critical incident, and surfaces that state in the compact Safety panel. Support bundles persist enough runtime, health, preflight, source, audio, event, and incident diagnostics to recover or escalate without making OBS logs the source of truth.

High-risk actions use a shared backend guard contract. Stream end, recording stop, and forced live-ops end require an allowed operator role and exact confirmation phrase, and sponsor-linked broadcasts also require explicit campaign-recording acknowledgement. Safe-mode and incident clearing still require an allowed operator role. Blocked attempts emit warning events and open incidents so live ops can audit failed actions; the frontend only provides compact arming controls and never becomes the policy source.

## Post-Show Packaging Decision

Post-show handoff is a Vanta package contract backed by local artifacts, not a placeholder remote path. Ending a broadcast creates an archive manifest, clip pack, sponsor proof export, caption VTT, transcript text, archive integrity metadata, and proof/clip counts under `VANTA_OBS_MEDIA_DIR`; sending to editor writes an editor handoff manifest and updates the package row. Future archive work should add real thumbnail generation, publish workflows, long-session segment recovery, and true media-proof extraction on top of this package boundary.

## Guest Collaboration Decision

Guest collaboration follows Vanta's live-room model, not generic OBS source sprawl. The backend persists a broadcast-scoped guest room, participants, invite URLs, scene promotion, guest-feed sources, mute/solo/safety-disable controls, selective-forwarding intent, mix-minus return-feed metadata, connection health, degrade policy, and isolated-recording intent. The studio UI exposes a compact Guests panel around the player. Future collaboration work should attach this contract to real low-latency participant media transport, shared screen/game return feeds, active-speaker layout logic, moderator roles, and per-participant recorded media without exposing conferencing-engine internals to creators.

## Recording Manifest Decision

Recording state is backed by a local Vanta manifest contract before it is treated as archive-ready media. Start recording creates a per-recording directory and pending manifest; stop recording writes feed-scoped segment files, SHA-256 integrity metadata, verified segment counts, and persists those paths on the recording job for post-show packaging and Runtime panel visibility. Future native recorder work should replace the segment payloads with captured program, clean-feed, and isolated audio/video media while keeping the manifest and integrity contract stable.

## Hotkey Decision

Hotkeys are Vanta studio commands with durable bindings, guard metadata, enabled state, and backend audit events. Triggering a hotkey dispatches through the same scene, replay, recording, broadcast, and safety actions as the visible controls instead of inventing a browser-only shortcut layer. The frontend only matches keyboard events to persisted bindings, ignores editable fields, and shows compact run/toggle controls; future operator controls should keep privileged or risky actions behind backend policy checks.
