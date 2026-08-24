# External Ad Hub Engineering Design

## Purpose

This document defines the engineering design for Vanta's external advertiser portal.

The existing Ad Hub is creator-facing. It lets creators review sponsorship offers, accept or decline them, see package templates, and submit campaign proof for advertiser review.

The external Ad Hub is the other side of that marketplace.

It should let advertisers discover Vanta's creator inventory, understand pricing and Qualified Attention evidence, configure campaigns, submit briefs, manage approvals, review work, comment on submissions, view delivery, and renew or expand buys with as much self-service as possible.

This is not only a financial portal.

It is also the advertiser work-review portal.

Advertisers need custom logins, campaign workspaces, review links, threaded comments, asset/version history, approval states, and an audit trail for what was approved, rejected, or revised.

The business model sits in the middle:

```text
Creators bring exclusive programming and audience
-> Vanta packages and verifies the inventory
-> Advertisers buy access through the external Ad Hub
-> Creators get paid
-> Vanta captures the marketplace spread / platform fee
```

## Current Implementation Baseline

### Existing Creator-Facing UI

Current frontend:

- `frontend/src/pages/AdHubPage.tsx`
- `frontend/src/pages/AdHubPage.css`
- `frontend/src/lib/repository.ts`
- `frontend/src/types/index.ts`

The current page is a creator dashboard for:

- marketplace offer list;
- pending / active / in-review / approved / declined counts;
- gross offer amount;
- creator payout amount;
- advertiser details;
- campaign details;
- package details;
- offer requirements;
- accept / decline actions;
- review submission with URL and notes;
- available package templates.

### Existing Frontend API Methods

Current repository methods:

```ts
repository.fetchAdHub()
repository.acceptAdOffer(id)
repository.declineAdOffer(id)
repository.submitAdOfferReview(id, { submissionUrl, notes })
```

Current creator routes:

```text
GET  /api/v1/creator/me/ad-hub
POST /api/v1/creator/me/ad-offers/:offer_id/accept
POST /api/v1/creator/me/ad-offers/:offer_id/decline
POST /api/v1/creator/me/ad-offers/:offer_id/submissions
```

Current backend implementation:

- `backend/src/api/creator/business/marketplace.rs`
- `backend/src/api/creator/business/mod.rs`
- `backend/src/api/tests/ad_marketplace.rs`

### Existing Marketplace Tables

Current ad marketplace migrations:

- `backend/migrations/0048_ad_marketplace.sql`
- `backend/migrations/postgres/0052_ad_marketplace.sql`

Current tables:

```text
ad_marketplace_advertisers
ad_marketplace_inventory_packages
ad_marketplace_campaigns
ad_marketplace_offers
ad_marketplace_submissions
ad_marketplace_payment_providers
```

Current object model:

- advertiser owns campaigns;
- campaign owns offers;
- offer is assigned to a creator and package;
- offer contains gross amount, creator payout, platform fee, status, review status, due date, brief, and requirements;
- creator can accept, decline, or submit review proof;
- submissions attach URLs and notes to offers.

### Existing Attention Proof Layer

Current attention rollup tables:

- `backend/migrations/0044_creator_attention_rollups.sql`
- `backend/migrations/0045_creator_attention_measured_viewers.sql`
- `backend/migrations/0046_creator_attention_rollup_baseline.sql`

Current table:

```text
creator_attention_daily
```

Key columns:

```text
creator_id
day
algorithm_version
qualified_viewers
verified_viewer_score
creator_attention_value
baseline_value_per_qualified_viewer
average_watch_minutes
attention_multiplier
engagement_multiplier
retention_multiplier
audience_quality_multiplier
data_confidence_multiplier
qualified_viewer_rate
returning_viewer_rate
measured_sessions
measured_viewers
```

Current frontend type:

```ts
CreatorAttentionScore
```

Important product rule:

> Qualified Attention is not the advertiser outcome. It is the proof layer that makes claimed outcomes defensible.

Advertisers still buy normal outcomes such as reach, category ownership, sponsorship association, traffic, conversion, brand lift, consideration, or sales. Qualified Attention proves whether the purchased media generated meaningful human attention.

