import { useEffect, useMemo, useRef, useState } from "react";
import type { ComponentType } from "react";
import {
  AlertCircle,
  CheckCircle2,
  Download,
  Film,
  Link,
  MessageSquare,
  PackageCheck,
  Radio,
  Save,
  Scissors,
  Search,
  ShieldCheck,
  StepBack,
  StepForward,
  Upload,
  Wand2,
} from "lucide-react";
import type { LucideProps } from "lucide-react";
import { VideoPlayer } from "@/components/player/VideoPlayer";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import {
  createEditorClip,
  createEditorComment,
  createEditorProofLink,
  createEditorRenderJob,
  fetchEditorExports,
  fetchEditorProofLinks,
  fetchEditorProject,
  fetchEditorProjects,
  fetchEditorRenderJobs,
  fetchEditorReviewRequests,
  patchEditorTimeline,
  publishEditorExport,
  submitEditorAdvertiserReview,
  updateEditorClip,
  uploadEditorAsset,
  validateEditorAdSlot,
  type EditorBundle,
  type EditorProject,
} from "@/lib/api";

const poster = "https://images.unsplash.com/photo-1485846234645-a62644f84728?auto=format&fit=crop&w=1600&q=80";

type DragState = {
  readonly clipId: string;
  readonly mode: "move" | "trim-start" | "trim-end";
  readonly startX: number;
  readonly originalIn: number;
  readonly originalOut: number;
};

type RailItem = readonly [string, ComponentType<LucideProps>];

const railItems: readonly RailItem[] = [
  ["media", Film],
  ["campaign", ShieldCheck],
  ["transcript", MessageSquare],
  ["comments", MessageSquare],
  ["renders", Radio],
];

