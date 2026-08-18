const DEV_ACCESS_TOKEN = "lifestream-local-dev-token";

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

export function getApiBaseUrl(): string {
  const configured = import.meta.env.VITE_LIFESTREAM_API_BASE_URL?.trim();
  if (configured) return trimTrailingSlash(configured);
  return "http://127.0.0.1:8080";
}

export function getApiWebSocketBaseUrl(): string {
  const base = getApiBaseUrl();
  if (base.startsWith("https://")) {
    return `wss://${base.slice("https://".length)}`;
  }
  if (base.startsWith("http://")) {
    return `ws://${base.slice("http://".length)}`;
  }
  return base;
}

export function getAccessToken(): string | null {
  if (typeof window !== "undefined") {
    const local = window.localStorage.getItem("lifestream.accessToken")?.trim();
    if (local) return local;
    if (window.location.hostname === "localhost" || window.location.hostname === "127.0.0.1") {
      return DEV_ACCESS_TOKEN;
    }
  }

  const configured = import.meta.env.VITE_LIFESTREAM_ACCESS_TOKEN?.trim();
  return configured && configured.length > 0 ? configured : null;
}

interface RequestJsonOptions {
  readonly method?: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
  readonly body?: unknown;
  readonly auth?: boolean;
}

export async function requestJson<T>(
  path: string,
  { method = "GET", body, auth = true }: RequestJsonOptions = {},
): Promise<T> {
  const headers = new Headers();
  headers.set("Accept", "application/json");

  const token = auth ? getAccessToken() : null;
  if (auth && !token) {
    throw new Error(`Missing access token for ${path}`);
  }
  if (token) headers.set("Authorization", `Bearer ${token}`);

  let payload: BodyInit | undefined;
  if (body !== undefined) {
    headers.set("Content-Type", "application/json");
    payload = JSON.stringify(body);
  }

  const response = await fetch(`${getApiBaseUrl()}${path}`, {
    method,
    headers,
    body: payload,
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(`Request failed for ${path}: ${response.status} ${text}`);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  const text = await response.text();
  if (!text.trim()) {
    return undefined as T;
  }

  return JSON.parse(text) as T;
}