## Product Goal

The external Ad Hub should make Vanta self-service for advertisers without making it low-ticket.

High-ticket buyers should be able to inspect inventory, pricing, creator fit, audience quality, package terms, and reporting examples before speaking to a rep.

Sales reps should become closers, strategists, and deal shapers.

They should not be information gatekeepers.

The portal should also be programmatic at its core.

Advertisers must be guided into specific, structured choices instead of being allowed to describe a campaign through vague free-form text.

The UX should still feel fast and premium, but the data model should force clarity wherever ambiguity would create production, approval, pricing, or reporting risk.

Guiding rule:

> Use structured fields, menus, toggles, checkboxes, ranges, and controlled selections for everything the system needs to price, route, approve, measure, or enforce.

Free-text fields should exist only for context that cannot reasonably be captured structurally.

## User Roles

### Advertiser Admin

Can manage the advertiser account, billing, team members, campaigns, brand safety settings, work-review permissions, and purchase approvals.

### Advertiser Buyer

Can browse inventory, build plans, create briefs, request proposals, launch permitted campaigns, review creator work, comment, approve, and request revisions.

### Advertiser Analyst

Can view reporting, exports, Qualified Attention details, delivery status, and renewal recommendations.

### Advertiser Reviewer

Can access assigned campaign review rooms, inspect submitted work, comment, request revisions, and approve assets when permissioned.

### Vanta Sales / Ops

Can approve advertiser accounts, curate inventory, override pricing, assemble custom packages, manage deal status, review briefs, and coordinate creator offers.

### Creator

Already supported by the creator Ad Hub. Creators receive offers, accept or decline, submit proof, and respond to review requirements.

## Core Advertiser Portal Surfaces

### 1. Advertiser Home

The home dashboard should answer:

- what campaigns are active;
- what is awaiting advertiser review;
- what packages are available;
- what creators or categories match the advertiser;
- what spend is committed;
- what Qualified Attention has been delivered;
- what renewal or expansion opportunities exist.

Primary cards:

- Active campaigns
- Pending approvals
- Qualified Attention delivered
- Spend committed
- Forecasted delivery
- Renewal opportunities

### 2. Inventory Discovery

Advertisers should browse inventory by:

- creator;
- series;
- category;
- audience type;
- Qualified Viewers;
- Verified Viewer Score;
- average watch minutes;
- returning viewer rate;
- campaign objective;
- package type;
- price range;
- availability window;
- brand safety suitability;
- promotion commitment.

This is the self-service heart of the product.

The advertiser should be able to understand what exists without a sales call.

### 3. Creator / Series Inventory Detail

Each creator or series detail page should include:

- creator profile;
- series description;
- category and audience thesis;
- available placements;
- available campaign windows;
- base pricing;
- package options;
- historic Qualified Attention;
- Verified Viewer Score;
- average watch minutes;
- returning viewer rate;
- measured viewer confidence;
- creator promotion commitments;
- content examples;
- brand safety notes;
- excluded categories;
- sample report preview;
- recommended advertiser categories.

Key framing:

> The creator is the distribution channel. Vanta is the exclusive destination and measurement layer.

### 4. Package Builder

Advertisers should be able to build a campaign from structured options:

- objective;
- target category;
- creator or creator bundle;
- placement type;
- budget;
- flight dates;
- creative format;
- promotion requirements;
- brand safety exclusions;
- reporting requirements;
- optional performance tracking;
- optional category exclusivity.

The builder should estimate:

- base price;
- expected creator payout;
- Vanta platform fee;
- forecasted Qualified Viewers;
- forecasted average watch minutes;
- forecasted Verified Viewer Score range;
- forecasted delivery confidence;
- service level.

The output should be a campaign draft, request-for-proposal, or bookable package depending on inventory readiness and buyer permissions.

The package builder should prevent invalid combinations.

Examples:

