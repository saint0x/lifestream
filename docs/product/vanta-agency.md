# VANTA Agency

## Purpose

VANTA Agency is the standalone dashboard for external media buyers.

This is not the creator Ad Hub in the main frontend. This is a separate app for agencies, advertiser representatives, and media-buying teams.

It exists so media buyers can quickly discover, compare, configure, and purchase advertising inventory across Vanta's creator media properties.

The product goal is to remove friction.

A buyer should not need five sales calls just to understand what exists, who the audience is, what it costs, what proof exists, and how to buy. VANTA Agency should make Vanta inventory feel easy to inspect and easy to purchase without making the inventory feel cheap.

## Product Role

VANTA Agency is the demand-side half of Vanta's advertising marketplace.

```text
Media buyers enter VANTA Agency
-> browse creators, niches, packages, pricing, and proof
-> configure campaign details
-> place or request the buy
-> Vanta routes the work to creators
-> Creator Ad Hub handles execution and proof
```

The buyer-facing app supports Vanta's business because self-service buying expands demand. The easier it is for buyers to purchase qualified creator attention, the more budget Vanta can capture.

## Current Surface

Current implementation lives in:

- `vanta-agency/`
- `vanta-agency/src/app/App.tsx`
- `vanta-agency/src/data/portal.ts`
- `vanta-agency/src/domain/types.ts`
- `vanta-agency/src/components/ui`

The directory also includes its own README and product notes:

- `vanta-agency/README.md`
- `vanta-agency/docs/first-principles.md`

The current product surface includes:

- overview dashboard;
- creator discovery;
- niche discovery;
- platform stats;
- creator detail pages;
- episode previews;
- inventory packages;
- cart and checkout;
- order history;
- approvals;
- review rooms;
- reports;
- advertiser account permissions.

## Business Importance

This app is extremely important because it turns Vanta's inventory into a buyer-accessible marketplace.

Without this product, sales depends too heavily on manual explanation:

- what creators exist;
- which audiences are valuable;
- what packages can be bought;
- how pricing works;
- what proof supports the buy;
- what has been approved;
- what was delivered.

VANTA Agency makes that information legible to buyers. That reduces friction, increases confidence, and should increase buying volume.

## The Marketplace Coin

VANTA Agency and Creator Ad Hub are two sides of one system.

```text
VANTA Agency = buyers purchase the inventory
Creator Ad Hub = creators execute the inventory
Vanta = infrastructure, measurement, review, reporting, and marketplace fee
```

Vanta captures value because it operates the infrastructure in the middle:

- inventory packaging;
- pricing;
- buyer discovery;
- creator routing;
- campaign state;
- review workflows;
- proof collection;
- reporting;
- renewals.

## What Good Looks Like

VANTA Agency should help a buyer answer:

- Who can I buy?
- What audience do they bring?
- What proof does Vanta have?
- What package fits my goal?
- What does it cost?
- What needs sales review?
- What have I already ordered?
- What is awaiting approval?
- What did my campaign deliver?

The product succeeds when buyers can move from curiosity to purchase with minimal handholding, while still trusting that Vanta inventory is premium, measurable, and operationally real.
