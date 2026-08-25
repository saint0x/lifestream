import { useCallback, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import {
  Activity,
  BarChart3,
  CalendarClock,
  Check,
  ChevronRight,
  Clapperboard,
  Clock3,
  Edit3,
  Eye,
  FileVideo,
  FolderOpen,
  Gauge,
  MessageSquare,
  PlayCircle,
  Radio,
  RefreshCw,
  Save,
  Send,
  Sparkles,
  UploadCloud,
  Users,
  Wallet,
} from "lucide-react";
import { cdnAsset } from "@/lib/assets";
import { repository } from "@/lib/repository";
import { Button } from "@/components/ui/Button";
import { PageTrail } from "@/components/navigation/PageTrail";
import { Input } from "@/components/ui/Input";
import { formatNumber, formatRelativeTime, formatRuntime, formatViewers } from "@/lib/format";
import type { AnalyticsPoint, Broadcast, MediaAsset, TopContent, Upload, UploadJob } from "@/types";
import "./StudioPage.css";

const visibilityOptions = ["private", "unlisted", "public"] as const;
const kindOptions = ["film", "episode"] as const;

type StudioView = "stream" | "series";

interface UploadJobForm {
  readonly kind: string;
  readonly title: string;
  readonly intendedVisibility: string;
  readonly bytesExpected: string;
  readonly storageKey: string;
  readonly mimeType: string;
}

interface PublishForm {
  readonly slug: string;
  readonly description: string;
  readonly visibility: string;
}

const initialForm: UploadJobForm = {
  kind: "episode",
  title: "",
  intendedVisibility: "private",
  bytesExpected: "1048576",
  storageKey: "",
  mimeType: "video/mp4",
};

function slugFromTitle(title: string): string {
  return title
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64);
}

function storageKeyFromTitle(title: string): string {
  const slug = slugFromTitle(title);
  return `studio/${Date.now()}/${slug || "untitled-upload"}.mp4`;
}

function formatBytes(value: number): string {
  if (value >= 1024 * 1024 * 1024) return `${(value / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  if (value >= 1024 * 1024) return `${(value / (1024 * 1024)).toFixed(1)} MB`;
  if (value >= 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${value} B`;
}

function money(value: number): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 0,
  }).format(value);
}

function pct(value: number): string {
  return `${Math.round(value)}%`;
}

function compactDate(value?: string | null): string {
  if (!value) return "not scheduled";
  return new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
    timeZone: "UTC",
  }).format(new Date(value));
}

function latestAnalytics(analytics: ReadonlyArray<AnalyticsPoint>) {
  return analytics.at(-1) ?? null;
}

function uploadsForView(uploads: ReadonlyArray<Upload>, view: StudioView): ReadonlyArray<Upload> {
  if (view === "stream") return uploads.filter((item) => item.kind === "vod" || item.kind === "clip" || item.kind === "trailer");
  return uploads.filter((item) => item.kind === "episode" || item.kind === "film");
}

function uploadLabel(upload: Upload): string {
  if (upload.seriesTitle && upload.seasonNumber && upload.episodeNumber) {
    return `${upload.seriesTitle} / S${upload.seasonNumber} E${upload.episodeNumber}`;
  }
  return upload.kind;
}

function MetricCard({ label, value, note }: { readonly label: string; readonly value: string; readonly note: string }) {
  return (
    <div className="ls-studio__metric">
      <span className="mono">{label}</span>
      <strong>{value}</strong>
      <em>{note}</em>
    </div>
  );
}

function ToolCard({
  icon,
  title,
  body,
  action,
  onClick,
}: {
  readonly icon: ReactNode;
  readonly title: string;
  readonly body: string;
  readonly action: string;
  readonly onClick: () => void;
}) {
  return (
    <button type="button" className="ls-studio__tool" onClick={onClick}>
      <span className="ls-studio__tool-icon">{icon}</span>
      <span className="ls-studio__tool-copy">
        <strong>{title}</strong>
        <span>{body}</span>
      </span>
      <span className="ls-studio__tool-action mono">
        {action}
        <ChevronRight size={14} strokeWidth={1.75} />
      </span>
    </button>
  );
}

function TopContentRow({ item }: { readonly item: TopContent }) {
  return (
    <div className="ls-studio__top-row">
      <img src={item.thumbnail} alt="" />
      <span>
        <strong>{item.title}</strong>
        <span className="mono">
          {item.kind} / {formatNumber(item.views)} views / {formatNumber(item.watchHours)} hours
        </span>
      </span>
      <em className={item.trend >= 0 ? "is-positive" : ""}>{item.trend >= 0 ? "+" : ""}{pct(item.trend)}</em>
    </div>
  );
}

