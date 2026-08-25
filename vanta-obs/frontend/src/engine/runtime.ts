import type { ObsDashboard } from "@/types";

export type RuntimeSocketState = "connecting" | "live" | "reconnecting" | "offline";

export function dashboardFromRuntimeMessage(message: string): ObsDashboard | null {
  try {
    const payload = JSON.parse(message) as Record<string, unknown>;
    const dashboard = payload.dashboard;
    if (!dashboard || typeof dashboard !== "object" || Array.isArray(dashboard)) return null;
    if (!("broadcast" in dashboard) || !("runtime" in dashboard)) return null;
    return dashboard as ObsDashboard;
  } catch {
    return null;
  }
}

export function runtimeSocketTone(state: RuntimeSocketState): "hd" | "premium" | "neutral" {
  if (state === "live") return "hd";
  if (state === "offline") return "premium";
  return "neutral";
}
