# Vanta OBS Remaining Engineering Plan

## Direction

Vanta OBS is a standalone Vanta-native live production workstation that uses OBS as an implementation reference and compatibility source, not as the default product shell.

OBS is valuable because it has already solved a decade of hard capture, compositing, encoding, recording, device, and plugin problems. Vanta should take every applicable implementation lesson from OBS while keeping the product architecture focused on Vanta's Twitch-like live platform, creator monetization, sponsor proof, archive generation, clips, guests, moderation, and publishing.

The default path is:

- study OBS internals aggressively;
- mirror the useful media concepts in Vanta-native Rust/domain contracts;
- interoperate with local OBS through WebSocket first;
- import and export OBS scene collections;
- build native capture/encode/replay helpers for production-grade output;
- create a lightweight OBS plugin only for creators who keep OBS Studio;
- avoid carrying OBS features that do not support Vanta Live, sponsor inventory, guest production, archive, or publishing workflows.

Directly vendoring OBS or libobs is allowed only as an explicit product/legal decision. OBS Studio is distributed under GPL-2.0-or-later, so copied or linked implementation has licensing and distribution consequences that must be accepted before it enters the production tree.

## Value Filter

Every OBS-derived feature must pass this filter before implementation:

- it directly improves Vanta live creation, preview/program production, source quality, stream reliability, guest production, audio quality, sponsor execution, proof capture, recording, replay, clips, archive, moderation, publishing, or live ops recovery;
- it can be expressed through Vanta's product model without exposing generic OBS configuration sprawl;
- it has a clear end-to-end user workflow, persistence model, runtime behavior, and verification path;
- it does not add niche source/filter/plugin complexity that only benefits edge-case OBS setups;
- it can be cut without harming creators who use Vanta as the primary Twitch-like live studio.

Reject or defer:

- novelty filters and decorative effects without sponsor, stream, brand, accessibility, or production value;
- broad plugin-hosting surfaces before bridge/import workflows prove demand;
- advanced encoder knobs that Vanta can choose automatically from quality/latency profiles;
- generic scene collection settings that do not affect Vanta output;
- OBS UI conventions that make the app text-heavy or operator-hostile;
- any feature whose only argument is parity rather than creator value.

## Current Foundation

Do not re-plan these as future work:

- standalone `vanta-obs` app separated from `vanta-video-editor`;
- Rust API with SQLite persistence;
- React control-room shell using Vanta UI primitives and video-first layout;
- seeded broadcast, scene collection, scenes, sources, source instances, audio channels, cues, replay rows, runtime state, health state, preflight result, and post-show package rows;
- basic scene creation, patching, duplication, and send-to-program state transitions;
- basic source creation and patching;
- basic source instance creation and patching;
- basic broadcast creation, start, and end state transitions;
- basic recording start and stop state transitions;
- basic replay marker save;
- basic live cue creation and trigger state;
- basic preflight persistence;
- basic post-show send-to-editor state;
- Fozzy scenario asset and canonical trace for the extracted app;
- initial Aegis browser validation for the player-focused UI shell.

Everything below is remaining work only.

## Product Target

Build a professional Vanta Live Studio that supports:

- production-grade preview/program operation;
- scheduled live broadcasts bound to Vanta runtime;
- creator, producer, moderator, and guest roles;
- real scene/source composition;
- local and runtime streaming output;
- local and runtime recording output;
- replay buffer and clip drafting;
- sponsor/ad inventory execution and proof capture;
- live chat, moderation, alerts, and audience signals;
- archive packaging;
- Vanta Editor and publishing handoff;
- live ops safety and recovery.

The interface stays video-first: program window dominant, preview/program workflow obvious, compact operational details around the player, and minimal explanatory copy.

## Architecture Target

```text
vanta-obs/
  backend/
    src/
      api/
      auth/
      domain/
      obs/
      runtime/
      bridge/
      media/
      store/
      worker/
  frontend/
    src/
      app/
      components/
      engine/
      lib/
      pages/
      styles/
      types/
  native/
    capture/
    audio/
    encode/
    replay/
  plugin/
    obs/
  tests/
```

Remaining architecture work:

- replace one-file frontend app composition with page, shell, player, rail, mixer, inspector, cue, and runtime components;
- create `frontend/engine` for preview graph, source permissions, device sessions, canvas rendering, and local synchronization;
- create native helper protocol packages before implementing platform-specific binaries;
- keep OBS import/export code behind explicit adapters.

## OBS-Derived Implementation Program

### Compatibility Harness

Needed:

- export golden files;
- transform/crop/opacity/z-order parity assertions for export;
- WebSocket mock server with event sequencing;
- compatibility matrix for OBS versions and platform differences.

### Optional Vendored OBS Track

Needed before any vendored OBS code lands:

- legal approval for GPL obligations;
- explicit open-source distribution posture if Vanta ships a derivative;
- build isolation plan for C/C++/Qt/libobs dependencies;
- patch management strategy against upstream OBS;
- security update workflow;
- reproducible builds for macOS and Windows;
- clear boundary between vendored OBS code and Vanta-native product code;
- removal plan if vendoring blocks commercial distribution.

## OBS Compatibility

### OBS Export

Needed:

- export representable Vanta scenes and sources into OBS scene collection JSON;
- include browser sources for Vanta overlays, sponsor cards, chat, alerts, QR cards, and runtime widgets;
- include warnings for Vanta-native runtime features that cannot be expressed in OBS;
- generate setup instructions and asset bundle paths.

### Vanta OBS Plugin

Needed after bridge workflows prove durable:

- authenticate to Vanta;
- expose sponsor cue dock;
- expose stream health dock;
- trigger proof markers;
- send replay markers to Vanta;
- sync live state and archive status;
- avoid duplicating the full Vanta Live Studio UI inside OBS.

## Native Media Engine

Needed:

- local native helper process;
- signed macOS binary first, then Windows;
- explicit localhost or stdio protocol;
- health heartbeat;
- crash recovery;
- version compatibility checks;
- sandboxed file permissions;
- deterministic logs and trace events;
- graceful degradation to browser preview plus external ingest instructions.

### Capture

Needed:

- camera enumeration and permission flow;
- microphone enumeration and permission flow;
- display capture;
- window capture;
- application audio capture where platform allows;
- device hotplug handling;
- reconnect behavior;
- source health events;
- low-latency preview frames;
- explicit unsupported-device errors.

### Compositing

Needed:

- real preview renderer;
- real program renderer;
- deterministic scene graph;
- transform, crop, opacity, fit, fill, stretch, rotation, safe-area, lock, and visibility semantics;
- nested scenes and groups;
- GPU acceleration path;
- software fallback path;
- frame pacing;
- dropped-frame reporting.

### Encoding And Muxing

Needed:

- H.264 output;
- H.265 and AV1 capability detection;
- AAC and Opus output where appropriate;
- hardware encoder selection;
- bitrate control;
- keyframe interval control;
- latency profiles;
- fragmented MP4 and/or MKV recording;
- HLS/CMAF packaging for archive and playback;
- muxer recovery after partial failure;
- final playable output validation.

### Replay Buffer

Needed:

- rolling encoded buffer;
- configurable 15s, 30s, 60s, and custom saves;
- sponsor proof tagging;
- clip draft creation;
- disk pressure handling;
- memory pressure handling;
- instant Vanta upload or deferred local queue.

## Audio Engine

Needed:

- live input capture;
- audio graph;
- per-source gain, mute, solo, monitor, delay, and routing;
- program bus;
- monitor bus;
- guest mix-minus bus;
- desktop audio capture;
- media-source audio;
- audio meters;
- peak and clipping warnings;
- noise suppression;
- noise gate;
- compressor;
- limiter;
- channel routing;
- per-participant isolated recording;
- audio drift detection and sync correction.

## Source System

Remaining source implementations:

- real camera;
- real microphone;
- real display capture;
- real window capture;
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

Each source still needs:

- schema validation;
- editor controls;
- renderer implementation;
- health model;
- permission model;
- serialization;
- OBS import/export mapping;
- test fixtures.

## Scene And Transition System

Needed:

- safe scene deletion;
- scene reordering;
- scene validation rules beyond stored status;
- scene templates;
- scene group editing;
- nested scene references;
- hotkeys;
- true cut, fade, dip to black, swipe, and stinger transitions;
- configurable transition duration applied by renderer;
- transition preview;
- transition interruption rules;
- audit log for live scene changes.

## Vanta Live Runtime

Needed:

- bind OBS workspace to `broadcasts`;
- bind runtime state to `live_ingest_sessions`;
- bind stream outputs to `live_runtime_outputs`;
- bind telemetry to `live_runtime_telemetry`;
- bind targets to `live_runtime_targets`;
- bind viewer playback readiness to live playback grants;
- publish creator live websocket updates;
- receive runtime error events;
- expose reconnect, packaging, archive, and source validation state;
- support emergency disconnect and holding scene routing;
- support Vanta Live Ops override workflows.

## Streaming Output

Needed:

- stream key/session issuance;
- ingest target negotiation;
- RTMP, SRT, WebRTC, or runtime-backed output decision;
- real stream start;
- retry and reconnect policy;
- bandwidth estimation;
- dynamic bitrate response;
- stream health thresholds;
- viewer playback readiness check;
- start confirmation;
- end confirmation;
- forced end for safety/live ops.

## Recording Output

Needed:

- real media recording, not only persisted recording state;
- record without streaming;
- record while streaming;
- pause/resume where safe;
- single program recording;
- clean-feed recording;
- isolated audio recording;
- per-participant archive;
- local short-session recording;
- runtime-backed long-session recording;
- automatic upload;
- archive integrity validation;
- failed segment recovery;
- publish as Vanta asset.

