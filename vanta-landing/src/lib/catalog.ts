const PRODUCTION_API_BASE_URL = "https://api-production-4becb.up.railway.app";
export const VANTA_APP_BASE_URL = "https://streamvanta.tv";

export type ImageSet = {
  readonly poster: string;
  readonly backdrop: string;
  readonly thumbnail: string;
  readonly logo?: string | null;
};

export type CatalogItem = {
  readonly id: string;
  readonly kind: "series" | "film";
  readonly slug: string;
  readonly title: string;
  readonly tagline?: string;
  readonly synopsis: string;
  readonly year: number;
  readonly rating: string;
  readonly genres: readonly string[];
  readonly images: ImageSet;
  readonly score: number;
  readonly isOriginal: boolean;
  readonly trending: boolean;
  readonly heroColor: string;
};

export type LiveItem = {
  readonly id: string;
  readonly kind: "live";
  readonly slug: string;
  readonly title: string;
  readonly category: string;
  readonly tags: readonly string[];
  readonly viewers: number;
  readonly thumbnail: string;
  readonly streamer: {
    readonly displayName: string;
    readonly handle: string;
    readonly avatar?: string | null;
  };
};

export type HomeCatalog = {
  readonly trendingSeries: readonly CatalogItem[];
  readonly trendingFilms: readonly CatalogItem[];
  readonly featuredLive: readonly LiveItem[];
};

const apiBase = (import.meta.env.VITE_VANTA_API_BASE_URL ?? PRODUCTION_API_BASE_URL).replace(/\/$/, "");

export function appHref(item: CatalogItem | LiveItem): string {
  if (item.kind === "series") return `${VANTA_APP_BASE_URL}/series/${item.slug}`;
  if (item.kind === "film") return `${VANTA_APP_BASE_URL}/film/${item.slug}`;
  return `${VANTA_APP_BASE_URL}/live/${item.slug}`;
}

export async function fetchHomeCatalog(signal?: AbortSignal): Promise<HomeCatalog> {
  const response = await fetch(`${apiBase}/api/v1/home`, {
    headers: { Accept: "application/json" },
    signal,
  });

  if (!response.ok) {
    throw new Error("Unable to load Vanta shows.");
  }

  return response.json() as Promise<HomeCatalog>;
}
