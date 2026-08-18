import type {
  AnalyticsPoint,
  Broadcast,
  BillingPlan,
  Category,
  ContentItem,
  CreatorNotification,
  CreatorProfile,
  Episode,
  Film,
  FollowingFeedResponse,
  Genre,
  LiveStream,
  RevenueEntry,
  Series,
  Streamer,
  TopContent,
  TrafficSource,
  Upload,
  UploadStatus,
  User,
  UserLibrary,
  UserNotification,
  UserProfileDetails,
  UserSettingsBundle,
  ViewerAppState,
  WatchlistResponse,
} from "@/types";
import { requestJson } from "./api";

interface BootstrapPayload {
  readonly creator: {
    readonly profile: CreatorProfile;
    readonly currentBroadcast: Broadcast | null;
    readonly scheduledBroadcasts: ReadonlyArray<Broadcast>;
    readonly recentBroadcasts: ReadonlyArray<Broadcast>;
    readonly analytics: ReadonlyArray<AnalyticsPoint>;
    readonly trafficSources: ReadonlyArray<TrafficSource>;
    readonly topContent: ReadonlyArray<TopContent>;
    readonly revenue: ReadonlyArray<RevenueEntry>;
    readonly notifications: ReadonlyArray<CreatorNotification>;
    readonly uploads: ReadonlyArray<Upload>;
  };
  readonly home: unknown;
  readonly me: User;
}

interface RepositoryState {
  readonly series: ReadonlyArray<Series>;
  readonly films: ReadonlyArray<Film>;
  readonly liveStreams: ReadonlyArray<LiveStream>;
  readonly streamers: ReadonlyArray<Streamer>;
  readonly categories: ReadonlyArray<Category>;
  readonly currentUser: User;
  readonly viewerState: ViewerAppState;
  readonly creatorProfile: CreatorProfile;
  readonly broadcasts: ReadonlyArray<Broadcast>;
  readonly uploads: ReadonlyArray<Upload>;
  readonly analytics: ReadonlyArray<AnalyticsPoint>;
  readonly trafficSources: ReadonlyArray<TrafficSource>;
  readonly topContent: ReadonlyArray<TopContent>;
  readonly revenue: ReadonlyArray<RevenueEntry>;
  readonly creatorNotifications: ReadonlyArray<CreatorNotification>;
}

let state: RepositoryState | null = null;

function requireState(): RepositoryState {
  if (!state) {
    throw new Error("Repository accessed before hydrate()");
  }
  return state;
}

function dedupeBroadcasts(items: ReadonlyArray<Broadcast>): ReadonlyArray<Broadcast> {
  return Array.from(new Map(items.map((item) => [item.id, item])).values());
}

function normalizeUpload(upload: Upload): Upload {
  return {
    ...upload,
    kind: upload.kind as Upload["kind"],
    status: upload.status as UploadStatus,
  };
}

