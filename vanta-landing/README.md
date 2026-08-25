# Vanta Landing

Marketing landing site for `landing.streamvanta.tv`.

The app owns the public Vanta marketing homepage plus hidden funnel pages for creators and buyers:

- `/` explains Vanta as HBO plus Twitch for creator-native media.
- `/creators` is the creator acquisition funnel.
- `/buyers` is the advertiser and agency acquisition funnel.

## Structure

- `src/app` owns page composition and layout.
- `src/content` owns editable marketing copy, FAQs, metrics, and form fields.
- `src/components` owns reusable page components.
- `src/lib` owns browser-to-backend contracts.
- `src/styles` owns global design tokens.
- `public/media` contains static visual assets used by the landing pages.

## Run

```bash
npm install
npm run dev
```

Set `VITE_VANTA_API_BASE_URL` when the landing origin should submit to a separate API origin.

## Build

```bash
npm run build
```

## Backend Contract

Signup forms submit to:

```text
POST /api/v1/landing/signups
```

The production backend stores submissions in `landing_signups` via the SQLite migration
`0057_landing_signups.sql` and Postgres migration `0062_landing_signups.sql`.
