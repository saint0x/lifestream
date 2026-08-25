export type ReplayPreset = 15 | 30 | 60 | "custom";

export interface ReplayDraftOptions {
  readonly durationSeconds: number;
  readonly sponsorProof: boolean;
}

export const REPLAY_DURATION_PRESETS = [15, 30, 60] as const;

export function replayDurationFromPreset(preset: ReplayPreset, customDuration: number): number {
  return preset === "custom" ? clampReplayDuration(customDuration) : preset;
}

export function clampReplayDuration(durationSeconds: number): number {
  if (!Number.isFinite(durationSeconds)) return 30;
  return Math.min(300, Math.max(5, Math.round(durationSeconds)));
}