export function EditorApp() {
  const fileRef = useRef<HTMLInputElement>(null);
  const [projects, setProjects] = useState<EditorProject[]>([]);
  const [bundle, setBundle] = useState<EditorBundle | null>(null);
  const [exports, setExports] = useState<Record<string, any>[]>([]);
  const [jobs, setJobs] = useState<Record<string, any>[]>([]);
  const [proofLinks, setProofLinks] = useState<Record<string, any>[]>([]);
  const [reviewRequests, setReviewRequests] = useState<Record<string, any>[]>([]);
  const [activePanel, setActivePanel] = useState("media");
  const [notice, setNotice] = useState("Loading editor state");
  const [loading, setLoading] = useState(true);

  const project = bundle?.project;
  const timelineBundle = bundle?.timeline;
  const timeline = timelineBundle?.timeline;
  const adSlots = timelineBundle?.ad_slots ?? [];
  const activeSlot = adSlots[0];
  const uiState = timeline?.ui_state_json ?? {};
  const playhead = Number(uiState.playhead_seconds ?? 0);
  const duration = Math.max(Number(timeline?.duration_seconds ?? 742), 1);

  useEffect(() => {
    void hydrate();
  }, []);

  async function hydrate() {
    setLoading(true);
    try {
      const list = await fetchEditorProjects();
      setProjects(list);
      const selected = list[0];
      if (selected) {
        const next = await fetchEditorProject(selected.id);
        setBundle(next);
        setExports(await fetchEditorExports(selected.id));
        setJobs(await fetchEditorRenderJobs(selected.id));
        setProofLinks(await fetchEditorProofLinks(selected.id));
        setReviewRequests(await fetchEditorReviewRequests(selected.id));
        setNotice("Editor state synced");
      }
    } catch (error) {
      setNotice(error instanceof Error ? error.message : "Editor state could not be loaded");
    } finally {
      setLoading(false);
    }
  }

  async function seek(seconds: number) {
    if (!project) return;
    const next = await patchEditorTimeline(project.id, { playhead_seconds: Math.max(0, Math.min(duration, seconds)) });
    setBundle((current) => current ? { ...current, timeline: next } : current);
  }

  async function saveVersion() {
    if (!project || !timelineBundle) return;
    const next = await patchEditorTimeline(project.id, {
      change_summary: "Editor checkpoint",
      edl_json: timelineBundle,
    });
    setBundle((current) => current ? { ...current, timeline: next } : current);
    setNotice("Timeline version saved");
  }

  async function splitAtPlayhead() {
    if (!project || !timelineBundle) return;
    const clip = timelineBundle.clips.find((item) => {
      const start = Number(item.timeline_in_seconds);
      const end = Number(item.timeline_out_seconds);
      return playhead > start + 0.25 && playhead < end - 0.25;
    });
    if (!clip) {
      setNotice("Move the playhead inside a clip to split");
      return;
    }

    const clipStart = Number(clip.timeline_in_seconds);
    const sourceStart = Number(clip.source_in_seconds);
    const splitSource = sourceStart + ((playhead - clipStart) * Number(clip.speed ?? 1));
    const metadata = typeof clip.metadata_json === "object" && clip.metadata_json !== null ? clip.metadata_json : {};
    await updateEditorClip(String(clip.id), {
      ...clip,
      source_out_seconds: splitSource,
      timeline_out_seconds: playhead,
      metadata_json: { ...metadata, split_side: "left" },
    });
    await createEditorClip(project.id, {
      ...clip,
      label: `${clip.label ?? "Timeline clip"} split`,
      source_in_seconds: splitSource,
      timeline_in_seconds: playhead,
      metadata_json: { ...metadata, split_from_clip_id: clip.id },
    });
    await hydrate();
    setNotice("Clip split saved");
  }

  async function validateInventory() {
    if (!activeSlot) return;
    const result = await validateEditorAdSlot(String(activeSlot.id));
    setNotice(result.valid ? "Sold inventory is render-safe" : `Blocked: ${result.blockers.join(", ")}`);
    await hydrate();
  }

  async function uploadSelected(file?: File) {
    if (!project || !file) return;
    setNotice("Uploading and generating proxy media");
    await uploadEditorAsset(project.id, file, "raw_video");
    await hydrate();
    setNotice("Upload ready with proxy, thumbnail, and waveform");
  }

  async function addComment() {
    if (!project) return;
    await createEditorComment(project.id, {
      body: "Review this frame before advertiser proof.",
      visibility: "vanta_internal",
      timeline_seconds: playhead,
    });
    await hydrate();
    setNotice("Frame comment added");
  }

  async function renderCut() {
    if (!project) return;
    const job = await createEditorRenderJob(project.id, {
      export_kind: "advertiser_review_cut",
      target: "review-hls",
    });
    await hydrate();
    setNotice(`Render ${job.status} at ${Math.round(Number(job.progress ?? 0) * 100)}%`);
  }

  async function proof(exportId: string) {
    const link = await createEditorProofLink(exportId);
    await hydrate();
    setNotice(`Proof link ready: ${link.url}`);
  }

  async function submitReview(exportId: string) {
    const result = await submitEditorAdvertiserReview(exportId);
    await hydrate();
    setNotice(`Advertiser room submitted: ${result.proof_link?.url ?? "Ad Hub"}`);
  }

  async function publish(exportId: string) {
    await publishEditorExport(exportId);
    await hydrate();
    setNotice("Export published into Vanta media pipeline");
  }

  const validationCount = useMemo(() => adSlots.reduce((count, slot) => {
    const blockers = slot.validation_json?.blockers;
    return count + (Array.isArray(blockers) ? blockers.length : 0);
  }, 0), [adSlots]);

  if (loading) {
    return (
      <main className="ve-boot grain">
        <div className="mono uppercase faint">vanta / editor</div>
        <h1>Opening edit bay</h1>
        <p>{notice}</p>
      </main>
    );
  }

  return (
    <div className="ve-app grain">
      <aside className="ve-rail" aria-label="Editor navigation">
        {railItems.map(([key, Icon]) => (
          <button key={String(key)} className={activePanel === key ? "is-active" : ""} title={String(key)} type="button" onClick={() => setActivePanel(String(key))}>
            <Icon size={18} />
          </button>
        ))}
      </aside>

      <main className="ve-shell">
        <header className="ve-topbar">
          <div className="ve-project">
            <span className="mono uppercase faint">vanta editor</span>
            <h1>{project?.title}</h1>
          </div>
          <div className="ve-topbar__meta">
            <Badge tone={validationCount === 0 ? "premium" : "live"} icon={validationCount === 0 ? <CheckCircle2 size={12} /> : <AlertCircle size={12} />}>
              {validationCount === 0 ? "Render safe" : `${validationCount} blockers`}
            </Badge>
            <span className="mono faint">{notice}</span>
          </div>
          <div className="ve-actions">
            <Button size="sm" icon={<Save size={15} />} onClick={saveVersion}>Save</Button>
            <Button size="sm" icon={<Wand2 size={15} />} onClick={validateInventory}>Validate</Button>
            <Button size="sm" variant="primary" icon={<Download size={15} />} onClick={renderCut}>Render</Button>
          </div>
        </header>

        <section className="ve-workspace">
          <Panel title={activePanel} projectCount={projects.length}>
            {activePanel === "media" && (
              <MediaBin assets={bundle?.assets ?? []} onUpload={() => fileRef.current?.click()} />
            )}
            {activePanel === "campaign" && <Requirements items={bundle?.requirements ?? []} />}
            {activePanel === "transcript" && <Transcript items={timelineBundle?.transcript ?? []} seek={seek} playhead={playhead} />}
            {activePanel === "comments" && <Comments comments={bundle?.comments ?? []} onAdd={addComment} />}
            {activePanel === "renders" && (
              <RenderPanel
                jobs={jobs}
                exports={exports}
                proofLinks={proofLinks}
                reviewRequests={reviewRequests}
                onRender={renderCut}
                onProof={proof}
                onPublish={publish}
                onSubmitReview={submitReview}
              />
            )}
          </Panel>

          <section className="ve-stage" aria-label="Video preview">
            <input ref={fileRef} type="file" accept="video/*,audio/*" hidden onChange={(event) => void uploadSelected(event.currentTarget.files?.[0])} />
            <div className="ve-stage__tools">
              <Button size="sm" icon={<StepBack size={15} />} onClick={() => void seek(playhead - 1)}>Frame</Button>
              <Button size="sm" icon={<Scissors size={15} />} onClick={() => void splitAtPlayhead()}>Split</Button>
              <Button size="sm" icon={<PackageCheck size={15} />} onClick={validateInventory}>Safe</Button>
              <Button size="sm" icon={<StepForward size={15} />} onClick={() => void seek(playhead + 1)}>Frame</Button>
            </div>
            <VideoPlayer poster={poster} title={project?.title ?? "Vanta edit"} subtitle="Editor preview" durationSec={duration} initialProgressSec={playhead} onProgress={seek} />
          </section>

          <Inspector slot={activeSlot} project={project} />
        </section>

        <Timeline bundle={timelineBundle} playhead={playhead} duration={duration} seek={seek} onClipCommit={async (clip) => {
          await updateEditorClip(String(clip.id), clip);
          await hydrate();
          setNotice("Timeline edit saved");
        }} />
      </main>
    </div>
  );
}

