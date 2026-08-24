import { useCallback, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { useNavigate, useParams } from "react-router-dom";
import {
  ArrowLeft,
  Check,
  Clapperboard,
  Eye,
  FileVideo,
  FolderOpen,
  Gauge,
  MessageSquare,
  Plus,
  Radio,
  Save,
  Scissors,
  Send,
  UploadCloud,
  Users,
  Wallet,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { PageTrail } from "@/components/navigation/PageTrail";
import { repository } from "@/lib/repository";
import { formatNumber, formatRelativeTime, formatRuntime, formatViewers } from "@/lib/format";
import type { Broadcast, CreatorDashboardPayload, MediaAsset, UploadJob } from "@/types";
import "./StudioPage.css";

type StudioToolKind = "stream-editor" | "live-ops" | "series-editor" | "file-manager";
const visibilityOptions = ["private", "unlisted", "public"] as const;
const uploadKindOptions = ["episode", "film", "clip", "trailer", "video", "vod", "live_archive"] as const;

interface StudioToolConfig {
  readonly title: string;
  readonly trailLabel: string;
  readonly body: string;
}

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

const initialUploadForm: UploadJobForm = {
  kind: "episode",
  title: "",
  intendedVisibility: "private",
  bytesExpected: "1048576",
  storageKey: "",
  mimeType: "video/mp4",
};

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
  const [form, setForm] = useState<UploadJobForm>(initialUploadForm);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [uploadModalOpen, setUploadModalOpen] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selectedTitle, setSelectedTitle] = useState("");
  const [selectedVisibility, setSelectedVisibility] = useState("private");
  const [selectedMimeType, setSelectedMimeType] = useState("video/mp4");
  const [publishForm, setPublishForm] = useState<PublishForm>({
    slug: "",
    description: "",
    visibility: "private",
  });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

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
      setForm(initialUploadForm);
      setUploadModalOpen(false);
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
      setUploadModalOpen(false);
      setForm(initialUploadForm);
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
      await loadTool();
      setStatus(`Published ${published.title}.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unable to publish upload.");
      setStatus(null);
    } finally {
      setSaving(false);
    }
  };

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
          <div className="ls-studio__title-actions">
            <Button variant="primary" icon={<Plus />} onClick={() => setUploadModalOpen(true)}>
              Upload
            </Button>
            <Button variant="outline" icon={<ArrowLeft />} onClick={() => navigate("/studio")}>
              Back to Studio
            </Button>
          </div>
        </div>
      </header>

      {status ? <div className="ls-studio__notice"><Check size={14} />{status}</div> : null}
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
                  <button
                    className={`ls-studio__asset-row ${item.id === selectedId ? "is-active" : ""}`}
                    key={item.id}
                    type="button"
                    onClick={() => selectJob(item)}
                  >
                    <strong className="ls-studio__job-title">{item.title}</strong>
                    <span className="ls-studio__job-meta mono">{item.status} / {item.kind} / {formatBytes(item.bytesReceived)} received</span>
                  </button>
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
        </>
      ) : null}

      {uploadModalOpen ? (
        <div className="ls-studio__modal-backdrop" role="presentation" onMouseDown={() => setUploadModalOpen(false)}>
          <div
            className="ls-studio__modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="studio-upload-title"
            onMouseDown={(event) => event.stopPropagation()}
          >
            <button
              className="ls-studio__modal-close"
              type="button"
              aria-label="Close upload modal"
              onClick={() => setUploadModalOpen(false)}
            >
              <X size={15} strokeWidth={1.8} />
            </button>
            <div className="ls-studio__panel-head">
              <div>
                <h2 id="studio-upload-title">Upload Job</h2>
                <p>Create a media slot or ingest a local source file.</p>
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
                    {uploadKindOptions.map((item) => <option key={item} value={item}>{item}</option>)}
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
              <div className="ls-studio__modal-actions">
                <Button variant="primary" icon={<UploadCloud />} onClick={() => void uploadSelectedFile()} disabled={saving || !selectedFile}>
                  {saving ? "Working..." : "Create and ingest file"}
                </Button>
                <Button variant="outline" icon={<UploadCloud />} onClick={() => void createJob()} disabled={saving}>
                  {saving ? "Creating..." : "Create job only"}
                </Button>
              </div>
            </div>
          </div>
        </div>
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
