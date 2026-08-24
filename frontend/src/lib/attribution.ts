const VISITOR_ID_KEY = "vanta.visitorId";
const LANDING_URL_KEY = "vanta.attribution.landingUrl";
const INITIAL_REFERRER_KEY = "vanta.attribution.initialReferrerUrl";

const MAX_URL_LENGTH = 2048;
const MAX_PARAM_LENGTH = 160;

function getStorage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

function trimForTransport(value: string | null | undefined, maxLength: number): string | null {
  const trimmed = value?.trim();
  if (!trimmed) return null;
  return trimmed.slice(0, maxLength);
}

function createVisitorId(): string {
  if (crypto.randomUUID) {
    return `vis_${crypto.randomUUID().replaceAll("-", "")}`;
  }
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return `vis_${Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

function readOrCreateStorageValue(key: string, createValue: () => string): string {
  const storage = getStorage();
  const existing = storage?.getItem(key)?.trim();
  if (existing) return existing;
  const value = createValue();
  storage?.setItem(key, value);
  return value;
}

function getLandingUrl(): string {
  return readOrCreateStorageValue(LANDING_URL_KEY, () => window.location.href);
}

function getInitialReferrerUrl(): string {
  return readOrCreateStorageValue(INITIAL_REFERRER_KEY, () => document.referrer || "");
}

export function getVisitorId(): string {
  return readOrCreateStorageValue(VISITOR_ID_KEY, createVisitorId);
}

export function getLiveAttributionParams(): URLSearchParams {
  const params = new URLSearchParams();
  const currentUrl = new URL(window.location.href);

  const values: Record<string, string | null> = {
    visitor_id: trimForTransport(getVisitorId(), 96),
    landing_url: trimForTransport(getLandingUrl(), MAX_URL_LENGTH),
    initial_referrer_url: trimForTransport(getInitialReferrerUrl(), MAX_URL_LENGTH),
    current_url: trimForTransport(currentUrl.href, MAX_URL_LENGTH),
    current_referrer_url: trimForTransport(document.referrer, MAX_URL_LENGTH),
    utm_source: trimForTransport(currentUrl.searchParams.get("utm_source"), MAX_PARAM_LENGTH),
    utm_medium: trimForTransport(currentUrl.searchParams.get("utm_medium"), MAX_PARAM_LENGTH),
    utm_campaign: trimForTransport(currentUrl.searchParams.get("utm_campaign"), MAX_PARAM_LENGTH),
    utm_term: trimForTransport(currentUrl.searchParams.get("utm_term"), MAX_PARAM_LENGTH),
    utm_content: trimForTransport(currentUrl.searchParams.get("utm_content"), MAX_PARAM_LENGTH),
  };

  for (const [key, value] of Object.entries(values)) {
    if (value) params.set(key, value);
  }

  return params;
}