- category exclusivity requires a defined category and campaign window;
- performance pricing requires a tracking method and attribution window;
- creator approval rights require review deadline selection;
- paid amplification usage rights require asset usage terms;
- high-risk brand categories require Vanta review;
- custom claims require legal/compliance notes;
- high-touch packages require sales approval;
- campaign launch cannot proceed while required fields are missing.

### 5. Campaign Brief

Advertisers need a structured brief form.

Fields:

- advertiser account;
- campaign name;
- primary objective;
- secondary objectives;
- product / offer being promoted;
- target customer;
- prohibited claims;
- required claims;
- creator talking points;
- do-not-say language;
- landing URL;
- tracking links;
- promo codes;
- creative assets;
- brand safety constraints;
- category exclusivity request;
- approval contacts;
- legal / compliance notes;
- billing contact.

The brief must be structured enough to generate creator offers and prevent vague campaign expectations.

Most brief fields should be controlled inputs.

Examples:

```text
primary_objective: awareness | consideration | traffic | conversion | sponsorship_association | category_ownership | launch
secondary_objectives: multi-select
target_customer: selected personas + optional notes
campaign_budget_range: selected range or exact budget
flight_window: date range
placement_types: multi-select
creator_categories: multi-select
required_creator_actions: checklist
brand_safety_exclusions: multi-select
approval_requirements: checklist
performance_tracking: none | links | codes | pixel | advertiser_side | third_party
attribution_window: 1d | 7d | 14d | 30d | custom_requires_sales_review
revision_rounds: 0 | 1 | 2 | custom_requires_sales_review
category_exclusivity: none | requested | required
usage_rights: none | organic_repost | paid_amplification | whitelisting | custom_requires_sales_review
```

Free text should be limited to:

- campaign context;
- product description;
- creator talking point notes;
- legal/compliance notes;
- special constraints requiring Vanta review.

Every free-text field should have a clear character limit and should be treated as explanatory context, not as the source of truth for deliverables.

If a requirement matters contractually, financially, or operationally, it needs a structured field.

### 6. Pricing And Availability

Advertisers should see real-time or near-real-time pricing before talking to sales.

Pricing should be transparent but not naive.

Inputs:

- package base price;
- creator strength;
- series strength;
- category demand;
- historical Qualified Attention;
- forecasted delivery;
- promotion commitments;
- placement depth;
- exclusivity premium;
- season / bundle duration;
- service level;
- discount rules;
- current inventory scarcity.

The portal should show:

- starting price;
- expected range;
- included deliverables;
- optional add-ons;
- when sales approval is required;
- why the price moves.

Important rule:

> Qualified Attention informs pricing, but it does not become the outcome itself.

### 7. Approvals Inbox

Advertisers need a direct counterpart to creator submissions.

When a creator submits proof or a rough cut, the advertiser sees:

- offer;
- creator;
- campaign;
- package;
- submission URL;
- notes;
- submitted timestamp;
- requested decision;
- approve button;
- request revision button;
- feedback field;
- revision due date.

Current creator submissions already exist in `ad_marketplace_submissions`, but advertiser-side review actions do not exist yet.

Needed states:

```text
review_pending
approved
revision_requested
rejected
expired
```

Offer-level `advertiser_review_status` should be updated from this advertiser workflow.

### 8. Campaign Review Room

Each campaign should have a dedicated review workspace.

This is where advertisers inspect work before approval.

The review room should include:

- campaign brief;
- creator offers;
- submitted cuts or proof links;
- asset versions;
- threaded comments;
- timestamped comments for video when possible;
- internal advertiser-only notes;
- Vanta/creator-visible feedback;
- required changes;
- approval status;
- revision history;
- reviewer identity;
- due dates;
- final approval record.

Review comments should be structured.

Comment visibility:

```text
internal_advertiser
vanta_only
creator_visible
```

This matters because brand teams may need to discuss privately before sending consolidated feedback to Vanta or the creator.

Asset versioning:

```text
v1 rough cut
-> advertiser comments
-> v2 revised cut
-> approval
-> final proof / live URL
```

The goal is to prevent advertiser review from leaking into email threads, text messages, and scattered documents.

### 9. Campaign Reporting

The reporting dashboard should answer:

