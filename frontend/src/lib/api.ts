const ACCESS_TOKEN_KEY = "vanta.accessToken";
const DEFAULT_TIMEOUT_MS = 15_000;

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

export function getApiBaseUrl(): string {
  const configured = import.meta.env.VITE_VANTA_API_BASE_URL?.trim();
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

export function resolveApiUrl(url: string): string {
  if (/^https?:\/\//i.test(url)) return url;
  return `${getApiBaseUrl()}${url}`;
}

export function getAccessToken(): string | null {
  if (typeof window !== "undefined") {
    const local = window.localStorage.getItem(ACCESS_TOKEN_KEY)?.trim();
    if (local) return local;
  }
  return null;
}

export function setAccessToken(token: string): void {
  window.localStorage.setItem(ACCESS_TOKEN_KEY, token.trim());
}

export function clearAccessToken(): void {
  window.localStorage.removeItem(ACCESS_TOKEN_KEY);
}

export class ApiError extends Error {
  readonly status?: number;
  readonly path: string;

  constructor(message: string, path: string, status?: number) {
    super(message);
    this.name = "ApiError";
    this.path = path;
    this.status = status;
  }
}

export interface AuthResponse {
  readonly accessToken: string;
}

export async function createGuestSession(): Promise<AuthResponse> {
  const response = await requestJson<AuthResponse>("/api/auth/sign-in/anonymous", {
    method: "POST",
    auth: false,
  });
  setAccessToken(response.accessToken);
  return response;
}

export async function signUpWithEmail(input: {
  readonly email: string;
  readonly password: string;
  readonly displayName?: string;
}): Promise<AuthResponse> {
  const response = await requestJson<AuthResponse>("/api/auth/sign-up/email", {
    method: "POST",
    auth: false,
    body: input,
  });
  setAccessToken(response.accessToken);
  return response;
}

export async function signInWithEmail(input: {
  readonly email: string;
  readonly password: string;
}): Promise<AuthResponse> {
  const response = await requestJson<AuthResponse>("/api/auth/sign-in/email", {
    method: "POST",
    auth: false,
    body: input,
  });
  setAccessToken(response.accessToken);
  return response;
}

export function startGoogleSignIn(): void {
  window.location.assign(`${getApiBaseUrl()}/api/auth/sign-in/google`);
}

interface RequestJsonOptions {
  readonly method?: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
  readonly body?: unknown;
  readonly auth?: boolean;
  readonly signal?: AbortSignal;
  readonly timeoutMs?: number;
  readonly headers?: HeadersInit;
  readonly credentials?: RequestCredentials;
}

export async function requestJson<T>(
  path: string,
  {
    method = "GET",
    body,
    auth = true,
    signal,
    timeoutMs = DEFAULT_TIMEOUT_MS,
    headers: extraHeaders,
    credentials,
  }: RequestJsonOptions = {},
): Promise<T> {
  const headers = new Headers(extraHeaders);
  headers.set("Accept", "application/json");

  const token = auth ? getAccessToken() : null;
  if (auth && !token) {
    throw new ApiError("Sign in to continue.", path, 401);
  }
  if (token) headers.set("Authorization", `Bearer ${token}`);

  let payload: BodyInit | undefined;
  if (body !== undefined) {
    headers.set("Content-Type", "application/json");
    payload = JSON.stringify(body);
  }

  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), timeoutMs);
  const abortFromCaller = () => controller.abort();
  signal?.addEventListener("abort", abortFromCaller, { once: true });

  let response: Response;
  try {
    response = await fetch(`${getApiBaseUrl()}${path}`, {
      method,
      headers,
      body: payload,
      signal: controller.signal,
      credentials,
    });
  } catch (error) {
    if (error instanceof DOMException && error.name === "AbortError") {
      throw new ApiError("The request timed out. Try again.", path);
    }
    throw new ApiError("Unable to reach VANTA. Check your connection and try again.", path);
  } finally {
    window.clearTimeout(timeout);
    signal?.removeEventListener("abort", abortFromCaller);
  }

  if (!response.ok) {
    const text = await response.text();
    throw new ApiError(normalizeApiError(response.status, text), path, response.status);
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

interface RequestBytesOptions {
  readonly method?: "PUT" | "POST";
  readonly body: BodyInit;
  readonly auth?: boolean;
  readonly signal?: AbortSignal;
  readonly timeoutMs?: number;
  readonly headers?: HeadersInit;
}

export async function requestBytes<T>(
  path: string,
  {
    method = "PUT",
    body,
    auth = true,
    signal,
    timeoutMs = DEFAULT_TIMEOUT_MS,
    headers: extraHeaders,
  }: RequestBytesOptions,
): Promise<T> {
  const headers = new Headers(extraHeaders);
  headers.set("Accept", "application/json");
  if (!headers.has("Content-Type")) {
    headers.set("Content-Type", "application/octet-stream");
  }

  const token = auth ? getAccessToken() : null;
  if (auth && !token) {
    throw new ApiError("Sign in to continue.", path, 401);
  }
  if (token) headers.set("Authorization", `Bearer ${token}`);

  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), timeoutMs);
  const abortFromCaller = () => controller.abort();
  signal?.addEventListener("abort", abortFromCaller, { once: true });

  let response: Response;
  try {
    response = await fetch(`${getApiBaseUrl()}${path}`, {
      method,
      headers,
      body,
      signal: controller.signal,
    });
  } catch (error) {
    if (error instanceof DOMException && error.name === "AbortError") {
      throw new ApiError("The request timed out. Try again.", path);
    }
    throw new ApiError("Unable to reach VANTA. Check your connection and try again.", path);
  } finally {
    window.clearTimeout(timeout);
    signal?.removeEventListener("abort", abortFromCaller);
  }

  if (!response.ok) {
    const text = await response.text();
    throw new ApiError(normalizeApiError(response.status, text), path, response.status);
  }

  const text = await response.text();
  if (!text.trim()) {
    return undefined as T;
  }

  return JSON.parse(text) as T;
}

function normalizeApiError(status: number, text: string): string {
  if (status === 401) return "Sign in to continue.";
  if (status === 403) return "You do not have access to that action.";
  if (status === 404) return "That resource is no longer available.";
  if (status === 409) return "That change conflicts with the latest server state.";
  if (status === 429) return "Too many requests. Wait a moment and try again.";
  if (status >= 500) return "VANTA is having trouble right now. Try again shortly.";
  const trimmed = text.trim();
  if (!trimmed) return "Request failed. Try again.";
  try {
    const parsed = JSON.parse(trimmed) as { readonly error?: unknown };
    if (typeof parsed.error === "string" && parsed.error.trim()) {
      return cleanApiError(parsed.error);
    }
  } catch {
    // Plain-text errors are still supported by the API client.
  }
  return cleanApiError(trimmed);
}

function cleanApiError(message: string): string {
  return message
    .trim()
    .replace(/^(bad request|conflict|unauthorized|forbidden):\s*/i, "")
    .slice(0, 180);
}
