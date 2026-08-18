import { Link } from "react-router-dom";
import { ArrowUpRight, Radio, Upload as UploadIcon, Calendar, Bell } from "lucide-react";
import { CreatorLayout } from "@/components/creator/CreatorLayout";
import { StatCard } from "@/components/creator/StatCard";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { repository } from "@/lib/repository";
import { formatViewers, formatDuration, formatRelativeTime } from "@/lib/format";
import "./Creator.css";
import "./CreatorOverviewPage.css";

export function CreatorOverviewPage() {
  const profile = repository.getCreatorProfile();
  const analytics = repository.getAnalytics();
  const last7 = analytics.slice(-7);
  const viewers7 = last7.map((p) => p.viewers);
  const revenue7 = last7.map((p) => p.revenue);
  const followers7 = last7.map((p) => p.newFollowers);
  const watch7 = last7.map((p) => p.watchMinutes);

  const current = repository.getCurrentBroadcast();
  const scheduled = repository.listBroadcastsByStatus("scheduled");
  const ended = repository.listBroadcastsByStatus("ended");
  const notifications = repository.listCreatorNotifications();
  const topContent = repository.getTopContent();

  const revenue30d = analytics.reduce((s, p) => s + p.revenue, 0);
  const viewers30d = analytics.reduce((s, p) => s + p.viewers, 0);
  const watch30d = analytics.reduce((s, p) => s + p.watchMinutes, 0);
  const followers30d = analytics.reduce((s, p) => s + p.newFollowers, 0);

  return (
    <CreatorLayout>
      <div className="ls-cpage">
        <header className="ls-cpage__head">
          <div>
            <h1 className="ls-cpage__title">Overview</h1>
            <p className="ls-cpage__sub">
              Last 30 days · all metrics refresh every 60 seconds from the ingest pipeline.
            </p>
          </div>
          <div className="ls-cov__quick">
            <Link to="/creator/live">
              <Button variant="danger" icon={<Radio fill="currentColor" />}>
                {profile.liveStatus === "live" ? "Manage Stream" : "Go Live"}
              </Button>
            </Link>
            <Link to="/creator/content">
              <Button variant="outline" icon={<UploadIcon />}>
                Upload Episode
              </Button>
            </Link>
          </div>
        </header>

        <section className="ls-cpage__stat-grid">
          <StatCard
            label="30d viewers"
            value={formatViewers(viewers30d)}
            delta={14.2}
            spark={viewers7}
            accent="#4ea1ff"
            footer="vs. previous 30d"
          />
          <StatCard
            label="30d revenue"
            value={`$${revenue30d.toLocaleString("en-US", { maximumFractionDigits: 0 })}`}
            delta={22.8}
            spark={revenue7}
            accent="#3dffb5"
            footer="after platform cut"
          />
          <StatCard
            label="Watch minutes"
            value={formatViewers(watch30d)}
            delta={9.1}
            spark={watch7}
            accent="#ffd83d"
            footer="across live + vod"
          />
          <StatCard
            label="New followers"
            value={followers30d.toLocaleString()}
            delta={-3.4}
            spark={followers7}
            accent="#ff3d7a"
            footer="net growth"
          />
        </section>

        <div className="ls-cpage__split">
          <section className="ls-cpage__section">
            <div className="ls-cpage__section-head">
              <div className="ls-cpage__section-label mono">Current & upcoming broadcasts</div>
              <Link to="/creator/live" className="ls-cov__link mono">
                Manage <ArrowUpRight size={12} />
              </Link>
            </div>

            {current && (
              <div className="ls-cov__current">
                <div
                  className="ls-cov__current-thumb"
                  style={{ backgroundImage: `url(${current.thumbnail})` }}
                >
                  <div className="ls-cov__live-tag">
                    <span className="ls-cov__live-dot" />
                    LIVE
                  </div>
                </div>
                <div className="ls-cov__current-body">
                  <div className="ls-cpage__card-title">Airing now</div>
                  <div className="ls-cov__current-title">{current.title}</div>
                  <div className="ls-cov__current-meta mono">
                    <span>{current.category}</span>
                    <span>·</span>
                    <span>started {formatRelativeTime(current.startedAt)}</span>
                  </div>
                  <div className="ls-cov__current-stats">
                    <div>
                      <div className="ls-cov__stat-num">{formatViewers(current.peakViewers)}</div>
                      <div className="ls-cov__stat-lbl mono">Peak</div>
                    </div>
                    <div>
                      <div className="ls-cov__stat-num">{formatViewers(current.averageViewers)}</div>
                      <div className="ls-cov__stat-lbl mono">Avg</div>
                    </div>
                    <div>
                      <div className="ls-cov__stat-num">+{current.newFollowers}</div>
                      <div className="ls-cov__stat-lbl mono">Followers</div>
                    </div>
                    <div>
                      <div className="ls-cov__stat-num">${current.revenue.toFixed(0)}</div>
                      <div className="ls-cov__stat-lbl mono">Revenue</div>
                    </div>
                  </div>
                </div>
              </div>
            )}

            {scheduled.length > 0 && (
              <div className="ls-cov__schedule">
                <div className="ls-cpage__card-title">
                  <Calendar size={12} /> Scheduled ({scheduled.length})
                </div>
                {scheduled.map((b) => (
                  <div key={b.id} className="ls-cov__schedule-row">
                    <div className="ls-cov__schedule-date mono">
                      {new Date(b.startedAt).toLocaleDateString("en-US", {
                        month: "short",
                        day: "numeric",
                      })}
                      <br />
                      <span className="faint">
                        {new Date(b.startedAt).toLocaleTimeString("en-US", {
                          hour: "numeric",
                          minute: "2-digit",
                        })}
                      </span>
                    </div>
                    <div className="ls-cov__schedule-body">
                      <div className="ls-cov__schedule-title">{b.title}</div>
                      <div className="ls-cov__schedule-cat mono">
                        <Badge tone="new">SCHEDULED</Badge>
                        <span>{b.category}</span>
                      </div>
                    </div>
                    <Button variant="ghost" size="sm">Edit</Button>
                  </div>
                ))}
              </div>
            )}

            {ended.length > 0 && (
              <div className="ls-cov__recent">
                <div className="ls-cpage__card-title">Recent broadcasts</div>
                <table className="ls-cov__table">
                  <thead>
                    <tr>
                      <th>Title</th>
                      <th className="num">Duration</th>
                      <th className="num">Peak</th>
                      <th className="num">Avg</th>
                      <th className="num">Revenue</th>
                    </tr>
                  </thead>
                  <tbody>
                    {ended.map((b) => (
                      <tr key={b.id}>
                        <td>
                          <div className="ls-cov__row-title">{b.title}</div>
                          <div className="ls-cov__row-meta mono">
                            {b.category} · {b.endedAt ? formatRelativeTime(b.endedAt) : ""}
                          </div>
                        </td>
                        <td className="num mono">
                          {b.durationSec !== undefined ? formatDuration(b.durationSec) : "—"}
                        </td>
                        <td className="num mono">{formatViewers(b.peakViewers)}</td>
                        <td className="num mono">{formatViewers(b.averageViewers)}</td>
                        <td className="num mono">${b.revenue.toFixed(0)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>

          <aside className="ls-cov__aside">
            <section className="ls-cpage__section">
              <div className="ls-cpage__section-label mono">
                <Bell size={11} style={{ marginRight: 4 }} /> Creator activity
              </div>
              <div className="ls-cov__notif">
                {notifications.map((n) => (
                  <div key={n.id} className="ls-cov__notif-row">
                    <div className={`ls-cov__notif-mark ls-cov__notif-mark--${n.kind}`} />
                    <div>
                      <div className="ls-cov__notif-body">
                        {n.body}
                        {n.amount !== undefined && (
                          <span className="ls-cov__notif-amount"> · ${n.amount}</span>
                        )}
                      </div>
                      <div className="ls-cov__notif-time mono">
                        {formatRelativeTime(n.sentAt)}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </section>

            <section className="ls-cpage__section">
              <div className="ls-cpage__section-label mono">Top content · last 30d</div>
              <div className="ls-cov__top">
                {topContent.map((c, i) => (
                  <div key={c.id} className="ls-cov__top-row">
                    <div className="ls-cov__top-rank mono">
                      {String(i + 1).padStart(2, "0")}
                    </div>
                    <img className="ls-cov__top-thumb" src={c.thumbnail} alt="" />
                    <div className="ls-cov__top-body">
                      <div className="ls-cov__top-title">{c.title}</div>
                      <div className="ls-cov__top-meta mono">
                        {formatViewers(c.views)} views · {formatViewers(c.watchHours)}h
                      </div>
                    </div>
                    <div
                      className={`ls-cov__top-trend mono ${c.trend >= 0 ? "up" : "down"}`}
                    >
                      {c.trend >= 0 ? "+" : ""}
                      {c.trend.toFixed(1)}%
                    </div>
                  </div>
                ))}
              </div>
            </section>
          </aside>
        </div>
      </div>
    </CreatorLayout>
  );
}