> Did this advertiser buy real, qualified, creator-imported attention?

Report sections:

- campaign summary;
- spend;
- creator / series / package;
- placements delivered;
- creator promotion delivered;
- raw views;
- measured viewers;
- Qualified Viewers;
- qualified-viewer rate;
- Verified Viewer Score;
- average watch minutes;
- returning viewer rate;
- engagement signals;
- traffic attribution;
- content completion;
- creator promotion performance;
- delivery versus forecast;
- brand safety incidents;
- algorithm version;
- measurement limitations;
- renewal recommendation.

Qualified Attention should be visualized as evidence.

It should sit beside the advertiser's chosen outcome:

- brand campaign: qualified reach, attention depth, repeat exposure, recall proxies;
- performance campaign: clicks, landing traffic, code usage, conversion events when integrated;
- sponsorship campaign: category fit, creator association, repeat exposure, audience quality;
- category campaign: cross-creator reach, concentration, exclusivity, share of qualified attention.

### 10. Renewals And Expansion

The portal should recommend next buys:

- renew same creator;
- expand to season sponsorship;
- expand to category ownership;
- add creator bundle;
- retarget Qualified Viewers;
- run a launch package on a related creator;
- convert pilot into annual sponsorship.

Renewals should be based on:

- delivered Qualified Attention;
- advertiser objective;
- creator promotion quality;
- creator inventory availability;
- pricing changes;
- buyer category;
- service load;
- prior approval speed.

## Backend Design

### New Advertiser Auth And Account Model

Current `ad_marketplace_advertisers` is an entity table, not a full auth/account model.

Needed tables:

```text
advertiser_users
advertiser_memberships
advertiser_invites
advertiser_api_sessions
advertiser_billing_profiles
advertiser_review_permissions
```

Suggested shape:

```text
advertiser_users
- id
- email
- name
- status
- created_at
- updated_at

advertiser_memberships
- advertiser_id
- user_id
- role
- created_at
- updated_at

advertiser_billing_profiles
- advertiser_id
- billing_name
- billing_email
- payment_provider_key
- external_customer_id
- status
- created_at
- updated_at

advertiser_review_permissions
- advertiser_id
- user_id
- campaign_id
- can_comment
- can_request_revision
- can_approve
- created_at
- updated_at
```

Authorization rule:

> Advertiser users may only read and mutate data for advertiser accounts where they have membership.

Review authorization rule:

> Approval actions require explicit reviewer permission, not merely advertiser account membership.

### Inventory Catalog

The existing package table is not enough for self-service discovery.

Add inventory listing entities or projections:

```text
ad_marketplace_creator_inventory
ad_marketplace_series_inventory
ad_marketplace_category_inventory
ad_marketplace_inventory_availability
ad_marketplace_inventory_suitability
ad_marketplace_inventory_price_quotes
```

These can be materialized views or tables depending on freshness requirements.

Minimum v1 can compute from:

- `creator_profiles`;
- creator series/catalog tables;
- `ad_marketplace_inventory_packages`;
- `creator_attention_daily`;
- manual sales/ops configuration.

### Campaign Drafts And Briefs

Current `ad_marketplace_campaigns` contains budget/objective/status, but the external portal needs drafts and structured briefs.

Add:

```text
ad_marketplace_campaign_briefs
ad_marketplace_campaign_assets
ad_marketplace_campaign_tracking
ad_marketplace_campaign_approvals
ad_marketplace_review_threads
ad_marketplace_review_comments
ad_marketplace_asset_versions
```

Briefs should be versioned or immutable after launch-critical approval.

Brief validation should be schema-driven.

The backend should reject:

- unknown objective values;
- unknown placement types;
- missing campaign windows;
- missing budget;
- missing approval contact;
- performance campaigns without tracking method;
- conversion campaigns without attribution configuration;
- category exclusivity without category definition;
- custom usage rights without sales review;
- high-risk brand categories without Vanta review;
- free-text-only requirements that are not mapped to structured deliverables.

The API should return field-level validation errors so the frontend can guide the buyer directly to the missing choice.

Campaign assets should support:

