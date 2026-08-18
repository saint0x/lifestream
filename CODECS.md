# Lifestream Lattice

## Unfinished Media Runtime Work

### 1. Collaboration Routing Runtime

Still unfinished:

- media-engine execution of host-plus-guest routing graphs
- media-engine execution of mirrored co-stream fanout
- runtime audio graph execution with enforced mix-minus or equivalent echo prevention

Must close:

### 2. Low-Latency Delivery Evolution

Closed in backend control/runtime:

- blocking reload behavior for low-latency playlists

### 3. Media Quality Improvements

Closed in backend control/runtime:

- per-title encoding heuristics
- device-aware ladder policy

## Current Backend Closure Order

1. Execute the collaboration routing graph in the media engine.
