import { useEffect, useMemo, useState } from "react";
import type { Dispatch, ReactNode, SetStateAction } from "react";
import {
  BarChart3,
  Building2,
  Check,
  ClipboardCheck,
  CreditCard,
  FileCheck2,
  LockKeyhole,
  MessageSquare,
  ExternalLink,
  Play,
  Plus,
  ReceiptText,
  Search,
  ShieldCheck,
  ShoppingCart,
  Trash2,
  UserPlus,
  Users,
} from "lucide-react";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { portal } from "@/data/portal";
import type {
  AdvertiserAccount,
  AdvertiserPermission,
  AdvertiserRole,
  CartLine,
  InventoryItem,
  Objective,
  Order,
  PortalView,
  ReviewStatus,
} from "@/domain/types";
import { compactDate, compactNumber, money, percent } from "@/lib/format";

const views: ReadonlyArray<{ readonly id: PortalView; readonly label: string }> = [
  { id: "overview", label: "Overview" },
  { id: "creators", label: "Creators" },
  { id: "niches", label: "Niches" },
  { id: "stats", label: "Platform Stats" },
  { id: "cart", label: "Checkout" },
  { id: "orders", label: "Orders" },
  { id: "approvals", label: "Approvals" },
  { id: "review", label: "Review" },
  { id: "reports", label: "Reports" },
  { id: "account", label: "Account" },
];

const objectiveLabels: Record<Objective, string> = {
  awareness: "Awareness",
  consideration: "Consideration",
  traffic: "Traffic",
  conversion: "Conversion",
  sponsorship_association: "Sponsorship",
  category_ownership: "Category ownership",
  launch: "Launch",
};

const permissionLabels: Record<AdvertiserPermission, string> = {
  manage_account: "Manage account",
  manage_team: "Manage team",
  manage_billing: "Manage billing",
  buy_media: "Buy media",
  approve_work: "Approve work",
  view_reports: "View reports",
};

function hasPermission(account: AdvertiserAccount, permission: AdvertiserPermission): boolean {
  return account.currentSeat.permissions.includes(permission);
}

const startDate = "2026-09-14";
const endDate = "2026-10-12";

function viewFromHash(): PortalView {
  const value = window.location.hash.replace("#", "");
  return views.some((item) => item.id === value) || value === "creator" ? (value as PortalView) : "overview";
}

function statusTone(status: string): "neutral" | "new" | "premium" | "live" {
  if (status === "review_pending" || status === "submitted" || status === "sales_review") return "new";
  if (status === "approved" || status === "paid" || status === "active" || status === "reporting") return "premium";
  if (status === "revision_requested" || status === "in_review") return "live";
  return "neutral";
}

function brandSafetyLabel(value: InventoryItem["brandSafety"]): string {
  if (value === "sensitive_review") return "VANTA Agency review";
  if (value === "restricted") return "Restricted";
  return "Standard";
}

function itemFor(line: CartLine): InventoryItem {
  return portal.inventory.find((item) => item.id === line.inventoryId) ?? portal.inventory[0];
}

function lineSubtotal(line: CartLine): number {
  const item = itemFor(line);
  const exclusivity = line.categoryExclusivity ? Math.round(item.basePriceCents * 0.22) : 0;
  const usage = line.usageRights === "paid_amplification" ? Math.round(item.basePriceCents * 0.16) : 0;
  const tracking = line.tracking === "third_party" ? 75000 : 0;
  return item.basePriceCents * line.units + exclusivity + usage + tracking;
}

function needsSalesReview(line: CartLine): boolean {
  const item = itemFor(line);
  return item.brandSafety !== "standard" || line.categoryExclusivity || line.usageRights === "paid_amplification";
}

function quoteFor(lines: ReadonlyArray<CartLine>) {
  const subtotalCents = lines.reduce((sum, line) => sum + lineSubtotal(line), 0);
  const serviceCents = Math.round(subtotalCents * 0.06);
  return { subtotalCents, serviceCents, totalCents: subtotalCents + serviceCents };
}

function platformStats() {
  const qualifiedViewers = portal.inventory.reduce((sum, item) => sum + item.attention.qualifiedViewers, 0);
  const measuredSessions = portal.inventory.reduce((sum, item) => sum + item.attention.measuredSessions, 0);
  const averageScore = Math.round(
    portal.inventory.reduce((sum, item) => sum + item.attention.verifiedViewerScore, 0) / portal.inventory.length,
  );
  const minimumPrice = Math.min(...portal.inventory.map((item) => item.basePriceCents));
  return { qualifiedViewers, measuredSessions, averageScore, minimumPrice };
}

function nicheStats() {
  return Array.from(new Set(portal.inventory.map((item) => item.category))).map((category) => {
    const items = portal.inventory.filter((item) => item.category === category);
    return {
      category,
      creators: items.length,
      qualifiedViewers: items.reduce((sum, item) => sum + item.attention.qualifiedViewers, 0),
      startingPrice: Math.min(...items.map((item) => item.basePriceCents)),
      bestScore: Math.max(...items.map((item) => item.attention.verifiedViewerScore)),
      items,
    };
  });
}

function defaultLine(item: InventoryItem, suffix = `${Date.now()}`): CartLine {
  return {
    id: `line-${item.id}-${suffix}`,
    inventoryId: item.id,
    units: item.minUnits,
    objective: item.objectiveFit[0],
    flightStart: startDate,
    flightEnd: endDate,
    tracking: item.objectiveFit.includes("conversion") ? "codes" : "links",
    usageRights: "organic_repost",
    categoryExclusivity: item.objectiveFit.includes("category_ownership"),
    approvalRounds: 1,
    context: "",
  };
}

