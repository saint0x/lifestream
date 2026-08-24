# vanta

Vanta is a streaming platform that combines creator-led live broadcasting with premium on-demand entertainment in one product.

## Features

- creator channels with standard HLS live streaming, audience chat, presence, and realtime activity
- creator-hosted collaborative live sessions with guest/member collaboration UX deferred from launch
- premium viewing across films, series, episodes, specials, clips, and creator video libraries
- publishing flows for livestreams, archived replays, trailers, long-form uploads, and episodic releases
- channel-first storytelling that supports both premium entertainment and always-on creator programming
- moderation, creator controls, delivery health, and platform-wide operational visibility

## Experience

- viewers can move between live shows and on-demand entertainment without leaving the platform
- creators can run their own channels, host guests, co-stream with other creators, and publish premium-form content
- live broadcasts, archives, shows, films, and channel media all live inside one connected product surface

## Production Baseline

- Railway for the API/runtime container
- Postgres for production persistence
- Cloudflare R2 plus public/custom-domain CDN delivery for production media
- Vercel for the frontend and custom domain routing
- SQLite plus local filesystem storage for local development only
- `/live` as the canonical live discovery surface

See [docs/production-decisions.md](docs/production-decisions.md) for the launch decisions that should not be reopened during deployment work.
