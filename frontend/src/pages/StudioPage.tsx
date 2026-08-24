import { useCallback, useEffect, useMemo, useState } from "react";
import { Check, FileVideo, RefreshCw, Save, UploadCloud } from "lucide-react";
import { repository } from "@/lib/repository";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import type { MediaAsset, UploadJob } from "@/types";
import "./StudioPage.css";

const visibilityOptions = ["private", "unlisted", "public"] as const;
const kindOptions = ["film", "episode"] as const;

interface UploadJobForm {
  readonly kind: string;
  readonly title: string;
  readonly intendedVisibility: string;
  readonly bytesExpected: string;
  readonly storageKey: string;
  readonly mimeType: string;
}

const initialForm: UploadJobForm = {
  kind: "film",
  title: "",
  intendedVisibility: "private",
  bytesExpected: "1048576",
  storageKey: "",
  mimeType: "video/mp4",
};

function storageKeyFromTitle(title: string): string {
  const slug = title
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64);
  return `studio/${Date.now()}/${slug || "untitled-upload"}.mp4`;
}

function formatBytes(value: number): string {
  if (value >= 1024 * 1024 * 1024) return `${(value / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  if (value >= 1024 * 1024) return `${(value / (1024 * 1024)).toFixed(1)} MB`;
  if (value >= 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${value} B`;
}

