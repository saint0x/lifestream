import { Eye, FileStack, FileVideo, Radio, RefreshCw, Square, Video } from "lucide-react";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import type {
  MediaCapabilities,
  MediaCaptureArtifact,
  MediaCaptureFrame,
  MediaCaptureInventory,
  MediaCaptureSession,
  MediaEncodeJob,
  MediaPackage,
  MediaSourceArtifact,
  ObsRow,
} from "@/types";
import { boolish, text } from "@/types";
import { Panel } from "./Panel";

export function MediaPanel({
  selectedSource,
  capabilities,
  inventory,
  captureSessions,
  captureFrames,
  captureArtifacts,
  sourceArtifacts,
  encodeJobs,
  packages,
  busy,
  onStartCapture,
  onStopCapture,
  onCaptureReconcile,
  onCapturePreviewFrame,
  onCaptureSegment,
  onSourceAudioIngest,
  onStartEncode,
  onStopEncode,
  onRenderEncode,
  onPackageEncode,
}: {
  readonly selectedSource: ObsRow | null;
  readonly capabilities: MediaCapabilities | null;
  readonly inventory: MediaCaptureInventory | null;
  readonly captureSessions: readonly MediaCaptureSession[];
  readonly captureFrames: readonly MediaCaptureFrame[];
  readonly captureArtifacts: readonly MediaCaptureArtifact[];
  readonly sourceArtifacts: readonly MediaSourceArtifact[];
  readonly encodeJobs: readonly MediaEncodeJob[];
  readonly packages: readonly MediaPackage[];
  readonly busy: boolean;
  readonly onStartCapture: () => void;
  readonly onStopCapture: (sessionId: string) => void;
  readonly onCaptureReconcile: (sessionId: string) => void;
  readonly onCapturePreviewFrame: (sessionId: string) => void;
  readonly onCaptureSegment: (sessionId: string) => void;
  readonly onSourceAudioIngest: () => void;
  readonly onStartEncode: (sessionId: string) => void;
  readonly onStopEncode: (jobId: string) => void;
  readonly onRenderEncode: (jobId: string) => void;
  readonly onPackageEncode: (jobId: string) => void;
}) {
  const activeCapture = captureSessions.find((session) => text(session, "status") === "capturing") ?? null;
  const hardwareFamily = mediaHardwareFamily(capabilities);
  const latestJob = encodeJobs[0] ?? null;
  const latestFrame = captureFrames[0] ?? null;
  const latestArtifact = captureArtifacts[0] ?? sourceArtifacts[0] ?? null;
  const nativeDeviceCount = Array.isArray(inventory?.devices) ? inventory.devices.length : 0;
  const selectedMediaPath = selectedSource ? sourceMediaPath(selectedSource) : "";
  return (
    <Panel
      title="Media"
      icon={<Radio />}
      summary={<><strong>{activeCapture ? "capturing" : "idle"}</strong><span>{nativeDeviceCount} native</span><span>{artifactSummary(latestArtifact) || frameSummary(latestFrame) || text(latestJob, "status", "no job")}</span></>}
      defaultCollapsed
    >
      <div className="obs-media">
        <div className="obs-media__actions">
          <Button size="sm" variant="secondary" icon={<Video />} onClick={onStartCapture} disabled={busy || !selectedSource}>
            Capture
          </Button>
          <Button
            size="sm"
            variant="secondary"
            icon={<Radio />}
            onClick={() => activeCapture && onStartEncode(activeCapture.id)}
            disabled={busy || !activeCapture}
          >
            Encode
          </Button>
          <Button
            size="sm"
            variant="secondary"
            icon={<FileStack />}
            onClick={onSourceAudioIngest}
            disabled={busy || !selectedMediaPath || !isMediaSource(selectedSource)}
          >
            Audio
          </Button>
        </div>
        {capabilities ? (
          <div className="obs-media__caps">
            <Badge tone={boolish(capabilities, "h265") ? "hd" : "neutral"}>H265</Badge>
            <Badge tone={boolish(capabilities, "av1") ? "premium" : "neutral"}>AV1</Badge>
            <Badge tone={boolish(capabilities, "opus") ? "hd" : "neutral"}>Opus</Badge>
            <Badge tone={hardwareFamily ? "live" : "neutral"}>{hardwareFamily || "SW"}</Badge>
          </div>
        ) : null}
        {inventory ? (
          <div className="obs-media__caps">
            <Badge tone={nativeDeviceCount > 0 ? "hd" : "neutral"}>{text(inventory, "transport", "native")}</Badge>
            <Badge tone={nativeSupport(inventory, "camera") ? "hd" : "neutral"}>Camera</Badge>
            <Badge tone={nativeSupport(inventory, "microphone") ? "hd" : "neutral"}>Mic</Badge>
            <Badge tone={nativeSupport(inventory, "desktop_audio") ? "hd" : "neutral"}>Desktop</Badge>
            <Badge tone={nativeSupport(inventory, "system_audio") ? "hd" : "neutral"}>System</Badge>
            <Badge tone={nativeSupport(inventory, "display") ? "hd" : "neutral"}>Display</Badge>
            <Badge tone={nativeSupport(inventory, "window") ? "hd" : "neutral"}>Window</Badge>
            <Badge tone={nativeSupport(inventory, "application_audio") ? "hd" : "neutral"}>App Audio</Badge>
            <Badge tone={permissionTone(inventory, "camera")}>Cam {permissionStatus(inventory, "camera")}</Badge>
            <Badge tone={permissionTone(inventory, "microphone")}>Mic {permissionStatus(inventory, "microphone")}</Badge>
          </div>
        ) : null}
        {captureSessions.slice(0, 2).map((session) => {
          const segmentBacked = isSegmentBacked(session);
          const displayBacked = isDisplayBacked(session);
          return (
          <div className="obs-media__row" key={session.id}>
            <span>
              <strong>{text(session, "capture_kind")}</strong>
              <em className="mono">{text(session, "source_id")}</em>
            </span>
            <Badge tone={text(session, "status") === "capturing" ? "live" : "neutral"}>{text(session, "status")}</Badge>
            <Button
              size="sm"
              variant="ghost"
              icon={<RefreshCw />}
              aria-label="Reconnect capture session"
              onClick={() => onCaptureReconcile(session.id)}
              disabled={busy || text(session, "status") === "stopped"}
            />
            <Button
              size="sm"
              variant="ghost"
              icon={<Eye />}
              aria-label="Capture preview frame"
              onClick={() => onCapturePreviewFrame(session.id)}
              disabled={busy || !displayBacked || text(session, "status") !== "capturing"}
            />
            <Button
              size="sm"
              variant="ghost"
              icon={<FileVideo />}
              aria-label="Capture media segment"
              onClick={() => onCaptureSegment(session.id)}
              disabled={busy || !segmentBacked || text(session, "status") !== "capturing"}
            />
            <Button size="sm" variant="ghost" icon={<Square />} onClick={() => onStopCapture(session.id)} disabled={busy} />
          </div>
        );
        })}
        {latestArtifact ? (
          <div className="obs-media__row">
            <span>
              <strong>{text(latestArtifact, "artifact_kind")}</strong>
              <em className="mono">{artifactSummary(latestArtifact)}</em>
            </span>
            <Badge tone="hd">{text(latestArtifact, "status")}</Badge>
          </div>
        ) : null}
        {latestFrame ? (
          <div className="obs-media__row">
            <span>
              <strong>preview_png</strong>
              <em className="mono">{frameSummary(latestFrame)}</em>
            </span>
            <Badge tone="hd">{text(latestFrame, "status")}</Badge>
          </div>
        ) : null}
        {encodeJobs.slice(0, 2).map((job) => (
          <div className="obs-media__row" key={job.id}>
            <span>
              <strong>{text(job, "codec")}</strong>
              <em className="mono">{mediaJobDetail(job)}</em>
            </span>
            <Badge tone={text(job, "status") === "encoding" ? "live" : "premium"}>{text(job, "status")}</Badge>
            <Badge tone="neutral">{text(job, "latency_profile", "auto")}</Badge>
            <Button size="sm" variant="ghost" icon={<FileVideo />} onClick={() => onRenderEncode(job.id)} disabled={busy} />
            <Button size="sm" variant="ghost" icon={<FileStack />} onClick={() => onPackageEncode(job.id)} disabled={busy || text(job, "status") !== "playable"} />
            <Button size="sm" variant="ghost" icon={<Square />} onClick={() => onStopEncode(job.id)} disabled={busy} />
          </div>
        ))}
        {packages.slice(0, 1).map((pkg) => (
          <div className="obs-media__row" key={pkg.id}>
            <span>
              <strong>{text(pkg, "package_kind")}</strong>
              <em className="mono">{text(pkg, "manifest_path")}</em>
            </span>
            <Badge tone="hd">{text(pkg, "status")}</Badge>
          </div>
        ))}
      </div>
    </Panel>
  );
}

