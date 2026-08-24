# Qualified Attention

Vanta's proprietary Qualified Viewer system is the trust and pricing layer for creator attention.

Not every impression has equal economic value. Vanta therefore should not simply tell advertisers:

> This video received 500,000 views.

Instead, Vanta attempts to determine:

> How many of these viewers represent real, engaged, commercially meaningful humans?

The platform evaluates behavioral signals across the product, turns those signals into Qualified Viewers, and then converts verified creator attention into an advertiser-facing value model.

## Qualified Viewers Vs. Raw Views

Traditional advertising frequently sells:

> Impressions

or:

> Views

Vanta's thesis is that it can increasingly sell:

> Verified Qualified Attention

Instead of:

> 1,000,000 views

Vanta might report:

> 1,000,000 total views<br>
> 280,000 engaged viewers<br>
> 92,000 Qualified Viewers

The 92,000 Qualified Viewers could be substantially more commercially valuable than hundreds of thousands of low-intent impressions.

## The Qualified Viewer Algorithm

The system combines behavioral signals into an internal model representing viewer quality.

Potential signals include:

- Watch time
- Percentage watched
- Repeat viewing
- Number of episodes watched
- Series completion
- Live-stream participation
- Frequency of visits
- Session duration
- Creator follows
- Search activity
- Content discovery
- Likes
- Comments
- Shares
- Audience support
- Return frequency
- Account age
- Interaction patterns
- Cross-series activity
- Historical engagement

Current production scoring starts from measured qualified unique viewers, then adjusts that audience by attention depth, engagement, retention, commercial audience quality, and measurement confidence.

## Creator Attention Value

Creator Attention Value (CAV) is Vanta's canonical model for valuing verified creator attention. It is not a raw view count and it is not a decorative dashboard number. It is the internal pricing and reporting system for advertiser-facing creator inventory.

For creator `c` over period `t`:

```text
CAV(c,t) = U(c,t) * B * A(c,t) * E(c,t) * R(c,t) * Q(c,t) * D(c,t)
```

Where:

```text
B = $0.05
```

`B` is the baseline value of one qualified unique viewer before quality adjustment.

## Canonical Inputs

The source of truth is raw viewer attention, not static analytics rows.

Current implemented sources:

- `live_viewer_sessions`
- `viewer_events`
- `content_credits`
- creator-owned `uploads`
- `visitor_id`
- optional `user_id`
- stream id
- content id
- episode id
- playback progress
- watchlist actions
- page-level dwell events where content attribution is known
- connected time
- last seen time
- disconnected time

`user_watch_history` remains the signed-in library/resume-state persistence layer. It is not counted as a primary CAV input unless a measured playback or dwell event exists, because history rows are mutable user state while CAV must be rebuilt from raw attention signals.
- attribution source / medium / campaign
- UTM fields
- landing URL and referrer

The frontend creates a durable anonymous visitor id in `localStorage` under:

```text
vanta.visitorId
```

Signed-in users are resolved above anonymous visitors. Viewer identity precedence is:

```text
user_id -> visitor_id -> websocket session token
```

That precedence is important. One human should not be counted multiple times merely because they reconnect or open multiple sockets.

## Qualified Unique Viewers

`U` is the count of unique resolved viewers who cross the qualified-view threshold.

Current v2 threshold:

```text
qualified = total creator watch seconds >= 90
```

A viewer who opens a stream or title for a few seconds is measured, but does not become a qualified viewer.

VOD progress events are deduplicated at viewer/content/day granularity. The algorithm uses the highest persisted progress point for that viewer and content rather than summing every player heartbeat. This prevents noisy clients from inflating attention by emitting repeated progress events.

Daily rollups are UTC calendar-day materializations. Current v2 assigns live viewer sessions to the day of `connected_at` and viewer events to the day of `occurred_at`; the worker reconciles today and yesterday so late disconnects and late events update the affected rollup without inventing unmeasured time.

## Attention Multiplier

Attention measures depth of qualified viewing.

```text
A = 1 + 0.25 * ln(1 + M / 10)
```

Where:

- `M` = average watched minutes per qualified viewer

The curve is logarithmic so long viewing matters without letting artificially long streams dominate the score.

## Engagement Multiplier

Engagement measures demonstrated viewer action.

The full target model includes chat, comments, follows, shares, profile exploration, clicks, watchlist activity, clips, notification opt-ins, and audience support.

Current v2 only uses events that are actually captured in source tables:

- live chat participation
- live clip requests
- live notification opt-ins
- watchlist actions
- deep content behavior from playback progress and multi-content consumption

Current v2:

```text
E = 1 + 0.40 * I
```

Where `I` is a normalized measured engagement intensity:

```text
I = 0.25 * chat + 0.12 * clips + 0.15 * notify + 0.25 * watchlist + 0.23 * depth
```

Signals that are not yet captured are not silently estimated.

## Retention Multiplier

Retention measures whether qualified viewers return.

```text
r = returning qualified viewers / qualified viewers
R = 0.8 + 0.8 * r
```

