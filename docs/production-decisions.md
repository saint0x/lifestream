# Production Decisions

This is the launch baseline for the current production code path.

## Platform

- Production runs on Railway for the API/runtime container.
- Production persistence is Railway Postgres, sized conservatively until real platform load justifies larger capacity.
- Production media storage is Cloudflare R2 with public/custom-domain CDN delivery in front of cacheable playback artifacts.
- Local development keeps SQLite and local filesystem storage behind provider boundaries.
- There is no long-term product requirement to keep SQLite and Postgres behavior identical beyond local development and migration support.

## Delivery

- Live delivery launches as standard HLS first. Low-latency HLS is deferred until audience size and creator expectations justify the extra operational complexity.
- VOD encoding defaults to H.264 video, AAC stereo audio at 48 kHz, 6 second HLS segments, and an adaptive ladder capped at the source resolution.
- Playback media should be CDN-cacheable. Session-specific authorization belongs at manifest/session/cookie boundaries, not on every segment URL in production object storage mode.

## Product Surface

- Public live playback is allowed at launch.
- Subscribe, chat emotes/settings, and guest/member collaboration UX are not launch UI commitments.
- Admin APIs, entitlement reconciliation, and upload-job ingest controls remain operational/internal surfaces for launch.
- Creator-side live and host collaboration controls can stay in the studio where already implemented.
- `/live` is the canonical live discovery route. `/browse` is not a separate launch surface.