- uploaded files;
- external URLs;
- brand guidelines;
- landing pages;
- promo codes;
- tracking URLs.

Review assets should support versioned work:

- rough cut URLs;
- final cut URLs;
- thumbnails;
- scripts;
- talking points;
- proof screenshots;
- live placement URLs;
- post-launch proof assets.

Comments should support:

- author;
- visibility;
- body;
- optional asset id;
- optional version id;
- optional timestamp seconds for video;
- optional resolved status;
- created_at;
- updated_at.

### Pricing Quotes

Pricing should be inspectable before booking.

Add:

```text
ad_marketplace_price_quotes
```

Fields:

```text
id
advertiser_id
campaign_id nullable
creator_id nullable
series_id nullable
category_code nullable
package_id
quoted_price_cents
creator_payout_cents
platform_fee_cents
currency
pricing_version
inputs_json
expires_at
created_at
created_by_user_id nullable
created_by_internal_user_id nullable
```

The quote should store `inputs_json` so future disputes can answer why a price existed at the time.

### Advertiser Review Actions

Current creator-side submission flow inserts `ad_marketplace_submissions` and sets offers to `in_review`.

Advertiser side needs:

```text
POST /api/v1/advertiser/me/submissions/:submission_id/approve
POST /api/v1/advertiser/me/submissions/:submission_id/request-revision
POST /api/v1/advertiser/me/submissions/:submission_id/reject
GET  /api/v1/advertiser/me/campaigns/:campaign_id/review-room
POST /api/v1/advertiser/me/review-threads
POST /api/v1/advertiser/me/review-comments
PATCH /api/v1/advertiser/me/review-comments/:comment_id
POST /api/v1/advertiser/me/review-comments/:comment_id/resolve
```

Review mutations should:

- verify advertiser membership;
- verify submission belongs to the advertiser through offer -> campaign -> advertiser;
- update submission status;
- update offer `advertiser_review_status`;
- store feedback;
- store reviewer id;
- store reviewed timestamp;
- optionally set revision due date.

Comment mutations should:

- verify advertiser review permission;
- preserve visibility boundaries;
- attach comments to the relevant campaign, offer, submission, asset, or version;
- preserve edit history where needed;
- emit creator-visible feedback only when visibility allows it;
- update unresolved comment counts;
- update review room activity timestamps.

### Reporting

Add campaign reporting endpoints that join campaign, offer, creator, content, promotion, and attention data.

Candidate endpoints:

```text
GET /api/v1/advertiser/me/dashboard
GET /api/v1/advertiser/me/inventory
GET /api/v1/advertiser/me/inventory/creators/:creator_id
GET /api/v1/advertiser/me/inventory/series/:series_id
POST /api/v1/advertiser/me/price-quotes
POST /api/v1/advertiser/me/campaign-drafts
PATCH /api/v1/advertiser/me/campaign-drafts/:campaign_id
POST /api/v1/advertiser/me/campaign-drafts/:campaign_id/submit
GET /api/v1/advertiser/me/campaigns
GET /api/v1/advertiser/me/campaigns/:campaign_id
GET /api/v1/advertiser/me/campaigns/:campaign_id/report
GET /api/v1/advertiser/me/approvals
GET /api/v1/advertiser/me/review-rooms/:campaign_id
```

Reporting should use raw measurement tables and `creator_attention_daily` as derived proof, following the existing Qualified Attention implementation rule.

Do not store arbitrary advertiser-facing score fixtures.

## Frontend Design

### Route Structure

Suggested routes:

```text
/advertiser
/advertiser/inventory
/advertiser/inventory/creators/:creatorId
/advertiser/inventory/series/:seriesId
/advertiser/campaigns
/advertiser/campaigns/new
/advertiser/campaigns/:campaignId
/advertiser/approvals
/advertiser/review/:campaignId
/advertiser/reports/:campaignId
/advertiser/billing
/advertiser/settings
```

### Shared Components With Creator Ad Hub

Reusable concepts:

- money formatting;
- compact date formatting;
- package cards;
- offer/campaign status badges;
- deliverables list;
- requirements list;
- submission list;
- campaign facts;
- attention metric cards.

