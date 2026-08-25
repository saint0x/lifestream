const DEFAULT_BASE = "http://127.0.0.1:4127";

export function getApiBaseUrl(): string {
  return import.meta.env.VITE_VANTA_OBS_API_BASE_URL?.trim() || DEFAULT_BASE;
}

export function getApiWebSocketUrl(path: string): string {
  const url = new URL(`${getApiBaseUrl()}${path}`);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  return url.toString();
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
