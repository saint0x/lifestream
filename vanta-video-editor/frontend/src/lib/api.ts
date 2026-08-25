const DEFAULT_BASE = "http://127.0.0.1:4117";

export function getApiBaseUrl(): string {
  return import.meta.env.VITE_VANTA_EDITOR_API_BASE_URL?.trim() || DEFAULT_BASE;
}

export async function requestJson<T>(path: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(`${getApiBaseUrl()}${path}`, {
    ...init,
    headers: {
      Accept: "application/json",
      "X-Vanta-User-Id": "user_creator_owner",
      "X-Vanta-Role": "creator_owner",
      ...(init.body ? { "Content-Type": "application/json" } : {}),
      ...init.headers,
    },
  });
  if (!response.ok) {
    const text = await response.text();
    throw new Error(text || `Request failed: ${response.status}`);
  }
  return response.json() as Promise<T>;
}

export interface EditorProject {
  readonly id: string;
  readonly title: string;
  readonly description: string;
  readonly status: string;
  readonly campaign_id: string | null;
  readonly offer_id: string | null;
}

export interface EditorBundle {
  readonly project: EditorProject;
  readonly assets: readonly Record<string, any>[];
  readonly requirements: readonly Record<string, any>[];
  readonly comments: readonly Record<string, any>[];
  readonly timeline: {
    readonly timeline: Record<string, any>;
    readonly tracks: readonly Record<string, any>[];
    readonly clips: readonly Record<string, any>[];
    readonly ad_slots: readonly Record<string, any>[];
    readonly transcript: readonly Record<string, any>[];
    readonly versions: readonly Record<string, any>[];
  };
}

export function fetchEditorProjects(): Promise<EditorProject[]> {
  return requestJson("/api/v1/editor/me/projects");
}

export function fetchEditorProject(projectId: string): Promise<EditorBundle> {
  return requestJson(`/api/v1/editor/me/projects/${projectId}`);
}

export function patchEditorTimeline(projectId: string, body: Record<string, unknown>): Promise<EditorBundle["timeline"]> {
  return requestJson(`/api/v1/editor/me/projects/${projectId}/timeline`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
}

export function updateEditorClip(clipId: string, body: Record<string, unknown>): Promise<Record<string, any>> {
  return requestJson(`/api/v1/editor/me/clips/${clipId}`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
}

export function createEditorClip(projectId: string, body: Record<string, unknown>): Promise<Record<string, any>> {
  return requestJson(`/api/v1/editor/me/projects/${projectId}/clips`, {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export function validateEditorAdSlot(adSlotId: string): Promise<{ valid: boolean; blockers: string[] }> {
  return requestJson(`/api/v1/editor/me/ad-slots/${adSlotId}/validate`, { method: "POST" });
}

export function createEditorComment(projectId: string, body: Record<string, unknown>): Promise<Record<string, any>> {
  return requestJson(`/api/v1/editor/me/projects/${projectId}/comments`, {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export function createEditorRenderJob(projectId: string, body: Record<string, unknown>): Promise<Record<string, any>> {
  return requestJson(`/api/v1/editor/me/projects/${projectId}/render-jobs`, {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export function fetchEditorRenderJobs(projectId: string): Promise<Record<string, any>[]> {
  return requestJson(`/api/v1/editor/me/projects/${projectId}/render-jobs`);
}

export function fetchEditorExports(projectId: string): Promise<Record<string, any>[]> {
  return requestJson(`/api/v1/editor/me/projects/${projectId}/exports`);
}

export function createEditorProofLink(exportId: string): Promise<Record<string, any>> {
  return requestJson(`/api/v1/editor/me/exports/${exportId}/proof-link`, { method: "POST" });
}

export function fetchEditorProofLinks(projectId: string): Promise<Record<string, any>[]> {
  return requestJson(`/api/v1/editor/me/projects/${projectId}/proof-links`);
}

export function fetchEditorReviewRequests(projectId: string): Promise<Record<string, any>[]> {
  return requestJson(`/api/v1/editor/me/projects/${projectId}/review-requests`);
}

export function submitEditorAdvertiserReview(exportId: string): Promise<Record<string, any>> {
  return requestJson(`/api/v1/editor/me/exports/${exportId}/submit-advertiser-review`, { method: "POST" });
}

export function publishEditorExport(exportId: string): Promise<Record<string, any>> {
  return requestJson(`/api/v1/editor/me/exports/${exportId}/publish`, { method: "POST" });
}

export async function uploadEditorAsset(projectId: string, file: File, role: string): Promise<Record<string, any>> {
  const data = new FormData();
  data.append("role", role);
  data.append("display_name", file.name);
  data.append("file", file);
  const response = await fetch(`${getApiBaseUrl()}/api/v1/editor/me/projects/${projectId}/assets/upload`, {
    method: "POST",
    headers: {
      "X-Vanta-User-Id": "user_creator_owner",
      "X-Vanta-Role": "creator_owner",
    },
    body: data,
  });
  if (!response.ok) {
    const text = await response.text();
    throw new Error(text || `Upload failed: ${response.status}`);
  }
  return response.json() as Promise<Record<string, any>>;
}