function seededLines(): ReadonlyArray<CartLine> {
  return [defaultLine(portal.inventory[0], "seed-1"), defaultLine(portal.inventory[2], "seed-2")];
}

function initialCart(): ReadonlyArray<CartLine> {
  const params = new URLSearchParams(window.location.search);
  if (params.get("seed") !== "cart") return [];
  return seededLines();
}

function initialOrders(): ReadonlyArray<Order> {
  const params = new URLSearchParams(window.location.search);
  if (params.get("seed") !== "order") return [];
  const lines = seededLines();
  const quote = quoteFor(lines);
  return [{
    id: "AGENCY-20260824-001",
    createdAt: "2026-08-24T12:00:00.000Z",
    advertiser: portal.account.company.name,
    lines,
    subtotalCents: quote.subtotalCents,
    serviceCents: quote.serviceCents,
    totalCents: quote.totalCents,
    paymentMethod: "Corporate card ending 4242",
    status: lines.some(needsSalesReview) ? "sales_review" : "paid",
  }];
}

function Metric({
  label,
  value,
  detail,
  icon,
}: {
  readonly label: string;
  readonly value: string;
  readonly detail?: string;
  readonly icon?: ReactNode;
}) {
  return (
    <div className="ea-card ea-metric">
      <span className="ea-label mono">{label}</span>
      <strong>{value}</strong>
      {detail ? <em>{detail}</em> : null}
      {icon ? <span className="ea-metric__icon">{icon}</span> : null}
    </div>
  );
}

function OverviewView({
  lines,
  openCreators,
  openNiches,
  openStats,
  openCreator,
}: {
  readonly lines: ReadonlyArray<CartLine>;
  readonly openCreators: () => void;
  readonly openNiches: () => void;
  readonly openStats: () => void;
  readonly openCreator: (item: InventoryItem) => void;
}) {
  const stats = platformStats();
  const top = [...portal.inventory].sort((a, b) => b.attention.qualifiedViewers - a.attention.qualifiedViewers).slice(0, 2);

  return (
    <div className="ea-stack">
      <section className="ea-hero">
        <img src={top[0].image} alt="" />
        <div className="ea-hero__copy">
          <span className="ea-label mono">Recommended buy</span>
          <h2>{top[0].creator}</h2>
          <p>{top[0].salesNote}</p>
          <div className="ea-actions">
            <Button variant="primary" icon={<Play />} onClick={() => openCreator(top[0])}>View media</Button>
            <Button variant="outline" icon={<ShoppingCart />} disabled={lines.length === 0}>Cart ready</Button>
          </div>
        </div>
      </section>

      <section className="ea-metrics">
        <Metric label="Qualified viewers" value={compactNumber(stats.qualifiedViewers)} detail="Across available packages" icon={<BarChart3 />} />
        <Metric label="Measured sessions" value={compactNumber(stats.measuredSessions)} detail="Recent inventory proof" icon={<ShieldCheck />} />
        <Metric label="Avg verified score" value={`${stats.averageScore}`} detail="Creator inventory" icon={<Check />} />
        <Metric label="Starting at" value={money(stats.minimumPrice)} detail="Per media unit" icon={<ShoppingCart />} />
      </section>

      <section className="ea-grid">
        <button type="button" className="ea-portal-card" onClick={openCreators}>
          <strong>Popular creators</strong>
          <p>Browse visual creator profiles, episodes, proof, and packages.</p>
        </button>
        <button type="button" className="ea-portal-card" onClick={openNiches}>
          <strong>Popular niches</strong>
          <p>Start from buyer category: outdoor, workshop, automotive, and more.</p>
        </button>
        <button type="button" className="ea-portal-card" onClick={openStats}>
          <strong>Platform stats</strong>
          <p>See the aggregate proof layer before choosing creators.</p>
        </button>
        <button type="button" className="ea-portal-card" onClick={() => openCreator(top[1])}>
          <strong>Featured media</strong>
          <p>{top[1].creator}: {compactNumber(top[1].attention.qualifiedViewers)} forecast Qualified Viewers.</p>
        </button>
      </section>
    </div>
  );
}

function CreatorsView({
  openCreator,
}: {
  readonly openCreator: (item: InventoryItem) => void;
}) {
  const [query, setQuery] = useState("");
  const filtered = portal.inventory.filter((item) => {
    const text = `${item.creator} ${item.series} ${item.category}`.toLowerCase();
    return text.includes(query.trim().toLowerCase());
  });

  return (
    <section className="ea-panel">
      <div className="ea-panel__head">
        <div>
          <h2>Popular creators</h2>
          <p>Open a creator profile to watch media, inspect stats, and add packages.</p>
        </div>
        <Input icon={<Search />} value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search creators" />
      </div>
      <div className="ea-media-grid">
        {filtered.map((item) => (
          <button key={item.id} type="button" className="ea-media-card" onClick={() => openCreator(item)}>
            <img src={item.image} alt="" />
            <span>
              <strong>{item.creator}</strong>
              <em>{item.series} / {item.category}</em>
            </span>
            <span className="ea-soft-proof">
              <span>{compactNumber(item.attention.qualifiedViewers)} QV</span>
              <span>{item.attention.verifiedViewerScore} score</span>
            </span>
          </button>
        ))}
      </div>
    </section>
  );
}