function Panel({ title, projectCount, children }: { readonly title: string; readonly projectCount: number; readonly children: React.ReactNode }) {
  return (
    <aside className="ve-panel scroll-y">
      <div className="ve-panel__head">
        <span className="mono uppercase faint">{projectCount} active projects</span>
        <h2>{title}</h2>
      </div>
      {children}
    </aside>
  );
}

function MediaBin({ assets, onUpload }: { readonly assets: readonly Record<string, any>[]; readonly onUpload: () => void }) {
  return (
    <div className="ve-list">
      <Input icon={<Search size={14} />} placeholder="Search assets" />
      <Button icon={<Upload size={14} />} onClick={onUpload} full>Upload media</Button>
      {assets.map((asset) => (
        <div className="ve-row" key={asset.id}>
          <div><strong>{asset.display_name}</strong><span>{asset.role} / {Math.round(Number(asset.duration_seconds ?? 0))}s</span></div>
          <Badge tone={asset.rights_status === "cleared" ? "premium" : "neutral"}>{asset.processing_status}</Badge>
        </div>
      ))}
    </div>
  );
}

function Requirements({ items }: { readonly items: readonly Record<string, any>[] }) {
  return <div className="ve-list">{items.map((item) => {
    const body = typeof item.body_json === "string" ? {} : (item.body_json ?? {});
    return <div className="ve-requirement" key={item.id}><Badge tone="hd">{item.status}</Badge><h3>{item.title}</h3><p>{body.objective ?? "Structured campaign requirement"}</p><span className="mono faint">{item.due_at}</span></div>;
  })}</div>;
}

