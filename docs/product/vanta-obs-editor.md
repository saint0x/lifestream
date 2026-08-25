# Vanta OBS Editor

## Purpose

Vanta OBS Editor is the free live-streaming and recording studio for creators on the Twitch-style side of Vanta.

It supports streamers, live hosts, guests, producers, and creators running real-time programming. The product replaces simple webcam streaming with a full OBS-style control room built around Vanta's own platform needs.

This tool is not paid.

It exists to help creators produce better live shows, capture cleaner recordings, fulfill live sponsor inventory, and turn streams into durable Vanta media.

## Product Role

Vanta OBS Editor gives live creators a professional broadcast surface:

- scenes;
- sources;
- program and preview canvases;
- studio mode;
- camera sources;
- microphone sources;
- screen sources;
- guest feeds;
- source inspector;
- audio mixer;
- stream health;
- sponsor cues;
- replay buffer;
- recording controls;
- runtime events;
- post-show packages.

It supports the live side of the platform in the same way Vanta Video Editor supports the episodic director side.

```text
Vanta Video Editor = directors and episodic series
Vanta OBS Editor = streamers and live programming
```

## Current Surface

Current implementation lives in:

- `vanta-obs/`
- `vanta-obs/frontend/src/App.tsx`
- `vanta-obs/frontend/src/App.css`
- `vanta-obs/frontend/src/types.ts`
- `vanta-obs/backend/src/obs`
- `vanta-obs/backend/tests`

The current app already expresses the intended product shape:

- live studio top bar;
- go-live control;
- end-stream control;
- recording control;
- replay save action;
- scene list;
- source list;
- program canvas;
- preview canvas;
- transition panel;
- audio mixer;
- source inspector;
- health panel;
- sponsor cue panel;
- runtime panel.

Some implementation is still in progress, but the product should be understood as Vanta's full live studio surface.

## Business Importance

Live programming creates a different kind of heat than episodic VOD.

A strong live show can generate:

- recurring audience behavior;
- chat activity;
- live sponsor moments;
- replay clips;
- stream archives;
- guest collaborations;
- event-style attention;
- higher audience urgency.

Vanta OBS Editor helps creators produce that attention at a higher quality level.

That matters because live attention can become sellable inventory:

- sponsored live reads;
- branded overlays;
- live product demos;
- sponsor cards;
- pinned CTAs;
- post-show proof clips;
- archive sponsorships.

## Creator-Benefit Lens

The creator should feel like Vanta gives them a real live control room without asking them to assemble five separate tools.

They should be able to run a polished show, record it, save important moments, fulfill sponsor cues, and hand the archive into the broader Vanta media system.

The product exists to make streamers more capable, not to charge them for basic live production.

## What Good Looks Like

Vanta OBS Editor succeeds when a creator can:

- prepare a live show;
- switch scenes confidently;
- monitor audio and stream health;
- bring on guests;
- record the broadcast;
- capture replay moments;
- execute sponsor obligations;
- end with a usable archive;
- push that archive or clips into the rest of Vanta.

The OBS Editor is creator infrastructure for the live side of the platform. Better live infrastructure creates better programming, better audience attention, and more valuable inventory for Vanta to monetize.
