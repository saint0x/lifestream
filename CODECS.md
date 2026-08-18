# Lifestream Lattice

`Lifestream Lattice` is the canonical name for the unified streaming software stack behind the platform.

It is not just a backend.

It is the combined control plane, media plane, playback plane, and operator plane that together make Lifestream feel like Twitch plus HBO Max while preserving Lifestream as the source of truth.

## Purpose

Lattice exists to support four classes of product behavior inside one coherent system:

1. Live broadcasting to an audience.
2. Recorded media publishing for films, long-form videos, episodes, trailers, and clips.
3. Collaborative live streaming, including guests, co-hosts, and mirrored co-streaming onto multiple channels.
4. Commercial and trust authority, including entitlements, moderation, operator controls, and auditability.

## Product Thesis

The system is not "a website that can upload and stream video."

It is a streaming operating system with four planes:

1. Identity and Authority Plane.
2. Content and Playback Plane.
3. Live Session and Collaboration Plane.
4. Trust, Monetization, and Operator Plane.

If these planes are built as one authority model, the product stays coherent.

If they are built as separate feature silos, the product will fragment under real load.

## Design Goals

Lattice must optimize for:

- low control-plane latency
- low live-stream latency
- deterministic authority decisions
- creator-grade reliability
- elite media quality
- explicit failure handling
- operator recoverability
- end-to-end auditability
- no placeholder logic in production paths

## Non-Goals

Lattice should not:

- outsource product authority to frontend logic
- make entitlement or collaboration decisions client-side
- confuse transport state with source-of-truth state
- bake media-vendor assumptions into core product objects
- treat "works in development" as production readiness

## Core Planes

### 1. Identity and Authority Plane

This plane owns:

- users
- creators
- channels
- sessions
- scopes
- roles
- moderation permissions
- collaboration permissions
- entitlements
- ownership

All watch, publish, invite, accept, monetize, and moderate decisions must resolve through this plane.

### 2. Content and Playback Plane

This plane owns:

- upload jobs
- resumable ingest
- media assets
- processing runs
- playback manifests
- posters
- ABR variants
- catalog objects
- release scheduling
- playback grants
- purchase and subscription gating

This plane must support:

- films
- episodes
- series
- regular creator channel videos
- trailers
- clips
- live VOD archives

### 3. Live Session and Collaboration Plane

This plane owns:

- broadcasts
- ingest sessions
- health samples
- collaboration sessions
- invitations
- participants
- roles such as host, guest, co-host, and mirrored co-streamer
- intended output topology
- mirrored-channel policies
- guest visibility and chat policies

This plane is the missing spine between a single-host live product and a true multiplayer creator network.

### 4. Trust, Monetization, and Operator Plane

This plane owns:

- subscriber tiers
- creator memberships
- direct purchases
- chat permissions
- moderation actions
- reports
- notification triggers
- audit trails
- repair surfaces
- operator controls

## Control Plane Responsibilities

The control plane must be the source of truth for:

- who is allowed to stream
- which broadcast is live
- which media assets belong to which published object
- who can watch which content
- who has joined a collaboration session
- whether a guest is backstage, live, mirrored, or removed
- how a collaborative stream is intended to fan out
- what recovery action is required after failure

The control plane does not encode raw media itself.

It decides what should happen and records what did happen.

## Media Plane Responsibilities

The media plane executes the realtime audio and video path:

- ingest
- relay
- routing
- compositing
- fanout
- low-latency delivery
- recording capture

The media plane may use FFmpeg and low-level codecs initially.

Over time it can adopt custom low-latency components where the performance win is real.

### Media Plane Principles

- codec choice is a means, not the product
- authority always stays above the codec layer
- the system should accept codec evolution without changing core product objects
- segmenting, muxing, relay, and ladder policy should be controllable by backend authority

## Control And Media Contract

Lattice must keep a clean boundary between the control plane and the media runtime.

The control plane decides:

- who may ingest
- what class of ingest is being created
- which broadcast or upload job owns the session
- whether the resulting output is live-only, archived, published, mirrored, private, scheduled, or blocked
- which playback objects are eligible to be issued
- whether a participant may appear on host output, mirrored guest output, or both

The media runtime reports:

- session attached
- heartbeat health
- source probe metadata
- packaging progress
- output manifest readiness
- archive completion
- terminal failure detail