function Transcript({ items, seek, playhead }: { readonly items: readonly Record<string, any>[]; readonly seek: (seconds: number) => Promise<void>; readonly playhead: number }) {
  return <div className="ve-list">{items.map((item) => <button className={playhead >= item.start_seconds && playhead <= item.end_seconds ? "ve-transcript is-active" : "ve-transcript"} key={item.id} type="button" onClick={() => void seek(Number(item.start_seconds))}><span className="mono">{item.speaker} / {Math.round(Number(item.start_seconds))}s</span><p>{item.text}</p></button>)}</div>;
}

function Comments({ comments, onAdd }: { readonly comments: readonly Record<string, any>[]; readonly onAdd: () => Promise<void> }) {
  return <div className="ve-list"><Button icon={<MessageSquare size={14} />} onClick={() => void onAdd()} full>Add frame comment</Button>{comments.map((comment) => <div className="ve-row" key={comment.id}><div><strong>{comment.body}</strong><span>{comment.visibility} / {Math.round(Number(comment.timeline_seconds))}s</span></div><Badge>{comment.resolved ? "resolved" : "open"}</Badge></div>)}</div>;
}

function RenderPanel({ jobs, exports, proofLinks, reviewRequests, onRender, onProof, onPublish, onSubmitReview }: { readonly jobs: readonly Record<string, any>[]; readonly exports: readonly Record<string, any>[]; readonly proofLinks: readonly Record<string, any>[]; readonly reviewRequests: readonly Record<string, any>[]; readonly onRender: () => Promise<void>; readonly onProof: (id: string) => Promise<void>; readonly onPublish: (id: string) => Promise<void>; readonly onSubmitReview: (id: string) => Promise<void> }) {
  return (
    <div className="ve-list">
      <Button variant="primary" icon={<Download size={14} />} onClick={() => void onRender()} full>Render advertiser cut</Button>
      {jobs.map((job) => <div className="ve-row" key={job.id}><div><strong>{job.export_kind}</strong><span>{job.status} / {Math.round(Number(job.progress ?? 0) * 100)}%</span></div><Badge>{job.status}</Badge></div>)}
      {exports.map((item) => <div className="ve-row" key={item.id}><div><strong>{item.export_kind}</strong><span>{item.status}</span></div><span className="ve-row__actions"><Button size="sm" icon={<Link size={13} />} onClick={() => void onProof(String(item.id))}>Proof</Button><Button size="sm" icon={<Radio size={13} />} onClick={() => void onSubmitReview(String(item.id))}>Review</Button><Button size="sm" icon={<PackageCheck size={13} />} onClick={() => void onPublish(String(item.id))}>Publish</Button></span></div>)}
      {reviewRequests.map((item) => <div className="ve-row" key={item.id}><div><strong>{item.review_kind}</strong><span>{item.status}</span></div><Badge>{item.status}</Badge></div>)}
      {proofLinks.map((item) => <div className="ve-row" key={item.id}><div><strong>Advertiser proof</strong><span>{item.url}</span></div><Badge>{item.status}</Badge></div>)}
    </div>
  );
}

function Inspector({ slot, project }: { readonly slot?: Record<string, any>; readonly project?: EditorProject }) {
  return (
    <aside className="ve-inspector scroll-y">
      <span className="mono uppercase faint">inspector</span>
      <h2>{slot?.label ?? "No selection"}</h2>
      <dl>
        <dt>Campaign</dt><dd>{project?.campaign_id ?? "Unlinked"}</dd>
        <dt>Placement</dt><dd>{slot?.placement_type}</dd>
        <dt>Status</dt><dd>{slot?.status}</dd>
        <dt>Review</dt><dd>{slot?.review_status}</dd>
        <dt>Measurement</dt><dd>{slot?.measurement_key}</dd>
      </dl>
      <div className="ve-inspector__rules">
        <h3>Required</h3>
        {(slot?.requirements_json?.talking_points ?? []).map((point: string) => <span key={point}>{point}</span>)}
      </div>
    </aside>
  );
}

