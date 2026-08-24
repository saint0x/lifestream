import { getAccessToken, resolveApiUrl } from "@/lib/api";
import { getVisitorId, getViewerAttribution } from "@/lib/attribution";

export interface ViewerEventPayload {
  readonly eventType: string;
  readonly contentId?: string;
  readonly contentKind?: string;
  readonly episodeId?: string;
  readonly streamId?: string;
  readonly sessionId?: string;
  readonly path?: string;
  readonly url?: string;
  readonly referrerUrl?: string;
  readonly progressSec?: number;
  readonly durationSec?: number;
  readonly watchTimeMs?: number;
  readonly metadata?: Record<string, unknown>;
}

export function trackViewerEvent(payload: ViewerEventPayload): void {
  if (typeof window === "undefined") return;
  const token = getAccessToken();
  const attribution = getViewerAttribution();
  const body = {
    ...attribution,
    visitorId: getVisitorId(),
    path: window.location.pathname,
    url: window.location.href,
    referrerUrl: document.referrer || undefined,
    occurredAt: new Date().toISOString(),
    ...payload,
  };

  const headers = new Headers({ "Content-Type": "application/json", Accept: "application/json" });
  if (token) headers.set("Authorization", `Bearer ${token}`);
  void fetch(resolveApiUrl("/api/v1/analytics/events"), {
    method: "POST",
    headers,
    body: JSON.stringify(body),
    keepalive: true,
  }).catch(() => {});
}