The runtime must not invent product authority.

It must only execute authorized work and publish factual runtime outcomes back into the control plane.

## Runtime Classes

Lattice needs three distinct media-runtime classes:

### 1. Live Ingest Runtime

This runtime terminates incoming live contribution feeds and normalizes them into an internal production format.

Initial accepted contribution protocols:

- RTMP for broad encoder compatibility
- SRT as the preferred resilient contribution protocol

Future-ready protocols:

- WHIP for browser-native or WebRTC-native contribution
- direct RTP contribution for specialized operator workflows

### 2. VOD Processing Runtime

This runtime owns:

- source validation
- probe extraction
- transcode planning
- HLS packaging
- poster and thumbnail derivation
- subtitle normalization
- archive generation

### 3. Collaboration Routing Runtime

This runtime owns:

- host and guest attachment
- live participant routing
- mirrored output fanout
- mix-minus or equivalent audio routing
- session recording taps

This may begin with FFmpeg-adjacent routing plus narrow relays, but the design must leave room for an SFU-class subsystem if collaboration concurrency grows.

## Source Acceptance Policy

The platform should accept a wide source envelope while normalizing aggressively after ingest.

### Recorded Source Acceptance

Preferred video codecs:

- H.264 / AVC
- H.265 / HEVC
- ProRes 422 and 422 HQ
- VP9 for imports where already available

Preferred audio codecs:

- AAC-LC
- PCM
- Opus

Accepted containers:

- MP4
- MOV
- MKV
- MPEG-TS for transport-originated assets that are remuxed immediately

Hard rejection rules:

- encrypted consumer DRM containers
- missing duration or unreadable probe metadata
- variable-frame-rate sources that exceed normalization tolerance
- zero-audio and zero-video files
- corrupted streams, truncated moov atoms without recoverability, or non-seekable incomplete uploads

### Live Source Acceptance

Initial live contribution should normalize to:

- H.264 video
- AAC audio
- constant GOP policy suitable for segment generation

Preferred ingest ceilings:

- 1080p60 for broad creator support
- optional 1440p60 or 2160p30 contribution where infrastructure budget allows, while still generating a capped distribution ladder

## Canonical Distribution Codecs

For the current Lattice generation, distribution should standardize on:

- HLS as the canonical playback format
- H.264 video as the baseline distribution codec
- AAC-LC audio as the baseline distribution codec

Reasoning:

- maximal device reach
- straightforward FFmpeg pipeline support
- low operational ambiguity
- simpler entitlement-aware playback issuance

Future codec expansion should add rather than replace:

- HEVC distribution ladder for premium devices and bandwidth efficiency
- AV1 VOD ladder where encode cost and device coverage justify it

The control plane objects must not change when that codec expansion happens.

## Live Latency Classes

Lattice should define explicit latency classes instead of one vague "low-latency" promise.

### Class A: Creator Control Latency

- ingest heartbeat to control-plane reflection: under 2 seconds p95
- creator-live websocket state fanout: under 500 ms p95 after committed state change
- collaboration control event visibility: under 500 ms p95 after committed state change

### Class B: Audience Playback Latency

- standard HLS live latency target: 6 to 12 seconds glass-to-glass
- low-latency HLS target for premium paths: 3 to 6 seconds glass-to-glass

### Class C: Chat And Presence Latency

- chat fanout under 300 ms p95 within one region
- presence counter convergence under 2 seconds p95

These classes keep product promises honest and allow tradeoffs without architectural confusion.

## Packaging Policy

### Live Packaging

Baseline live packaging:

- HLS master manifest
- variant playlists per ladder rung
- MPEG-TS segments initially for broad compatibility

Low-latency evolution path:

- CMAF/fMP4 segments
- partial segments for LL-HLS
- blocking playlist reload only after the rest of the runtime is stable

Live packaging rules:

- aligned segment boundaries across all variants
- deterministic sequence numbering
- explicit discontinuity handling during reconnect or source restart
- archive splice policy that tolerates transient contribution loss without corrupting final playback assets

### VOD Packaging

VOD packaging must produce:

- HLS master playlist
- variant playlists
- poster image
- at least one scrub-friendly thumbnail representation
- normalized media metadata persisted into the control plane

VOD manifests must be immutable once published unless a deliberate reprocessing generation replaces them.

