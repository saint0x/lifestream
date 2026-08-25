# Vanta Editor Engineering Design

## Purpose

This document defines the engineering design for Vanta's standalone video editor platform.

The editor should live inside:

```text
vanta-editor/
```

It should be a separate internal app, but it should reuse Vanta's frontend design system and component language end to end. The visual baseline should come from the existing Vanta player and watch page: dark premium chrome, focused video surface, restrained controls, monospaced technical accents, sharp information hierarchy, and a production-grade media feel.

The editor is not only a tool for trimming videos.

It is the production workspace where creators, Vanta operators, and eventually approved collaborators can turn raw film, existing episodes, sponsor requirements, advertising inventory, review feedback, and final publishing requirements into finished Vanta-ready media.

The business model sits directly underneath the product:

```text
Creators make high-quality programming
-> creators and Vanta place, produce, and approve ad inventory
-> Vanta packages that qualified attention for advertisers
-> finished media goes live with defensible inventory and measurement
-> creators get paid
-> Vanta captures the marketplace spread / platform fee
```

The editor exists to make that chain operational.

If Vanta is the engine that converts creator heat into money, Vanta Editor is one of the machines that helps shape the heat into sellable inventory.

## Current Implementation Baseline

### Existing Vanta Player UI

Current frontend:

- `frontend/src/pages/WatchPage.tsx`
- `frontend/src/pages/WatchPage.css`
- `frontend/src/components/player/VideoPlayer.tsx`
- `frontend/src/components/player/VideoPlayer.css`
- `frontend/src/styles/global.css`
- `frontend/src/components/ui/Button.tsx`
- `frontend/src/components/ui/Input.tsx`
- `frontend/src/components/ui/Badge.tsx`

Important design traits:

- black video-first surface;
- rounded but restrained player shell;
- subtle borders and elevated surfaces;
- player controls that appear as precise overlay chrome;
- compact typography;
- monospaced metadata;
- minimal visual noise around playback;
- lucide icons for player actions;
- production links and SEO references use `https://streamvanta.tv`.

The editor should reuse this design system instead of inventing a separate visual language.

### Existing Vanta Media Pipeline

Current backend media foundation includes:

- `media_assets`
- `upload_jobs`
- `uploads`
- media asset variants
- playback manifests
- playback grants
- source, poster, and playback relative paths
- asset status transitions such as uploaded, processing, ready, published, failed, and scheduled

Relevant implementation areas:

- `backend/migrations/0007_media_pipeline.sql`
- `backend/migrations/0030_media_timeline_previews.sql`
- `backend/src/api/media/pipeline`
- `backend/src/api/media/jobs`
- `backend/src/api/media/access`
- `backend/src/api/playback`
- `backend/src/api/playauth`
- `backend/src/api/uploads`

The editor should not bypass the existing media pipeline.

It should extend it.

Raw files, rendered versions, proxy media, waveform data, thumbnails, captions, transcript segments, and final exports should all resolve back to durable Vanta media assets.

### Existing Advertising Marketplace

Current ad marketplace entities:

```text
ad_marketplace_advertisers
ad_marketplace_inventory_packages
ad_marketplace_campaigns
ad_marketplace_offers
ad_marketplace_submissions
ad_marketplace_payment_providers
```

The editor must eventually connect to these objects.

Creators and Vanta operators do not merely need to insert generic ads. They need to produce the specific advertising inventory that was sold:

- episode sponsorships;
- season sponsorships;
- pre-roll, mid-roll, and post-roll inventory;
- host-read sponsorships;
- integrated product placements;
- sponsor cards;
- lower thirds;
- branded segments;
- custom advertiser deliverables;
- revision-bound campaign assets;
- final proofs for advertiser review.

The editor is therefore part of the ad marketplace execution path, not just the creator upload path.

### Roxcodes Reference

The `~/Desktop/roxcodes` project is a useful inspiration point.

Relevant surfaces:

