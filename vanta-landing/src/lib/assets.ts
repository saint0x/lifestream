const DEFAULT_CDN_BASE_URL = "https://pub-4cffb671265940d19168dde582d31087.r2.dev";

const cdnBaseUrl = (import.meta.env.VITE_VANTA_CDN_BASE_URL ?? DEFAULT_CDN_BASE_URL).replace(/\/$/, "");

export function cdnAsset(path: string): string {
  return `${cdnBaseUrl}/${path.replace(/^\/+/, "")}`;
}
