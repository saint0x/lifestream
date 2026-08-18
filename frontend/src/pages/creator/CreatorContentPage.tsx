import { useEffect, useState } from "react";
import {
  Upload as UploadIcon,
  Search,
  Filter,
  Globe,
  Lock,
  Link as LinkIcon,
  Trash2,
  Download,
} from "lucide-react";
import { CreatorLayout } from "@/components/creator/CreatorLayout";
import { Button } from "@/components/ui/Button";
import { requestJson } from "@/lib/api";
import type {
  CreatorContentResponse,
  Upload,
  UploadKind,
  UploadStatus,
  Visibility,
} from "@/types";
import { formatDuration, formatRelativeTime, formatViewers } from "@/lib/format";
import "./Creator.css";
import "./CreatorContentPage.css";

type KindFilter = "all" | UploadKind;
type StatusFilter = "all" | UploadStatus;
type RowAction = "make_public" | "make_unlisted" | "unpublish" | "archive" | "takedown" | "delete";

const kindFilters: ReadonlyArray<{ key: KindFilter; label: string }> = [
  { key: "all", label: "All" },
  { key: "film", label: "Films" },
  { key: "episode", label: "Episodes" },
  { key: "vod", label: "VODs" },
  { key: "clip", label: "Clips" },
  { key: "trailer", label: "Trailers" },
];

const statusFilters: ReadonlyArray<{ key: StatusFilter; label: string }> = [
  { key: "all", label: "Any status" },
  { key: "published", label: "Published" },
  { key: "scheduled", label: "Scheduled" },
  { key: "processing", label: "Processing" },
  { key: "draft", label: "Drafts" },
  { key: "archived", label: "Archived" },
  { key: "taken_down", label: "Taken down" },
];

