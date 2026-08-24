# VANTA External Ad Hub

Separate advertiser-facing portal for VANTA's media marketplace.

The creator Ad Hub lets creators receive offers, accept or decline campaigns, and submit work for review. This app is the other side of that marketplace: external advertisers browse visual creator media, inspect niches and platform stats, add multiple buys to a combined checkout order, pay with a hard-coded payment surface, and then manage approvals and reporting after purchase.

## Structure

- `src/app` owns the app shell, route-like views, checkout/cart state, order state, and advertiser workflow composition.
- `src/components/ui` is copied one-to-one from the existing VANTA frontend design system.
- `src/domain` contains advertiser portal contracts.
- `src/data` contains local portal data shaped like the future advertiser API responses.
- `src/lib` contains stable formatting helpers.
- `src/styles` contains the copied VANTA global tokens and base styles.

## Run

```bash
cd external-ad-hub
bun install
bun run dev
```

## Test

```bash
bun run typecheck
bun run build
bun run lint
```

When advertiser backend endpoints are available, replace `src/data/portal.ts` and the local order state with repository methods that return the same domain contracts. Keep campaign pricing, checkout, approvals, and reporting state structured; do not move contractually meaningful choices into free text.

For browser verification without clicking through the mock cart, use:

```text
/?seed=cart#cart
/?seed=cart#cart
/?seed=order#orders
```