- `frontend/src/pages/ads-page.tsx`
- `frontend/src/components/video/video-player.tsx`
- `frontend/src/components/video/timeline.tsx`
- `frontend/src/components/video/viewport-scrubber.tsx`
- `frontend/src/hooks/use-timeline-composition.ts`
- `frontend/src/hooks/use-marker-actions.ts`
- `frontend/src/components/markers/marker-list.tsx`
- `frontend/src/components/transcript/transcript-pane.tsx`
- `frontend/src/components/modals/episode-upload-modal.tsx`
- `frontend/src/components/modals/ad-upload-modal.tsx`
- `frontend/src/components/modals/static-ad-select-modal.tsx`
- `frontend/src/components/modals/ab-test-select-modal.tsx`
- `frontend/src/components/jobs/job-status-bar.tsx`
- `backend/src/services/playback-service.ts`
- `backend/src/services/processing-service.ts`
- `backend/src/services/uploads-service.ts`

Roxcodes already demonstrates:

- timeline markers;
- draggable ad insertion segments;
- marker undo/redo;
- transcript-aligned navigation;
- automatic marker suggestions;
- static ad selection;
- A/B ad variants;
- upload handling;
- processing job status;
- playback manifest construction;
- render plan generation.

Vanta Editor should borrow the useful product patterns but not copy the visual system directly. Roxcodes is lighter and more prototype-like. Vanta Editor should feel like the production Vanta player expanded into a full editing bay.

## Product Goal

Vanta Editor should let the team and creators produce final, platform-ready media without leaving Vanta's operating environment.

The editor should support two primary workflows:

### 1. Creative Editing

Creators can import raw film, arrange clips, trim footage, cut episodes, add captions, add graphics, manage versions, review transcripts, create thumbnails, export final cuts, and publish to Vanta.

This helps creators make stronger work.

The creator-benefit frame matters: the editor should reduce the operational gap between an internet creator and a serious studio. It should give creators a more professional production workflow without requiring them to stitch together separate editing, storage, review, ad, and publishing tools.

### 2. Advertising Inventory Production

Creators and Vanta operators can turn sold inventory into actual deliverables.

That includes:

- placing ad breaks;
- selecting sponsor assets;
- producing host-read segments;
- inserting sponsor cards;
- aligning placements with contract requirements;
- generating preview cuts;
- routing revisions;
- locking advertiser-approved versions;
- exporting final masters;
- creating proof links;
- tying completed placements back to campaigns and offers.

This helps creators get paid because it makes advertiser value executable.

The editor should always preserve this chain:

```text
sold inventory
-> structured deliverable requirements
-> timeline placement
-> preview/review
-> approval
-> final export
-> published media
-> measurement/reporting
```

## User Roles

### Creator Owner

Can upload raw media, create projects, edit timelines, place sold ad inventory, invite collaborators, submit cuts for review, export versions, and publish approved media.

### Creator Collaborator

Can edit assigned projects, comment, upload assets, adjust timeline segments, manage transcripts, and prepare cuts depending on permission.

### Creator Viewer / Reviewer

Can open review links, watch cuts, add comments, and approve internally when permissioned.

### Vanta Operator

Can access creator projects tied to Vanta campaigns, inspect deliverables, add or lock ad inventory, coordinate advertiser feedback, approve final outputs, and publish or schedule Vanta-ready versions.

### Vanta Sales / Ad Ops

Can connect campaign requirements to editor projects, verify that sold inventory has been produced, create proof assets, and route final submissions to advertisers.

### Advertiser Reviewer

Eventually accesses only the submitted review room or proof link, not the full editor. They can comment, request revisions, and approve work through the external Ad Hub workflow.

## Core Editor Surfaces

### 1. Project Home

The project home should answer:

- what projects exist;
- which projects are tied to campaigns;
- what still needs editing;
- what is awaiting creator review;
- what is awaiting Vanta review;
- what is awaiting advertiser review;
- what has failed processing;
- what is ready to publish;
- what deadlines matter.

Primary modules:

- Active projects
- Draft cuts
- Campaign-linked projects
- Review queue
- Render jobs
- Recently edited
- Publish-ready exports
- Missing deliverables

### 2. Editor Workspace

The workspace is the main product surface.

Suggested layout:

```text
Top bar: project, version, save/render state, publish/review actions
Left rail: media bin, campaign requirements, effects, graphics, transcripts, comments
Center: Vanta-style video preview/player
Right inspector: selected clip, marker, asset, caption, ad spot, or comment
Bottom: multi-track timeline
```

The preview/player should visually feel like the existing Vanta player:

- same dark surface;
- same control density;
- same playback icon language;
- same time display style;
- same fullscreen behavior;
- same restrained border treatment;
- same subtitle/caption presentation where applicable.