function artifactSummary(artifact: MediaCaptureArtifact | MediaSourceArtifact | null): string {
  const validation = artifact?.validation_json;
  if (!validation || typeof validation !== "object" || Array.isArray(validation)) return "";
  const frameCount = (validation as Record<string, unknown>).observed_video_frames;
  const duration = (validation as Record<string, unknown>).validated_duration_seconds;
  const sampleRate = (validation as Record<string, unknown>).sample_rate;
  if (typeof duration !== "number") return "";
  if (typeof frameCount === "number") return `${frameCount}f / ${duration.toFixed(1)}s`;
  if (typeof sampleRate === "number") return `${sampleRate / 1000}k / ${duration.toFixed(1)}s`;
  return "";
}

function sourceMediaPath(source: ObsRow): string {
  const settings = source.default_settings_json;
  if (settings && typeof settings === "object" && !Array.isArray(settings)) {
    const value = (settings as Record<string, unknown>).media_path;
    if (typeof value === "string" && value.trim()) return value;
  }
  const direct = source.media_path;
  return typeof direct === "string" ? direct : "";
}

function isMediaSource(source: ObsRow | null): boolean {
  return ["media_file", "vanta_video_asset", "vanta_clip"].includes(text(source, "source_kind"));
}

function isSegmentBacked(session: MediaCaptureSession): boolean {
  return ["camera", "display", "program_canvas", "window", "microphone", "desktop_audio", "system_audio", "application_audio"].includes(text(session, "capture_kind"));
}

