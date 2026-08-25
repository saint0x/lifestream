import type { ObsRow } from "@/types";

const LABELS: Record<string, string> = {
  cut: "Cut",
  fade: "Fade",
  dip_to_black: "Dip Black",
  swipe: "Swipe",
  stinger: "Stinger",
};

const ACTION_LABELS: Record<string, string> = {
  clear_stinger_overlay: "Clear",
  commit_program: "Commit",
  crossfade_incoming: "Fade In",
  crossfade_outgoing: "Fade Out",
  fade_in_from_black: "Black In",
  fade_out_to_black: "Black Out",
  play_stinger_overlay: "Overlay",
  swap_program: "Swap",
  swap_program_at_cut_point: "Cut Point",
  swap_program_under_black: "Swap",
  wipe_from_preview: "Wipe",
};

export function transitionLabel(plan: unknown): string {
  const kind = stringValue(objectValue(plan).kind);
  return LABELS[kind] ?? "Transition";
}

export function transitionRenderer(plan: unknown): string {
  return stringValue(objectValue(plan).renderer, "renderer");
}

export function transitionPhaseSummary(plan: unknown): readonly string[] {
  const phases = objectValue(plan).phases;
  if (!Array.isArray(phases)) return [];
  return phases.map((phase) => {
    const item = objectValue(phase);
    const action = stringValue(item.action, "phase");
    const duration = numberValue(item.duration_ms);
    return `${ACTION_LABELS[action] ?? action.replaceAll("_", " ")} ${duration}ms`;
  });
}

export function transitionPlanFromPreview(preview: ObsRow | null): Record<string, unknown> {
  return objectValue(preview?.transition);
}

function objectValue(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

function stringValue(value: unknown, fallback = ""): string {
  return typeof value === "string" && value.trim() ? value : fallback;
}

function numberValue(value: unknown): number {
  return typeof value === "number" ? value : 0;
}