Editor-specific controls should extend that surface:

- frame step backward / forward;
- mark in / out;
- split at playhead;
- ripple delete;
- add marker;
- add ad spot;
- toggle waveform;
- toggle safe areas;
- compare versions;
- open comments;
- render preview.

### 3. Media Bin

The media bin should contain every asset available to the project:

- raw video;
- audio;
- sponsor creative;
- campaign assets;
- graphics;
- thumbnails;
- stills;
- logos;
- captions;
- transcripts;
- music;
- previously exported versions;
- reusable creator clips.

Each asset should show:

- upload status;
- processing status;
- duration;
- resolution;
- file type;
- owner;
- source;
- rights status;
- campaign association when relevant;
- usage restrictions.

The editor should prevent assets with unclear rights, failed processing, or incompatible status from being used in final publishable exports without an explicit Vanta override.

### 4. Timeline

The timeline is the product core.

It should support:

- multiple video tracks;
- multiple audio tracks;
- ad marker tracks;
- caption/subtitle tracks;
- graphics/lower-third tracks;
- sponsor overlay tracks;
- comment markers;
- locked review markers;
- waveform display;
- frame-accurate playhead;
- snap-to-cut, snap-to-marker, and snap-to-transcript;
- zoom controls;
- undo/redo;
- drag-to-trim;
- split and join operations;
- marker creation and editing;
- render-safe validation.

Roxcodes' marker model is a good starting point:

```text
marker_type: auto | static | ab
position_seconds
duration_seconds
order_index
label
```

Vanta Editor should expand this into a broader timeline model:

```text
timeline_clips
timeline_tracks
timeline_markers
timeline_ad_slots
timeline_graphics
timeline_captions
timeline_comments
timeline_versions
```

Ad inventory should not be treated as generic annotations.

Ad slots should be typed, priced, reviewable, and traceable back to marketplace inventory.

### 5. Ad Inventory Track

The ad inventory track should be a first-class timeline layer.

Supported placement types:

- pre-roll;
- mid-roll;
- post-roll;
- host-read;
- integrated segment;
- sponsor card;
- lower-third sponsor;
- branded bumper;
- end-card;
- live-read conversion into VOD;
- dynamic insertion placeholder;
- static locked insertion;
- A/B variant insertion.

Each ad slot should show:

- advertiser;
- campaign;
- offer;
- package;
- placement type;
- duration;
- status;
- review requirement;
- linked creative;
- allowed replacement rules;
- measurement requirement;
- due date;
- approval state.

Ad slot states:

```text
draft
placed
needs_asset
needs_creator_recording
needs_vanta_review
submitted_to_advertiser
revision_requested
approved
locked
rendered
published
```

The editor should prevent final publish when a required ad slot is missing, invalid, too short, too long, out of flight, unapproved, or not aligned with the sold deliverable.

### 6. Campaign Requirements Panel

When a project is tied to an advertiser campaign, the editor should display the structured requirements directly inside the workspace.

The panel should show:

- campaign name;
- advertiser;
- objective;
- required placements;
- talking points;
- prohibited claims;
- required claims;
- brand assets;
- tracking links;
- promo codes;
- usage rights;
- review deadlines;
- revision limits;
- final due date;
- approval contacts;
- brand safety constraints.

The user should not need to cross-reference spreadsheets, Slack threads, or email to understand what must be produced.

Important rule:

> If a requirement matters contractually, financially, or operationally, it should appear as structured editor state.

### 7. Transcript Editing

Transcripts should be tightly connected to the timeline.

The transcript pane should support:

- click-to-seek;
- search;
- speaker labels;
- active segment highlighting;
- text-based rough cutting;
- caption export;
- finding good ad break candidates;
- finding sponsor-relevant moments;
- identifying claims that may require compliance review;
- generating clips from transcript ranges.

Roxcodes' transcript pane is a useful baseline for active transcript navigation.

Vanta Editor should go further by making transcript segments operational inputs for editing, ad placement, caption generation, and review comments.

### 8. Review And Comments

Editing without review becomes chaos quickly.

The editor should support:

- timestamped comments;
- frame-linked comments;
- timeline-range comments;
- clip-level comments;
- asset-level comments;
- private creator-team notes;
- Vanta-visible notes;
- advertiser-visible notes;
- resolved/unresolved states;
- version history;
- reviewer identity;
- final approval records.

