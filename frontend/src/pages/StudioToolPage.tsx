import { useCallback, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { useNavigate, useParams } from "react-router-dom";
import {
  ArrowLeft,
  Clapperboard,
  Eye,
  FileVideo,
  FolderOpen,
  Gauge,
  MessageSquare,
  Radio,
  Scissors,
  UploadCloud,
  Users,
  Wallet,
} from "lucide-react";
import { Button } from "@/components/ui/Button";
import { PageTrail } from "@/components/navigation/PageTrail";
import { repository } from "@/lib/repository";
import { formatNumber, formatRelativeTime, formatRuntime, formatViewers } from "@/lib/format";
import type { Broadcast, CreatorDashboardPayload, MediaAsset, UploadJob } from "@/types";
import "./StudioPage.css";

type StudioToolKind = "stream-editor" | "live-ops" | "series-editor" | "file-manager";

interface StudioToolConfig {
  readonly title: string;
  readonly trailLabel: string;
  readonly body: string;
}

const toolConfigs: Record<StudioToolKind, StudioToolConfig> = {
  "stream-editor": {
    title: "Streaming Editing Hub",
    trailLabel: "Stream",
    body: "Scene packaging, clips, show notes, live assets, and broadcast handoff.",
  },
  "live-ops": {
    title: "Live Operations Console",
    trailLabel: "Live",
    body: "Audience pulse, chat and tips, health signals, and live-room readiness.",
  },
  "series-editor": {
    title: "Series Editing Hub",
    trailLabel: "Series",
    body: "Episode cuts, season packaging, release status, and metadata polish.",
  },
  "file-manager": {
    title: "File Manager",
    trailLabel: "Files",
    body: "Uploaded sources, processed media, playback assets, posters, and delivery readiness.",
  },
};

function isToolKind(value: string | undefined): value is StudioToolKind {
  return value === "stream-editor" || value === "live-ops" || value === "series-editor" || value === "file-manager";
}

function money(value: number): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 0,
  }).format(value);
}

function formatBytes(value: number): string {
  if (value >= 1024 * 1024 * 1024) return `${(value / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  if (value >= 1024 * 1024) return `${(value / (1024 * 1024)).toFixed(1)} MB`;
  if (value >= 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${value} B`;
}

function recentSeriesUploads(dashboard: CreatorDashboardPayload) {
  return dashboard.uploads
    .filter((item) => item.kind === "episode" || item.kind === "film")
    .slice(0, 8);
}

function recentStreams(dashboard: CreatorDashboardPayload): ReadonlyArray<Broadcast> {
  return [
    ...(dashboard.currentBroadcast ? [dashboard.currentBroadcast] : []),
    ...dashboard.recentBroadcasts,
  ].slice(0, 8);
}

function StudioFact({ label, value }: { readonly label: string; readonly value: string }) {
  return (
    <div>
      <span className="mono">{label}</span>
      {value}
    </div>
  );
}

function ToolMetric({ icon, label, value }: { readonly icon: ReactNode; readonly label: string; readonly value: string }) {
  return (
    <div className="ls-studio__metric">
      <span className="mono">{label}</span>
      <strong>{value}</strong>
      <em>{icon}</em>
    </div>
  );
}