function isDisplayBacked(session: MediaCaptureSession): boolean {
  return ["camera", "display", "program_canvas", "window"].includes(text(session, "capture_kind"));
}

function frameSummary(frame: MediaCaptureFrame | null): string {
  const validation = frame?.validation_json;
  if (!validation || typeof validation !== "object" || Array.isArray(validation)) return "";
  const width = (validation as Record<string, unknown>).width;
  const height = (validation as Record<string, unknown>).height;
  if (typeof width !== "number" || typeof height !== "number") return "";
  return `${width}x${height}`;
}

function nativeSupport(inventory: MediaCaptureInventory, kind: string): boolean {
  const support = inventory.support;
  return Boolean(support && support[kind] === true);
}

function permissionStatus(inventory: MediaCaptureInventory, kind: string): string {
  const permission = permissionRecord(inventory, kind);
  const status = permission.status;
  return typeof status === "string" && status.trim() ? status.replace("_", " ") : "unknown";
}

function permissionTone(inventory: MediaCaptureInventory, kind: string): "neutral" | "hd" | "premium" | "live" {
  const status = permissionStatus(inventory, kind);
  if (status === "ready" || status === "prompt required") return "hd";
  if (status === "denied") return "premium";
  return "neutral";
}

function permissionRecord(inventory: MediaCaptureInventory, kind: string): Record<string, unknown> {
  const permissions = inventory.permissions;
  if (!permissions || typeof permissions !== "object" || Array.isArray(permissions)) return {};
  const permission = (permissions as Record<string, unknown>)[kind];
  return permission && typeof permission === "object" && !Array.isArray(permission)
    ? permission as Record<string, unknown>
    : {};
}

function mediaHardwareFamily(capabilities: MediaCapabilities | null): string {
  const families = capabilities?.hardware_video;
  if (!families || typeof families !== "object" || Array.isArray(families)) return "";
  for (const family of ["videotoolbox", "nvenc", "qsv", "amf"]) {
    if ((families as Record<string, unknown>)[family] === true) return family;
  }
  return "";
}

function mediaJobDetail(job: MediaEncodeJob): string {
  const validation = mediaValidation(job);
  const selectedEncoder = typeof validation.selected_encoder === "string" ? validation.selected_encoder : "";
  return selectedEncoder || text(job, "output_path", text(job, "container"));
}

function mediaValidation(job: MediaEncodeJob): Record<string, unknown> {
  const health = job.health_json;
  if (!health || typeof health !== "object" || Array.isArray(health)) return {};
  const validation = (health as Record<string, unknown>).validation;
  if (!validation || typeof validation !== "object" || Array.isArray(validation)) return {};
  return validation as Record<string, unknown>;
}