Comment visibility:

```text
creator_team
vanta_internal
advertiser_visible
```

Advertiser comments should enter through the external Ad Hub review room, then appear in the editor as structured revision work.

### 9. Versioning

Every meaningful output should be versioned.

Version types:

- working draft;
- internal review cut;
- Vanta review cut;
- advertiser review cut;
- revised advertiser cut;
- final master;
- published version;
- clipped derivative;
- proof asset.

Version records should preserve:

- source timeline revision;
- render settings;
- media asset ids;
- author;
- created timestamp;
- approval state;
- comments snapshot;
- campaign deliverable mapping;
- published upload id when applicable.

Creators should be able to compare versions without losing confidence about which file is final.

### 10. Render And Export

Render jobs should be explicit, observable, and reproducible.

Supported outputs:

- preview proxy;
- advertiser review cut;
- final Vanta master;
- HLS playback package;
- MP4 download when permissioned;
- caption files;
- thumbnail images;
- short clips;
- proof clips;
- ad-only proof segments;
- campaign proof package.

Render job states:

```text
queued
running
waiting_for_asset
waiting_for_approval
completed
failed
cancelled
superseded
```

Render plans should be stored.

At minimum:

```text
target
source_assets
timeline_revision_id
ffmpeg_filtergraph_or_equivalent
hls_variants
caption_outputs
thumbnail_outputs
ad_slot_outputs
validation_warnings
created_at
created_by
```

The system should be able to answer:

> Which exact timeline, assets, approvals, and render settings produced this published episode?

## Backend Design

### New Editor Entities

Add editor-specific tables rather than overloading `uploads`.

Candidate tables:

```text
editor_projects
editor_project_members
editor_media_assets
editor_timelines
editor_timeline_versions
editor_tracks
editor_clips
editor_markers
editor_ad_slots
editor_campaign_requirements
editor_transcript_segments
editor_comments
editor_review_requests
editor_render_jobs
editor_exports
editor_publish_links
```

The existing `media_assets` table should remain the durable asset layer.

Editor records should reference media assets rather than copying source media state.

### Project Model

Suggested shape:

```text
editor_projects
- id
- creator_id
- owner_user_id
- title
- description
- source_kind: upload | series_episode | film | live_archive | imported_raw | campaign_work
- source_upload_id nullable
- source_content_id nullable
- series_id nullable
- campaign_id nullable
- offer_id nullable
- status
- active_timeline_id nullable
- created_at
- updated_at
```

Project statuses:

```text
draft
editing
internal_review
vanta_review
advertiser_review
revision_requested
approved
rendering
ready_to_publish
published
archived
```

### Timeline Model

The timeline should be edit-decision-list oriented.

Do not destructively modify source media.

Suggested shape:

```text
editor_timelines
- id
- project_id
- name
- duration_seconds
- frame_rate
- resolution_width
- resolution_height
- sample_rate
- status
- created_at
- updated_at

editor_timeline_versions
- id
- timeline_id
- version_number
- parent_version_id nullable
- change_summary
- edl_json
- created_by_user_id
- created_at
```

`edl_json` can store the complete ordered composition snapshot for reproducible renders, while normalized clip/ad/comment tables support querying and UI operations.

### Tracks And Clips

Suggested shape:

```text
editor_tracks
- id
- timeline_id
- kind: video | audio | ad | caption | graphics | comments
- name
- order_index
- locked
- muted
- visible
- created_at
- updated_at

editor_clips
- id
- timeline_id
- track_id
- media_asset_id
- source_in_seconds
- source_out_seconds
- timeline_in_seconds
- timeline_out_seconds
- speed
- volume
- opacity
- transform_json
- metadata_json
- created_at
- updated_at
```

### Ad Slots

Suggested shape:

```text
editor_ad_slots
- id
- project_id
- timeline_id
- track_id
- campaign_id nullable
- offer_id nullable
- package_id nullable
- advertiser_id nullable
- placement_type
- insertion_mode: dynamic | static | host_read | integrated | overlay
- timeline_in_seconds
- timeline_out_seconds
- required_duration_seconds nullable
- selected_media_asset_id nullable
- selected_ad_marketplace_submission_id nullable
- status
- review_status
- measurement_key
- requirements_json
- validation_json
- created_at
- updated_at
```