export function StudioToolPage() {
  const { tool } = useParams();
  const navigate = useNavigate();
  const kind: StudioToolKind = isToolKind(tool) ? tool : "stream-editor";
  const config = toolConfigs[kind];
  const [dashboard, setDashboard] = useState<CreatorDashboardPayload | null>(null);
  const [jobs, setJobs] = useState<ReadonlyArray<UploadJob>>([]);
  const [assets, setAssets] = useState<ReadonlyArray<MediaAsset>>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadTool = useCallback(async (signal?: AbortSignal) => {
    setError(null);
    const [nextDashboard, nextJobs, nextAssets] = await Promise.all([
      repository.fetchCreatorDashboard(signal),
      repository.listUploadJobs(signal),
      repository.listMediaAssets(signal),
    ]);
    setDashboard(nextDashboard);
    setJobs(nextJobs);
    setAssets(nextAssets);
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    void loadTool(controller.signal)
      .catch((err) => {
        if (!controller.signal.aborted) {
          setError(err instanceof Error ? err.message : "Unable to load studio tool.");
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [loadTool]);

  const streamRows = useMemo(() => (dashboard ? recentStreams(dashboard) : []), [dashboard]);
  const seriesRows = useMemo(() => (dashboard ? recentSeriesUploads(dashboard) : []), [dashboard]);
  const totalAssetBytes = assets.reduce((sum, item) => sum + item.fileSizeBytes, 0);
  const readyAssets = assets.filter((item) => item.playbackPath).length;
  const latestAnalytics = dashboard?.analytics.at(-1) ?? null;
  const currentBroadcast = dashboard?.currentBroadcast ?? null;

  return (
    <div className="ls-studio">
      <header className="ls-studio__head">
        <PageTrail
          className="ls-studio__kicker mono"
          items={[
            { label: "Dashboard", href: "/" },
            { label: "Creator Studio", href: "/studio" },
            { label: config.trailLabel },
          ]}
        />
        <div className="ls-studio__title-row">
          <div>
            <h1 className="ls-studio__title">{config.title}</h1>
            <p className="ls-studio__sub">{config.body}</p>
          </div>
          <Button variant="outline" icon={<ArrowLeft />} onClick={() => navigate("/studio")}>
            Back to Studio
          </Button>
        </div>
      </header>

      {error ? <div className="ls-studio__error">{error}</div> : null}

      {kind === "stream-editor" ? (
        <>
          <section className="ls-studio__metrics" aria-label="Stream editing metrics">
            <ToolMetric icon={<Radio size={14} />} label="Broadcasts" value={formatNumber(streamRows.length)} />
            <ToolMetric icon={<Scissors size={14} />} label="Clip candidates" value={formatNumber(dashboard?.topContent.length ?? 0)} />
            <ToolMetric icon={<Eye size={14} />} label="Audience" value={formatViewers(latestAnalytics?.viewers ?? 0)} />
            <ToolMetric icon={<Wallet size={14} />} label="Revenue" value={money(latestAnalytics?.revenue ?? 0)} />
          </section>
          <section className="ls-studio__dashboard-grid">
            <div className="ls-studio__panel ls-studio__panel--wide">
              <div className="ls-studio__panel-head">
                <div>
                  <h2>Broadcast Package</h2>
                  <p>{currentBroadcast ? "Current stream package is live." : "Prepare packaging from recent stream data."}</p>
                </div>
                <Radio size={18} strokeWidth={1.75} />
              </div>
              <div className="ls-studio__recent-list">
                {streamRows.map((item) => (
                  <div className="ls-studio__recent-row" key={item.id}>
                    <img src={item.thumbnail} alt="" />
                    <span>
                      <strong>{item.title}</strong>
                      <span className="mono">{item.status} / {item.category} / {formatRuntime(item.durationSec ?? 0)}</span>
                    </span>
                    <em>{formatViewers(item.peakViewers)} peak</em>
                  </div>
                ))}
                {!loading && streamRows.length === 0 ? <div className="ls-studio__empty">No stream package data yet.</div> : null}
              </div>
            </div>
          </section>
        </>
      ) : null}

      {kind === "live-ops" ? (
        <>
          <section className="ls-studio__metrics" aria-label="Live operations metrics">
            <ToolMetric icon={<Users size={14} />} label="Live viewers" value={formatViewers(currentBroadcast?.averageViewers ?? 0)} />
            <ToolMetric icon={<MessageSquare size={14} />} label="Chat" value={formatNumber(currentBroadcast?.chatMessages ?? 0)} />
            <ToolMetric icon={<Wallet size={14} />} label="Tips" value={money(currentBroadcast?.revenue ?? 0)} />
            <ToolMetric icon={<Gauge size={14} />} label="Status" value={currentBroadcast?.status ?? "offline"} />
          </section>
          <section className="ls-studio__dashboard-grid">
            <div className="ls-studio__panel ls-studio__panel--wide">
              <div className="ls-studio__panel-head">
                <div>
                  <h2>Live Room State</h2>
                  <p>{currentBroadcast ? "The live room has an active broadcast." : "The live room is offline."}</p>
                </div>
                <MessageSquare size={18} strokeWidth={1.75} />
              </div>
              <div className="ls-studio__live-grid">
                <StudioFact label="Broadcast" value={currentBroadcast?.title ?? "Offline"} />
                <StudioFact label="Viewers" value={formatViewers(currentBroadcast?.averageViewers ?? 0)} />
                <StudioFact label="Subscribers" value={formatNumber(currentBroadcast?.newSubscribers ?? 0)} />
                <StudioFact label="Followers" value={formatNumber(currentBroadcast?.newFollowers ?? 0)} />
              </div>
            </div>
          </section>
        </>
      ) : null}

      {kind === "series-editor" ? (
        <>
          <section className="ls-studio__metrics" aria-label="Series editing metrics">
            <ToolMetric icon={<Clapperboard size={14} />} label="Titles" value={formatNumber(seriesRows.length)} />
            <ToolMetric icon={<Eye size={14} />} label="Views" value={formatNumber(seriesRows.reduce((sum, item) => sum + item.views, 0))} />
            <ToolMetric icon={<Users size={14} />} label="Engagement" value={formatNumber(seriesRows.reduce((sum, item) => sum + item.likes + item.comments, 0))} />
            <ToolMetric icon={<UploadCloud size={14} />} label="Processing" value={formatNumber(seriesRows.filter((item) => item.status === "processing").length)} />
          </section>
          <section className="ls-studio__dashboard-grid">
            <div className="ls-studio__panel ls-studio__panel--wide">
              <div className="ls-studio__panel-head">
                <div>
                  <h2>Episode Packaging</h2>
                  <p>Recent long-form releases and work-in-progress cuts.</p>
                </div>
                <Clapperboard size={18} strokeWidth={1.75} />
              </div>
              <div className="ls-studio__recent-list">
                {seriesRows.map((item) => (
                  <div className="ls-studio__recent-row" key={item.id}>
                    <img src={item.thumbnail} alt="" />
                    <span>
                      <strong>{item.title}</strong>
                      <span className="mono">{item.status} / {item.kind} / {formatRuntime(item.durationSec)}</span>
                    </span>
                    <em>{formatNumber(item.views)} views</em>
                  </div>
                ))}
                {!loading && seriesRows.length === 0 ? <div className="ls-studio__empty">No series cuts yet.</div> : null}
              </div>
            </div>
          </section>
        </>
      ) : null}

      {kind === "file-manager" ? (
        <>
          <section className="ls-studio__metrics" aria-label="File manager metrics">
            <ToolMetric icon={<FileVideo size={14} />} label="Jobs" value={formatNumber(jobs.length)} />
            <ToolMetric icon={<FolderOpen size={14} />} label="Assets" value={formatNumber(assets.length)} />
            <ToolMetric icon={<Eye size={14} />} label="Playable" value={formatNumber(readyAssets)} />
            <ToolMetric icon={<UploadCloud size={14} />} label="Storage" value={formatBytes(totalAssetBytes)} />
          </section>
          <section className="ls-studio__dashboard-grid">
            <div className="ls-studio__panel">
              <div className="ls-studio__panel-head">
                <div>
                  <h2>Upload Jobs</h2>
                  <p>{jobs.length} source jobs</p>
                </div>
                <UploadCloud size={18} strokeWidth={1.75} />
              </div>
              <div className="ls-studio__jobs">
                {jobs.slice(0, 12).map((item) => (
                  <div className="ls-studio__asset-row" key={item.id}>
                    <strong className="ls-studio__job-title">{item.title}</strong>
                    <span className="ls-studio__job-meta mono">{item.status} / {item.kind} / {formatBytes(item.bytesReceived)} received</span>
                  </div>
                ))}
                {!loading && jobs.length === 0 ? <div className="ls-studio__empty">No upload jobs yet.</div> : null}
              </div>
            </div>
            <div className="ls-studio__panel">
              <div className="ls-studio__panel-head">
                <div>
                  <h2>Media Assets</h2>
                  <p>{assets.length} processed records</p>
                </div>
                <FolderOpen size={18} strokeWidth={1.75} />
              </div>
              <div className="ls-studio__jobs">
                {assets.slice(0, 12).map((item) => (
                  <div className="ls-studio__asset-row" key={item.id}>
                    <strong className="ls-studio__job-title">{item.title}</strong>
                    <span className="ls-studio__job-meta mono">{item.status} / {item.mimeType} / {formatRuntime(item.durationSec)}</span>
                    <span className="ls-studio__job-meta mono">{item.playbackPath ?? item.sourcePath}</span>
                  </div>
                ))}
                {!loading && assets.length === 0 ? <div className="ls-studio__empty">No media assets yet.</div> : null}
              </div>
            </div>
          </section>
        </>
      ) : null}

      <section className="ls-studio__command-grid">
        <Button variant="outline" icon={<Radio />} onClick={() => navigate("/studio/tool/stream-editor")}>
          Streaming Hub
        </Button>
        <Button variant="outline" icon={<MessageSquare />} onClick={() => navigate("/studio/tool/live-ops")}>
          Live Ops
        </Button>
        <Button variant="outline" icon={<Clapperboard />} onClick={() => navigate("/studio/tool/series-editor")}>
          Series Hub
        </Button>
        <Button variant="outline" icon={<FolderOpen />} onClick={() => navigate("/studio/tool/file-manager")}>
          File Manager
        </Button>
      </section>
    </div>
  );
}
