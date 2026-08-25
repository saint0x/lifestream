export interface ObsRow {
  readonly id: string;
  readonly [key: string]: unknown;
}

export interface ObsDashboard {
  readonly broadcast: ObsRow;
  readonly collection: ObsRow;
  readonly scenes: readonly ObsRow[];
  readonly scene_templates: readonly ObsRow[];
  readonly sources: readonly ObsRow[];
  readonly instances: readonly ObsRow[];
  readonly audio: readonly ObsRow[];
  readonly cues: readonly ObsRow[];
  readonly runtime: ObsRow;
  readonly health: ObsRow;
  readonly preflight: ObsRow;
  readonly replays: readonly ObsRow[];
  readonly events: readonly ObsRow[];
  readonly safety: ObsRow;
  readonly moderation: ObsRow;
  readonly audience: ObsRow;
  readonly engagement: ObsRow;
  readonly sponsor: ObsRow;
  readonly post_show: ObsRow;
  readonly guests: ObsRow;
  readonly hotkeys: readonly ObsRow[];
}

export interface ObsImportReport extends ObsRow {
  readonly collection_id?: string;
}

export interface ObsExportJob extends ObsRow {
  readonly collection_id: string;
}

export interface ObsBridgeConnection extends ObsRow {
  readonly sync_status: string;
}

export interface NativeHelperSession extends ObsRow {
  readonly helper_kind: string;
  readonly status: string;
}

export interface NativeHelperPackage extends ObsRow {
  readonly package_id: string;
  readonly helper_kind: string;
  readonly platform: string;
  readonly status: string;
}

export interface MediaCaptureSession extends ObsRow {
  readonly source_id: string;
  readonly capture_kind: string;
  readonly status: string;
}

export interface MediaCaptureFrame extends ObsRow {
  readonly capture_session_id: string;
  readonly artifact_path: string;
  readonly frame_kind: string;
  readonly status: string;
}

export interface MediaCaptureArtifact extends ObsRow {
  readonly capture_session_id: string;
  readonly artifact_path: string;
  readonly artifact_kind: string;
  readonly status: string;
}

export interface MediaSourceArtifact extends ObsRow {
  readonly source_id: string;
  readonly artifact_path: string;
  readonly artifact_kind: string;
  readonly status: string;
}

export interface MediaEncodeJob extends ObsRow {
  readonly broadcast_id: string;
  readonly capture_session_id: string;
  readonly status: string;
  readonly codec: string;
}

export interface MediaPackage extends ObsRow {
  readonly encode_job_id: string;
  readonly package_kind: string;
  readonly status: string;
  readonly manifest_path: string;
}

export interface MediaCapabilities extends ObsRow {
  readonly h265: boolean;
  readonly av1: boolean;
  readonly opus: boolean;
}

export interface MediaCaptureInventory extends ObsRow {
  readonly platform: string;
  readonly transport: string;
  readonly status: string;
  readonly devices?: readonly ObsRow[];
  readonly support?: Record<string, unknown>;
}

export function text(row: ObsRow | undefined | null, key: string, fallback = ""): string {
  const value = row?.[key];
  return typeof value === "string" ? value : fallback;
}

export function num(row: ObsRow | undefined | null, key: string, fallback = 0): number {
  const value = row?.[key];
  return typeof value === "number" ? value : fallback;
}

export function boolish(row: ObsRow | undefined | null, key: string): boolean {
  const value = row?.[key];
  return value === true || value === 1;
}

export function jsonArray(row: ObsRow | undefined | null, key: string): readonly ObsRow[] {
  const value = row?.[key];
  return Array.isArray(value) ? value as readonly ObsRow[] : [];
}