export const repository = {
  async hydrate(): Promise<void> {
    const [
      bootstrap,
      viewerState,
      series,
      films,
      liveStreams,
      streamers,
      categories,
    ] = await Promise.all([
      requestJson<BootstrapPayload>("/api/v1/bootstrap"),
      requestJson<ViewerAppState>("/api/v1/me/state"),
      requestJson<ReadonlyArray<Series>>("/api/v1/catalog/series"),
      requestJson<ReadonlyArray<Film>>("/api/v1/catalog/films"),
      requestJson<ReadonlyArray<LiveStream>>("/api/v1/live/streams", { auth: false }),
      requestJson<ReadonlyArray<Streamer>>("/api/v1/streamers", { auth: false }),
      requestJson<ReadonlyArray<Category>>("/api/v1/categories", { auth: false }),
    ]);

    const creator = bootstrap.creator;
    state = {
      series,
      films,
      liveStreams,
      streamers,
      categories,
      currentUser: viewerState.user,
      viewerState,
      creatorProfile: creator.profile,
      broadcasts: dedupeBroadcasts([
        ...(creator.currentBroadcast ? [creator.currentBroadcast] : []),
        ...creator.scheduledBroadcasts,
        ...creator.recentBroadcasts,
      ]),
      uploads: creator.uploads.map(normalizeUpload),
      analytics: creator.analytics,
      trafficSources: creator.trafficSources,
      topContent: creator.topContent,
      revenue: creator.revenue,
      creatorNotifications: creator.notifications,
    };
  },

  replaceCurrentUser(user: User): void {
    const current = requireState();
    state = {
      ...current,
      currentUser: user,
      viewerState: {
        ...current.viewerState,
        user,
        profile: {
          ...current.viewerState.profile,
          user,
        },
      },
    };
  },

  replaceViewerState(viewerState: ViewerAppState): void {
    const current = requireState();
    state = {
      ...current,
      currentUser: viewerState.user,
      viewerState,
    };
  },

  // ---------- series ----------
  listSeries(): ReadonlyArray<Series> {
    return requireState().series;
  },
  getSeriesBySlug(slug: string): Series | undefined {
    return requireState().series.find((item) => item.slug === slug);
  },
  getSeriesById(id: string): Series | undefined {
    return requireState().series.find((item) => item.id === id);
  },
  getEpisode(episodeId: string): Episode | undefined {
    for (const series of requireState().series) {
      for (const season of series.seasons) {
        const episode = season.episodes.find((item) => item.id === episodeId);
        if (episode) return episode;
      }
    }
    return undefined;
  },

  // ---------- films ----------
  listFilms(): ReadonlyArray<Film> {
    return requireState().films;
  },
  getFilmBySlug(slug: string): Film | undefined {
    return requireState().films.find((item) => item.slug === slug);
  },
  getFilmById(id: string): Film | undefined {
    return requireState().films.find((item) => item.id === id);
  },

  // ---------- streams ----------
  listLiveStreams(): ReadonlyArray<LiveStream> {
    return requireState().liveStreams;
  },
  getLiveStreamBySlug(slug: string): LiveStream | undefined {
    return requireState().liveStreams.find((item) => item.slug === slug);
  },
  getLiveStreamById(id: string): LiveStream | undefined {
    return requireState().liveStreams.find((item) => item.id === id);
  },
  getLiveStreamsByCategory(category: Genre): ReadonlyArray<LiveStream> {
    return requireState().liveStreams.filter((item) => item.category === category);
  },

  // ---------- streamers ----------
  listStreamers(): ReadonlyArray<Streamer> {
    return requireState().streamers;
  },
  getStreamer(id: string): Streamer | undefined {
    return requireState().streamers.find((item) => item.id === id);
  },

  // ---------- categories ----------
  listCategories(): ReadonlyArray<Category> {
    return requireState().categories;
  },
  getCategory(slug: string): Category | undefined {
    return requireState().categories.find((item) => item.slug === slug);
  },

  // ---------- user ----------
  getCurrentUser(): User {
    return requireState().currentUser;
  },

  getViewerState(): ViewerAppState {
    return requireState().viewerState;
  },

  getUserLibrary(): UserLibrary {
    return requireState().viewerState.library;
  },

  getWatchlistResponse(): WatchlistResponse {
    return requireState().viewerState.watchlist;
  },

  getFollowingFeed(): FollowingFeedResponse {
    return requireState().viewerState.following;
  },

  getUserProfileDetails(): UserProfileDetails {
    return requireState().viewerState.profile;
  },

  getUserSettings(): UserSettingsBundle {
    return requireState().viewerState.settings;
  },

  getBillingPlan(): BillingPlan {
    return requireState().viewerState.plan;
  },

  listUserNotifications(): ReadonlyArray<UserNotification> {
    return requireState().viewerState.notifications;
  },

  // ---------- aggregation helpers ----------
  listAllContent(): ReadonlyArray<ContentItem> {
    const current = requireState();
    return [...current.series, ...current.films, ...current.liveStreams];
  },

  listTrending(): ReadonlyArray<ContentItem> {
    return this.listAllContent().filter((item) => {
      if (item.kind === "live") return item.viewers > 5000;
      return item.trending;
    });
  },

  listOriginals(): ReadonlyArray<Series | Film> {
    const current = requireState();
    return [...current.series, ...current.films].filter((item) => item.isOriginal);
  },

  listByGenre(genre: Genre): ReadonlyArray<Series | Film> {
    const current = requireState();
    return [...current.series, ...current.films].filter((item) => item.genres.includes(genre));
  },

  getByAnyId(id: string): ContentItem | undefined {
    return this.listAllContent().find((item) => item.id === id);
  },

  // ---------- creator ----------
  getCreatorProfile(): CreatorProfile {
    return requireState().creatorProfile;
  },

  listBroadcasts(): ReadonlyArray<Broadcast> {
    return requireState().broadcasts;
  },
  getBroadcast(id: string): Broadcast | undefined {
    return requireState().broadcasts.find((item) => item.id === id);
  },
  listBroadcastsByStatus(status: Broadcast["status"]): ReadonlyArray<Broadcast> {
    return requireState().broadcasts.filter((item) => item.status === status);
  },
  getCurrentBroadcast(): Broadcast | undefined {
    return requireState().broadcasts.find((item) => item.status === "live");
  },

  listUploads(): ReadonlyArray<Upload> {
    return requireState().uploads;
  },
  listUploadsByStatus(status: UploadStatus): ReadonlyArray<Upload> {
    return requireState().uploads.filter((item) => item.status === status);
  },
  getUpload(id: string): Upload | undefined {
    return requireState().uploads.find((item) => item.id === id);
  },

  getAnalytics(): ReadonlyArray<AnalyticsPoint> {
    return requireState().analytics;
  },
  getTrafficSources(): ReadonlyArray<TrafficSource> {
    return requireState().trafficSources;
  },
  getTopContent(): ReadonlyArray<TopContent> {
    return requireState().topContent;
  },
  listRevenue(): ReadonlyArray<RevenueEntry> {
    return requireState().revenue;
  },
  listCreatorNotifications(): ReadonlyArray<CreatorNotification> {
    return requireState().creatorNotifications;
  },

  search(query: string): ReadonlyArray<ContentItem> {
    const q = query.trim().toLowerCase();
    if (!q) return [];
    return this.listAllContent().filter((item) => {
      const title = item.title.toLowerCase();
      if (title.includes(q)) return true;
      if (item.kind === "live") {
        return (
          item.tags.some((tag) => tag.toLowerCase().includes(q)) ||
          item.streamer.displayName.toLowerCase().includes(q)
        );
      }
      return (
        item.synopsis.toLowerCase().includes(q) ||
        item.genres.some((genre) => genre.toLowerCase().includes(q))
      );
    });
  },
} as const;