## ABR Ladder Policy

The ladder should be policy-driven by asset class, not one universal template.

### Baseline VOD Ladder

- 426x240 at ~400-700 kbps video
- 640x360 at ~800-1200 kbps video
- 854x480 at ~1400-2200 kbps video
- 1280x720 at ~2800-4500 kbps video
- 1920x1080 at ~5000-8000 kbps video

Audio baseline:

- stereo AAC-LC
- 96 kbps for low rungs
- 128-192 kbps for mid/high rungs

### Live Ladder

Live ladders must bias for encoder predictability and cost control:

- 360p
- 480p
- 720p
- 1080p

Live must not automatically generate premium ladders that the source, budget, or operator policy cannot sustain.

### Ladder Exceptions

- animation can tolerate different bitrate heuristics
- high-motion game streams need more bitrate headroom than talking-head content
- long-form cinematic uploads may justify slower presets and better perceptual allocation

The transcode planner should therefore accept content-class hints from the control plane.

## GOP And Segment Rules

Recorded and live assets should follow deterministic GOP and segment expectations:

- keyframe interval aligned to segment boundaries
- stable GOP cadence
- no scene-cut behavior that destroys ABR alignment when packaging requires alignment

Initial target:

- 2-second GOP
- 4- or 6-second standard HLS segment duration
- lower segment durations only where LL-HLS is explicitly enabled

## Audio Policy

Audio must be treated as a first-class product surface.

Required baseline behaviors:

- preserve primary language metadata
- normalize channel layout into safe distribution representations
- retain source language labeling
- support dubbed and alternate tracks as explicit asset attachments
- store loudness-related probe data for future QC and normalization

For collaboration:

- host and guest routing must prevent recursive echo
- mirrored output policies must define whether chat alerts, music, or host-only buses are included

## Subtitle And Caption Policy

Lattice should support:

- sidecar WebVTT as the primary playback caption format
- source subtitle import from SRT and WebVTT
- normalized language tagging
- forced, SDH, and standard subtitle role labeling

The control plane must know whether subtitles are:

- source-provided
- auto-generated
- human-reviewed
- published

## Poster, Thumbnail, And Preview Policy

Every playable asset class should define required derived artifacts.

### Required Derivatives

- poster image
- card thumbnail
- player thumbnail
- timeline preview strip or equivalent scrub asset for long-form content

### Optional Derivatives

- hero background still
- square social thumbnail
- clip teaser assets

These are not frontend cosmetics.

They are media-plane outputs with control-plane state and failure handling.

## Canonical Domain Objects

### User

A human identity that can watch, chat, purchase, subscribe, join a collaboration, or own a creator identity.

### Creator

A publishing and broadcasting authority bound to a user.

### Channel

The public-facing home for a creator's live and recorded output.

### Broadcast

A live event anchored to one host creator, optionally with a collaboration session.

### Collaboration Session

The authority object that represents a multi-party live session.

It binds:

- one host creator
- one source broadcast
- zero or more invited participants
- role and chat permissions
- mirrored co-stream intent
- output topology intent

### Participant

A user or creator inside a collaboration session with a role and lifecycle state.

### Invite

A pending request from a host into a collaboration session.

### Upload

A published recorded object or a container for one, such as a film, episode, VOD, trailer, or clip.

### Media Asset

The processed source-of-truth media representation behind an upload.

### Playback Session

A scoped capability to access a prepared playback asset.

## Collaboration Model

Lattice must support three collaborative live modes:

1. Guest mode.
   A participant appears on the host stream but does not mirror onto their own channel.

2. Co-host mode.
   A participant appears on the host stream and may receive elevated collaboration controls.

3. Mirrored co-stream mode.
   A participant appears on the host stream and may also pick up the collaborative session onto their own channel.

### Participant States

The control plane must explicitly model participant state:

- invited
- accepted
- backstage
- live
- removed
- left
- declined

### Output Topology

The control plane must model intended outputs separately from media execution:

- host-only output
- host plus mirrored guest output
- host plus multiple mirrored outputs
- recording enabled or disabled per session

This allows the control plane to stay authoritative even if media routing changes underneath it.

## Recorded Media Program

Recorded media must support:

- resumable ingest
- durable storage keys
- probe and validation
- poster generation
- ABR packaging
- scheduled release
- entitlement gating
- public and creator-private access paths