function SpotlightMedia({
  eyebrow,
  title,
  subtitle,
  image,
  meta,
  children,
}: {
  readonly eyebrow: string;
  readonly title: string;
  readonly subtitle: string;
  readonly image: string;
  readonly meta: string;
  readonly children: ReactNode;
}) {
  return (
    <div className="ls-studio__spotlight">
      <img src={image} alt="" />
      <div className="ls-studio__spotlight-shade" />
      <div className="ls-studio__spotlight-copy">
        <span className="ls-studio__pill mono">{eyebrow}</span>
        <h2>{title}</h2>
        <p>{subtitle}</p>
        <span className="ls-studio__spotlight-meta mono">{meta}</span>
      </div>
      <div className="ls-studio__spotlight-actions">{children}</div>
    </div>
  );
}

function MediaTile({
  image,
  title,
  meta,
  value,
}: {
  readonly image: string;
  readonly title: string;
  readonly meta: string;
  readonly value: string;
}) {
  return (
    <article className="ls-studio__media-tile">
      <div className="ls-studio__media-thumb">
        <img src={image} alt="" />
        <span><PlayCircle size={16} strokeWidth={1.75} /></span>
      </div>
      <strong>{title}</strong>
      <span className="mono">{meta}</span>
      <em>{value}</em>
    </article>
  );
}