function NichesView({
  openCreator,
}: {
  readonly openCreator: (item: InventoryItem) => void;
}) {
  return (
    <section className="ea-panel">
      <div className="ea-panel__head">
        <div>
          <h2>Popular niches</h2>
          <p>Start by category, then drill into the creators carrying that audience.</p>
        </div>
        <Badge tone="premium">{nicheStats().length} active niches</Badge>
      </div>
      <div className="ea-niche-grid">
        {nicheStats().map((niche) => (
          <article key={niche.category} className="ea-niche">
            <img src={niche.items[0].image} alt="" />
            <div>
              <span className="ea-label mono">{niche.creators} creator{niche.creators === 1 ? "" : "s"}</span>
              <h3>{niche.category}</h3>
              <p>{compactNumber(niche.qualifiedViewers)} forecast Qualified Viewers / starts at {money(niche.startingPrice)}</p>
            </div>
            <div className="ea-actions">
              {niche.items.map((item) => (
                <Button key={item.id} variant="outline" size="sm" icon={<Play />} onClick={() => openCreator(item)}>
                  {item.creator}
                </Button>
              ))}
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}

function StatsView() {
  const stats = platformStats();

  return (
    <div className="ea-stack">
      <section className="ea-metrics">
        <Metric label="Qualified viewers" value={compactNumber(stats.qualifiedViewers)} detail="Available inventory" icon={<BarChart3 />} />
        <Metric label="Measured sessions" value={compactNumber(stats.measuredSessions)} detail="Proof source" icon={<ShieldCheck />} />
        <Metric label="Average score" value={`${stats.averageScore}`} detail="Verified Viewer Score" icon={<Check />} />
        <Metric label="Starting price" value={money(stats.minimumPrice)} detail="Lowest package" icon={<ShoppingCart />} />
      </section>
      <div className="ea-panel">
        <div className="ea-panel__head">
          <div>
            <h2>Platform by niche</h2>
            <p>High-level proof before the buyer chooses a creator or package.</p>
          </div>
        </div>
        <div className="ea-stat-list">
          {nicheStats().map((niche) => (
            <div key={niche.category} className="ea-stat-row">
              <strong>{niche.category}</strong>
              <span>{compactNumber(niche.qualifiedViewers)} QV</span>
              <span>{niche.bestScore} best score</span>
              <span>{money(niche.startingPrice)} starting</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function CreatorDetailView({
  item,
  addToCart,
  canBuy,
}: {
  readonly item: InventoryItem;
  readonly addToCart: (item: InventoryItem) => void;
  readonly canBuy: boolean;
}) {
  return (
    <section className="ea-grid ea-grid--wide-left">
      <div className="ea-panel">
        <div className="ea-creator-hero">
          <img src={item.image} alt="" />
          <div>
            <span className="ea-label mono">{item.category}</span>
            <h2>{item.creator}</h2>
            <p>{item.audience}</p>
            <div className="ea-actions">
              <Button variant="primary" icon={<ShoppingCart />} disabled={!canBuy} onClick={() => addToCart(item)}>Add package</Button>
              <a className="ea-link-button" href={item.profileUrl} target="_blank" rel="noreferrer">
                <ExternalLink size={15} /> Open platform profile
              </a>
            </div>
          </div>
        </div>
        <div className="ea-metrics ea-metrics--tight">
          <Metric label="Qualified viewers" value={compactNumber(item.attention.qualifiedViewers)} />
          <Metric label="Verified score" value={`${item.attention.verifiedViewerScore}`} />
          <Metric label="Avg watch" value={`${item.attention.averageWatchMinutes}m`} />
          <Metric label="Returning" value={percent(item.attention.returningViewerRate)} />
        </div>
        <div className="ea-panel__head ea-panel__head--sub">
          <div>
            <h2>Episodes</h2>
            <p>Media buyers can preview the actual programming before buying.</p>
          </div>
        </div>
        <div className="ea-episode-grid">
          {item.episodes.map((episode) => (
            <a key={episode.id} className="ea-episode" href={episode.playbackUrl} target="_blank" rel="noreferrer">
              <img src={episode.image} alt="" />
              <span><Play size={16} /> Play</span>
              <strong>{episode.title}</strong>
              <em>{episode.duration} / {compactNumber(episode.views)} views</em>
            </a>
          ))}
        </div>
      </div>
      <div className="ea-panel">
        <div className="ea-panel__head">
          <div>
            <h2>{item.package}</h2>
            <p>{item.salesNote}</p>
          </div>
          <Badge tone={item.brandSafety === "standard" ? "premium" : "new"}>{brandSafetyLabel(item.brandSafety)}</Badge>
        </div>
        <div className="ea-buy-strip">
          <strong>{money(item.basePriceCents)}</strong>
          <span>per {item.unitLabel}</span>
          <span>{item.availability}</span>
        </div>
        <div className="ea-facts">
          <div><span className="ea-label mono">Placement</span>{item.placement}</div>
          <div><span className="ea-label mono">Promotion</span>{item.promotion}</div>
          <div><span className="ea-label mono">Objective fit</span>{item.objectiveFit.map((value) => objectiveLabels[value]).join(", ")}</div>
        </div>
        <div className="ea-checks">
          {item.deliverables.map((deliverable) => <span key={deliverable}><ShieldCheck size={14} />{deliverable}</span>)}
        </div>
        <Button variant="primary" icon={<Plus />} disabled={!canBuy} onClick={() => addToCart(item)}>Add to order</Button>
        {!canBuy ? <p className="ea-muted">Your role can browse media, but needs Buy media permission to add packages.</p> : null}
      </div>
    </section>
  );
}

function CartView({
  lines,
  updateLine,
  removeLine,
  goShop,
  placeOrder,
  addUpsell,
  canBuy,
}: {
  readonly lines: ReadonlyArray<CartLine>;
  readonly updateLine: (id: string, patch: Partial<CartLine>) => void;
  readonly removeLine: (id: string) => void;
  readonly goShop: () => void;
  readonly placeOrder: (paymentMethod: string) => void;
  readonly addUpsell: (item: InventoryItem) => void;
  readonly canBuy: boolean;
}) {
  const [paymentMethod, setPaymentMethod] = useState("Corporate card ending 4242");
  const [poNumber, setPoNumber] = useState("NS-2026-0914");
  const quote = quoteFor(lines);
  const units = lines.reduce((sum, line) => sum + line.units, 0);
  const salesReview = lines.some(needsSalesReview);
  const upsells = portal.inventory.filter((item) => !lines.some((line) => line.inventoryId === item.id)).slice(0, 2);

  return (
    <section className="ea-grid ea-grid--wide-left">
      <div className="ea-panel">
        <div className="ea-panel__head">
          <div>
            <h2>Checkout</h2>
            <p>Review packages, tune campaign details, and place the order on one page.</p>
          </div>
          <Button variant="outline" icon={<Plus />} onClick={goShop}>Add another</Button>
        </div>
        {lines.length === 0 ? (
          <div className="ea-empty">
            <strong>Your cart is empty</strong>
            <p>Add one or more ad packages to start an order.</p>
            <Button variant="primary" icon={<ShoppingCart />} onClick={goShop}>Shop packages</Button>
          </div>
        ) : (
          <div className="ea-list">
            {lines.map((line) => {
              const item = itemFor(line);
              return (
                <article key={line.id} className="ea-cart-line">
                  <div className="ea-cart-line__head">
                    <span>
                      <strong>{item.package}</strong>
                      <em>{item.creator} / {money(lineSubtotal(line))}</em>
                    </span>
                    <Button variant="ghost" size="sm" icon={<Trash2 />} onClick={() => removeLine(line.id)}>Remove</Button>
                  </div>
                  <div className="ea-order-form">
                    <label>
                      <span className="ea-label mono">Units</span>
                      <input type="number" min={item.minUnits} max={item.maxUnits} value={line.units} onChange={(event) => updateLine(line.id, { units: Number(event.target.value) })} />
                    </label>
                    <label>
                      <span className="ea-label mono">Goal</span>
                      <select value={line.objective} onChange={(event) => updateLine(line.id, { objective: event.target.value as Objective })}>
                        {item.objectiveFit.map((value) => <option key={value} value={value}>{objectiveLabels[value]}</option>)}
                      </select>
                    </label>
                    <label>
                      <span className="ea-label mono">Start</span>
                      <input type="date" value={line.flightStart} onChange={(event) => updateLine(line.id, { flightStart: event.target.value })} />
                    </label>
                    <label>
                      <span className="ea-label mono">End</span>
                      <input type="date" value={line.flightEnd} onChange={(event) => updateLine(line.id, { flightEnd: event.target.value })} />
                    </label>
                    <label>
                      <span className="ea-label mono">Tracking</span>
                      <select value={line.tracking} onChange={(event) => updateLine(line.id, { tracking: event.target.value as CartLine["tracking"] })}>
                        <option value="none">None</option>
                        <option value="links">Links</option>
                        <option value="codes">Promo codes</option>
                        <option value="third_party">Third party</option>
                      </select>
                    </label>
                    <label>
                      <span className="ea-label mono">Usage</span>
                      <select value={line.usageRights} onChange={(event) => updateLine(line.id, { usageRights: event.target.value as CartLine["usageRights"] })}>
                        <option value="none">None</option>
                        <option value="organic_repost">Organic repost</option>
                        <option value="paid_amplification">Paid amplification</option>
                      </select>
                    </label>
                    <label>
                      <span className="ea-label mono">Reviews</span>
                      <select value={line.approvalRounds} onChange={(event) => updateLine(line.id, { approvalRounds: Number(event.target.value) as CartLine["approvalRounds"] })}>
                        <option value={0}>0 rounds</option>
                        <option value={1}>1 round</option>
                        <option value={2}>2 rounds</option>
                      </select>
                    </label>
                    <label className="ea-toggle">
                      <input type="checkbox" checked={line.categoryExclusivity} onChange={(event) => updateLine(line.id, { categoryExclusivity: event.target.checked })} />
                      <span>Category exclusivity</span>
                    </label>
                    <label className="ea-order-form__wide">
                      <span className="ea-label mono">Campaign context</span>
                      <textarea value={line.context} maxLength={360} onChange={(event) => updateLine(line.id, { context: event.target.value })} placeholder="Product, offer, and brand context." />
                    </label>
                  </div>
                  <div className="ea-actions">
                    <Badge tone={needsSalesReview(line) ? "new" : "premium"}>{needsSalesReview(line) ? "Sales review" : "Ready"}</Badge>
                    <span className="ea-muted">{compactNumber(Math.round(item.attention.qualifiedViewers * line.units))} forecast Qualified Viewers</span>
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </div>
      <aside className="ea-panel ea-summary">
        <div className="ea-panel__head">
          <div>
            <h2>Payment</h2>
            <p>{units} media unit{units === 1 ? "" : "s"} selected.</p>
          </div>
          <CreditCard size={18} strokeWidth={1.75} />
        </div>
        <div className="ea-facts">
          <div><span className="ea-label mono">Media</span>{money(quote.subtotalCents)}</div>
          <div><span className="ea-label mono">Service</span>{money(quote.serviceCents)}</div>
          <div><span className="ea-label mono">Total</span>{money(quote.totalCents)}</div>
          <div><span className="ea-label mono">Status</span>{salesReview ? "Sales review" : "Ready"}</div>
        </div>
        <div className="ea-order-form ea-order-form--payment">
          <label>
            <span className="ea-label mono">Method</span>
            <select value={paymentMethod} onChange={(event) => setPaymentMethod(event.target.value)}>
              <option>Corporate card ending 4242</option>
              <option>Invoice net 30</option>
              <option>ACH on file</option>
            </select>
          </label>
          <label>
            <span className="ea-label mono">PO</span>
            <input value={poNumber} onChange={(event) => setPoNumber(event.target.value)} />
          </label>
          <label className="ea-order-form__wide">
            <span className="ea-label mono">Next</span>
            <textarea readOnly value="VANTA Agency reserves inventory, routes review-required packages to ops, creates creator offers, and opens approval rooms when work is submitted." />
          </label>
        </div>
        <Button variant="primary" icon={<CreditCard />} full disabled={lines.length === 0 || !canBuy} onClick={() => placeOrder(paymentMethod)}>
          Place order
        </Button>
        {!canBuy ? <p className="ea-muted">Only seats with Buy media permission can submit checkout.</p> : null}
        {upsells.length > 0 ? (
          <div className="ea-upsell">
            <span className="ea-label mono">Recommended add-ons</span>
            {upsells.map((item) => (
              <button key={item.id} type="button" onClick={() => addUpsell(item)}>
                <img src={item.image} alt="" />
                <span>
                  <strong>{item.creator}</strong>
                  <em>{item.package} / {money(item.basePriceCents)}</em>
                </span>
                <Plus size={15} />
              </button>
            ))}
          </div>
        ) : null}
      </aside>
    </section>
  );
}

function OrdersView({
  orders,
  goShop,
}: {
  readonly orders: ReadonlyArray<Order>;
  readonly goShop: () => void;
}) {
  const orderedSpend = orders.reduce((sum, order) => sum + order.totalCents, 0);
  const committedSpend = portal.campaigns.reduce((sum, campaign) => sum + campaign.committedSpendCents, 0);

  return (
    <div className="ea-stack">
      <section className="ea-metrics">
        <Metric label="Session orders" value={`${orders.length}`} detail="Submitted now" icon={<ReceiptText />} />
        <Metric label="New spend" value={money(orderedSpend)} detail="Media + service" icon={<CreditCard />} />
        <Metric label="Committed spend" value={money(committedSpend)} detail="Existing campaigns" icon={<BarChart3 />} />
        <Metric label="Approval queue" value={`${portal.approvals.filter((approval) => approval.status === "review_pending").length}`} detail="Post-purchase" icon={<FileCheck2 />} />
      </section>
      <section className="ea-panel">
        <div className="ea-panel__head">
          <div>
            <h2>Orders and campaigns</h2>
            <p>Every order becomes a campaign workspace with review and reporting.</p>
          </div>
          <Button variant="primary" icon={<Plus />} onClick={goShop}>Buy more</Button>
        </div>
        <div className="ea-list">
          {orders.map((order) => (
            <article key={order.id} className="ea-card ea-order">
              <div>
                <strong>{order.id}</strong>
                <p>{order.lines.length} packages / {order.paymentMethod}</p>
              </div>
              <Badge tone={statusTone(order.status)}>{order.status.replace("_", " ")}</Badge>
              <span>{money(order.totalCents)}</span>
            </article>
          ))}
          {portal.campaigns.map((campaign) => (
            <article key={campaign.id} className="ea-card ea-order">
              <div>
                <strong>{campaign.name}</strong>
                <p>{campaign.creator} / {campaign.package}</p>
              </div>
              <Badge tone={statusTone(campaign.status)}>{campaign.status.replace("_", " ")}</Badge>
              <span>{money(campaign.committedSpendCents)}</span>
            </article>
          ))}
        </div>
      </section>
    </div>
  );
}

function ApprovalsView({ canApprove }: { readonly canApprove: boolean }) {
  const [decisions, setDecisions] = useState<Record<string, ReviewStatus>>({});

  return (
    <section className="ea-panel">
      <div className="ea-panel__head">
        <div>
          <h2>Approvals</h2>
          <p>Review purchased campaign work when creators submit it.</p>
        </div>
        <ClipboardCheck size={18} strokeWidth={1.75} />
      </div>
      <div className="ea-list">
        {portal.approvals.map((approval) => {
          const status = decisions[approval.id] ?? approval.status;
          return (
            <div key={approval.id} className="ea-card ea-approval">
              <div>
                <strong>{approval.campaign}</strong>
                <p>{approval.creator} / {approval.package}</p>
              </div>
              <div className="ea-facts">
                <div><span className="ea-label mono">Submitted</span>{compactDate(approval.submittedAt)}</div>
                <div><span className="ea-label mono">Due</span>{compactDate(approval.decisionDueAt)}</div>
                <div><span className="ea-label mono">Link</span>{approval.submissionUrl}</div>
              </div>
              <div className="ea-actions">
                <Badge tone={statusTone(status)}>{status.replace("_", " ")}</Badge>
                <Button variant="primary" icon={<Check />} disabled={!canApprove} onClick={() => setDecisions((current) => ({ ...current, [approval.id]: "approved" }))}>Approve</Button>
                <Button variant="outline" icon={<MessageSquare />} disabled={!canApprove} onClick={() => setDecisions((current) => ({ ...current, [approval.id]: "revision_requested" }))}>Revision</Button>
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
}

function ReviewView({ canApprove }: { readonly canApprove: boolean }) {
  return (
    <section className="ea-grid ea-grid--wide-left">
      <div className="ea-panel">
        <div className="ea-panel__head">
          <div>
            <h2>Review room</h2>
            <p>A simple workspace for assets, comments, and final approval.</p>
          </div>
          <FileCheck2 size={18} strokeWidth={1.75} />
        </div>
        <div className="ea-review-media">
          <span className="ea-label mono">Submitted asset</span>
          <strong>v2 rough cut</strong>
          <p>https://review.vanta.local/v2</p>
        </div>
        <div className="ea-list">
          {portal.reviewRoom.comments.map((comment) => (
            <div key={comment.id} className="ea-comment">
              <span>
                <strong>{comment.author}</strong>
                <Badge tone={comment.visibility === "creator_visible" ? "premium" : "neutral"}>{comment.visibility.replace("_", " ")}</Badge>
              </span>
              <p>{comment.timestampSeconds ? `${comment.timestampSeconds}s / ` : ""}{comment.body}</p>
              <em>{comment.resolved ? "Resolved" : "Open"}</em>
            </div>
          ))}
        </div>
      </div>
      <div className="ea-panel">
        <div className="ea-panel__head">
          <div>
            <h2>Campaign brief</h2>
            <p>Source of truth from the purchase order.</p>
          </div>
          <LockKeyhole size={18} strokeWidth={1.75} />
        </div>
        <div className="ea-facts">
          {portal.reviewRoom.brief.map((item) => <div key={item.label}><span className="ea-label mono">{item.label}</span>{item.value}</div>)}
        </div>
        <div className="ea-actions">
          <Button variant="primary" icon={<Check />} disabled={!canApprove}>Approve final</Button>
          <Button variant="outline" icon={<MessageSquare />}>Add comment</Button>
        </div>
      </div>
    </section>
  );
}

function ReportsView() {
  const reported = portal.campaigns.find((campaign) => campaign.status === "reporting") ?? portal.campaigns[0];
  const inventory = portal.inventory.find((item) => item.creator === reported.creator) ?? portal.inventory[0];

  return (
    <section className="ea-grid ea-grid--wide-right">
      <div className="ea-panel">
        <div className="ea-panel__head">
          <div>
            <h2>Campaign report</h2>
            <p>{reported.name} / {reported.flight}</p>
          </div>
          <BarChart3 size={18} strokeWidth={1.75} />
        </div>
        <div className="ea-report">
          <Metric label="Spend" value={money(reported.committedSpendCents)} />
          <Metric label="Delivered QV" value={compactNumber(reported.deliveredQualifiedViewers)} detail={`${compactNumber(reported.forecastQualifiedViewers)} forecast`} />
          <Metric label="Verified score" value={`${inventory.attention.verifiedViewerScore}`} />
          <Metric label="Avg watch" value={`${inventory.attention.averageWatchMinutes}m`} />
        </div>
        <div className="ea-card ea-card--compact">
          <strong>Outcome</strong>
          <p>This purchased campaign beat forecast and produced repeat exposure in a category-fit audience.</p>
        </div>
      </div>
      <div className="ea-panel">
        <div className="ea-panel__head">
          <div>
            <h2>Proof</h2>
            <p>Measurement details for the media already purchased.</p>
          </div>
          <ShieldCheck size={18} strokeWidth={1.75} />
        </div>
        <div className="ea-facts">
          <div><span className="ea-label mono">Algorithm</span>{inventory.attention.algorithmVersion}</div>
          <div><span className="ea-label mono">Sessions</span>{compactNumber(inventory.attention.measuredSessions)}</div>
          <div><span className="ea-label mono">Confidence</span>{percent(inventory.attention.dataConfidence)}</div>
          <div><span className="ea-label mono">Attribution</span>UTM link and promo-code window</div>
        </div>
      </div>
    </section>
  );
}

function AccountView({
  account,
  setAccount,
}: {
  readonly account: AdvertiserAccount;
  readonly setAccount: Dispatch<SetStateAction<AdvertiserAccount>>;
}) {
  const canManageAccount = hasPermission(account, "manage_account");
  const canManageTeam = hasPermission(account, "manage_team");
  const [inviteEmail, setInviteEmail] = useState("");
  const [inviteName, setInviteName] = useState("");
  const [inviteRole, setInviteRole] = useState<AdvertiserRole>("buyer");

  const presetFor = (role: AdvertiserRole) => account.permissionPresets.find((preset) => preset.role === role);

  const updateSeatRole = (userId: string, role: AdvertiserRole) => {
    const permissions = presetFor(role)?.permissions ?? [];
    setAccount((current) => ({
      ...current,
      currentSeat: current.currentSeat.userId === userId ? { ...current.currentSeat, role, permissions } : current.currentSeat,
      seats: current.seats.map((seat) => (seat.userId === userId ? { ...seat, role, permissions } : seat)),
    }));
  };

  const updateSeatStatus = (userId: string, status: "active" | "suspended") => {
    setAccount((current) => ({
      ...current,
      currentSeat: current.currentSeat.userId === userId ? { ...current.currentSeat, status } : current.currentSeat,
      seats: current.seats.map((seat) => (seat.userId === userId ? { ...seat, status } : seat)),
    }));
  };

  const createInvite = () => {
    const email = inviteEmail.trim().toLowerCase();
    if (!email.includes("@")) return;
    const preset = presetFor(inviteRole);
    setAccount((current) => ({
      ...current,
      invites: [
        {
          id: `adv-invite-${Date.now()}`,
          email,
          role: inviteRole,
          permissions: preset?.permissions ?? [],
          status: "pending",
          invitedByUserId: current.currentSeat.userId,
          createdAt: new Date().toISOString(),
          expiresAt: new Date(Date.now() + 14 * 24 * 60 * 60 * 1000).toISOString(),
        },
        ...current.invites,
      ],
    }));
    setInviteEmail("");
    setInviteName("");
  };

  return (
    <section className="ea-stack">
      <div className="ea-metrics">
        <Metric label="Active seats" value={`${account.seats.filter((seat) => seat.status === "active").length}`} detail="Company access" icon={<Users />} />
        <Metric label="Pending invites" value={`${account.invites.filter((invite) => invite.status === "pending").length}`} detail="14 day expiry" icon={<UserPlus />} />
        <Metric label="Current role" value={presetFor(account.currentSeat.role)?.label ?? account.currentSeat.role} detail={account.currentSeat.name} icon={<LockKeyhole />} />
        <Metric label="Billing" value={account.company.billingStatus} detail={account.company.billingEmail} icon={<CreditCard />} />
      </div>
      <div className="ea-panel">
        <div className="ea-panel__head">
          <div>
            <h2>Company profile</h2>
            <p>Shared advertiser identity, billing owner, and buying permissions.</p>
          </div>
          <Building2 size={18} strokeWidth={1.75} />
        </div>
        <div className="ea-grid ea-account-grid">
          <div className="ea-facts">
            <div><span className="ea-label mono">Advertiser</span>{account.company.name}</div>
            <div><span className="ea-label mono">Industry</span>{account.company.industry}</div>
            <div><span className="ea-label mono">Website</span>{account.company.websiteUrl ?? "Not set"}</div>
            <div><span className="ea-label mono">Billing</span>{account.company.billingName}</div>
          </div>
          <div className="ea-facts">
            <div><span className="ea-label mono">Signed in</span>{account.currentSeat.name}</div>
            <div><span className="ea-label mono">Email</span>{account.currentSeat.email}</div>
            <div><span className="ea-label mono">Role</span>{presetFor(account.currentSeat.role)?.label ?? account.currentSeat.role}</div>
            <div><span className="ea-label mono">Status</span>{account.currentSeat.status}</div>
          </div>
        </div>
        {!canManageAccount ? <p className="ea-muted ea-note">This seat can view company information but cannot edit account or billing settings.</p> : null}
      </div>
      <div className="ea-panel">
        <div className="ea-panel__head">
          <div>
            <h2>Team seats</h2>
            <p>Give buyers, analysts, and reviewers exactly the access they need.</p>
          </div>
          <Users size={18} strokeWidth={1.75} />
        </div>
        <div className="ea-seat-list">
          {account.seats.map((seat) => (
            <article key={seat.userId} className="ea-seat">
              <div>
                <strong>{seat.name}</strong>
                <p>{seat.email}</p>
              </div>
              <div className="ea-order-form ea-seat__controls">
                <label>
                  <span className="ea-label mono">Role</span>
                  <select value={seat.role} disabled={!canManageTeam} onChange={(event) => updateSeatRole(seat.userId, event.target.value as AdvertiserRole)}>
                    {account.permissionPresets.map((preset) => <option key={preset.role} value={preset.role}>{preset.label}</option>)}
                  </select>
                </label>
                <label>
                  <span className="ea-label mono">Status</span>
                  <select value={seat.status} disabled={!canManageTeam || seat.userId === account.currentSeat.userId} onChange={(event) => updateSeatStatus(seat.userId, event.target.value as "active" | "suspended")}>
                    <option value="active">Active</option>
                    <option value="suspended">Suspended</option>
                  </select>
                </label>
              </div>
              <div className="ea-permission-row">
                {seat.permissions.map((permission) => <Badge key={permission} tone="neutral">{permissionLabels[permission]}</Badge>)}
              </div>
            </article>
          ))}
        </div>
      </div>
      <section className="ea-grid">
        <div className="ea-panel">
          <div className="ea-panel__head">
            <div>
              <h2>Invite seat</h2>
              <p>Pending invites inherit a preset permission bundle from the chosen role.</p>
            </div>
            <UserPlus size={18} strokeWidth={1.75} />
          </div>
          <div className="ea-order-form ea-order-form--invite">
            <label>
              <span className="ea-label mono">Name</span>
              <input value={inviteName} disabled={!canManageTeam} onChange={(event) => setInviteName(event.target.value)} placeholder="Optional" />
            </label>
            <label>
              <span className="ea-label mono">Email</span>
              <input value={inviteEmail} disabled={!canManageTeam} onChange={(event) => setInviteEmail(event.target.value)} placeholder="person@company.com" />
            </label>
            <label>
              <span className="ea-label mono">Role</span>
              <select value={inviteRole} disabled={!canManageTeam} onChange={(event) => setInviteRole(event.target.value as AdvertiserRole)}>
                {account.permissionPresets.map((preset) => <option key={preset.role} value={preset.role}>{preset.label}</option>)}
              </select>
            </label>
            <Button variant="primary" icon={<UserPlus />} disabled={!canManageTeam || !inviteEmail.includes("@")} onClick={createInvite}>Send invite</Button>
          </div>
          {!canManageTeam ? <p className="ea-muted ea-note">Only admins with Manage team permission can invite or change seats.</p> : null}
        </div>
        <div className="ea-panel">
          <div className="ea-panel__head">
            <div>
              <h2>Pending access</h2>
              <p>Invites can be accepted into the same advertiser company account.</p>
            </div>
            <ClipboardCheck size={18} strokeWidth={1.75} />
          </div>
          <div className="ea-list">
            {account.invites.map((invite) => (
              <article key={invite.id} className="ea-card ea-invite">
                <span>
                  <strong>{invite.email}</strong>
                  <em>Expires {compactDate(invite.expiresAt)}</em>
                </span>
                <Badge tone={statusTone(invite.status)}>{invite.status}</Badge>
                <span>{presetFor(invite.role)?.label ?? invite.role}</span>
              </article>
            ))}
          </div>
        </div>
      </section>
    </section>
  );
}

export function App() {
  const [view, setView] = useState<PortalView>(() => viewFromHash());
  const [account, setAccount] = useState<AdvertiserAccount>(portal.account);
  const [selectedCreatorId, setSelectedCreatorId] = useState(portal.inventory[0].id);
  const [lines, setLines] = useState<ReadonlyArray<CartLine>>(() => initialCart());
  const [orders, setOrders] = useState<ReadonlyArray<Order>>(() => initialOrders());
  const cartQuote = useMemo(() => quoteFor(lines), [lines]);
  const cartUnits = lines.reduce((sum, line) => sum + line.units, 0);
  const selectedCreator = portal.inventory.find((item) => item.id === selectedCreatorId) ?? portal.inventory[0];
  const canBuy = hasPermission(account, "buy_media");
  const canApprove = hasPermission(account, "approve_work");

  const navigateView = (nextView: PortalView) => {
    if (nextView === "overview") history.pushState(null, "", window.location.pathname);
    else window.location.hash = nextView;
    setView(nextView);
  };

  const openCreator = (item: InventoryItem) => {
    setSelectedCreatorId(item.id);
    navigateView("creator");
  };

  useEffect(() => {
    const syncHash = () => setView(viewFromHash());
    window.addEventListener("hashchange", syncHash);
    return () => window.removeEventListener("hashchange", syncHash);
  }, []);

  const addToCart = (item: InventoryItem) => {
    setLines((current) => [...current, defaultLine(item)]);
    navigateView("cart");
  };

  const updateLine = (id: string, patch: Partial<CartLine>) => {
    setLines((current) => current.map((line) => (line.id === id ? { ...line, ...patch } : line)));
  };

  const placeOrder = (paymentMethod: string) => {
    const quote = quoteFor(lines);
    const order: Order = {
      id: `AGENCY-${new Date().toISOString().slice(0, 10).replaceAll("-", "")}-${String(orders.length + 1).padStart(3, "0")}`,
      createdAt: new Date().toISOString(),
      advertiser: account.company.name,
      lines,
      subtotalCents: quote.subtotalCents,
      serviceCents: quote.serviceCents,
      totalCents: quote.totalCents,
      paymentMethod,
      status: lines.some(needsSalesReview) ? "sales_review" : "paid",
    };
    setOrders((current) => [order, ...current]);
    setLines([]);
    navigateView("orders");
  };

  const title = view === "creator" ? selectedCreator.creator : views.find((item) => item.id === view)?.label ?? "Overview";

  return (
    <div className="ea-app grain">
      <aside className="ea-shell">
        <div className="ea-brand">
          <span className="mono">VANTA</span>
          <strong>Agency</strong>
        </div>
        <nav className="ea-nav" aria-label="VANTA Agency">
          {views.map((item) => (
            <button key={item.id} type="button" className={view === item.id ? "is-active" : ""} onClick={() => navigateView(item.id)}>
              {item.label}
            </button>
          ))}
        </nav>
        <div className="ea-account">
          <span className="ea-label mono">{account.currentSeat.role}</span>
          <strong>{account.company.name}</strong>
          <p>{cartUnits} units / {money(cartQuote.totalCents)}</p>
        </div>
      </aside>
      <main className="ea-main">
        <header className="ea-head">
          <div>
            <span className="ea-kicker mono">VANTA Agency desk</span>
            <h1>{title}</h1>
            <p>Shop creator media packages, checkout, and manage the campaign work that follows.</p>
          </div>
          <Button variant={lines.length > 0 ? "primary" : "outline"} icon={<ShoppingCart />} onClick={() => navigateView(lines.length > 0 ? "cart" : "creators")}>
            {lines.length > 0 ? `${cartUnits} units / ${money(cartQuote.totalCents)}` : "Browse creators"}
          </Button>
        </header>

        {view === "overview" ? (
          <OverviewView
            lines={lines}
            openCreators={() => navigateView("creators")}
            openNiches={() => navigateView("niches")}
            openStats={() => navigateView("stats")}
            openCreator={openCreator}
          />
        ) : null}
        {view === "creators" ? <CreatorsView openCreator={openCreator} /> : null}
        {view === "niches" ? <NichesView openCreator={openCreator} /> : null}
        {view === "stats" ? <StatsView /> : null}
        {view === "creator" ? <CreatorDetailView item={selectedCreator} addToCart={addToCart} canBuy={canBuy} /> : null}
        {view === "cart" ? (
          <CartView
            lines={lines}
            updateLine={updateLine}
            removeLine={(id) => setLines((current) => current.filter((line) => line.id !== id))}
            goShop={() => navigateView("creators")}
            placeOrder={placeOrder}
            addUpsell={(item) => setLines((current) => [...current, defaultLine(item)])}
            canBuy={canBuy}
          />
        ) : null}
        {view === "orders" ? <OrdersView orders={orders} goShop={() => navigateView("creators")} /> : null}
        {view === "approvals" ? <ApprovalsView canApprove={canApprove} /> : null}
        {view === "review" ? <ReviewView canApprove={canApprove} /> : null}
        {view === "reports" ? <ReportsView /> : null}
        {view === "account" ? <AccountView account={account} setAccount={setAccount} /> : null}
      </main>
    </div>
  );
}