### Target Improvements Beyond Current Baseline

Current real functionality exists for probe, poster, and HLS generation.

To reach elite production quality, Lattice still needs:

- per-title encoding heuristics
- device-aware ladder policy
- LL-HLS support where appropriate
- scrubbing previews and chapter assets
- stronger validation and repair jobs
- origin and cache policy hardening

### Required Validation Gates

Before a recorded asset becomes publishable, the runtime must prove:

- source checksum recorded
- probe metadata stored
- duration is sane for the declared asset class
- frame size and frame rate are sane
- at least one playable output variant exists
- poster generation succeeded
- manifest generation succeeded
- persisted media-asset metadata matches generated files

### Processing State Machine

The processing lifecycle should remain explicit:

- `created`
- `uploading`
- `uploaded`
- `probing`
- `validated`
- `processing`
- `packaged`
- `ready`
- `failed`
- `replaced`
- `takedown`

Operator retries must always create a new processing attempt while preserving the original audit trail.

## Live Delivery Program

Single-host ingest authority already exists.

To reach the desired product, the live program must grow into:

- session graph authority
- guest invite and accept flows
- mirrored co-stream policy
- SFU or equivalent router coordination
- reconnect-safe participant lifecycle
- chat and moderation rules per participant role
- recording topology control
- operator tools for teardown and repair

### Live Session Runtime State

The live runtime must distinguish:

- contribution attached
- contribution healthy
- contribution degraded
- contribution stale
- contribution disconnected
- packaging active
- packaging degraded
- archive finalizing
- archive complete

The control plane should use these runtime facts to drive visibility, playback issuance, and operator alerts.

### Collaboration Output Modes

Every collaboration session should declare one of these output intents:

1. Host only
2. Host plus one mirrored guest channel
3. Host plus many mirrored guest channels
4. Host plus recording-only guest appearance

That output intent must map to deterministic media-runtime wiring, not ad hoc participant behavior.

## Storage Layout Policy

At the filesystem or object-key layer, outputs should remain generation-scoped and immutable.

Suggested shape:

- `/media/<creator>/<yyyy>/<mm>/uploads/...` for raw accepted sources
- `/processed/<creator>/<asset-id>/<generation>/hls/...` for packaged playback generations
- `/processed/<creator>/<asset-id>/<generation>/images/...` for posters and thumbnails
- `/processed/<creator>/<asset-id>/<generation>/captions/...` for normalized subtitle outputs
- `/archives/live/<creator>/<broadcast-id>/<generation>/...` for live archives

Never overwrite a published generation in place.

Publish by pointer swap in the control plane.

## Origin And Playback Grant Policy

Playback manifests and segments must only be served through backend-authorized paths.

Requirements:

- short-lived playback sessions
- explicit content binding
- entitlement or live-visibility validation before session issuance
- revocation-safe session expiry
- immutable generated asset paths underneath revocable session grants

This lets storage stay cache-friendly while authority remains revocable.

## Canonical Control Objects

Lattice should standardize on a small number of canonical authority objects that every plane understands:

- `creator`
- `channel`
- `broadcast`
- `upload_job`
- `media_asset`
- `playback_session`
- `collaboration_session`
- `collaboration_participant`
- `collaboration_mirror_grant`
- `entitlement`
- `moderation_action`

Rules:

- a `broadcast` is the authority object for a live event
- an `upload_job` is the authority object for recorded ingest and processing
- a `media_asset` is the canonical processed result, not the raw source file
- a `playback_session` is the revocable watch grant, not the content itself
- a `collaboration_session` is the authority graph for multiplayer live behavior

The media runtime may produce many files and transient runtime handles, but those must collapse back into these objects.

## Ingest Session Classes

Lattice should explicitly distinguish ingest session classes because they have different operational and security properties:

### 1. Live Creator Ingest

Used when a creator is originating a live broadcast to their own channel.

Authority requirements:

- bound to one creator
- bound to one broadcast
- validated by stream key or equivalent ephemeral ingest token
- heartbeat-driven liveness
- explicit transition into `connected`, `stale`, `terminated`, or `completed`

### 2. Collaboration Guest Ingest

Used when a guest or co-host contributes media into a collaboration session.

Authority requirements:

- bound to one collaboration participant
- bound to one collaboration session
- may publish to host output, mirrored guest output, or both, according to authority state
- must be revocable immediately if the participant is removed or downgraded

### 3. Recorded Upload Ingest

Used for films, episodes, clips, and regular creator uploads.

Authority requirements:

- bound to one upload job
- resumable and checksum-aware
- immutable after completion
- never considered publishable until processing produces a valid `media_asset`

## Codec And Packaging Policy

The initial codec stack should optimize for broad device reach and operational predictability rather than premature exotic complexity.

### Contribution Acceptance

Accept at ingest:

- H.264 / AVC
- H.265 / HEVC
- ProRes 422 family
- AAC-LC
- Opus
- PCM

Conditionally accept:

- VP9 imports for recorded content
- AV1 imports for recorded content when runtime validation is strong

### Playback Packaging Baseline

Initial playback baseline:

- HLS as the canonical public playback packaging
- fragmented MP4 or MPEG-TS depending on target ladder policy
- AAC-LC for widest playback compatibility
- WebVTT for normalized captions
- JPEG or WebP for poster and thumbnail derivatives

### Ladder Policy

The control plane should decide the allowed rendition ladder class, not the frontend.

Suggested baseline ladder classes:

- `mobile_low`
- `standard_sdr`
- `premium_sdr`
- `premium_hdr` when the source and runtime can support it safely

Each ladder class should define:

- maximum resolution
- maximum bitrate
- audio bitrate policy
- allowed frame-rate ceiling
- HDR eligibility
- packaging target

### Subtitle Policy

Subtitle normalization rules:

- normalize text-based subtitle streams to WebVTT
- preserve source language metadata where known
- explicitly mark default tracks
- reject silent subtitle corruption as a soft success

## Latency Classes

Lattice should intentionally support more than one latency class instead of pretending every workload is the same.

### Ultra Low Latency Live

Target use:

- creator livestreams
- multiplayer collaboration sessions
- live audience chat coupling

Properties:

- smallest safe segment or chunk strategy
- aggressive session heartbeat windows
- faster stale-session reconciliation
- stronger emphasis on transport stability over perfect compression efficiency

### Standard Live

Target use:

- less interaction-sensitive live broadcasts
- mirrored simulcast outputs

Properties:

- slightly larger buffering budget
- simpler packaging policy
- easier CDN behavior

### VOD Playback

Target use:

- films
- episodes
- archived live content

Properties:

- broader ladder
- stronger quality optimization
- less aggressive latency tradeoffs

These latency classes must be explicit in runtime configuration and surfaced in control-plane state where relevant.

## Collaboration Routing Contract

The collaboration runtime must treat session topology as an authority-driven graph.

Each participant should have explicit booleans or equivalent authority fields for:

- attached to session
- visible on host output
- mirrored to guest channel
- allowed in live chat
- allowed to keep local archive or recording
- currently contributing healthy media

The runtime should not infer these from UI state.

### Required Collaboration Paths

The runtime must support these routing paths:

- guest contribution to host program
- guest contribution to mirrored guest program
- host program to host audience playback
- mirrored guest program to guest audience playback
- isolated backstage contribution without public fanout

### Audio Principles

Audio routing must assume:

- mix-minus or equivalent participant-safe monitoring
- no feedback loops across mirrored sessions
- deterministic mute and removal behavior
- explicit authority for who may be heard on which output

## Reconciliation Contract

Every long-lived authority object should have a reconciler with a narrow, explicit responsibility.

Required reconciler classes:

- live ingest stale-session reconciler
- collaboration invite expiry reconciler
- collaboration mirror grant expiry reconciler
- playback session invalidation reconciler
- notification delivery reconciler
- media processing stale-job reconciler
- scheduled release reconciler
- moderation expiry reconciler
- entitlement expiry reconciler

Reconciliation rules:

- reconcilers operate on persisted facts, not frontend hints
- reconcilers must be idempotent
- reconcilers must record operator-meaningful reasons
- reconcilers must never invent product authority outside documented transitions

## WebSocket And Realtime Contract

Realtime transport exists to deliver timely state, not to replace authoritative writes.

Rules:

- websocket commands must map to explicit backend authority mutations
- commands must reject impossible transitions with typed reasons
- every accepted realtime mutation should produce a persisted state change or a persisted event
- reconnect must rebuild state from persisted authority, not from socket-local memory
- session presence should be treated as ephemeral evidence, never sole authority

