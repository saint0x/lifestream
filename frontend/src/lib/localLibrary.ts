import type { ContinueWatchingEntry, Film, Series, UserLibrary, WatchlistResponse } from "@/types";

const WATCHLIST_KEY = "vanta.local.watchlist.v1";
const PROGRESS_KEY = "vanta.local.progress.v1";

function storage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

function readJson<T>(key: string, fallback: T): T {
  const raw = storage()?.getItem(key);
  if (!raw) return fallback;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

function writeJson(key: string, value: unknown): void {
  storage()?.setItem(key, JSON.stringify(value));
}

function normalizeIds(ids: ReadonlyArray<string>): ReadonlyArray<string> {
  return Array.from(new Set(ids.map((id) => id.trim()).filter(Boolean))).slice(0, 500);
}

export function getLocalWatchlistIds(): ReadonlyArray<string> {
  return normalizeIds(readJson<ReadonlyArray<string>>(WATCHLIST_KEY, []));
}

export function setLocalWatchlistIds(ids: ReadonlyArray<string>): void {
  writeJson(WATCHLIST_KEY, normalizeIds(ids));
}

export function getLocalProgress(): ReadonlyArray<ContinueWatchingEntry> {
  return readJson<ReadonlyArray<ContinueWatchingEntry>>(PROGRESS_KEY, [])
    .filter((entry) => entry.contentId && entry.kind && entry.durationSec > 0)
    .slice(0, 500);
}

export function setLocalProgress(entries: ReadonlyArray<ContinueWatchingEntry>): void {
  writeJson(
    PROGRESS_KEY,
    entries
      .filter((entry) => entry.contentId && entry.kind && entry.durationSec > 0)
      .slice(0, 500),
  );
}

export function upsertLocalProgress(entry: ContinueWatchingEntry): ReadonlyArray<ContinueWatchingEntry> {
  const boundedProgress = Math.max(0, Math.min(entry.progressSec, entry.durationSec));
  const completed = boundedProgress >= entry.durationSec * 0.95;
  const current = getLocalProgress().filter((item) => item.contentId !== entry.contentId);
  const next = completed
    ? current
    : [{ ...entry, progressSec: boundedProgress }, ...current].slice(0, 500);
  setLocalProgress(next);
  return next;
}

export function removeLocalProgress(contentId: string): ReadonlyArray<ContinueWatchingEntry> {
  const next = getLocalProgress().filter((item) => item.contentId !== contentId);
  setLocalProgress(next);
  return next;
}

export function buildLocalWatchlistResponse(
  ids: ReadonlyArray<string>,
  catalog: ReadonlyArray<Series | Film>,
): WatchlistResponse {
  const byId = new Map(catalog.map((item) => [item.id, item]));
  const items = ids.map((id) => byId.get(id)).filter((item): item is Series | Film => Boolean(item));
  return {
    totalTitles: ids.length,
    series: items.filter((item): item is Series => item.kind === "series"),
    films: items.filter((item): item is Film => item.kind === "film"),
  };
}

export function buildLocalLibrary(): UserLibrary {
  return {
    continueWatching: getLocalProgress(),
    history: [],
    memberships: [],
    purchases: [],
  };
}