export function StudioPage() {
  const navigate = useNavigate();
  const [view, setView] = useState<StudioView>("stream");
  const [broadcasts, setBroadcasts] = useState<ReadonlyArray<Broadcast>>([]);
  const [creatorUploads, setCreatorUploads] = useState<ReadonlyArray<Upload>>([]);
  const [analytics, setAnalytics] = useState<ReadonlyArray<AnalyticsPoint>>([]);
  const [topContent, setTopContent] = useState<ReadonlyArray<TopContent>>([]);
  const [jobs, setJobs] = useState<ReadonlyArray<UploadJob>>([]);
  const [form, setForm] = useState<UploadJobForm>(initialForm);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selectedTitle, setSelectedTitle] = useState("");
  const [selectedVisibility, setSelectedVisibility] = useState("private");
  const [selectedMimeType, setSelectedMimeType] = useState("video/mp4");
  const [publishForm, setPublishForm] = useState<PublishForm>({
    slug: "",
    description: "",
    visibility: "private",
  });
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [assets, setAssets] = useState<ReadonlyArray<MediaAsset>>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const selectedJob = useMemo(
    () => jobs.find((job) => job.id === selectedId) ?? null,
    [jobs, selectedId],
  );

  const selectedAsset = useMemo(
    () => assets.find((asset) => asset.uploadJobId === selectedId) ?? null,
    [assets, selectedId],
  );

  const selectedJobCanPublish =
    selectedJob !== null
    && (selectedJob.status === "ready" || selectedJob.status === "published")
    && selectedAsset !== null
    && selectedAsset.playbackPath !== null
    && selectedAsset.playbackPath !== undefined;

  const selectedUploads = useMemo(
    () => uploadsForView(creatorUploads, view),
    [creatorUploads, view],
  );
  const recentUploads = selectedUploads.slice(0, 5);
  const recentBroadcasts = broadcasts.filter((item) => item.status === "ended").slice(0, 5);
  const liveBroadcast = broadcasts.find((item) => item.status === "live") ?? null;
  const currentAnalytics = latestAnalytics(analytics);
  const streamChatCount =
    (liveBroadcast?.chatMessages ?? 0)
    || recentBroadcasts.reduce((sum, item) => sum + item.chatMessages, 0);
  const streamRevenue =
    (liveBroadcast?.revenue ?? 0)
    || recentBroadcasts.reduce((sum, item) => sum + item.revenue, 0);
  const seriesViews = selectedUploads.reduce((sum, item) => sum + item.views, 0);
  const seriesWatchHours = selectedUploads.reduce((sum, item) => sum + item.watchHours, 0);
  const seriesEngagements = selectedUploads.reduce((sum, item) => sum + item.likes + item.comments, 0);
  const publishedSeriesUploads = selectedUploads.filter((item) => item.status === "published").length;
  const processingSeriesUploads = selectedUploads.filter((item) => item.status === "processing").length;
  const featuredBroadcast = liveBroadcast ?? recentBroadcasts[0] ?? broadcasts[0] ?? null;
  const featuredUpload = recentUploads[0] ?? selectedUploads[0] ?? null;
  const heroImage =
    view === "stream"
      ? featuredBroadcast?.thumbnail ?? topContent[0]?.thumbnail ?? featuredUpload?.thumbnail ?? cdnAsset("app-static/studio/streamer-ops.png")
      : featuredUpload?.thumbnail ?? topContent[0]?.thumbnail ?? featuredBroadcast?.thumbnail ?? cdnAsset("app-static/studio/series-director.png");
  const heroTitle =
    view === "stream"
      ? featuredBroadcast?.title ?? "Ready the next live room"
      : featuredUpload?.title ?? "Shape the next release";
  const heroSubtitle =
    view === "stream"
      ? liveBroadcast
        ? `${liveBroadcast.category} is live with ${formatViewers(liveBroadcast.averageViewers)} average viewers and ${formatNumber(liveBroadcast.chatMessages)} chat messages.`
        : "Review the last broadcast, package clips, and open live ops before the next stream."
      : featuredUpload
        ? `${uploadLabel(featuredUpload)} is ${featuredUpload.status} with ${formatNumber(featuredUpload.views)} views and ${formatNumber(featuredUpload.watchHours)} watch hours.`
        : "Bring source files, metadata, release status, and publish readiness into one director view.";
  const heroMeta =
    view === "stream"
      ? liveBroadcast
        ? "Live now"
        : `${recentBroadcasts.length} recent streams`
      : `${publishedSeriesUploads} published / ${processingSeriesUploads} processing`;
  const uploadProgress = selectedJob
    ? Math.min(100, Math.round((selectedJob.bytesReceived / Math.max(selectedJob.bytesExpected, 1)) * 100))
    : 0;

  const selectJob = useCallback((job: UploadJob) => {
    setSelectedId(job.id);
    setSelectedTitle(job.title);
    setSelectedVisibility(job.intendedVisibility);
    setSelectedMimeType(job.mimeType);
    setPublishForm((current) => ({
      ...current,
      slug: slugFromTitle(job.title),
      visibility: job.intendedVisibility,
    }));
  }, []);

  const loadStudio = useCallback(async (signal?: AbortSignal) => {
    setError(null);
    const [dashboard, nextJobs, nextAssets] = await Promise.all([
      repository.fetchCreatorDashboard(signal),
      repository.listUploadJobs(signal),
      repository.listMediaAssets(signal),
    ]);
    setBroadcasts([
      ...(dashboard.currentBroadcast ? [dashboard.currentBroadcast] : []),
      ...dashboard.scheduledBroadcasts,
      ...dashboard.recentBroadcasts,
    ]);
    setCreatorUploads(dashboard.uploads);
    setAnalytics(dashboard.analytics);
    setTopContent(dashboard.topContent);
    setJobs(nextJobs);
    setAssets(nextAssets);
    const firstJob = nextJobs[0];
    if (firstJob) {
      setSelectedId((currentId) => {
        if (currentId && nextJobs.some((job) => job.id === currentId)) return currentId;
        setSelectedTitle(firstJob.title);
        setSelectedVisibility(firstJob.intendedVisibility);
        setSelectedMimeType(firstJob.mimeType);
        setPublishForm((current) => ({
          ...current,
          slug: current.slug || slugFromTitle(firstJob.title),
          visibility: firstJob.intendedVisibility,
        }));
        return firstJob.id;
      });
    }
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    void loadStudio(controller.signal)
      .catch((err) => {
        if (!controller.signal.aborted) {
          setError(err instanceof Error ? err.message : "Unable to load Creator Studio.");
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [loadStudio]);

  const updateField = (field: keyof UploadJobForm, value: string) => {
    setForm((current) => ({ ...current, [field]: value }));
  };

  const updatePublishField = (field: keyof PublishForm, value: string) => {
    setPublishForm((current) => ({ ...current, [field]: value }));
  };

  const createJob = async () => {
    const title = form.title.trim();
    const bytesExpected = Number(form.bytesExpected);
    if (!title) {
      setError("Add a title before creating an upload job.");
      return;
    }
    if (!Number.isFinite(bytesExpected) || bytesExpected <= 0) {
      setError("Expected bytes must be greater than zero.");
      return;
    }

    setSaving(true);
    setStatus(null);
    setError(null);
    try {
      const created = await repository.createUploadJob({
        kind: form.kind,
        sourceType: "resumable-upload",
        title,
        intendedVisibility: form.intendedVisibility,
        bytesExpected,
        storageKey: form.storageKey.trim() || storageKeyFromTitle(title),
        mimeType: form.mimeType.trim() || "application/octet-stream",
      });
      setJobs((current) => [created, ...current.filter((job) => job.id !== created.id)]);
      selectJob(created);
      setForm(initialForm);
      setStatus("Upload job created.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unable to create upload job.");
    } finally {
      setSaving(false);
    }
  };

  const uploadSelectedFile = async () => {
    const file = selectedFile;
    if (!file) {
      setError("Choose a media file before ingest.");
      return;
    }
    const title = form.title.trim() || file.name.replace(/\.[^.]+$/, "");
    const storageKey = form.storageKey.trim() || storageKeyFromTitle(title);
    setSaving(true);
    setStatus("Creating upload job...");
    setError(null);
    try {
      const created = await repository.createUploadJob({
        kind: form.kind,
        sourceType: "resumable-upload",
        title,
        intendedVisibility: form.intendedVisibility,
        bytesExpected: file.size,
        storageKey,
        mimeType: form.mimeType.trim() || file.type || "application/octet-stream",
      });
      setJobs((current) => [created, ...current.filter((job) => job.id !== created.id)]);
      selectJob(created);

      setStatus("Starting ingest session...");
      const ticket = await repository.startUploadIngest(created.id);
      setStatus("Uploading file...");
      await repository.appendUploadChunk(created.id, ticket.uploadToken, 0, file);
      setStatus("Completing ingest...");
      const completed = await repository.completeUploadIngest(created.id, ticket.uploadToken);
      const asset = await repository.getMediaAssetForUploadJob(created.id);
      setJobs((current) => current.map((job) => (job.id === completed.id ? completed : job)));
      setAssets((current) => [asset, ...current.filter((item) => item.id !== asset.id)]);
      selectJob(completed);
      setSelectedFile(null);
      setForm(initialForm);
      setStatus("File uploaded and media asset created.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unable to ingest selected file.");
      setStatus(null);
    } finally {
      setSaving(false);
    }
  };

  const saveSelectedJob = async () => {
    if (!selectedJob) return;
    setSaving(true);
    setStatus(null);
    setError(null);
    try {
      const updated = await repository.updateUploadJob(selectedJob.id, {
        title: selectedTitle.trim(),
        intendedVisibility: selectedVisibility,
        mimeType: selectedMimeType.trim() || "application/octet-stream",
      });
      setJobs((current) => current.map((job) => (job.id === updated.id ? updated : job)));
      selectJob(updated);
      setStatus("Upload job saved.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unable to save upload job.");
    } finally {
      setSaving(false);
    }
  };

  const publishSelectedJob = async () => {
    if (!selectedJob) return;
    const slug = publishForm.slug.trim() || slugFromTitle(selectedJob.title);
    if (!slug) {
      setError("Add a slug before publishing.");
      return;
    }
    setSaving(true);
    setStatus("Publishing upload...");
    setError(null);
    try {
      const published = await repository.publishUploadJob(selectedJob.id, {
        slug,
        visibility: publishForm.visibility,
        description: publishForm.description.trim() || undefined,
      });
      await loadStudio();
      setStatus(`Published ${published.title}.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unable to publish upload.");
      setStatus(null);
    } finally {
      setSaving(false);
    }
  };

  const metricCards = view === "stream"
    ? [
        {
          label: "Live viewers",
          value: formatViewers(liveBroadcast?.averageViewers ?? currentAnalytics?.viewers ?? 0),
          note: liveBroadcast ? `${formatViewers(liveBroadcast.peakViewers)} peak now` : "latest audience point",
        },
        {
          label: "Chat velocity",
          value: formatNumber(streamChatCount),
          note: liveBroadcast ? "messages this broadcast" : "recent stream messages",
        },
        {
          label: "New follows",
          value: formatNumber(liveBroadcast?.newFollowers ?? currentAnalytics?.newFollowers ?? 0),
          note: "latest creator movement",
        },
        {
          label: "Tips / revenue",
          value: money(streamRevenue || currentAnalytics?.revenue || 0),
          note: "stream-side creator earnings",
        },
      ]
    : [
        {
          label: "Series viewers",
          value: formatNumber(seriesViews),
          note: `${formatNumber(seriesEngagements)} likes and comments`,
        },
        {
          label: "Watch hours",
          value: formatNumber(seriesWatchHours),
          note: `${formatNumber(seriesWatchHours)} watch hours`,
        },
        {
          label: "Published",
          value: formatNumber(publishedSeriesUploads),
          note: `${processingSeriesUploads} processing`,
        },
        {
          label: "Avg retention",
          value: selectedUploads.length > 0 ? `${Math.round(seriesWatchHours / selectedUploads.length)}h` : "0h",
          note: "watch hours per title",
        },
      ];

  return (
    <div className="ls-studio">
      <header className="ls-studio__head">
        <PageTrail
          className="ls-studio__kicker mono"
          items={[
            { label: "Dashboard", href: "/" },
            { label: "Creator Studio" },
          ]}
        />
        <div className="ls-studio__title-row">
          <div>
            <h1 className="ls-studio__title">Creator Studio</h1>
            <p className="ls-studio__sub">
              The operating dashboard for live broadcasts, long-form series, uploads, and production handoff.
            </p>
          </div>
          <Button
            variant="outline"
            icon={<RefreshCw />}
            onClick={() => {
              setLoading(true);
              void loadStudio().finally(() => setLoading(false));
            }}
            disabled={loading || saving}
          >
            Refresh
          </Button>
        </div>
      </header>

      {status ? <div className="ls-studio__notice"><Check size={14} />{status}</div> : null}
      {error ? <div className="ls-studio__error">{error}</div> : null}

      <section className="ls-studio__hero-grid">
        <SpotlightMedia
          eyebrow={view === "stream" ? "Streamer command" : "Series director command"}
          title={heroTitle}
          subtitle={heroSubtitle}
          image={heroImage}
          meta={heroMeta}
        >
          <Button
            variant="primary"
            icon={view === "stream" ? <Radio /> : <Edit3 />}
            onClick={() => navigate(view === "stream" ? "/studio/tool/live-ops" : "/studio/tool/series-editor")}
          >
            {view === "stream" ? "Open Live Ops" : "Open Series Hub"}
          </Button>
          <Button
            variant="outline"
            icon={view === "stream" ? <Edit3 /> : <FolderOpen />}
            onClick={() => navigate(view === "stream" ? "/studio/tool/stream-editor" : "/studio/tool/file-manager")}
          >
            {view === "stream" ? "Package Stream" : "Open Files"}
          </Button>
        </SpotlightMedia>

        <aside className="ls-studio__role-rail" aria-label="Creator mode">
          <div className="ls-studio__switch" role="tablist" aria-label="Creator Studio view">
            <button
              type="button"
              className={view === "stream" ? "is-active" : ""}
              onClick={() => setView("stream")}
              role="tab"
              aria-selected={view === "stream"}
            >
              <Radio size={15} strokeWidth={1.75} />
              Streamer
            </button>
            <button
              type="button"
              className={view === "series" ? "is-active" : ""}
              onClick={() => setView("series")}
              role="tab"
              aria-selected={view === "series"}
            >
              <Clapperboard size={15} strokeWidth={1.75} />
              Series Director
            </button>
          </div>
          <div className="ls-studio__focus-list">
            {view === "stream" ? (
              <>
                <div><Gauge size={15} /><span><strong>Room state</strong><em>{liveBroadcast ? "Live broadcast active" : "Offline and ready"}</em></span></div>
                <div><MessageSquare size={15} /><span><strong>Audience pulse</strong><em>{formatNumber(streamChatCount)} chat messages tracked</em></span></div>
                <div><CalendarClock size={15} /><span><strong>Recent runs</strong><em>{recentBroadcasts.length} broadcasts ready for packaging</em></span></div>
              </>
            ) : (
              <>
                <div><Clapperboard size={15} /><span><strong>Release slate</strong><em>{formatNumber(selectedUploads.length)} titles in the long-form library</em></span></div>
                <div><Clock3 size={15} /><span><strong>Pipeline</strong><em>{processingSeriesUploads} processing, {publishedSeriesUploads} published</em></span></div>
                <div><FileVideo size={15} /><span><strong>Upload jobs</strong><em>{jobs.length} source jobs available</em></span></div>
              </>
            )}
          </div>
        </aside>
      </section>

      <section className="ls-studio__metrics" aria-label="Engagement stats">
        {metricCards.map((item) => (
          <MetricCard key={item.label} label={item.label} value={item.value} note={item.note} />
        ))}
      </section>

      <section className="ls-studio__command-grid">
        <ToolCard
          icon={view === "stream" ? <Radio size={18} strokeWidth={1.75} /> : <Edit3 size={18} strokeWidth={1.75} />}
          title={view === "stream" ? "Streaming editing hub" : "Series editing hub"}
          body={view === "stream"
            ? "Scene layout, clips, show notes, and broadcast packaging."
            : "Episode cuts, seasons, credits, release windows, and metadata polish."}
          action="Open hub"
          onClick={() => navigate(view === "stream" ? "/studio/tool/stream-editor" : "/studio/tool/series-editor")}
        />
        <ToolCard
          icon={view === "stream" ? <MessageSquare size={18} strokeWidth={1.75} /> : <FolderOpen size={18} strokeWidth={1.75} />}
          title={view === "stream" ? "Live operations console" : "File manager"}
          body={view === "stream"
            ? "Audience pulse, chat moderation, live tips, and stream health."
            : "Browse source files, processed media, thumbnails, captions, and delivery assets."}
          action="Open"
          onClick={() => navigate(view === "stream" ? "/studio/tool/live-ops" : "/studio/tool/file-manager")}
        />
      </section>

      {view === "stream" ? (
        <section className="ls-studio__dashboard-grid">
          <div className="ls-studio__panel ls-studio__panel--wide">
            <div className="ls-studio__panel-head">
              <div>
                <h2>Live room</h2>
                <p>{liveBroadcast ? "Broadcast is currently live." : "No active broadcast."}</p>
              </div>
              <Activity size={18} strokeWidth={1.75} />
            </div>
            <div className="ls-studio__live-room">
              <div>
                <span className="mono">Now</span>
                <strong>{liveBroadcast?.title ?? "Offline"}</strong>
                <p>{liveBroadcast ? `${liveBroadcast.category} / ${liveBroadcast.tags.join(" · ")}` : "Prepare the next broadcast from the live operations console."}</p>
              </div>
              <div className="ls-studio__live-grid">
                <div><span className="mono">Viewers</span>{formatViewers(liveBroadcast?.averageViewers ?? 0)}</div>
                <div><span className="mono">Tips</span>{money(liveBroadcast?.revenue ?? 0)}</div>
                <div><span className="mono">Chat</span>{formatNumber(liveBroadcast?.chatMessages ?? 0)}</div>
                <div><span className="mono">Followers</span>{formatNumber(liveBroadcast?.newFollowers ?? 0)}</div>
              </div>
            </div>
          </div>

          <div className="ls-studio__panel">
            <div className="ls-studio__panel-head">
              <div>
                <h2>Recent stream replays</h2>
                <p>{recentBroadcasts.length} broadcasts ready for review and clip packaging</p>
              </div>
              <Radio size={18} strokeWidth={1.75} />
            </div>
            <div className="ls-studio__media-grid">
              {recentBroadcasts.length === 0 ? <div className="ls-studio__empty">No recent streams yet.</div> : null}
              {recentBroadcasts.slice(0, 4).map((item) => (
                <MediaTile
                  key={item.id}
                  image={item.thumbnail}
                  title={item.title}
                  meta={`${item.status} / ${item.category}`}
                  value={`${formatViewers(item.peakViewers)} peak`}
                />
              ))}
            </div>
          </div>

          <div className="ls-studio__panel">
            <div className="ls-studio__panel-head">
              <div>
                <h2>Chat and tips</h2>
                <p>Live audience signals for the streamer side.</p>
              </div>
              <MessageSquare size={18} strokeWidth={1.75} />
            </div>
            <div className="ls-studio__signal-list">
              <div><Users size={14} />{formatNumber(liveBroadcast?.newSubscribers ?? 0)} new subscribers</div>
              <div><Wallet size={14} />{money(liveBroadcast?.revenue ?? streamRevenue)} captured tips and stream revenue</div>
              <div><MessageSquare size={14} />{formatNumber(streamChatCount)} chat messages tracked</div>
              <div><Eye size={14} />{formatViewers(liveBroadcast?.peakViewers ?? 0)} peak live viewers</div>
            </div>
          </div>

          <div className="ls-studio__panel ls-studio__panel--wide">
            <div className="ls-studio__panel-head">
              <div>
                <h2>Top content</h2>
                <p>What is pulling audience attention back to the platform.</p>
              </div>
              <BarChart3 size={18} strokeWidth={1.75} />
            </div>
            <div className="ls-studio__top-list">
              <div className="ls-studio__media-grid ls-studio__media-grid--wide">
                {topContent.slice(0, 4).map((item) => (
                  <MediaTile
                    key={item.id}
                    image={item.thumbnail}
                    title={item.title}
                    meta={`${item.kind} / ${formatNumber(item.views)} views`}
                    value={`${item.trend >= 0 ? "+" : ""}${pct(item.trend)} trend`}
                  />
                ))}
              </div>
              {topContent.slice(4, 7).map((item) => <TopContentRow key={item.id} item={item} />)}
              {topContent.length === 0 ? <div className="ls-studio__empty">No top content yet.</div> : null}
            </div>
          </div>
        </section>
      ) : (
        <section className="ls-studio__layout">
          <div className="ls-studio__panel ls-studio__panel--wide">
            <div className="ls-studio__panel-head">
              <div>
                <h2>Director slate</h2>
                <p>{recentUploads.length} newest episodes and films with release state, attention, and packaging.</p>
              </div>
              <Clapperboard size={18} strokeWidth={1.75} />
            </div>
            <div className="ls-studio__media-grid ls-studio__media-grid--wide">
              {recentUploads.length === 0 ? <div className="ls-studio__empty">No series or film uploads yet.</div> : null}
              {recentUploads.slice(0, 6).map((item) => (
                <MediaTile
                  key={item.id}
                  image={item.thumbnail}
                  title={item.title}
                  meta={`${item.status} / ${uploadLabel(item)}`}
                  value={`${formatNumber(item.views)} views`}
                />
              ))}
            </div>
          </div>

          <div className="ls-studio__panel">
            <div className="ls-studio__panel-head">
              <div>
                <h2>Series health</h2>
                <p>Publishing, storage, and attention snapshot.</p>
              </div>
              <Sparkles size={18} strokeWidth={1.75} />
            </div>
            <div className="ls-studio__facts">
              <div><span className="mono">Uploads</span>{formatNumber(selectedUploads.length)}</div>
              <div><span className="mono">Storage</span>{formatBytes(selectedUploads.reduce((sum, item) => sum + item.sizeBytes, 0))}</div>
              <div><span className="mono">Latest</span>{compactDate(recentUploads[0]?.publishedAt ?? recentUploads[0]?.uploadedAt)}</div>
              <div><span className="mono">Engagement</span>{formatNumber(seriesEngagements)}</div>
            </div>
            {featuredUpload ? (
              <div className="ls-studio__release-card">
                <img src={featuredUpload.thumbnail} alt="" />
                <span className="ls-studio__pill mono">{featuredUpload.status}</span>
                <strong>{featuredUpload.title}</strong>
                <em>{formatRuntime(featuredUpload.durationSec)} / {featuredUpload.resolution}</em>
              </div>
            ) : null}
          </div>

          <div className="ls-studio__panel">
            <div className="ls-studio__panel-head">
              <div>
                <h2>Create upload</h2>
                <p>Prepare a media slot or ingest a local file.</p>
              </div>
              <UploadCloud size={18} strokeWidth={1.75} />
            </div>
            <div className="ls-studio__form">
              <label className="ls-studio__field">
                <span className="mono">Title</span>
                <Input value={form.title} onChange={(event) => updateField("title", event.target.value)} placeholder="Pilot cut" />
              </label>
              <label className="ls-studio__field">
                <span className="mono">Media file</span>
                <input
                  className="ls-studio__file"
                  type="file"
                  accept="video/*,audio/*"
                  onChange={(event) => {
                    const file = event.currentTarget.files?.[0] ?? null;
                    setSelectedFile(file);
                    if (file) {
                      setForm((current) => ({
                        ...current,
                        title: current.title || file.name.replace(/\.[^.]+$/, ""),
                        bytesExpected: String(file.size),
                        mimeType: file.type || current.mimeType,
                      }));
                    }
                  }}
                />
                {selectedFile ? <span className="ls-studio__file-meta mono">{selectedFile.name} / {formatBytes(selectedFile.size)}</span> : null}
              </label>
              <label className="ls-studio__field">
                <span className="mono">Storage key</span>
                <Input value={form.storageKey} onChange={(event) => updateField("storageKey", event.target.value)} placeholder="Auto-generated from title" />
              </label>
              <div className="ls-studio__split">
                <label className="ls-studio__field">
                  <span className="mono">Kind</span>
                  <select value={form.kind} onChange={(event) => updateField("kind", event.target.value)}>
                    {kindOptions.map((item) => <option key={item} value={item}>{item}</option>)}
                  </select>
                </label>
                <label className="ls-studio__field">
                  <span className="mono">Visibility</span>
                  <select value={form.intendedVisibility} onChange={(event) => updateField("intendedVisibility", event.target.value)}>
                    {visibilityOptions.map((item) => <option key={item} value={item}>{item}</option>)}
                  </select>
                </label>
              </div>
              <div className="ls-studio__split">
                <label className="ls-studio__field">
                  <span className="mono">Bytes expected</span>
                  <Input type="number" min={1} value={form.bytesExpected} onChange={(event) => updateField("bytesExpected", event.target.value)} />
                </label>
                <label className="ls-studio__field">
                  <span className="mono">MIME type</span>
                  <Input value={form.mimeType} onChange={(event) => updateField("mimeType", event.target.value)} />
                </label>
              </div>
              <Button variant="primary" icon={<UploadCloud />} onClick={() => void uploadSelectedFile()} disabled={saving || !selectedFile}>
                {saving ? "Working..." : "Create and ingest file"}
              </Button>
              <Button variant="outline" icon={<UploadCloud />} onClick={() => void createJob()} disabled={saving}>
                {saving ? "Creating..." : "Create job only"}
              </Button>
            </div>
          </div>

          <div className="ls-studio__panel">
            <div className="ls-studio__panel-head">
              <div>
                <h2>Upload jobs</h2>
                <p>{loading ? "Loading..." : `${jobs.length} queued or processed jobs`}</p>
              </div>
              <FileVideo size={18} strokeWidth={1.75} />
            </div>
            <div className="ls-studio__jobs">
              {jobs.length === 0 && !loading ? <div className="ls-studio__empty">No upload jobs yet.</div> : null}
              {jobs.map((job) => (
                <button key={job.id} type="button" className={`ls-studio__job ${job.id === selectedId ? "is-active" : ""}`} onClick={() => selectJob(job)}>
                  <span className="ls-studio__job-title">{job.title}</span>
                  <span className="ls-studio__job-meta mono">{job.status} / {job.intendedVisibility} / {formatBytes(job.bytesExpected)}</span>
                </button>
              ))}
            </div>
          </div>

          <div className="ls-studio__panel">
            <div className="ls-studio__panel-head">
              <div>
                <h2>Metadata</h2>
                <p>{selectedJob ? selectedJob.id : "Select a job to edit."}</p>
              </div>
              <Save size={18} strokeWidth={1.75} />
            </div>
            {selectedJob ? (
              <div className="ls-studio__form">
                <label className="ls-studio__field">
                  <span className="mono">Title</span>
                  <Input value={selectedTitle} onChange={(event) => setSelectedTitle(event.target.value)} />
                </label>
                <div className="ls-studio__split">
                  <label className="ls-studio__field">
                    <span className="mono">Visibility</span>
                    <select value={selectedVisibility} onChange={(event) => setSelectedVisibility(event.target.value)}>
                      {visibilityOptions.map((item) => <option key={item} value={item}>{item}</option>)}
                    </select>
                  </label>
                  <label className="ls-studio__field">
                    <span className="mono">MIME type</span>
                    <Input value={selectedMimeType} onChange={(event) => setSelectedMimeType(event.target.value)} />
                  </label>
                </div>
              <div className="ls-studio__facts">
                <div><span className="mono">Storage</span>{selectedJob.storageKey}</div>
                <div><span className="mono">Received</span>{formatBytes(selectedJob.bytesReceived)}</div>
                <div><span className="mono">Progress</span>{uploadProgress}%</div>
                <div><span className="mono">Updated</span>{formatRelativeTime(selectedJob.updatedAt)}</div>
              </div>
                <Button variant="primary" icon={<Save />} onClick={() => void saveSelectedJob()} disabled={saving}>
                  {saving ? "Saving..." : "Save metadata"}
                </Button>
              </div>
            ) : (
              <div className="ls-studio__empty">Select an upload job.</div>
            )}
          </div>

          <div className="ls-studio__panel">
            <div className="ls-studio__panel-head">
              <div>
                <h2>Publish</h2>
                <p>{selectedJob ? selectedJob.status : "Select a ready upload job."}</p>
              </div>
              <Send size={18} strokeWidth={1.75} />
            </div>
            {selectedJob ? (
              <div className="ls-studio__form">
                <label className="ls-studio__field">
                  <span className="mono">Slug</span>
                  <Input value={publishForm.slug} onChange={(event) => updatePublishField("slug", event.target.value)} placeholder="pilot-cut" />
                </label>
                <label className="ls-studio__field">
                  <span className="mono">Description</span>
                  <textarea className="ls-studio__textarea" value={publishForm.description} onChange={(event) => updatePublishField("description", event.target.value)} rows={4} />
                </label>
                <label className="ls-studio__field">
                  <span className="mono">Visibility</span>
                  <select value={publishForm.visibility} onChange={(event) => updatePublishField("visibility", event.target.value)}>
                    {visibilityOptions.map((item) => <option key={item} value={item}>{item}</option>)}
                  </select>
                </label>
                <div className="ls-studio__facts">
                  <div><span className="mono">Asset</span>{selectedAsset?.status ?? "pending"}</div>
                  <div><span className="mono">Playback</span>{selectedAsset?.playbackPath ?? "pending"}</div>
                  <div><span className="mono">Content</span>{selectedJob.publishedContentId ?? "not published"}</div>
                </div>
                <Button variant="primary" icon={<Send />} onClick={() => void publishSelectedJob()} disabled={saving || !selectedJobCanPublish}>
                  {saving ? "Publishing..." : "Publish upload"}
                </Button>
              </div>
            ) : (
              <div className="ls-studio__empty">Select an upload job.</div>
            )}
          </div>
        </section>
      )}
    </div>
  );
}