export function StudioPage() {
  const [jobs, setJobs] = useState<ReadonlyArray<UploadJob>>([]);
  const [form, setForm] = useState<UploadJobForm>(initialForm);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selectedTitle, setSelectedTitle] = useState("");
  const [selectedVisibility, setSelectedVisibility] = useState("private");
  const [selectedMimeType, setSelectedMimeType] = useState("video/mp4");
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

  const selectJob = useCallback((job: UploadJob) => {
    setSelectedId(job.id);
    setSelectedTitle(job.title);
    setSelectedVisibility(job.intendedVisibility);
    setSelectedMimeType(job.mimeType);
  }, []);

  const loadJobs = useCallback(async (signal?: AbortSignal) => {
    setError(null);
    const [nextJobs, nextAssets] = await Promise.all([
      repository.listUploadJobs(signal),
      repository.listMediaAssets(signal),
    ]);
    setJobs(nextJobs);
    setAssets(nextAssets);
    const firstJob = nextJobs[0];
    if (firstJob) {
      setSelectedId((currentId) => {
        if (currentId) return currentId;
        setSelectedTitle(firstJob.title);
        setSelectedVisibility(firstJob.intendedVisibility);
        setSelectedMimeType(firstJob.mimeType);
        return firstJob.id;
      });
    }
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    void loadJobs(controller.signal)
      .catch((err) => {
        if (!controller.signal.aborted) {
          setError(err instanceof Error ? err.message : "Unable to load studio uploads.");
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [loadJobs]);

  const updateField = (field: keyof UploadJobForm, value: string) => {
    setForm((current) => ({ ...current, [field]: value }));
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

  return (
    <div className="ls-studio">
      <header className="ls-studio__head">
        <div className="ls-studio__kicker mono">/ creator / studio</div>
        <div className="ls-studio__title-row">
          <div>
            <h1 className="ls-studio__title">Studio</h1>
            <p className="ls-studio__sub">
              Create upload jobs, prepare metadata, and keep media handoff state visible.
            </p>
          </div>
          <Button
            variant="outline"
            icon={<RefreshCw />}
            onClick={() => {
              setLoading(true);
              void loadJobs().finally(() => setLoading(false));
            }}
            disabled={loading || saving}
          >
            Refresh
          </Button>
        </div>
      </header>

      {status ? <div className="ls-studio__notice"><Check size={14} />{status}</div> : null}
      {error ? <div className="ls-studio__error">{error}</div> : null}

      <section className="ls-studio__layout">
        <div className="ls-studio__panel">
          <div className="ls-studio__panel-head">
            <div>
              <h2>Create upload job</h2>
              <p>Prepare a media slot before upload ingest starts.</p>
            </div>
            <UploadCloud size={18} strokeWidth={1.75} />
          </div>

          <div className="ls-studio__form">
            <label className="ls-studio__field">
              <span className="mono">Title</span>
              <Input
                value={form.title}
                onChange={(event) => updateField("title", event.target.value)}
                placeholder="Pilot cut"
              />
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
              {selectedFile ? (
                <span className="ls-studio__file-meta mono">
                  {selectedFile.name} / {formatBytes(selectedFile.size)}
                </span>
              ) : null}
            </label>
            <label className="ls-studio__field">
              <span className="mono">Storage key</span>
              <Input
                value={form.storageKey}
                onChange={(event) => updateField("storageKey", event.target.value)}
                placeholder="Auto-generated from title"
              />
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
                <select
                  value={form.intendedVisibility}
                  onChange={(event) => updateField("intendedVisibility", event.target.value)}
                >
                  {visibilityOptions.map((item) => <option key={item} value={item}>{item}</option>)}
                </select>
              </label>
            </div>
            <div className="ls-studio__split">
              <label className="ls-studio__field">
                <span className="mono">Bytes expected</span>
                <Input
                  type="number"
                  min={1}
                  value={form.bytesExpected}
                  onChange={(event) => updateField("bytesExpected", event.target.value)}
                />
              </label>
              <label className="ls-studio__field">
                <span className="mono">MIME type</span>
                <Input
                  value={form.mimeType}
                  onChange={(event) => updateField("mimeType", event.target.value)}
                />
              </label>
            </div>
            <Button
              variant="primary"
              icon={<UploadCloud />}
              onClick={() => void createJob()}
              disabled={saving}
            >
              {saving ? "Creating..." : "Create job"}
            </Button>
            <Button
              variant="outline"
              icon={<UploadCloud />}
              onClick={() => void uploadSelectedFile()}
              disabled={saving || !selectedFile}
            >
              {saving ? "Working..." : "Create and ingest file"}
            </Button>
          </div>
        </div>

        <div className="ls-studio__panel ls-studio__panel--jobs">
          <div className="ls-studio__panel-head">
            <div>
              <h2>Upload jobs</h2>
              <p>{loading ? "Loading..." : `${jobs.length} queued or processed jobs`}</p>
            </div>
            <FileVideo size={18} strokeWidth={1.75} />
          </div>

          <div className="ls-studio__jobs">
            {jobs.length === 0 && !loading ? (
              <div className="ls-studio__empty">No upload jobs yet.</div>
            ) : null}
            {jobs.map((job) => (
              <button
                key={job.id}
                type="button"
                className={`ls-studio__job ${job.id === selectedId ? "is-active" : ""}`}
                onClick={() => selectJob(job)}
              >
                <span className="ls-studio__job-title">{job.title}</span>
                <span className="ls-studio__job-meta mono">
                  {job.status} / {job.intendedVisibility} / {formatBytes(job.bytesExpected)}
                </span>
              </button>
            ))}
          </div>
        </div>

        <div className="ls-studio__panel ls-studio__panel--editor">
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
                  <select
                    value={selectedVisibility}
                    onChange={(event) => setSelectedVisibility(event.target.value)}
                  >
                    {visibilityOptions.map((item) => <option key={item} value={item}>{item}</option>)}
                  </select>
                </label>
                <label className="ls-studio__field">
                  <span className="mono">MIME type</span>
                  <Input
                    value={selectedMimeType}
                    onChange={(event) => setSelectedMimeType(event.target.value)}
                  />
                </label>
              </div>
              <div className="ls-studio__facts">
                <div><span className="mono">Storage</span>{selectedJob.storageKey}</div>
                <div><span className="mono">Received</span>{formatBytes(selectedJob.bytesReceived)}</div>
                <div><span className="mono">Updated</span>{selectedJob.updatedAt}</div>
              </div>
              <Button
                variant="primary"
                icon={<Save />}
                onClick={() => void saveSelectedJob()}
                disabled={saving}
              >
                {saving ? "Saving..." : "Save metadata"}
              </Button>
            </div>
          ) : (
            <div className="ls-studio__empty">Select an upload job.</div>
          )}
        </div>

        <div className="ls-studio__panel ls-studio__panel--assets">
          <div className="ls-studio__panel-head">
            <div>
              <h2>Media assets</h2>
              <p>{assets.length} uploaded shells and processed assets</p>
            </div>
            <FileVideo size={18} strokeWidth={1.75} />
          </div>
          <div className="ls-studio__jobs">
            {assets.length === 0 ? <div className="ls-studio__empty">No media assets yet.</div> : null}
            {assets.map((asset) => (
              <div key={asset.id} className="ls-studio__asset-row">
                <span className="ls-studio__job-title">{asset.title}</span>
                <span className="ls-studio__job-meta mono">
                  {asset.status} / {asset.visibility} / {formatBytes(asset.fileSizeBytes)}
                </span>
              </div>
            ))}
          </div>
        </div>
      </section>
    </div>
  );
}