const formatBytes = (bytes: number): string => {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(0)} MB`;
  return `${Math.round(bytes / 1e3)} KB`;
};

const visibilityIcon = (v: Visibility) => {
  if (v === "public") return <Globe size={11} />;
  if (v === "private") return <Lock size={11} />;
  return <LinkIcon size={11} />;
};

export function CreatorContentPage() {
  const [kind, setKind] = useState<KindFilter>("all");
  const [status, setStatus] = useState<StatusFilter>("all");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [sortKey, setSortKey] = useState<"uploaded" | "views" | "hours" | "title">(
    "uploaded",
  );
  const [content, setContent] = useState<CreatorContentResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [bulkPending, setBulkPending] = useState<string | null>(null);

  const loadContent = async (
    nextKind: KindFilter,
    nextStatus: StatusFilter,
    nextQuery: string,
    nextSortKey: "uploaded" | "views" | "hours" | "title",
  ) => {
    const params = new URLSearchParams();
    params.set("kind", nextKind);
    params.set("status", nextStatus);
    params.set("sort", nextSortKey);
    if (nextQuery.trim()) {
      params.set("q", nextQuery.trim());
    }
    return requestJson<CreatorContentResponse>(`/api/v1/creator/me/content?${params.toString()}`);
  };

  useEffect(() => {
    const controller = new AbortController();

    void (async () => {
      try {
        setLoading(true);
        setError(null);
        const nextContent = await loadContent(kind, status, query, sortKey);
        if (!controller.signal.aborted) {
          setContent(nextContent);
          setSelected((current) => new Set(
            [...current].filter((id) => nextContent.uploads.some((upload) => upload.id === id)),
          ));
        }
      } catch (nextError) {
        if (!controller.signal.aborted) {
          setError(
            nextError instanceof Error
              ? nextError.message
              : "Unable to load creator content.",
          );
        }
      } finally {
        if (!controller.signal.aborted) {
          setLoading(false);
        }
      }
    })();

    return () => controller.abort();
  }, [kind, status, query, sortKey]);

  const results = content?.uploads ?? [];
  const totals = content?.summary;

  const toggle = (id: string) => {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setSelected(next);
  };

  const toggleAll = () => {
    if (selected.size === results.length) setSelected(new Set());
    else setSelected(new Set(results.map((u) => u.id)));
  };

  const runBulkAction = async (action: "make_public" | "make_unlisted" | "archive" | "delete") => {
    const uploadIds = [...selected];
    if (uploadIds.length === 0) return;
    setBulkPending(action);
    setError(null);
    try {
      await requestJson<ReadonlyArray<Upload>>("/api/v1/creator/me/uploads/bulk", {
        method: "POST",
        body: {
          uploadIds,
          action,
        },
      });
      const nextContent = await loadContent(kind, status, query, sortKey);
      setContent(nextContent);
      setSelected(new Set());
    } catch (nextError) {
      setError(
        nextError instanceof Error ? nextError.message : "Unable to apply bulk content action.",
      );
    } finally {
      setBulkPending(null);
    }
  };

  const runRowAction = async (upload: Upload, action: RowAction) => {
    setError(null);
    setBulkPending(`${action}:${upload.id}`);
    try {
      if (action === "make_public" || action === "make_unlisted") {
        await requestJson<Upload>(`/api/v1/creator/me/uploads/${upload.id}/lifecycle`, {
          method: "PATCH",
          body: {
            visibility: action === "make_public" ? "public" : "unlisted",
          },
        });
      } else if (action === "unpublish") {
        await requestJson<Upload>(`/api/v1/creator/me/uploads/${upload.id}/unpublish`, {
          method: "POST",
        });
      } else if (action === "takedown") {
        await requestJson<Upload>(`/api/v1/creator/me/uploads/${upload.id}/takedown`, {
          method: "POST",
        });
      } else {
        await requestJson<ReadonlyArray<Upload>>("/api/v1/creator/me/uploads/bulk", {
          method: "POST",
          body: {
            uploadIds: [upload.id],
            action,
          },
        });
      }

      const nextContent = await loadContent(kind, status, query, sortKey);
      setContent(nextContent);
      setSelected((current) => {
        const next = new Set(current);
        next.delete(upload.id);
        return next;
      });
    } catch (nextError) {
      setError(
        nextError instanceof Error ? nextError.message : "Unable to update content lifecycle.",
      );
    } finally {
      setBulkPending(null);
    }
  };

  return (
    <CreatorLayout>
      <div className="ls-cpage">
        <header className="ls-cpage__head">
          <div>
            <h1 className="ls-cpage__title">Content</h1>
            <p className="ls-cpage__sub">
              Manage episodes, VODs, clips and trailers. Change visibility, schedule
              premieres, or pull things down.
            </p>
          </div>
          <div className="ls-cc__actions">
            <Button variant="ghost" icon={<Download />}>Export CSV</Button>
            <Button variant="primary" icon={<UploadIcon />}>Upload</Button>
          </div>
        </header>

        <section className="ls-cc__summary">
          <div className="ls-cc__sum-item">
            <div className="ls-cc__sum-num">{totals?.totalUploads ?? "—"}</div>
            <div className="ls-cc__sum-lbl mono">Total</div>
          </div>
          <div className="ls-cc__sum-item">
            <div className="ls-cc__sum-num">{totals?.publishedUploads ?? "—"}</div>
            <div className="ls-cc__sum-lbl mono">Published</div>
          </div>
          <div className="ls-cc__sum-item">
            <div className="ls-cc__sum-num">{totals?.scheduledUploads ?? "—"}</div>
            <div className="ls-cc__sum-lbl mono">Scheduled</div>
          </div>
          <div className="ls-cc__sum-item">
            <div className="ls-cc__sum-num">{totals?.processingUploads ?? "—"}</div>
            <div className="ls-cc__sum-lbl mono">Processing</div>
          </div>
          <div className="ls-cc__sum-item">
            <div className="ls-cc__sum-num">{totals?.draftUploads ?? "—"}</div>
            <div className="ls-cc__sum-lbl mono">Drafts</div>
          </div>
          <div className="ls-cc__sum-item">
            <div className="ls-cc__sum-num">{formatViewers(totals?.totalViews ?? 0)}</div>
            <div className="ls-cc__sum-lbl mono">Views</div>
          </div>
          <div className="ls-cc__sum-item">
            <div className="ls-cc__sum-num">{formatViewers(totals?.totalWatchHours ?? 0)}</div>
            <div className="ls-cc__sum-lbl mono">Watch hours</div>
          </div>
          <div className="ls-cc__sum-item">
            <div className="ls-cc__sum-num">{formatBytes(totals?.totalStorageBytes ?? 0)}</div>
            <div className="ls-cc__sum-lbl mono">Storage</div>
          </div>
        </section>

        <section className="ls-cc__filters">
          <div className="ls-cc__filter-group">
            {kindFilters.map((f) => (
              <button
                key={f.key}
                type="button"
                className={`ls-cc__tab ${kind === f.key ? "is-active" : ""}`}
                onClick={() => setKind(f.key)}
              >
                {f.label}
              </button>
            ))}
          </div>
          <label className="ls-cc__search">
            <Search size={14} />
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search by title…"
            />
          </label>
          <div className="ls-cc__dropdowns">
            <label className="ls-cc__select">
              <Filter size={12} />
              <select
                value={status}
                onChange={(e) => setStatus(e.target.value as StatusFilter)}
              >
                {statusFilters.map((f) => (
                  <option key={f.key} value={f.key}>
                    {f.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="ls-cc__select">
              <span className="mono">SORT</span>
              <select
                value={sortKey}
                onChange={(e) =>
                  setSortKey(e.target.value as "uploaded" | "views" | "hours" | "title")
                }
              >
                <option value="uploaded">Uploaded date</option>
                <option value="views">Views</option>
                <option value="hours">Watch hours</option>
                <option value="title">A–Z</option>
              </select>
            </label>
          </div>
        </section>

        {selected.size > 0 && (
          <div className="ls-cc__bulk">
            <div className="ls-cc__bulk-count mono">
              {selected.size} selected
            </div>
            <div className="ls-cc__bulk-actions">
              <Button
                size="sm"
                variant="ghost"
                onClick={() => void runBulkAction("make_public")}
                disabled={bulkPending !== null}
              >
                {bulkPending === "make_public" ? "Applying…" : "Make public"}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => void runBulkAction("make_unlisted")}
                disabled={bulkPending !== null}
              >
                {bulkPending === "make_unlisted" ? "Applying…" : "Make unlisted"}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => void runBulkAction("archive")}
                disabled={bulkPending !== null}
              >
                {bulkPending === "archive" ? "Applying…" : "Archive"}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                icon={<Trash2 />}
                onClick={() => void runBulkAction("delete")}
                disabled={bulkPending !== null}
              >
                {bulkPending === "delete" ? "Deleting…" : "Delete"}
              </Button>
              <Button size="sm" variant="ghost" onClick={() => setSelected(new Set())}>
                Clear
              </Button>
            </div>
          </div>
        )}

        <section className="ls-cc__table-wrap">
          <table className="ls-cc__table">
            <thead>
              <tr>
                <th className="ls-cc__th-check">
                  <input
                    type="checkbox"
                    checked={selected.size === results.length && results.length > 0}
                    onChange={toggleAll}
                    aria-label="Select all"
                  />
                </th>
                <th>Title</th>
                <th className="num">Views</th>
                <th className="num">Watch hours</th>
                <th>Visibility</th>
                <th>Status</th>
                <th>Uploaded</th>
                <th className="ls-cc__th-actions" />
              </tr>
            </thead>
            <tbody>
              {loading && (
                <tr>
                  <td colSpan={8}>
                    <div className="ls-cpage__empty">Loading creator content…</div>
                  </td>
                </tr>
              )}
              {!loading && error && (
                <tr>
                  <td colSpan={8}>
                    <div className="ls-cpage__empty">{error}</div>
                  </td>
                </tr>
              )}
              {!loading && !error && results.length === 0 && (
                <tr>
                  <td colSpan={8}>
                    <div className="ls-cpage__empty">No uploads match these filters.</div>
                  </td>
                </tr>
              )}
              {results.map((u) => (
                <UploadRow
                  key={u.id}
                  upload={u}
                  checked={selected.has(u.id)}
                  onToggle={() => toggle(u.id)}
                  pendingAction={bulkPending}
                  onAction={(action) => void runRowAction(u, action)}
                />
              ))}
            </tbody>
          </table>
        </section>
      </div>
    </CreatorLayout>
  );
}

function UploadRow({
  upload,
  checked,
  onToggle,
  pendingAction,
  onAction,
}: {
  readonly upload: Upload;
  readonly checked: boolean;
  readonly onToggle: () => void;
  readonly pendingAction: string | null;
  readonly onAction: (action: RowAction) => void;
}) {
  const actionKey = (action: RowAction) => `${action}:${upload.id}`;
  const allowPublicVisibility = upload.status !== "processing" && upload.status !== "taken_down";
  const canDelete =
    upload.status === "draft" || upload.status === "archived" || upload.status === "taken_down";
  const canArchive = upload.status !== "processing" && upload.status !== "taken_down";
  const canUnpublish = upload.status !== "taken_down" && upload.status !== "processing";

  return (
    <tr className={checked ? "is-selected" : ""}>
      <td className="ls-cc__td-check">
        <input type="checkbox" checked={checked} onChange={onToggle} aria-label={`Select ${upload.title}`} />
      </td>
      <td>
        <div className="ls-cc__row-main">
          <div className="ls-cc__row-thumb">
            <img src={upload.thumbnail} alt="" />
            <div className="ls-cc__row-dur mono">{formatDuration(upload.durationSec)}</div>
            {upload.status === "processing" && upload.transcodeProgress !== undefined && (
              <div className="ls-cc__row-progress">
                <div
                  className="ls-cc__row-progress-fill"
                  style={{ width: `${upload.transcodeProgress * 100}%` }}
                />
              </div>
            )}
          </div>
          <div className="ls-cc__row-meta">
            <div className="ls-cc__row-title">{upload.title}</div>
            <div className="ls-cc__row-sub mono">
              {upload.kind.toUpperCase()} · {upload.resolution} · {formatBytes(upload.sizeBytes)}
              {upload.seriesTitle !== undefined && ` · ${upload.seriesTitle}`}
            </div>
            <div className="ls-cc__row-desc">{upload.description}</div>
          </div>
        </div>
      </td>
      <td className="num mono">{formatViewers(upload.views)}</td>
      <td className="num mono">{formatViewers(upload.watchHours)}h</td>
      <td>
        <span className="ls-cc__vis mono">
          {visibilityIcon(upload.visibility)}
          {upload.visibility}
        </span>
      </td>
      <td>
        <span className={`ls-cpage__chip ls-cpage__chip--${upload.status}`}>
          {upload.status}
        </span>
      </td>
      <td className="mono ls-cc__td-date">
        {formatRelativeTime(upload.uploadedAt)}
      </td>
      <td className="ls-cc__td-actions">
        <div className="ls-cc__row-actions">
          {upload.visibility !== "public" && allowPublicVisibility ? (
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={() => onAction("make_public")}
              disabled={pendingAction !== null}
            >
              {pendingAction === actionKey("make_public") ? "Applying…" : "Public"}
            </Button>
          ) : null}
          {upload.visibility !== "unlisted" && allowPublicVisibility ? (
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={() => onAction("make_unlisted")}
              disabled={pendingAction !== null}
            >
              {pendingAction === actionKey("make_unlisted") ? "Applying…" : "Unlist"}
            </Button>
          ) : null}
          {upload.status === "published" && canUnpublish ? (
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={() => onAction("unpublish")}
              disabled={pendingAction !== null}
            >
              {pendingAction === actionKey("unpublish") ? "Applying…" : "Unpublish"}
            </Button>
          ) : null}
          {upload.status !== "archived" && canArchive ? (
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={() => onAction("archive")}
              disabled={pendingAction !== null}
            >
              {pendingAction === actionKey("archive") ? "Applying…" : "Archive"}
            </Button>
          ) : null}
          {upload.status !== "taken_down" ? (
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={() => onAction("takedown")}
              disabled={pendingAction !== null}
            >
              {pendingAction === actionKey("takedown") ? "Applying…" : "Takedown"}
            </Button>
          ) : null}
          {canDelete ? (
            <Button
              type="button"
              size="sm"
              variant="ghost"
              icon={<Trash2 size={12} />}
              onClick={() => onAction("delete")}
              disabled={pendingAction !== null}
            >
              {pendingAction === actionKey("delete") ? "Deleting…" : "Delete"}
            </Button>
          ) : null}
        </div>
      </td>
    </tr>
  );
}
