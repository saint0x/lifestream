// Centralized image helpers so artwork comes from a single deterministic source.
// Using picsum.photos seeded URLs for stable, varied mock artwork.

export const img = {
  poster: (seed: string): string =>
    `https://picsum.photos/seed/${encodeURIComponent(seed)}-p/600/900`,
  backdrop: (seed: string): string =>
    `https://picsum.photos/seed/${encodeURIComponent(seed)}-b/1920/1080`,
  thumbnail: (seed: string): string =>
    `https://picsum.photos/seed/${encodeURIComponent(seed)}-t/640/360`,
  square: (seed: string): string =>
    `https://picsum.photos/seed/${encodeURIComponent(seed)}-s/400/400`,
  avatar: (seed: string): string =>
    `https://picsum.photos/seed/${encodeURIComponent(seed)}-a/200/200`,
};
