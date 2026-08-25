import { useCallback, useEffect, useMemo, useState } from "react";
import {
  BadgeDollarSign,
  CalendarClock,
  Check,
  ChevronDown,
  ClipboardCheck,
  ExternalLink,
  FileCheck2,
  Globe2,
  Layers3,
  RefreshCw,
  Send,
  ShieldCheck,
  Sparkles,
  X,
} from "lucide-react";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { PageMetadata } from "@/components/seo/PageMetadata";
import { PageTrail } from "@/components/navigation/PageTrail";
import { repository } from "@/lib/repository";
import type { AdMarketplaceOffer, AdMarketplaceSummary, CreatorAdHubResponse } from "@/types";
import "./AdHubPage.css";

interface SubmissionForm {
  readonly submissionUrl: string;
  readonly notes: string;
}

interface AdvertiserVisual {
  readonly image: string;
  readonly eyebrow: string;
  readonly fit: string;
}

function money(cents: number, currency: string): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency,
    maximumFractionDigits: 0,
  }).format(cents / 100);
}

function compactDate(value?: string | null): string {
  if (!value) return "No due date";
  return new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
    timeZone: "UTC",
  }).format(new Date(value));
}

function summarizeOffers(offers: ReadonlyArray<AdMarketplaceOffer>): AdMarketplaceSummary {
  const billableOffers = offers.filter((offer) => offer.status !== "declined");
  return {
    pendingOffers: offers.filter((offer) => offer.status === "pending").length,
    activeOffers: offers.filter((offer) => offer.status === "accepted").length,
    inReviewOffers: offers.filter((offer) => offer.status === "in_review").length,
    approvedOffers: offers.filter((offer) => offer.status === "approved").length,
    declinedOffers: offers.filter((offer) => offer.status === "declined").length,
    totalOfferAmountCents: billableOffers.reduce((sum, offer) => sum + offer.offerAmountCents, 0),
    totalCreatorPayoutCents: billableOffers.reduce((sum, offer) => sum + offer.creatorPayoutCents, 0),
    currency: offers[0]?.currency ?? "USD",
  };
}

function statusTone(status: string): "neutral" | "new" | "premium" | "live" {
  if (status === "pending") return "new";
  if (status === "accepted" || status === "approved") return "premium";
  if (status === "in_review") return "live";
  return "neutral";
}

function visualForOffer(offer: AdMarketplaceOffer | null): AdvertiserVisual {
  const source = `${offer?.advertiser.name ?? ""} ${offer?.advertiser.industry ?? ""} ${offer?.title ?? ""} ${offer?.campaign.objective ?? ""}`.toLowerCase();
  if (source.includes("outdoor") || source.includes("trail") || source.includes("gear")) {
    return {
      image: "/ad-hub/outdoor-gear.png",
      eyebrow: "Outdoor gear partner",
      fit: "Best fit for practical product proof, field use, and high-trust host integration.",
    };
  }
  if (source.includes("auto") || source.includes("power") || source.includes("overland")) {
    return {
      image: "/ad-hub/portable-power.png",
      eyebrow: "Consumer hardware partner",
      fit: "Best fit for demonstration-heavy stories where audience confidence matters.",
    };
  }
  return {
    image: "/ad-hub/developer-tools.png",
    eyebrow: "Developer tooling partner",
    fit: "Best fit for technical audiences, workflow credibility, and thoughtful mid-roll reads.",
  };
}

function payoutRate(offer: AdMarketplaceOffer): number {
  if (offer.offerAmountCents <= 0) return 0;
  return Math.round((offer.creatorPayoutCents / offer.offerAmountCents) * 100);
}

function campaignWindow(offer: AdMarketplaceOffer): string {
  const start = compactDate(offer.campaign.startsAt);
  const end = compactDate(offer.campaign.endsAt);
  if (start === "No due date" && end === "No due date") return "Flexible flight";
  return `${start} - ${end}`;
}