Ad slot validation should check:

- required campaign association;
- required asset association;
- required duration;
- no overlap with locked content unless allowed;
- placement inside permitted segment;
- host-read or integrated segment proof;
- required review status;
- marketplace offer status;
- rights and usage terms;
- final render inclusion.

### Review Requests

Suggested shape:

```text
editor_review_requests
- id
- project_id
- timeline_version_id
- export_id nullable
- review_kind: creator_internal | vanta_internal | advertiser
- campaign_id nullable
- offer_id nullable
- status
- due_at nullable
- submitted_by_user_id
- submitted_at
- resolved_at nullable
```

Advertiser review requests should create or connect to the external Ad Hub review room.

### Render Jobs And Exports

Suggested shape:

```text
editor_render_jobs
- id
- project_id
- timeline_id
- timeline_version_id
- export_kind
- status
- progress
- render_plan_json
- error_message nullable
- output_media_asset_id nullable
- created_by_user_id
- created_at
- updated_at

editor_exports
- id
- project_id
- timeline_version_id
- render_job_id
- export_kind
- media_asset_id
- duration_seconds
- checksum
- status
- created_at
```

Rendering should be async and resumable.

No editor request should block while a final render runs.

## API Design

Candidate routes:

```text
GET    /api/v1/editor/me/projects
POST   /api/v1/editor/me/projects
GET    /api/v1/editor/me/projects/:project_id
PATCH  /api/v1/editor/me/projects/:project_id

GET    /api/v1/editor/me/projects/:project_id/assets
POST   /api/v1/editor/me/projects/:project_id/assets
POST   /api/v1/editor/me/projects/:project_id/import-media-asset

GET    /api/v1/editor/me/projects/:project_id/timeline
PATCH  /api/v1/editor/me/projects/:project_id/timeline
POST   /api/v1/editor/me/projects/:project_id/timeline/versions

POST   /api/v1/editor/me/projects/:project_id/clips
PATCH  /api/v1/editor/me/clips/:clip_id
DELETE /api/v1/editor/me/clips/:clip_id

POST   /api/v1/editor/me/projects/:project_id/ad-slots
PATCH  /api/v1/editor/me/ad-slots/:ad_slot_id
DELETE /api/v1/editor/me/ad-slots/:ad_slot_id
POST   /api/v1/editor/me/ad-slots/:ad_slot_id/validate
POST   /api/v1/editor/me/ad-slots/:ad_slot_id/lock

GET    /api/v1/editor/me/projects/:project_id/campaign-requirements
POST   /api/v1/editor/me/projects/:project_id/review-requests
GET    /api/v1/editor/me/projects/:project_id/comments
POST   /api/v1/editor/me/projects/:project_id/comments
PATCH  /api/v1/editor/me/comments/:comment_id
POST   /api/v1/editor/me/comments/:comment_id/resolve

POST   /api/v1/editor/me/projects/:project_id/render-jobs
GET    /api/v1/editor/me/render-jobs/:render_job_id
POST   /api/v1/editor/me/render-jobs/:render_job_id/cancel

POST   /api/v1/editor/me/exports/:export_id/publish
POST   /api/v1/editor/me/exports/:export_id/submit-advertiser-review
```

Authorization rules:

- creators may only access projects they own or have explicit membership in;
- Vanta internal users may access projects required for operations, campaign delivery, review, or support;
- advertiser users may not access the full editor;
- advertiser review access should happen through external Ad Hub review rooms and proof links;
- publish mutations require creator ownership or explicit Vanta operator permission;
- ad slot locking requires Vanta ad-ops permission when tied to sold inventory.

## Frontend Design

### App Structure

`vanta-editor/` should be a standalone app with its own package, build, routing, and deployment configuration.

It should copy or share the existing Vanta design system:

- global tokens from `frontend/src/styles/global.css`;
- UI primitives from `frontend/src/components/ui`;
- player component patterns from `frontend/src/components/player`;
- layout density from Vanta dashboard pages;
- icons through `lucide-react`;
- API client conventions from the existing frontend where possible.

Suggested routes:

```text
/editor
/editor/projects
/editor/projects/new
/editor/projects/:projectId
/editor/projects/:projectId/review
/editor/projects/:projectId/exports
/editor/assets
/editor/jobs
/editor/settings
```

