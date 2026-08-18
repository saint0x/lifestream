import { useMemo } from "react";
import { Download, CreditCard, ArrowDownRight, ArrowUpRight } from "lucide-react";
import { CreatorLayout } from "@/components/creator/CreatorLayout";
import { StatCard } from "@/components/creator/StatCard";
import { Button } from "@/components/ui/Button";
import { repository } from "@/lib/repository";
import type { RevenueEntry } from "@/types";
import "./Creator.css";
import "./CreatorRevenuePage.css";

const sourceLabels: Record<string, string> = {
  subscriptions: "Subscriptions",
  ads: "Ads",
  tips: "Tips",
  clips: "Clips",
  payout: "Payout",
};

const sourceAccents: Record<string, string> = {
  subscriptions: "#ffd83d",
  ads: "#4ea1ff",
  tips: "#3dffb5",
  clips: "#ff3d7a",
  payout: "#9b6bff",
};

export function CreatorRevenuePage() {
  const profile = repository.getCreatorProfile();
  const entries = repository.listRevenue();
  const analytics = repository.getAnalytics();

  const revenue30d = analytics.reduce((s, p) => s + p.revenue, 0);
  const revenue7 = analytics.slice(-7).map((p) => p.revenue);

  const breakdown = useMemo(() => {
    const totals: Record<string, number> = {};
    for (const e of entries) {
      if (e.amount > 0) totals[e.source] = (totals[e.source] ?? 0) + e.amount;
    }
    const sum = Object.values(totals).reduce((s, v) => s + v, 0) || 1;
    return Object.entries(totals).map(
      ([source, amount]) => ({
        source,
        amount,
        share: amount / sum,
      }),
    );
  }, [entries]);

  const payouts = entries.filter((e) => e.source === "payout");
  const nextPayoutAmount = revenue30d - payouts.reduce((s, e) => s + Math.abs(e.amount), 0);

  return (
    <CreatorLayout>
      <div className="ls-cpage">
        <header className="ls-cpage__head">
          <div>
            <h1 className="ls-cpage__title">Revenue</h1>
            <p className="ls-cpage__sub">
              Earnings, subscribers, payouts. Numbers shown are net of the LIFESTREAM
              platform cut and any applicable taxes withheld.
            </p>
          </div>
          <div className="ls-crev__actions">
            <Button variant="ghost" icon={<Download />}>Statements</Button>
            <Button variant="outline" icon={<CreditCard />}>Payout settings</Button>
          </div>
        </header>

        <section className="ls-cpage__stat-grid">
          <StatCard
            label="30d earnings"
            value={`$${revenue30d.toLocaleString("en-US", { maximumFractionDigits: 0 })}`}
            delta={22.8}
            spark={revenue7}
            accent="#3dffb5"
            footer="after platform cut"
          />
          <StatCard
            label="Subscribers"
            value={profile.subscribers.toLocaleString()}
            delta={6.2}
            accent="#ffd83d"
            footer="tier 1 + 2 + 3"
          />
          <StatCard
            label="Avg tier"
            value="$7.40"
            delta={1.4}
            accent="#ff3d7a"
            footer="blended monthly ARPU"
          />
          <StatCard
            label="Next payout"
            value={`$${Math.max(nextPayoutAmount, 0).toFixed(2)}`}
            accent="#9b6bff"
            footer="Friday · bank •••• 4821"
          />
        </section>

        <div className="ls-cpage__split">
          <section className="ls-crev__panel">
            <div className="ls-cpage__card-title">Revenue breakdown · this month</div>
            <div className="ls-crev__breakdown">
              {breakdown.map((b) => (
                <div key={b.source} className="ls-crev__break-row">
                  <div className="ls-crev__break-main">
                    <div className="ls-crev__break-label">
                      <span
                        className="ls-crev__break-dot"
                        style={{ background: sourceAccents[b.source] ?? "#9b6bff" }}
                      />
                      {sourceLabels[b.source] ?? b.source}
                    </div>
                    <div className="ls-crev__break-bar">
                      <div
                        className="ls-crev__break-fill"
                        style={{
                          width: `${b.share * 100}%`,
                          background: sourceAccents[b.source],
                        }}
                      />
                    </div>
                  </div>
                  <div className="ls-crev__break-values mono">
                    <span>${b.amount.toFixed(2)}</span>
                    <span className="faint">{(b.share * 100).toFixed(0)}%</span>
                  </div>
                </div>
              ))}
            </div>
          </section>

          <section className="ls-crev__panel">
            <div className="ls-cpage__card-title">Subscriber tiers</div>
            <div className="ls-crev__tiers">
              {[
                { tier: "Tier 1", price: 4.99, subs: 2_412, color: "#4ea1ff" },
                { tier: "Tier 2", price: 9.99, subs: 812, color: "#ffd83d" },
                { tier: "Tier 3", price: 24.99, subs: 188, color: "#ff3d7a" },
              ].map((t) => (
                <div key={t.tier} className="ls-crev__tier">
                  <div
                    className="ls-crev__tier-band"
                    style={{ background: t.color }}
                  />
                  <div>
                    <div className="ls-crev__tier-name">{t.tier}</div>
                    <div className="ls-crev__tier-meta mono">
                      ${t.price.toFixed(2)} / mo
                    </div>
                  </div>
                  <div className="ls-crev__tier-subs">
                    <div className="ls-crev__tier-count">{t.subs.toLocaleString()}</div>
                    <div className="ls-crev__tier-lbl mono">SUBS</div>
                  </div>
                  <div className="ls-crev__tier-rev mono">
                    ${(t.subs * t.price).toLocaleString("en-US", { maximumFractionDigits: 0 })}
                    <span className="faint"> /mo</span>
                  </div>
                </div>
              ))}
            </div>
          </section>
        </div>

        <section className="ls-crev__panel">
          <div className="ls-cpage__card-title">Transactions</div>
          <table className="ls-crev__table">
            <thead>
              <tr>
                <th>Date</th>
                <th>Source</th>
                <th>Description</th>
                <th className="num">Amount</th>
              </tr>
            </thead>
            <tbody>
              {entries.map((e) => (
                <tr key={e.id}>
                  <td className="mono">{e.date}</td>
                  <td>
                    <span
                      className="ls-crev__source-chip mono"
                      style={{
                        color: sourceAccents[e.source],
                        borderColor: `${sourceAccents[e.source]}40`,
                      }}
                    >
                      {sourceLabels[e.source]}
                    </span>
                  </td>
                  <td>{e.description}</td>
                  <td className={`num mono ${e.amount >= 0 ? "pos" : "neg"}`}>
                    {e.amount >= 0 ? (
                      <ArrowUpRight size={11} />
                    ) : (
                      <ArrowDownRight size={11} />
                    )}
                    ${Math.abs(e.amount).toFixed(2)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      </div>
    </CreatorLayout>
  );
}