## Guest And Collaboration

Needed:

- invite links;
- backstage room;
- guest device checks;
- guest promotion into scenes;
- guest removal;
- guest mute/solo;
- return audio;
- return video;
- mix-minus;
- guest connection health;
- isolated guest recording;
- mirrored guest channels where permitted;
- moderator controls;
- safety disable.

## Twitch-Like Platform Layer

Needed:

- live channel state;
- stream title/category/tags editing bound to runtime;
- mature content flag;
- language;
- schedule management;
- follower notifications;
- raids/redirects if product wants them;
- chat modes;
- slow mode;
- subscriber/follower-only modes;
- pinned messages;
- moderation queue;
- blocked terms;
- moderator roles;
- live polls/predictions if product wants them;
- alerts;
- tips/subscriptions/revenue telemetry;
- viewer count;
- uptime;
- peak viewers;
- average viewers;
- live discovery metadata.

## Sponsor And Ad Inventory

Needed:

- campaign attachment from Vanta backend;
- sponsor card source renderer;
- lower third source renderer;
- branded bumper source;
- pinned CTA;
- QR code source;
- promo code source;
- required/prohibited claim display;
- cue scheduling against runtime clock;
- proof marker capture from real media;
- proof clip generation;
- post-show proof export;
- missed-inventory warnings;
- ad ops review workflow;
- campaign performance measurement handoff.

## Clip And Archive Pipeline

Needed:

- mark live moments against the encoded timeline;
- save replay moments as media;
- create clip drafts;
- tag clips for sponsor proof;
- tag clips for social promotion;
- attach clips to broadcast/archive;
- send real clips to Vanta Editor;
- generate post-show clip pack;
- generate archive asset;
- generate captions/transcript;
- generate thumbnails;
- publish archive;
- publish highlights.

## Safety And Operations

Needed:

- real preflight gates that block unsafe starts;
- override permissions;
- risky-action confirmations;
- safe-mode workflow;
- emergency holding scene;
- guest disable;
- stream end confirmation;
- recording discard confirmation;
- campaign-linked recording warning;
- runtime incident log;
- local helper logs;
- source permission diagnostics;
- support bundle export.

## Persistence

Remaining durable models:

- source filters;
- hotkeys;
- OBS export jobs;
- native helper sessions;
- local recording manifests with segment integrity;
- runtime bindings to authoritative Vanta live tables;
- post-show package assets;
- media proof artifacts.

Persist enough original OBS metadata to debug import/export fidelity without making OBS the internal source of truth.

## Testing And Verification

Needed:

- Rust unit tests for domain validation;
- API integration tests for every route;
- SQLite migration tests;
- OBS export fixture tests;
- native helper protocol tests;
- media capture capability tests;
- audio graph tests;
- replay buffer tests;
- archive packaging tests;
- frontend component tests;
- Aegis browser flows for studio operation;
- Fozzy deterministic scenarios for stream start/end, recording, replay, sponsor proof, guest promotion, OBS bridge sync, and post-show handoff;
- trace verify/replay/ci for every production scenario.

## Delivery Order

1. Add OBS export with compatibility warnings and asset bundle manifests.
2. Add frontend import/sync UI around existing scene/source controls.
3. Split frontend into production component and engine modules.
4. Add browser device acquisition for camera, microphone, and display preview.
5. Add real browser preview compositing with canvas capture.
6. Add native helper protocol and process supervision.
7. Add native capture and encoding path.
8. Add real audio graph and meters.
9. Add replay buffer media capture.
10. Add Vanta runtime ingest start/end integration.
11. Add archive packaging and post-show clip/proof handoff.
12. Add guest/collaboration controls and mix-minus.
13. Add safety/live ops controls.
14. Add optional Vanta OBS plugin after bridge workflows prove durable.
15. Revisit vendored OBS/libobs only if the native/helper path cannot meet parity or performance targets.

## Non-Goals

- Do not make a text-heavy dashboard.
- Do not clone every OBS plugin surface.
- Do not carry scenes, sources, filters, outputs, or settings that have no Vanta creator, livestream, sponsor, archive, guest, moderation, or publishing use case.
- Do not let browser preview imply production-grade streaming if output is not actually captured, encoded, muxed, and delivered.
- Do not hide unsupported OBS import details.
- Do not vendor OBS code without accepting the license, build, distribution, and maintenance obligations.

## Success Criteria

Vanta OBS is production-ready when a creator can:

- import or create a scene package;
- connect devices;
- run preflight;
- go live to Vanta;
- operate preview/program scenes;
- mix audio;
- bring in guests;
- execute sponsor inventory;
- record the show;
- save replay moments;
- generate proof clips;
- end the stream;
- package the archive;
- send the archive and clips to Vanta Editor;
- publish the resulting Vanta media;
- recover from common device, network, and runtime failures without losing the show.