Do not force advertiser UI into the creator dashboard layout.

Advertisers need discovery, comparison, pricing, and proof.

Advertisers also need structured work review.

Creators need offer decisioning and submission workflows.

### Advertiser Dashboard UI

Dashboard modules:

- active campaigns table;
- pending approvals queue;
- recommended inventory;
- spend and delivery cards;
- Qualified Attention delivered;
- upcoming renewals;
- campaign report previews;
- saved inventory;
- recent review activity;
- unresolved comments;
- submissions awaiting decision.

### Inventory UI

Inventory should feel like a premium media-buying interface:

- dense filters;
- sortable columns;
- creator/series cards;
- category tabs;
- price ranges;
- availability;
- attention metrics;
- brand safety notes;
- package actions.

Avoid marketing fluff.

The buyer is trying to decide:

> Is this creator/category worth budget?

### Campaign Builder UI

The campaign builder should be a structured workflow:

```text
Objective
-> Inventory
-> Package
-> Brief
-> Pricing
-> Review
-> Submit / Book
```

It should show the buyer how each choice changes price, forecast, deliverables, and required sales approval.

The builder should prefer:

- segmented controls for objectives;
- dropdowns for categories and placement types;
- checkboxes for deliverables and creator actions;
- toggles for optional add-ons;
- date pickers for flight windows and review deadlines;
- numeric inputs or sliders for budget;
- selectable cards for creators, series, and packages;
- constrained text areas only for context and notes.

The UI should make specificity feel easy rather than bureaucratic.

The buyer should feel like the product is helping them define a better campaign, not trapping them in a form.

### Reporting UI

Reports should look like evidence, not decoration.

Core panels:

- delivery summary;
- Qualified Attention proof;
- objective-specific outcome panel;
- creator promotion proof;
- placement log;
- measurement transparency;
- renewal recommendation.

The measurement transparency panel is mandatory for serious buyers.

It should include:

- algorithm version;
- qualified-view threshold;
- measured signal list;
- unmeasured signal list;
- first-party versus third-party measurement status;
- attribution window;
- invalid traffic handling summary.

### Work Review UI

The work review UI should feel closer to a production approval system than a finance dashboard.

Core panels:

- submitted asset preview;
- version history;
- comment thread;
- timestamped notes;
- visibility selector;
- approval actions;
- required revisions checklist;
- campaign brief reference;
- creator requirements reference;
- reviewer audit trail.

Primary actions:

- approve;
- request revision;
- reject;
- add comment;
- resolve comment;
- mark final.

The advertiser should not need to email Vanta to say:

> Move the logo earlier.

That feedback should live inside the campaign review room.

## State Machines

### Campaign Status

Current campaign statuses include `planning` and seeded `booking`.

Recommended expanded campaign state:

```text
draft
submitted
sales_review
creator_offer_pending
booking
live
reporting
completed
renewal_pending
cancelled
```

### Offer Status

Current offer statuses:

```text
pending
accepted
in_review
approved
declined
```

Recommended additions:

```text
revision_requested
cancelled
expired
paid
```

### Submission Status

Current submission status:

```text
review_pending
```

Recommended expanded status:

```text
review_pending
approved
revision_requested
rejected
superseded
```

### Review Comment Status

```text
open
resolved
superseded
hidden
```

## Pricing Model

External Ad Hub v1 pricing should be quote-based, not a fully automated exchange.

Pricing should show enough to support self-service discovery while preserving sales control for high-value or scarce inventory.

Recommended model:

```text
displayed starting price
-> generated quote
-> quote expiration
-> sales review if high-touch, exclusive, or custom
-> campaign booking
```

Quote factors:

- package base price;
- creator attention score;
- forecasted Qualified Viewers;
- Verified Viewer Score;
- historical average watch minutes;
- creator promotion commitments;
- campaign objective;
- placement depth;
- exclusivity premium;
- category scarcity;
- sales service level;
- billing/payment risk.

## Dashboard Content Driven By Sales Research

The portal should directly answer what recent market research says advertisers care about.

