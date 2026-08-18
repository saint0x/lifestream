# Lifestream Lattice

## Unfinished Media Runtime Work

### 1. Live Ingest Runtime

Still unfinished:

- real ingest termination instead of control-only connect, heartbeat, and disconnect authority
- explicit runtime support for RTMP and SRT contribution classes
- contribution-state reporting beyond simple session liveness
- source probe capture for live contribution streams
- runtime-to-control-plane reporting for packaging attach, degradation, archive finalize, and terminal failure

Must close:

- one authoritative runtime attachment per live ingest session
- deterministic runtime state transitions for `attached`, `healthy`, `degraded`, `stale`, `disconnected`, `packaging_active`, `packaging_degraded`, `archive_finalizing`, and `archive_complete`
- durable ingest-runtime facts in SQLite

### 2. Live Packaging And Archive Runtime

Still unfinished:

- live HLS packaging owned by runtime rather than only control-plane metadata
- archive capture and archive finalization lifecycle
- discontinuity-safe reconnect handling
- operator-visible runtime failure details for live output

Must close:

- runtime-owned live master manifest and variant output lifecycle
- archive output completion reflected back into authoritative backend state
- playback-ready state only after runtime confirms manifest availability

### 3. Collaboration Routing Runtime

Still unfinished:

- actual host-plus-guest media routing
- mirrored co-stream media fanout
- output wiring for host-only, mirrored guest, and multi-mirror sessions
- runtime handling for recording policy
- audio-routing protections such as mix-minus or equivalent echo prevention

Must close:

- deterministic runtime topology derived from collaboration authority
- explicit runtime attachment per participant contribution
- mirrored channel outputs created only from backend-authorized session state

### 4. Immutable Media Generations

Still unfinished:

- generation-scoped processed output layout for every media processing attempt
- publish-by-pointer semantics across regenerated playback assets
- generation-aware archive output layout

Must close:

- processed outputs under immutable generation-specific roots
- no in-place overwrite of published playback generations
- clean pointer advancement in control-plane media state

### 5. Runtime Persistence And Repair

Still unfinished:

- durable runtime status for live packaging and archive completion
- repair surfaces for partially-generated live output
- stronger operator reconciliation for runtime/output drift

Must close:

- SQLite-backed runtime facts for live output readiness and failure
- operator repair actions that reconcile runtime output state without manual DB edits
- durable audit trail for runtime failure and retry paths

### 6. Low-Latency Delivery Evolution

Still unfinished:

- LL-HLS packaging path
- CMAF or fMP4 live segment strategy
- partial segment support
- blocking reload behavior for low-latency playlists

Must close:

- baseline standard HLS stays stable
- LL-HLS path added as an explicit runtime class, not ad hoc behavior

### 7. Media Quality Improvements

Still unfinished:

- per-title encoding heuristics
- content-class-aware ladder planning
- stronger source validation and repair decisions
- device-aware ladder policy

Must close:

- runtime chooses ladder policy from authoritative media characteristics
- repairable validation failures stay explicit and operator-visible

## Current Backend Closure Order

1. Finish immutable media generations and pointer-safe processed output layout.
2. Persist live runtime output state for packaging and archive lifecycle.
3. Add real live packaging and archive runtime reporting.
4. Add collaboration routing runtime with mirrored output wiring.
5. Add LL-HLS and lower-latency delivery options after the standard live runtime is durable.