## Operator Observability And SLO Signals

The system should expose enough internal signals that operators can tell whether the platform is healthy before users report failure.

Minimum signals:

- pending media jobs
- stale media jobs
- active live ingest sessions
- stale live ingest sessions
- active collaboration sessions
- disconnected collaboration sockets
- mirrored grant issuance versus revocation counts
- playback session issuance rate
- playback session invalidation rate
- notification dead-letter counts

Operator-facing repair surfaces should exist for:

- replaying or retrying a failed media job
- reconciling a specific playback session
- reconciling a specific collaboration session
- forcing live ingest termination
- revoking mirrored co-stream permissions

## Failure Domains

Lattice should treat these as separate failure domains:

- authority database failure
- ingest transport failure
- packaging runtime failure
- storage write failure
- websocket presence drift
- collaboration routing degradation
- playback manifest authorization failure

Each failure domain should have:

- a direct detection signal
- a reconciliation or retry path
- a terminal escalation rule
- an operator-visible audit trail

## Persistence Strategy

SQLite remains acceptable for local development and compact production authority slices where:

- write volume is moderate
- state is relational
- transactional correctness matters

SQLite should own:

- users
- creators
- uploads
- media assets
- playback sessions
- collaborations
- invitations
- participants
- entitlements
- moderation metadata

Current schema already materially reflects this direction with:

- `media_assets`
- `media_asset_variants`
- `media_processing_runs`
- reliability metadata on `upload_jobs`

That means the next work is runtime completion and stricter state handling, not a fresh schema invention.

Hot event streams and larger-scale runtime coordination may later split out, but only when real operational evidence demands it.

## API Principles

Every frontend action should resolve through one clean backend API.

The API must:

- return canonical objects
- reject unsupported state transitions
- expose explicit statuses
- avoid frontend reconstruction of authority
- use websocket or realtime signaling only where push semantics are truly needed

## Failure Model

Every stateful subsystem must define:

- normal transition
- retry transition
- timeout transition
- operator repair transition
- irreversible terminal transition

For collaborations specifically:

- invite expiry must be explicit
- duplicate acceptance must be idempotent
- mirror intent must survive reconnect
- host removal must revoke participant live state
- session end must deterministically close all non-terminal participant states

## Security and Trust

Trust boundaries:

- only authenticated sessions mutate creator state
- only authorized creators start or end broadcasts
- only invited users accept collaboration invites
- only valid entitlements unlock playback
- only role-authorized participants escalate collaboration state

Operator expectations:

- all high-risk actions should be auditable
- all commercial decisions should be reproducible
- moderation and collaboration removals should be attributable

## Milestone Program

### Milestone A: Finished Control Plane Spine

- collaboration session schema
- invite lifecycle
- participant lifecycle
- mirrored co-stream intent
- session snapshots
- admin-safe state transitions

### Milestone B: Media-Plane Integration

- guest ingress attachment
- host composite rules
- mirrored output routing
- recording topology

### Milestone C: Elite Playback and Encoding

- improved ladder policy
- low-latency playback options
- stronger validation and repair

### Milestone D: Trust and Operator Hardening

- moderation graph
- audit logs
- repair tooling
- incident controls

## Current Assessment

As of Tuesday, August 18, 2026:

- VOD and playback authority are materially real.
- Single-host live control plane is materially real.
- Monetization and entitlement authority are materially real.
- Collaboration control plane is materially real but still needs deeper media-runtime execution.
- The largest remaining production gap is the media/runtime layer: richer ingest, better packaging policy, collaboration routing, and stronger operator recovery.

That is why the next engineering pass must focus on finishing the media-runtime spine against the already-real control plane instead of scattering effort across smaller surfaces.

## Naming

Program name:

`Lifestream Lattice`

Reason:

- it suggests a graph, not a linear pipeline
- it fits collaborative streaming
- it fits control-plane authority
- it scales from single stream to many-channel mirrored sessions

## Honest Thesis

Lifestream becomes a real category-defining streaming platform only if collaboration is treated as a first-class authority model, not a bolt-on feature.

The control plane must therefore evolve from:

- creator starts stream

to:

- creator starts a live session graph with explicit participants, outputs, permissions, and commercial rules

That is the job of Lattice.