### Measurement And Standards

Show:

- Qualified Attention explanation;
- score components;
- algorithm version;
- measured versus unmeasured signals;
- first-party versus third-party verification status;
- campaign objective mapping.

Reason:

Advertisers are increasing creator spend, but measurement, standards, and business-outcome proof remain major pain points.

### Creator Selection

Show:

- audience fit;
- category;
- creator reputation notes;
- content quality;
- audience devotion proxies;
- promotion intensity;
- historic delivery;
- recommended advertiser categories.

Reason:

Choosing the right creator is one of the biggest real-world buyer challenges.

### Brand Safety And Suitability

Show:

- safety notes;
- excluded sponsor categories;
- sensitive content flags;
- approval rights;
- takedown/replacement policy;
- suitability fit by advertiser category.

Reason:

Brand safety and suitability are major enterprise buyer concerns, especially in creator media.

Do not claim active GARM compliance.

Use Vanta's own standards-inspired safety framework.

### Inventory Transparency

Show:

- exact creator;
- exact series;
- exact placements;
- content adjacency;
- campaign window;
- promotion commitments;
- deliverables;
- reporting window.

Reason:

Video and creator buyers increasingly distrust vague inventory.

### Business Outcome Alignment

Show:

- primary objective;
- success metric;
- measurement method;
- attribution method if performance-led;
- renewal recommendation.

Reason:

Qualified Attention is the proof layer. The buyer's objective defines the outcome.

## MVP Scope

### Build First

1. Advertiser auth/account membership.
2. Advertiser dashboard.
3. Inventory discovery using package, creator, series, and attention rollup data.
4. Creator/series inventory detail pages.
5. Price quote endpoint and UI.
6. Campaign draft and structured brief.
7. Advertiser approvals inbox.
8. Campaign review room with comments, versions, and approval actions.
9. Campaign reporting page.
10. Sales/ops admin hooks for quote approval, campaign activation, and review mediation.

### Defer

- fully automated marketplace clearing;
- programmatic bidding;
- third-party verification integrations;
- self-serve payment for high-ticket deals;
- automated creator matching;
- real-time forecasting beyond simple historical models;
- brand-lift studies;
- complex multi-touch attribution.

## Security And Privacy

Advertiser access must be scoped by account membership.

Advertisers should not see:

- creator private financial terms outside the campaign;
- other advertiser campaigns;
- raw viewer identity;
- user-level personal data;
- internal Vanta margin rules unless intentionally exposed;
- non-public creator analytics outside approved inventory surfaces.

Advertiser reports should aggregate viewer data.

Any retargeting or conversion integration must respect privacy, consent, and applicable law.

## Implementation Notes

### Backend Module Placement

Add advertiser-side modules parallel to creator business modules:

```text
backend/src/api/advertiser/mod.rs
backend/src/api/advertiser/auth.rs
backend/src/api/advertiser/dashboard.rs
backend/src/api/advertiser/inventory.rs
backend/src/api/advertiser/campaigns.rs
backend/src/api/advertiser/quotes.rs
backend/src/api/advertiser/approvals.rs
backend/src/api/advertiser/reviews.rs
backend/src/api/advertiser/reports.rs
```

Register these routes from `backend/src/api.rs` or the existing route assembly layer.

### Frontend Module Placement

Add advertiser pages:

```text
frontend/src/pages/AdvertiserHubPage.tsx
frontend/src/pages/AdvertiserInventoryPage.tsx
frontend/src/pages/AdvertiserInventoryDetailPage.tsx
frontend/src/pages/AdvertiserCampaignBuilderPage.tsx
frontend/src/pages/AdvertiserCampaignPage.tsx
frontend/src/pages/AdvertiserApprovalsPage.tsx
frontend/src/pages/AdvertiserReviewRoomPage.tsx
frontend/src/pages/AdvertiserReportPage.tsx
```

Add advertiser repository methods near the existing Ad Hub methods, but keep creator and advertiser API names distinct.

Example:

```ts
repository.fetchAdvertiserHub()
repository.listAdvertiserInventory()
repository.createAdvertiserPriceQuote(input)
repository.createAdvertiserCampaignDraft(input)
repository.submitAdvertiserCampaignDraft(id)
repository.approveAdSubmission(id)
repository.requestAdSubmissionRevision(id, input)
repository.fetchAdvertiserReviewRoom(campaignId)
repository.createAdvertiserReviewComment(input)
repository.resolveAdvertiserReviewComment(commentId)
repository.fetchAdvertiserCampaignReport(id)
```

### Shared Type Pattern

Do not reuse `CreatorAdHubResponse` for advertiser views.

Create explicit advertiser-side types:

```ts
AdvertiserHubResponse
AdvertiserInventoryItem
AdvertiserInventoryDetail
AdvertiserPriceQuote
AdvertiserCampaignDraft
AdvertiserCampaignReport
AdvertiserApprovalItem
AdvertiserReviewRoom
AdvertiserReviewComment
AdvertiserAssetVersion
```

The creator hub and advertiser hub are connected through the same marketplace tables, but they are different products with different user needs.

## Testing Plan

Follow the workspace testing policy and prefer Fozzy for system readiness.

Minimum backend tests:

- advertiser can list only its own campaigns;
- advertiser cannot access another advertiser's campaign;
- advertiser can create a campaign draft;
- advertiser can generate a price quote;
- quote stores pricing inputs and expiration;
- advertiser can submit a campaign draft for sales review;
- campaign draft rejects free-text-only requirements that should be structured fields;
- invalid combinations return field-level validation errors;
- campaign draft can generate creator offers;
- creator sees generated offers in existing creator Ad Hub;
- creator can accept and submit review proof;
- advertiser can approve or request revision;
- advertiser reviewer without approval permission can comment but cannot approve;
- internal advertiser comments do not leak to creators;
- creator-visible comments are visible in the creator workflow;
- asset version history is preserved across revisions;
- reporting endpoint returns campaign + attention proof;
- report does not expose raw viewer identity.

Minimum frontend checks:

- advertiser dashboard renders non-empty seeded state;
- inventory filters work;
- quote builder updates price and forecast;
- brief form validates required fields;
- brief form uses structured controls for objective, placements, tracking, usage rights, revision rounds, and safety exclusions;
- approvals inbox can approve/request revision;
- review room supports comments and version selection;
- reviewer permissions hide disallowed actions;
- reporting page shows Qualified Attention as proof, not as the sole outcome.

Recommended deterministic scenarios:

```text
advertiser_inventory_discovery
advertiser_campaign_quote_to_creator_offer
advertiser_submission_approval
advertiser_review_room_comments
advertiser_campaign_reporting
```

## Open Decisions

- What auth model should advertiser accounts use relative to existing user auth?
- Should advertiser self-serve checkout be allowed immediately or only after sales approval?
- Which package prices are public versus quote-gated?
- Which creator attention metrics are public to advertisers before a deal?
- How should Vanta define its first brand safety and suitability taxonomy?
- When should third-party verification become a required integration?
- Should creator payout be visible to advertisers or only total package price?
- How much pricing logic should be deterministic versus sales-configured?
- Should timestamped comments require Vanta-hosted video playback, or can v1 support external review URLs only?
- Which comments should creators see directly versus through Vanta mediation?
- Should final advertiser approval lock the creator submission from further edits?

## Success Definition

The external Ad Hub succeeds when an advertiser can:

1. Understand Vanta without a sales call.
2. Browse real creator and series inventory.
3. See credible pricing before talking to a rep.
4. Understand why Qualified Attention proves media quality.
5. Build a campaign brief.
6. Submit or book a package.
7. Review creator submissions.
8. Comment on work and manage revisions inside the portal.
9. Approve final work with an audit trail.
10. Receive transparent reporting.
11. Renew or expand based on evidence.

The engineering goal is not merely to add advertiser pages.

The goal is to connect both sides of the marketplace:

```text
creator-facing Ad Hub
<-> Vanta marketplace engine
<-> external advertiser Ad Hub
```

That connection is where Vanta captures the arbitrage.