### Editor Shell

The shell should be dense and functional.

Avoid a landing page.

The first screen should be the actual project dashboard or most recent editor workspace.

Core shell regions:

- persistent project top bar;
- compact left rail;
- centered video preview;
- right inspector;
- bottom timeline;
- modal stack for upload, render, export, ad selection, and review submission.

### Component Set

Core components:

- `EditorShell`
- `ProjectDashboard`
- `ProjectTopBar`
- `EditorPlayer`
- `Timeline`
- `TimelineTrack`
- `TimelineClip`
- `TimelineAdSlot`
- `Playhead`
- `MediaBin`
- `CampaignRequirementsPanel`
- `AdSlotInspector`
- `ClipInspector`
- `TranscriptPane`
- `CommentsPanel`
- `VersionHistory`
- `RenderJobPanel`
- `ExportDialog`
- `PublishDialog`
- `ReviewRequestDialog`

Roxcodes concepts worth adapting:

- optimistic marker updates with server reconciliation;
- marker undo/redo;
- draggable insertion segments;
- deterministic preview selection for A/B variants;
- timeline waveform generation;
- transcript click-to-seek;
- job status bar;
- upload modal pattern;
- static ad and A/B selection modals.

Vanta-specific changes:

- use Vanta visual tokens, not Roxcodes styling;
- support full timeline clips, not only ad markers;
- treat campaign requirements as structured state;
- support versioned review cuts;
- enforce publish validation;
- connect exports to Vanta media assets and upload records.

### Interaction Requirements

Expected editor interactions:

- upload media;
- import existing Vanta upload;
- create timeline from source;
- play/pause;
- seek;
- frame step;
- zoom timeline;
- scrub with preview;
- split clip;
- trim clip;
- move clip;
- add ad slot;
- select ad creative;
- create host-read placeholder;
- add caption segment;
- search transcript;
- create clip from transcript;
- add comment at playhead;
- resolve comment;
- create version;
- render preview;
- submit review;
- export final;
- publish to Vanta.

Keyboard shortcuts should exist for core editing actions, but the UI should not depend on shortcuts being known.

### Validation UX

The editor should continuously tell the user whether the current timeline is publishable.

Validation examples:

- missing required ad asset;
- ad slot overlaps forbidden segment;
- sponsor card shorter than package minimum;
- host-read placeholder has no recorded proof;
- advertiser review is required before publish;
- render uses a superseded timeline version;
- source asset failed processing;
- transcript/caption generation still running;
- output resolution below requirement;
- required tracking link missing;
- campaign flight dates invalid.

Validation should be attached to specific timeline objects wherever possible.

The user should be able to click a warning and land directly on the broken clip, marker, ad slot, asset, or requirement.

## State Machines

### Project Status

```text
draft
editing
internal_review
vanta_review
advertiser_review
revision_requested
approved
rendering
ready_to_publish
published
archived
```

### Timeline Version Status

```text
working
submitted
reviewed
approved
superseded
published
```

### Ad Slot Status

```text
draft
placed
needs_asset
needs_creator_recording
needs_vanta_review
submitted_to_advertiser
revision_requested
approved
locked
rendered
published
```

### Render Job Status

```text
queued
running
waiting_for_asset
waiting_for_approval
completed
failed
cancelled
superseded
```

### Review Request Status

```text
draft
submitted
in_review
changes_requested
approved
rejected
expired
cancelled
```

## Integration With External Ad Hub

The editor and external Ad Hub should meet at the review and deliverable layer.

External Ad Hub is where advertisers:

- buy inventory;
- submit structured briefs;
- review submitted work;
- comment;
- approve;
- request revisions;
- view reporting.

Vanta Editor is where creators and Vanta operators:

- produce the work;
- place inventory;
- attach assets;
- render review cuts;
- respond to revisions;
- lock final deliverables;
- publish finished content.

The integration should look like:

```text
Campaign brief
-> editor campaign requirements
-> ad slots and deliverables
-> review export
-> external Ad Hub review room
-> advertiser comments / approvals
-> editor revision work
-> final export
-> campaign proof and reporting
```

Do not let advertiser feedback live only in email or chat.

The review loop must become structured product state.

## Integration With Vanta Publishing