function Timeline({ bundle, playhead, duration, seek, onClipCommit }: { readonly bundle?: EditorBundle["timeline"]; readonly playhead: number; readonly duration: number; readonly seek: (seconds: number) => Promise<void>; readonly onClipCommit: (clip: Record<string, any>) => Promise<void> }) {
  const laneRef = useRef<HTMLDivElement>(null);
  const [clips, setClips] = useState<Record<string, any>[]>([]);
  const [drag, setDrag] = useState<DragState | null>(null);
  const tracks = bundle?.tracks ?? [];
  const slots = bundle?.ad_slots ?? [];

  useEffect(() => setClips([...(bundle?.clips ?? [])]), [bundle?.clips]);

  function applyDrag(clientX: number, final = false) {
    if (!drag || !laneRef.current) return;
    const rect = laneRef.current.getBoundingClientRect();
    const deltaSeconds = ((clientX - drag.startX) / rect.width) * duration;
    const nextClips = clips.map((clip) => {
      if (clip.id !== drag.clipId) return clip;
      const length = drag.originalOut - drag.originalIn;
      if (drag.mode === "move") {
        const start = Math.max(0, Math.min(duration - length, drag.originalIn + deltaSeconds));
        return { ...clip, timeline_in_seconds: start, timeline_out_seconds: start + length };
      }
      if (drag.mode === "trim-start") {
        return { ...clip, timeline_in_seconds: Math.max(0, Math.min(drag.originalOut - 1, drag.originalIn + deltaSeconds)) };
      }
      return { ...clip, timeline_out_seconds: Math.min(duration, Math.max(drag.originalIn + 1, drag.originalOut + deltaSeconds)) };
    });
    setClips(nextClips);
    if (final) {
      const changed = nextClips.find((clip) => clip.id === drag.clipId);
      setDrag(null);
      if (changed) void onClipCommit(changed);
    }
  }

  return (
    <section className="ve-timeline" aria-label="Timeline" onPointerMove={(event) => applyDrag(event.clientX)} onPointerUp={(event) => applyDrag(event.clientX, true)}>
      <div className="ve-timeline__ruler" onClick={(event) => {
        const rect = event.currentTarget.getBoundingClientRect();
        void seek(((event.clientX - rect.left) / rect.width) * duration);
      }}>
        <span className="ve-playhead" style={{ left: `${(playhead / duration) * 100}%` }} />
        <span className="mono">{Math.round(playhead)}s / {Math.round(duration)}s</span>
      </div>
      {tracks.map((track) => (
        <div className="ve-track" key={track.id}>
          <div className="ve-track__label"><strong>{track.name}</strong><span>{track.kind}</span></div>
          <div className="ve-track__lane" ref={track.kind === "video" ? laneRef : undefined}>
            {clips.filter((clip) => clip.track_id === track.id).map((clip) => <span className="ve-clip" key={clip.id} style={{ left: `${(Number(clip.timeline_in_seconds) / duration) * 100}%`, width: `${((Number(clip.timeline_out_seconds) - Number(clip.timeline_in_seconds)) / duration) * 100}%` }} onPointerDown={(event) => {
              const box = event.currentTarget.getBoundingClientRect();
              const mode = event.clientX - box.left < 10 ? "trim-start" : box.right - event.clientX < 10 ? "trim-end" : "move";
              event.currentTarget.setPointerCapture(event.pointerId);
              setDrag({ clipId: String(clip.id), mode, startX: event.clientX, originalIn: Number(clip.timeline_in_seconds), originalOut: Number(clip.timeline_out_seconds) });
            }}>{clip.label}</span>)}
            {slots.filter((slot) => slot.track_id === track.id).map((slot) => <span className="ve-slot" key={slot.id} style={{ left: `${(Number(slot.timeline_in_seconds) / duration) * 100}%`, width: `${((Number(slot.timeline_out_seconds) - Number(slot.timeline_in_seconds)) / duration) * 100}%` }}>{slot.label}</span>)}
          </div>
        </div>
      ))}
    </section>
  );
}
