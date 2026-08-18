import { useState, useMemo } from "react";
import { CreatorLayout } from "@/components/creator/CreatorLayout";
import { StatCard } from "@/components/creator/StatCard";
import { Sparkline } from "@/components/creator/Sparkline";
import { repository } from "@/lib/repository";
import { formatViewers } from "@/lib/format";
import "./Creator.css";
import "./CreatorAnalyticsPage.css";

type Range = "7d" | "30d" | "90d";

export function CreatorAnalyticsPage() {
  const [range, setRange] = useState<Range>("30d");
  const [metric, setMetric] = useState<"viewers" | "watchMinutes" | "revenue" | "newFollowers">(
    "viewers",
  );

  const analytics = repository.getAnalytics();
  const series = useMemo(() => {
    const n = range === "7d" ? 7 : range === "30d" ? 30 : 90;
    return analytics.slice(-Math.min(n, analytics.length));
  }, [analytics, range]);

  const traffic = repository.getTrafficSources();
  const top = repository.getTopContent();

  const values = series.map((p) => p[metric] as number);
  const total = values.reduce((s, v) => s + v, 0);
  const max = values.length > 0 ? Math.max(...values) : 0;
  const min = values.length > 0 ? Math.min(...values) : 0;
  const avg = values.length > 0 ? total / values.length : 0;
  const delta = values.length >= 2
    ? ((values[values.length - 1]! - values[0]!) / (values[0] || 1)) * 100
    : 0;

  const metricLabels = {
    viewers: "Unique viewers",
    watchMinutes: "Watch minutes",
    revenue: "Revenue",
    newFollowers: "New followers",
  } as const;

  const metricAccents = {
    viewers: "#4ea1ff",
    watchMinutes: "#ffd83d",
    revenue: "#3dffb5",
    newFollowers: "#ff3d7a",
  } as const;

  const format = (v: number): string => {
    if (metric === "revenue") return `$${Math.round(v).toLocaleString()}`;
    return formatViewers(v);
  };

  return (
    <CreatorLayout>
      <div className="ls-cpage">
        <header className="ls-cpage__head">
          <div>
            <h1 className="ls-cpage__title">Analytics</h1>
            <p className="ls-cpage__sub">
              Viewer, revenue, retention and traffic breakdown. All figures net of platform cut.
            </p>
          </div>
          <div className="ls-ca__ranges">
            {(["7d", "30d", "90d"] as const).map((r) => (
              <button
                key={r}
                type="button"
                className={`ls-ca__range ${range === r ? "is-active" : ""}`}
                onClick={() => setRange(r)}
              >
                {r.toUpperCase()}
              </button>
            ))}
          </div>
        </header>

        <section className="ls-cpage__stat-grid">
          <StatCard
            label="Total viewers"
            value={formatViewers(series.reduce((s, p) => s + p.viewers, 0))}
            delta={14.2}
            spark={series.map((p) => p.viewers)}
            accent="#4ea1ff"
          />
          <StatCard
            label="Watch minutes"
            value={formatViewers(series.reduce((s, p) => s + p.watchMinutes, 0))}
            delta={9.1}
            spark={series.map((p) => p.watchMinutes)}
            accent="#ffd83d"
          />
          <StatCard
            label="Revenue"
            value={`$${Math.round(series.reduce((s, p) => s + p.revenue, 0)).toLocaleString()}`}
            delta={22.8}
            spark={series.map((p) => p.revenue)}
            accent="#3dffb5"
          />
          <StatCard
            label="New followers"
            value={series.reduce((s, p) => s + p.newFollowers, 0).toLocaleString()}
            delta={-3.4}
            spark={series.map((p) => p.newFollowers)}
            accent="#ff3d7a"
          />
        </section>

        <section className="ls-ca__chart-card">
          <div className="ls-ca__chart-head">
            <div>
              <div className="ls-cpage__card-title">{metricLabels[metric]}</div>
            <div className="ls-ca__chart-meta mono">
              <span>{series.length} days</span>
              <span className="ls-ca__sep">·</span>
              Total <strong>{format(total)}</strong>
              <span className="ls-ca__sep">·</span>
              Avg <strong>{format(avg)}</strong>
                <span className="ls-ca__sep">·</span>
                Peak <strong>{format(max)}</strong>
                <span className="ls-ca__sep">·</span>
                Low <strong>{format(min)}</strong>
                <span className="ls-ca__sep">·</span>
                <span className={delta >= 0 ? "up" : "down"}>
                  {delta >= 0 ? "+" : ""}
                  {delta.toFixed(1)}%
                </span>
              </div>
            </div>
            <div className="ls-ca__metric-picker">
              {(Object.keys(metricLabels) as Array<keyof typeof metricLabels>).map((key) => (
                <button
                  key={key}
                  type="button"
                  className={`ls-ca__metric ${metric === key ? "is-active" : ""}`}
                  onClick={() => setMetric(key)}
                >
                  {metricLabels[key]}
                </button>
              ))}
            </div>
          </div>

          <div className="ls-ca__chart">
            <Sparkline
              values={values}
              width={1200}
              height={260}
              accent={metricAccents[metric]}
              className="ls-ca__chart-svg"
            />
            <div className="ls-ca__chart-grid">
              <span>{format(max)}</span>
              <span>{format((max + min) / 2)}</span>
              <span>{format(min)}</span>
            </div>
            <div className="ls-ca__chart-xaxis mono">
              <span>
                {series[0]?.date.slice(5) ?? ""}
              </span>
              <span>
                {series[Math.floor(series.length / 2)]?.date.slice(5) ?? ""}
              </span>
              <span>
                {series[series.length - 1]?.date.slice(5) ?? ""}
              </span>
            </div>
          </div>
        </section>

        <div className="ls-cpage__split">
          <section className="ls-ca__panel">
            <div className="ls-cpage__card-title">Traffic sources</div>
            <div className="ls-ca__traffic">
              {traffic.map((t) => (
                <div key={t.source} className="ls-ca__traffic-row">
                  <div className="ls-ca__traffic-main">
                    <div className="ls-ca__traffic-label">{t.source}</div>
                    <div className="ls-ca__traffic-bar">
                      <div
                        className="ls-ca__traffic-fill"
                        style={{ width: `${t.share * 100}%` }}
                      />
                    </div>
                  </div>
                  <div className="ls-ca__traffic-values mono">
                    <span>{(t.share * 100).toFixed(0)}%</span>
                    <span className="faint">{formatViewers(t.sessions)}</span>
                  </div>
                </div>
              ))}
            </div>
          </section>

          <section className="ls-ca__panel">
            <div className="ls-cpage__card-title">Top content · {range.toUpperCase()}</div>
            <div className="ls-ca__top">
              {top.map((c, i) => (
                <div key={c.id} className="ls-ca__top-row">
                  <div className="ls-ca__top-rank mono">
                    {String(i + 1).padStart(2, "0")}
                  </div>
                  <img src={c.thumbnail} alt="" className="ls-ca__top-thumb" />
                  <div className="ls-ca__top-body">
                    <div className="ls-ca__top-title">{c.title}</div>
                    <div className="ls-ca__top-meta mono">
                      {formatViewers(c.views)} views · {formatViewers(c.watchHours)}h
                    </div>
                  </div>
                  <div className={`ls-ca__top-trend ${c.trend >= 0 ? "up" : "down"}`}>
                    {c.trend >= 0 ? "+" : ""}
                    {c.trend.toFixed(1)}%
                  </div>
                </div>
              ))}
            </div>
          </section>
        </div>

        <section className="ls-ca__panel">
          <div className="ls-cpage__card-title">Data window</div>
          <div className="ls-ca__traffic">
            <div className="ls-ca__traffic-row">
              <div className="ls-ca__traffic-main">
                <div className="ls-ca__traffic-label">Analytics points returned</div>
              </div>
              <div className="ls-ca__traffic-values mono">
                <span>{series.length}</span>
              </div>
            </div>
            <div className="ls-ca__traffic-row">
              <div className="ls-ca__traffic-main">
                <div className="ls-ca__traffic-label">Window start</div>
              </div>
              <div className="ls-ca__traffic-values mono">
                <span>{series[0]?.date ?? "n/a"}</span>
              </div>
            </div>
            <div className="ls-ca__traffic-row">
              <div className="ls-ca__traffic-main">
                <div className="ls-ca__traffic-label">Window end</div>
              </div>
              <div className="ls-ca__traffic-values mono">
                <span>{series[series.length - 1]?.date ?? "n/a"}</span>
              </div>
            </div>
          </div>
        </section>
      </div>
    </CreatorLayout>
  );
}