Publishing from the editor should create or update the correct Vanta content object:

- episode;
- film;
- live archive;
- bonus clip;
- proof asset;
- campaign proof segment.

Publishing should require:

- successful final render;
- ready media asset;
- required title/metadata;
- required series/season/episode mapping where applicable;
- access policy;
- visibility;
- poster/thumbnail;
- captions when required;
- ad slot validation;
- rights validation;
- approval validation;
- publish schedule.

The editor should publish to Vanta's production domain:

```text
https://streamvanta.tv
```

All generated public links, proof links, preview links intended for external viewing, and final content links should use production `streamvanta.tv` links only.

## Implementation Phases

### Phase 1: Standalone App Shell

Build `vanta-editor/` as a standalone React/Vite app using the Vanta design system.

Deliver:

- project dashboard;
- editor shell;
- Vanta-style editor player;
- mock timeline;
- media bin;
- inspector panel;
- static sample project data;
- responsive desktop-first layout.

The goal is to establish the product surface.

### Phase 2: Real Media Asset Import

Connect to existing Vanta media assets and upload jobs.

Deliver:

- import existing uploads;
- upload raw media;
- processing status;
- playback proxy;
- thumbnail and duration metadata;
- project creation from media asset;
- media bin backed by API.

### Phase 3: Timeline Editing Core

Implement non-destructive edit decisions.

Deliver:

- timeline tracks;
- clip trim/move/split;
- playhead sync;
- undo/redo;
- version snapshots;
- transcript click-to-seek where transcript exists;
- preview render jobs.

### Phase 4: Ad Inventory Track

Make ad inventory first-class.

Deliver:

- ad slot track;
- pre/mid/post-roll placements;
- static ad selection;
- A/B variant selection;
- dynamic insertion placeholders;
- campaign requirement panel;
- ad slot validation;
- render plan includes ad slots.

### Phase 5: Review And Approval

Connect production work to review workflows.

Deliver:

- timestamped comments;
- versioned review cuts;
- creator internal review;
- Vanta review;
- advertiser review export;
- external Ad Hub review-room integration;
- revision states;
- approval audit trail.

### Phase 6: Final Render And Publish

Ship the finished media path.

Deliver:

- final render jobs;
- HLS outputs;
- MP4 master when permissioned;
- caption outputs;
- thumbnail outputs;
- publish validation;
- publish to episode/film/live archive;
- proof package generation;
- campaign deliverable completion.

## Testing Strategy

Use deterministic tests for timeline and render-plan logic.

Core test areas:

- timeline clip ordering;
- trim bounds;
- split behavior;
- overlapping ad slot rejection;
- ad slot duration validation;
- campaign requirement validation;
- timeline version snapshot reproducibility;
- render plan generation;
- review status transitions;
- publish gating;
- authorization boundaries.

System tests should cover:

- upload -> project -> edit -> preview render;
- campaign requirement -> ad slot -> advertiser review export;
- revision request -> updated timeline -> approved export;
- final render -> publish -> playback grant;
- failed render recovery;
- unauthorized advertiser attempting full editor access.

Fozzy should be preferred for deterministic scenario coverage when implementation begins.

## Open Questions

- Should Vanta Editor initially be internal-only for Vanta operators, or available to selected creators from the first release?
- Should the first release support full multi-track editing, or begin with single-source trimming plus ad inventory placement?
- Should final rendering run inside the existing backend media pipeline, a separate worker fleet, or an external render service?
- Which formats should be supported first: MP4 upload, HLS source import, live archive import, or all three?
- Should Vanta Editor own caption generation, or call the existing media pipeline once transcript/caption assets exist?
- How should collaborative editing conflicts be resolved in v1: optimistic last-write-wins, locked sections, or real-time collaboration?
- Which advertiser review states should be owned by the editor versus external Ad Hub?
- What ad placement types are required for the first paid campaigns?
- What is the minimum export quality required for Vanta publishing?

## Guiding Principle

Vanta Editor should make creators more powerful and Vanta more commercially precise.

Creators should feel that they have a real studio workstation inside Vanta.

Vanta should feel that sold inventory can reliably become approved, measurable, publishable media.

The product is successful when it helps creators make better work, place higher-value advertising inventory, reduce production chaos, and turn qualified attention into revenue with less manual coordination.
