import { useCallback, useEffect, useMemo, useState } from "react";
import {
  BadgeDollarSign,
  Check,
  ChevronDown,
  ClipboardCheck,
  RefreshCw,
  Send,
  ShieldCheck,
  X,
} from "lucide-react";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { repository } from "@/lib/repository";
import type { AdMarketplaceOffer, AdMarketplaceSummary, CreatorAdHubResponse } from "@/types";
import "./AdHubPage.css";

interface SubmissionForm {
  readonly submissionUrl: string;
  readonly notes: string;
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
    () => offers.find((offer) => offer.id === selectedId) ?? offers[0] ?? null,
    [offers, selectedId],
  );

  const loadHub = useCallback(async (signal?: AbortSignal) => {
    setError(null);
    const nextHub = await repository.fetchAdHub(signal);
    setHub(nextHub);
    setSelectedId((current) => {
      if (current && nextHub.offers.some((offer) => offer.id === current)) return current;
      return nextHub.offers[0]?.id ?? null;
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
      <header className="ls-ad-hub__head">
        <div className="ls-ad-hub__kicker mono">/ creator / ad hub</div>
        <div className="ls-ad-hub__title-row">
          <div>
            <h1 className="ls-ad-hub__title">Ad Hub</h1>
            <p className="ls-ad-hub__sub">
              Review sponsorship offers, manage deliverables, and submit campaign proof.
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

      <section className="ls-ad-hub__metrics">
        <div className="ls-ad-hub__metric">
          <span className="mono">Pending</span>
          <strong>{summary?.pendingOffers ?? 0}</strong>
        </div>
        <div className="ls-ad-hub__metric">
          <span className="mono">Active</span>
          <strong>{summary?.activeOffers ?? 0}</strong>
        </div>
        <div className="ls-ad-hub__metric">
          <span className="mono">In review</span>
          <strong>{summary?.inReviewOffers ?? 0}</strong>
        </div>
        <div className="ls-ad-hub__metric">
          <span className="mono">Creator payout</span>
          <strong>{money(summary?.totalCreatorPayoutCents ?? 0, summary?.currency ?? "USD")}</strong>
        </div>
      </section>

      <section className="ls-ad-hub__layout">
        <div className="ls-ad-hub__panel">
          <div className="ls-ad-hub__panel-head">
            <div>
              <h2>Offers</h2>
              <p>{loading ? "Loading..." : `${offers.length} marketplace offers`}</p>
            </div>
            <BadgeDollarSign size={18} strokeWidth={1.75} />
          </div>

          <div className="ls-ad-hub__offers">
            {offers.length === 0 && !loading ? (
              <div className="ls-ad-hub__empty">No advertiser offers yet.</div>
            ) : null}
            {offers.map((offer) => (
              <button
                key={offer.id}
                type="button"
                className={`ls-ad-hub__offer ${offer.id === selectedOffer?.id ? "is-active" : ""}`}
                onClick={() => setSelectedId(offer.id)}
              >
                <span className="ls-ad-hub__offer-top">
                  <span className="ls-ad-hub__offer-title">{offer.title}</span>
                  <Badge tone={statusTone(offer.status)}>{offer.status.replace("_", " ")}</Badge>
                </span>
                <span className="ls-ad-hub__offer-meta mono">
                  {offer.advertiser.name} / {offer.package.title} / {compactDate(offer.dueAt)}
                </span>
                <span className="ls-ad-hub__offer-price">
                  {money(offer.creatorPayoutCents, offer.currency)}
                </span>
              </button>
            ))}
          </div>
        </div>

        <div className="ls-ad-hub__panel ls-ad-hub__panel--detail">
          <div className="ls-ad-hub__panel-head">
            <div>
              <h2>Offer details</h2>
              <p>{selectedOffer ? selectedOffer.campaign.name : "Select an offer."}</p>
            </div>
            <ClipboardCheck size={18} strokeWidth={1.75} />
          </div>

          {selectedOffer ? (
            <div className="ls-ad-hub__detail">
              <div className="ls-ad-hub__detail-title">
                <h3>{selectedOffer.title}</h3>
                <Badge tone={statusTone(selectedOffer.status)}>
                  {selectedOffer.status.replace("_", " ")}
                </Badge>
              </div>
              <p>{selectedOffer.brief}</p>

              <div className="ls-ad-hub__facts">
                <div><span className="mono">Advertiser</span>{selectedOffer.advertiser.name}</div>
                <div><span className="mono">Industry</span>{selectedOffer.advertiser.industry}</div>
                <div><span className="mono">Package</span>{selectedOffer.package.title}</div>
                <div><span className="mono">Placement</span>{selectedOffer.package.placementKind.replace("_", " ")}</div>
                <div><span className="mono">Gross offer</span>{money(selectedOffer.offerAmountCents, selectedOffer.currency)}</div>
                <div><span className="mono">Creator payout</span>{money(selectedOffer.creatorPayoutCents, selectedOffer.currency)}</div>
                <div><span className="mono">Due</span>{compactDate(selectedOffer.dueAt)}</div>
                <div><span className="mono">Review</span>{selectedOffer.advertiserReviewStatus.replace("_", " ")}</div>
              </div>

              <div className="ls-ad-hub__requirements">
                <h4>Requirements</h4>
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
              <p>{selectedOffer ? selectedOffer.advertiser.name : "No offer selected"}</p>
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
              <h2>Inventory packages</h2>
              <p>{hub?.packages.length ?? 0} sellable package templates</p>
            </div>
            <BadgeDollarSign size={18} strokeWidth={1.75} />
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
                {expandedPackageId === pkg.id ? (
                  <span className="ls-ad-hub__package-detail">
                    <span>{pkg.description}</span>
                    <span className="ls-ad-hub__package-facts">
                      <span><span className="mono">Code</span>{pkg.code}</span>
                      <span><span className="mono">Length</span>{pkg.spotLengthSeconds ? `${pkg.spotLengthSeconds}s` : "Flexible"}</span>
                      <span><span className="mono">Status</span>{pkg.status}</span>
                    </span>
                    {pkg.deliverables.length > 0 ? (
                      <span className="ls-ad-hub__package-deliverables">
                        {pkg.deliverables.map((item) => (
                          <span key={item}><ShieldCheck size={13} />{item}</span>
                        ))}
                      </span>
                    ) : null}
                  </span>
                ) : null}
              </button>
            ))}
          </div>
        </div>
      </section>
    </div>
  );
}
