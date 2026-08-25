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

Signup submissions go directly to the production Vanta API. `VITE_VANTA_API_BASE_URL`
may be set for controlled production API migrations, but the app has no local or
mock signup backend.

## Build

```bash
npm run build
```

## Backend Contract

Signup forms submit directly to the production backend:

```text
POST https://api-production-4becb.up.railway.app/api/v1/landing/signups
```

The production backend stores submissions in `landing_signups`.