function primaryOffer(offers: ReadonlyArray<AdMarketplaceOffer>, selected: AdMarketplaceOffer | null): AdMarketplaceOffer | null {
  return selected ?? offers.find((offer) => offer.status === "pending") ?? offers[0] ?? null;
}

const emptyForm: SubmissionForm = {
  submissionUrl: "",
  notes: "",
};

const emptyOffers: ReadonlyArray<AdMarketplaceOffer> = [];

export function AdHubPage() {
  const [hub, setHub] = useState<CreatorAdHubResponse | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [form, setForm] = useState<SubmissionForm>(emptyForm);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [expandedPackageId, setExpandedPackageId] = useState<string | null>(null);

  const offers = hub?.offers ?? emptyOffers;
  const selectedOffer = useMemo(
    () => primaryOffer(offers, offers.find((offer) => offer.id === selectedId) ?? null),
    [offers, selectedId],
  );
  const visual = visualForOffer(selectedOffer);

  const loadHub = useCallback(async (signal?: AbortSignal) => {
    setError(null);
    const nextHub = await repository.fetchAdHub(signal);
    setHub(nextHub);
    setSelectedId((current) => {
      if (current && nextHub.offers.some((offer) => offer.id === current)) return current;
      return nextHub.offers.find((offer) => offer.status === "pending")?.id ?? nextHub.offers[0]?.id ?? null;
    });
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    void loadHub(controller.signal)
      .catch((err) => {
        if (!controller.signal.aborted) {
          setError(err instanceof Error ? err.message : "Unable to load Ad Hub.");
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [loadHub]);

  const mergeOffer = (updated: AdMarketplaceOffer) => {
    setHub((current) => current
      ? (() => {
          const offers = current.offers.map((offer) => (offer.id === updated.id ? updated : offer));
          return { ...current, offers, summary: summarizeOffers(offers) };
        })()
      : current);
    setSelectedId(updated.id);
  };

  const acceptOffer = async (offer: AdMarketplaceOffer) => {
    setSaving(true);
    setStatus(null);
    setError(null);
    try {
      mergeOffer(await repository.acceptAdOffer(offer.id));
      setStatus("Offer accepted.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unable to accept offer.");
    } finally {
      setSaving(false);
    }
  };

  const declineOffer = async (offer: AdMarketplaceOffer) => {
    setSaving(true);
    setStatus(null);
    setError(null);
    try {
      mergeOffer(await repository.declineAdOffer(offer.id));
      setStatus("Offer declined.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unable to decline offer.");
    } finally {
      setSaving(false);
    }
  };

  const submitReview = async (offer: AdMarketplaceOffer) => {
    const submissionUrl = form.submissionUrl.trim();
    if (!submissionUrl) {
      setError("Add a review link before submitting.");
      return;
    }
    setSaving(true);
    setStatus(null);
    setError(null);
    try {
      mergeOffer(await repository.submitAdOfferReview(offer.id, {
        submissionUrl,
        notes: form.notes.trim() || undefined,
      }));
      setForm(emptyForm);
      setStatus("Submission sent for advertiser review.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unable to submit review.");
    } finally {
      setSaving(false);
    }
  };

  const summary = hub?.summary;
  const canSubmit = selectedOffer?.status === "accepted" || selectedOffer?.status === "in_review";

  return (
    <div className="ls-ad-hub">
      <PageMetadata
        title="VANTA Ad Hub - Creator advertising offers"
        description="VANTA Ad Hub lets creators review sponsorship offers, manage ad deliverables, and submit campaign proof for premium long-form episodic content inventory."
        path="/ad-hub"
        structuredData={{
          "@context": "https://schema.org",
          "@type": "WebApplication",
          name: "VANTA Ad Hub",
          applicationCategory: "BusinessApplication",
          operatingSystem: "Web",
          description:
            "Creator-side advertising marketplace hub for sponsorship offers, ad deliverables, campaign proof, and premium long-form episodic content inventory.",
          provider: {
            "@type": "Organization",
            name: "VANTA",
            url: "https://streamvanta.tv/",
          },
          audience: {
            "@type": "Audience",
            audienceType: "Creators and streamers",
          },
        }}
      />

      <header className="ls-ad-hub__head">
        <PageTrail
          className="ls-ad-hub__kicker mono"
          items={[
            { label: "Dashboard", href: "/" },
            { label: "Ad Hub" },
          ]}
        />
        <div className="ls-ad-hub__title-row">
          <div>
            <h1 className="ls-ad-hub__title">Ad Hub</h1>
            <p className="ls-ad-hub__sub">
              Preview advertiser partners, choose the right offers, and move accepted work through review.
            </p>
          </div>
          <Button
            variant="outline"
            icon={<RefreshCw />}
            onClick={() => {
              setLoading(true);
              void loadHub().finally(() => setLoading(false));
            }}
            disabled={loading || saving}
          >
            Refresh
          </Button>
        </div>
      </header>

      {status ? <div className="ls-ad-hub__notice"><Check size={14} />{status}</div> : null}
      {error ? <div className="ls-ad-hub__error">{error}</div> : null}

      <section className="ls-ad-hub__hero">
        <img src={visual.image} alt="" />
        <div className="ls-ad-hub__hero-copy">
          <span className="ls-ad-hub__eyebrow mono">{visual.eyebrow}</span>
          <h2>{selectedOffer?.advertiser.name ?? "Advertiser opportunities"}</h2>
          <p>{selectedOffer?.brief ?? "New sponsor offers will appear here with the brand context creators need before saying yes."}</p>
          <div className="ls-ad-hub__hero-actions">
            {selectedOffer ? (
              <>
                <Button
                  variant="primary"
                  icon={<Check />}
                  disabled={saving || selectedOffer.status !== "pending"}
                  onClick={() => void acceptOffer(selectedOffer)}
                >
                  Accept offer
                </Button>
                <Button
                  variant="outline"
                  icon={<ExternalLink />}
                  disabled={!selectedOffer.advertiser.websiteUrl}
                  onClick={() => {
                    if (selectedOffer.advertiser.websiteUrl) window.open(selectedOffer.advertiser.websiteUrl, "_blank", "noopener,noreferrer");
                  }}
                >
                  Visit company
                </Button>
              </>
            ) : null}
          </div>
        </div>
        {selectedOffer ? (
          <div className="ls-ad-hub__hero-proof">
            <span className="mono">Creator payout</span>
            <strong>{money(selectedOffer.creatorPayoutCents, selectedOffer.currency)}</strong>
            <em>{payoutRate(selectedOffer)}% of gross offer</em>
          </div>
        ) : null}
      </section>

      <section className="ls-ad-hub__metrics">
        <div className="ls-ad-hub__metric">
          <span className="mono">Pending</span>
          <strong>{summary?.pendingOffers ?? 0}</strong>
          <em>Needs a yes or no</em>
        </div>
        <div className="ls-ad-hub__metric">
          <span className="mono">Active</span>
          <strong>{summary?.activeOffers ?? 0}</strong>
          <em>Accepted work</em>
        </div>
        <div className="ls-ad-hub__metric">
          <span className="mono">In review</span>
          <strong>{summary?.inReviewOffers ?? 0}</strong>
          <em>With advertisers</em>
        </div>
        <div className="ls-ad-hub__metric">
          <span className="mono">Creator payout</span>
          <strong>{money(summary?.totalCreatorPayoutCents ?? 0, summary?.currency ?? "USD")}</strong>
          <em>Open non-declined value</em>
        </div>
      </section>

      <section className="ls-ad-hub__layout">
        <div className="ls-ad-hub__panel ls-ad-hub__panel--offers">
          <div className="ls-ad-hub__panel-head">
            <div>
              <h2>Advertiser offers</h2>
              <p>{loading ? "Loading..." : `${offers.length} companies pitching your audience`}</p>
            </div>
            <BadgeDollarSign size={18} strokeWidth={1.75} />
          </div>

          <div className="ls-ad-hub__offers">
            {offers.length === 0 && !loading ? (
              <div className="ls-ad-hub__empty">No advertiser offers yet.</div>
            ) : null}
            {offers.map((offer) => {
              const offerVisual = visualForOffer(offer);
              return (
                <button
                  key={offer.id}
                  type="button"
                  className={`ls-ad-hub__offer ${offer.id === selectedOffer?.id ? "is-active" : ""}`}
                  onClick={() => setSelectedId(offer.id)}
                >
                  <img src={offerVisual.image} alt="" />
                  <span className="ls-ad-hub__offer-copy">
                    <span className="ls-ad-hub__offer-top">
                      <span className="ls-ad-hub__offer-title">{offer.advertiser.name}</span>
                      <Badge tone={statusTone(offer.status)}>{offer.status.replace("_", " ")}</Badge>
                    </span>
                    <span className="ls-ad-hub__offer-meta mono">
                      {offer.package.title} / {compactDate(offer.dueAt)}
                    </span>
                    <span className="ls-ad-hub__offer-bottom">
                      <strong>{money(offer.creatorPayoutCents, offer.currency)}</strong>
                      <em>{offer.advertiser.industry}</em>
                    </span>
                  </span>
                </button>
              );
            })}
          </div>
        </div>

        <div className="ls-ad-hub__panel ls-ad-hub__panel--detail">
          <div className="ls-ad-hub__panel-head">
            <div>
              <h2>Company fit</h2>
              <p>{selectedOffer ? selectedOffer.campaign.name : "Select an advertiser."}</p>
            </div>
            <Sparkles size={18} strokeWidth={1.75} />
          </div>

          {selectedOffer ? (
            <div className="ls-ad-hub__detail">
              <div className="ls-ad-hub__brand-card">
                <img src={visual.image} alt="" />
                <div>
                  <span className="mono">{selectedOffer.advertiser.industry}</span>
                  <h3>{selectedOffer.title}</h3>
                  <p>{visual.fit}</p>
                </div>
              </div>

              <div className="ls-ad-hub__facts">
                <div><Globe2 size={14} /><span className="mono">Advertiser</span>{selectedOffer.advertiser.name}</div>
                <div><Layers3 size={14} /><span className="mono">Package</span>{selectedOffer.package.title}</div>
                <div><BadgeDollarSign size={14} /><span className="mono">Gross offer</span>{money(selectedOffer.offerAmountCents, selectedOffer.currency)}</div>
                <div><BadgeDollarSign size={14} /><span className="mono">Creator payout</span>{money(selectedOffer.creatorPayoutCents, selectedOffer.currency)}</div>
                <div><CalendarClock size={14} /><span className="mono">Due</span>{compactDate(selectedOffer.dueAt)}</div>
                <div><FileCheck2 size={14} /><span className="mono">Review</span>{selectedOffer.advertiserReviewStatus.replace("_", " ")}</div>
              </div>

              <div className="ls-ad-hub__brief">
                <h4>What the company wants</h4>
                <p>{selectedOffer.brief}</p>
              </div>

              <div className="ls-ad-hub__requirements">
                <h4>Creator deliverables</h4>
                {selectedOffer.requirements.map((item) => (
                  <div key={item}><ShieldCheck size={14} />{item}</div>
                ))}
              </div>

              <div className="ls-ad-hub__actions">
                <Button
                  variant="primary"
                  icon={<Check />}
                  disabled={saving || selectedOffer.status !== "pending"}
                  onClick={() => void acceptOffer(selectedOffer)}
                >
                  Accept
                </Button>
                <Button
                  variant="outline"
                  icon={<X />}
                  disabled={saving || selectedOffer.status === "declined" || selectedOffer.status === "approved"}
                  onClick={() => void declineOffer(selectedOffer)}
                >
                  Decline
                </Button>
              </div>
            </div>
          ) : (
            <div className="ls-ad-hub__empty">Select an advertiser offer.</div>
          )}
        </div>

        <div className="ls-ad-hub__panel ls-ad-hub__panel--submit">
          <div className="ls-ad-hub__panel-head">
            <div>
              <h2>Submit for review</h2>
              <p>{selectedOffer ? `${selectedOffer.advertiser.name} creative approval` : "No offer selected"}</p>
            </div>
            <Send size={18} strokeWidth={1.75} />
          </div>

          {selectedOffer ? (
            <div className="ls-ad-hub__form">
              <label className="ls-ad-hub__field">
                <span className="mono">Review link</span>
                <Input
                  value={form.submissionUrl}
                  onChange={(event) => setForm((current) => ({ ...current, submissionUrl: event.target.value }))}
                  placeholder="https://..."
                  disabled={!canSubmit}
                />
              </label>
              <label className="ls-ad-hub__field">
                <span className="mono">Notes</span>
                <textarea
                  className="ls-ad-hub__textarea"
                  value={form.notes}
                  onChange={(event) => setForm((current) => ({ ...current, notes: event.target.value }))}
                  placeholder="Context for advertiser review"
                  disabled={!canSubmit}
                />
              </label>
              <Button
                variant="primary"
                icon={<Send />}
                disabled={saving || !canSubmit}
                onClick={() => void submitReview(selectedOffer)}
              >
                Submit review
              </Button>

              <div className="ls-ad-hub__submissions">
                {selectedOffer.submissions.length === 0 ? (
                  <div className="ls-ad-hub__empty">No submissions yet.</div>
                ) : null}
                {selectedOffer.submissions.map((submission) => (
                  <div key={submission.id} className="ls-ad-hub__submission">
                    <span>{submission.submissionUrl}</span>
                    <span className="mono">{submission.status} / {compactDate(submission.submittedAt)}</span>
                  </div>
                ))}
              </div>
            </div>
          ) : (
            <div className="ls-ad-hub__empty">Select an offer first.</div>
          )}
        </div>

        <div className="ls-ad-hub__panel ls-ad-hub__panel--packages">
          <div className="ls-ad-hub__panel-head">
            <div>
              <h2>Your sellable packages</h2>
              <p>{hub?.packages.length ?? 0} templates advertisers can buy</p>
            </div>
            <ClipboardCheck size={18} strokeWidth={1.75} />
          </div>
          <div className="ls-ad-hub__packages">
            {(hub?.packages ?? []).map((pkg) => (
              <button
                key={pkg.id}
                type="button"
                className={`ls-ad-hub__package ${expandedPackageId === pkg.id ? "is-expanded" : ""}`}
                aria-expanded={expandedPackageId === pkg.id}
                onClick={() => setExpandedPackageId((current) => (current === pkg.id ? null : pkg.id))}
              >
                <span className="ls-ad-hub__package-main">
                  <span>
                    <strong>{pkg.title}</strong>
                    <span className="mono">{pkg.placementKind.replace("_", " ")}</span>
                  </span>
                  <span className="ls-ad-hub__package-price">
                    {money(pkg.basePriceCents, pkg.currency)}
                    <ChevronDown size={15} strokeWidth={1.75} />
                  </span>
                </span>
                <span className="ls-ad-hub__package-detail" aria-hidden={expandedPackageId !== pkg.id}>
                  <span>{pkg.description}</span>
                  <span className="ls-ad-hub__package-facts">
                    <span><span className="mono">Code</span>{pkg.code}</span>
                    <span><span className="mono">Length</span>{pkg.spotLengthSeconds ? `${pkg.spotLengthSeconds}s` : "Flexible"}</span>
                    <span><span className="mono">Flight</span>{selectedOffer ? campaignWindow(selectedOffer) : "Advertiser-defined"}</span>
                  </span>
                  {pkg.deliverables.length > 0 ? (
                    <span className="ls-ad-hub__package-deliverables">
                      {pkg.deliverables.map((item) => (
                        <span key={item}><ShieldCheck size={13} />{item}</span>
                      ))}
                    </span>
                  ) : null}
                </span>
              </button>
            ))}
          </div>
        </div>
      </section>
    </div>
  );
}
