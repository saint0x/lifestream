export type SeriesCreditsVariant = "glass-list" | "glass-squares";

function normalizeSeriesCreditsVariant(value: string | null | undefined): SeriesCreditsVariant | null {
  if (value === "glass-list" || value === "list") return "glass-list";
  if (value === "glass-squares" || value === "squares") return "glass-squares";
  return null;
}

export function seriesCreditsVariant(): SeriesCreditsVariant {
  if (typeof window !== "undefined") {
    const params = new URLSearchParams(window.location.search);
    const queryVariant = normalizeSeriesCreditsVariant(params.get("creditsVariant"));
    if (queryVariant) return queryVariant;

    const localVariant = normalizeSeriesCreditsVariant(
      window.localStorage.getItem("vanta.seriesCreditsVariant"),
    );
    if (localVariant) return localVariant;
  }

  return normalizeSeriesCreditsVariant(import.meta.env.VITE_SERIES_CREDITS_VARIANT) ?? "glass-squares";
}