Current v2 defines returning viewers as qualified viewers with at least two measured sessions for the creator or meaningful attention across at least two creator-attributed content objects.

## Audience Quality Multiplier

Audience quality represents advertiser demand by creator category.

Current v2 starts with explicit category coefficients:

| Category | Q |
| :-- | --: |
| General | 1.00 |
| Gaming | 1.10 |
| Sports / outdoor / lifestyle | 1.15 |
| Technology | 1.35 |
| Software / AI / developer | 1.50 |

These values are seed coefficients. They should eventually be calibrated from advertiser clearing prices, renewal rates, and campaign performance.

## Data Confidence Multiplier

Data confidence represents measurement quality.

```text
D = identity_confidence * attribution_confidence
```

Current v2:

- authenticated qualified viewers increase identity confidence
- attributed qualified viewers increase attribution confidence
- anonymous but stable `visitor_id` traffic is still valid, but discounted versus authenticated first-party identity
- traffic without attribution is discounted

Bot and invalid-traffic filtering belongs in this multiplier, but v2 does not pretend to have a full fraud model yet.

Current v2 confidence includes:

- identity confidence from signed-in viewers and stable anonymous visitor IDs
- attribution confidence from UTM/referrer/landing markers
- behavior confidence from repeat sessions, multi-content viewing, watchlist action, or sustained playback progress
- invalid-traffic penalty for abnormal event volume or implausibly long single-day watch time

It is still not a final fraud system. It is a stricter measured-confidence layer that refuses to sell low-evidence traffic at full value.

## Bot And Low-Quality Traffic Filtering

The Qualified Viewer system also creates a trust layer.

The platform can analyze behavioral patterns to identify:

- Bots
- Automated traffic
- Fraudulent views
- Click farms
- Extremely low-engagement sessions
- Suspicious account behavior
- Artificial audience inflation

The advertiser therefore receives a much stronger claim than:

> Your advertisement received one million impressions.

The platform can instead say:

> You purchased access to an audience we have independently evaluated as real and meaningfully engaged.

This increases confidence in the inventory.

## Verified Viewer Score

Advertisers need a legible quality score in addition to the dollar CAV estimate.

The Verified Viewer Score is a 0-100 normalized index derived from:

- attention multiplier
- engagement multiplier
- retention multiplier
- audience quality multiplier
- data confidence multiplier
- qualified-viewer rate

The score is not a view count. It is the advertiser-facing quality rating of a creator's measured audience.

Current backend algorithm version:

```text
cav-v2.0.0
```

Algorithm versions must be stored and returned with score output. Any future formula change must bump the version.

## Qualified Viewer As The Advertising Unit

Over time, Vanta can potentially make the Qualified Viewer the fundamental unit of its advertising economy.

This would be analogous to platforms selling CPM, CPC, CPA, or other advertising units, except Vanta creates its own quality-adjusted audience metric.

Instead of asking:

> How many views did this creator receive?

advertisers begin asking:

> How many Qualified Viewers does this creator have?

That distinction could become extremely important.

It turns Vanta's analytics system itself into part of the product.

## Why Qualified Attention Commands Premium Pricing

Consider two hypothetical media properties.

### Platform A

1,000,000 impressions.

Very little information exists about the audience.

### Vanta

250,000 Qualified Viewers.

Vanta can demonstrate that these people:

- Are real
- Repeatedly consume the content
- Watch for meaningful durations
- Return to the platform
- Interact with creators
- Display high-intent behavior

An advertiser may rationally prefer the second audience despite the lower raw number.

Vanta therefore attempts to move advertising economics away from:

> Quantity of impressions

toward:

> Quality of attention.

## Dashboard Outputs

Creator analytics should expose:

- Verified Viewer Score
- Qualified viewers
- Creator Attention Value
- average watch minutes
- qualified viewer rate
- returning viewer rate
- measured sessions
- baseline value
- attention multiplier
- engagement multiplier
- retention multiplier
- audience quality multiplier
- data confidence multiplier
- algorithm version

This is the minimum advertiser-ready surface.

## Value Capture Rate

CAV estimates attributed creator value. Revenue measures actual monetization.

```text
VCR = actual creator revenue / attributed creator value
```

If CAV is `$20,000` and actual creator revenue is `$6,000`, then:

```text
VCR = 30%
```

That means the audience is not weak. It means the monetization engine is leaving value uncaptured.

## Implementation Rule

Raw measurement tables are the source of truth. CAV and Verified Viewer Score are derived outputs.

Do not store arbitrary score numbers on creator rows. Do not show static score fixtures. Do not backfill fake engagement signals. Every multiplier must come from measured data, explicit coefficients, or a documented algorithm version.

The backend persists derived daily rows in `creator_attention_daily` using `(creator_id, day, algorithm_version)` as the key. Those rows are materialized from the canonical raw measurement calculation and may be safely overwritten by later reconciliation passes for the same day/version. Every `creator_profiles` row receives a daily materialization for the active algorithm version; creators without measured qualified attention get explicit zero-value rows instead of being absent from the metric table.

Production uses Railway Postgres for this calculation. SQLite is only a development and test mirror.
