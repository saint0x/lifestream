# Creator Ad Hub

## Purpose

Creator Ad Hub is the creator-side advertising workspace inside the main Vanta frontend.

This is not the standalone external buyer app. This surface lives in the base frontend and is used by Vanta creators.

It is the creator's personal command center for sponsorship offers, campaign requirements, payout visibility, review submissions, and proof. If the external Ad Hub is where media buyers purchase Vanta inventory, Creator Ad Hub is where creators receive, evaluate, execute, and submit that inventory back into the system.

This is one half of the advertising coin.

```text
External Ad Hub = buyer demand
Creator Ad Hub = creator execution
Vanta = marketplace infrastructure in the middle
```

## Product Role

Creator Ad Hub helps creators understand the business opportunity attached to their work.

Creators should be able to answer:

- Which advertisers want to buy my audience?
- What campaign is being offered?
- What do I have to make or submit?
- What is the gross offer?
- What do I personally get paid?
- What is due and when?
- What has been accepted, declined, submitted, or approved?
- What proof does Vanta or the advertiser need?

The product should make advertising feel like structured creator leverage, not random sponsorship chaos.

## Current Surface

Current implementation lives in:

- `frontend/src/pages/AdHubPage.tsx`
- `frontend/src/pages/AdHubPage.css`
- `frontend/src/lib/repository.ts`
- `frontend/src/types/index.ts`

Related creator-side API methods include:

- `repository.fetchAdHub()`
- `repository.acceptAdOffer(id)`
- `repository.declineAdOffer(id)`
- `repository.submitAdOfferReview(id, input)`

The current page supports:

- offer list;
- pending, active, in-review, approved, and declined counts;
- gross offer amount;
- creator payout amount;
- advertiser details;
- campaign details;
- package details;
- requirements;
- accept and decline actions;
- review submission links;
- submission notes;
- package templates.

## Business Importance

Creator Ad Hub exists because Vanta does not only sell ads. Vanta coordinates a marketplace.

The creator needs a clean place to see what is being asked, why it matters, and how much they can make. The buyer needs confidence that sold inventory can become actual deliverables. Vanta needs the state machine in the middle to be structured enough to price, track, review, approve, report, and renew.

That is why the creator-side hub matters. It prevents sales from dissolving into DMs, email threads, vague asks, and missing proof.

## Creator-Benefit Lens

Creator Ad Hub should always feel creator-benefit focused.

The creator is not being asked to blindly serve advertisers. The creator is being given a clearer way to convert the attention around their programming into money.

The mental model:

```text
Creator makes high-quality content
-> creator promotes it aggressively
-> audience creates qualified attention
-> Vanta sells advertiser access
-> Creator Ad Hub turns the deal into clear work, clear proof, and clear payout
```

## What Good Looks Like

The creator should feel:

- the offer is understandable;
- the payout is transparent;
- the requirements are not ambiguous;
- the review process is contained;
- the advertiser proof is easy to submit;
- Vanta is helping them make money from the value they created.

Creator Ad Hub succeeds when creators accept better-fit deals, execute them cleanly, submit proof without confusion, and trust that Vanta is aligning advertiser demand with creator upside.
