# Vanta Video Editor

## Purpose

Vanta Video Editor is the free high-powered editing workspace for creators making premium episodic programming on Vanta.

It supports the HBO Max side of the platform: directors, producers, long-form creators, series builders, documentary creators, and anyone turning raw media into polished Vanta programming.

This tool is not a paid subscription product.

It exists to make creators better and to keep more of the production workflow inside Vanta.

## Product Role

The editor helps creators produce finished Vanta-ready media without needing to leave the platform for every part of the workflow.

It supports:

- raw media upload;
- media bin management;
- timeline editing;
- trimming;
- splitting;
- transcript review;
- comments;
- campaign requirement panels;
- ad slot validation;
- render jobs;
- proof links;
- advertiser review cuts;
- final publishing into Vanta.

The product gives creators infrastructure that normally requires a separate editing stack, review stack, file handoff, proof workflow, and publishing workflow.

## Current Surface

Current implementation lives in:

- `vanta-video-editor/`
- `vanta-video-editor/frontend/src/EditorApp.tsx`
- `vanta-video-editor/frontend/src/components/editor/editor.css`
- `vanta-video-editor/frontend/src/components/player/VideoPlayer.tsx`
- `vanta-video-editor/frontend/src/lib/api.ts`
- `vanta-video-editor/backend/src`

The current app already expresses the intended shape:

- project rail;
- media panel;
- campaign panel;
- transcript panel;
- comments panel;
- render panel;
- Vanta-style preview player;
- inspector;
- timeline;
- render-safe validation;
- advertiser proof and review actions;
- publish actions.

Some implementation is still in progress, but product documentation should describe the intended product as a real Vanta surface.

## Business Importance

The editor supports the creator supply side of the business.

Vanta needs high-quality episodic series and premium long-form programming. Creators can make stronger work when the platform gives them a clean production environment, review workflow, and publishing path.

The editor therefore helps Vanta indirectly monetize:

```text
better creator tools
-> better programming
-> stronger audience attention
-> more valuable ad inventory
-> more creator revenue and Vanta marketplace revenue
```

It is free because its purpose is to support the media supply that powers the marketplace.

## Creator-Benefit Lens

The creator should experience Vanta Video Editor as leverage.

They should feel:

- I can make a better episode here.
- I can keep track of campaign requirements.
- I can produce advertiser proof without chaos.
- I can render and publish without leaving Vanta.
- I can turn my series into a more professional media property.

The editor should reduce friction between making the work, approving the work, monetizing the work, and publishing the work.

## What Good Looks Like

Vanta Video Editor succeeds when directors and episodic creators can produce stronger shows, handle sponsor/ad deliverables cleanly, create review cuts, generate proof, and publish final media to Vanta without stitching together a messy external workflow.

The editor is not the business by itself. It is creator infrastructure that makes the business better.
