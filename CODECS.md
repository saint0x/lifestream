# Lifestream Lattice

## Unfinished Media Runtime Work

### 1. Collaboration Routing Runtime

Still unfinished:

- actual host-plus-guest media routing
- mirrored co-stream media fanout
- output wiring for host-only, mirrored guest, and multi-mirror sessions
- audio-routing protections such as mix-minus or equivalent echo prevention

Must close:

### 2. Low-Latency Delivery Evolution

Closed in backend control/runtime:

- blocking reload behavior for low-latency playlists

### 3. Media Quality Improvements

Closed in backend control/runtime:

- per-title encoding heuristics
- device-aware ladder policy

## Current Backend Closure Order

1. Add collaboration routing runtime with mirrored output wiring.
