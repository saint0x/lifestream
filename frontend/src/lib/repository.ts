import type {
  AdMarketplaceOffer,
  AnalyticsPoint,
  Broadcast,
  BillingPlan,
  Category,
  ContentItem,
  Credit,
  CreatorNotification,
  CreatorAdHubResponse,
  CreatorProfile,
  Episode,
  Film,
  FollowingFeedResponse,
  Genre,
  LiveStream,
  MediaAsset,
  PersonProfile,
  RevenueEntry,
  SearchResult,
  Series,
  Streamer,
  TopContent,
  TrafficSource,
  UpdatePersonProfileRequest,
  UpdateProjectCreditsRequest,
  Upload,
  UploadIngestTicket,
  UploadJob,
  UploadStatus,
  User,
  UserLibrary,
  UserNotification,
  UserProfileDetails,
  UserSettingsBundle,
  ViewerAppState,
  WatchlistResponse,
} from "@/types";
import {
  clearAccessToken,
  createGuestSession,
  getAccessToken,
  requestJson,
  requestBytes,
} from "./api";

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
  } | null;
  readonly home: HomePayload;
  readonly me: User | null;
  readonly viewer: ViewerAppState | null;
}

interface RepositoryState {
  readonly series: ReadonlyArray<Series>;
  readonly films: ReadonlyArray<Film>;
  readonly liveStreams: ReadonlyArray<LiveStream>;
  readonly streamers: ReadonlyArray<Streamer>;
  readonly categories: ReadonlyArray<Category>;
  readonly currentUser: User;
  readonly viewerState: ViewerAppState;
  readonly creatorProfile: CreatorProfile | null;
  readonly broadcasts: ReadonlyArray<Broadcast>;
  readonly uploads: ReadonlyArray<Upload>;
  readonly analytics: ReadonlyArray<AnalyticsPoint>;
  readonly trafficSources: ReadonlyArray<TrafficSource>;
  readonly topContent: ReadonlyArray<TopContent>;
  readonly revenue: ReadonlyArray<RevenueEntry>;
  readonly creatorNotifications: ReadonlyArray<CreatorNotification>;
}

export interface SearchPayload {
  readonly items: ReadonlyArray<SearchResult>;
  readonly series: ReadonlyArray<Series>;
  readonly films: ReadonlyArray<Film>;
  readonly liveStreams: ReadonlyArray<LiveStream>;
}

export interface SearchPagePayload extends SearchPayload {
  readonly total: number;
  readonly limit: number;
  readonly offset: number;
  readonly hasMore: boolean;
}

export interface HomePayload {
  readonly trendingSeries: ReadonlyArray<Series>;
  readonly trendingFilms: ReadonlyArray<Film>;
  readonly featuredLive: ReadonlyArray<LiveStream>;
  readonly categories: ReadonlyArray<Category>;
  readonly continueWatching: ReadonlyArray<ViewerAppState["library"]["continueWatching"][number]>;
}

export interface CreateUploadJobInput {
  readonly kind: string;
  readonly sourceType: string;
  readonly title: string;
  readonly intendedVisibility: string;
  readonly bytesExpected: number;
  readonly storageKey: string;
  readonly mimeType?: string;
  readonly seriesId?: string | null;
}

export interface UpdateUploadJobInput {
  readonly title?: string;
  readonly intendedVisibility?: string;
  readonly mimeType?: string;
  readonly seriesId?: string | null;
}

export interface PublishUploadJobInput {
  readonly description?: string;
  readonly visibility?: string;
  readonly slug?: string;
  readonly releaseAt?: string;
  readonly accessPolicy?: string;
  readonly accessTierId?: string;
  readonly priceCents?: number;
  readonly currency?: string;
  readonly rentalWindowHours?: number;
  readonly seasonNumber?: number;
  readonly episodeNumber?: number;
  readonly seasonTitle?: string;
  readonly seasonSynopsis?: string;
}

export interface CategoryBrowsePayload {
  readonly category: Category;
  readonly liveStreams: ReadonlyArray<LiveStream>;
  readonly series: ReadonlyArray<Series>;
  readonly films: ReadonlyArray<Film>;
  readonly totalVodTitles: number;
}

export interface LiveDiscoveryOptions {
  readonly category?: Genre | "all";
  readonly sort?: "viewers" | "newest";
  readonly limit?: number;
}

export interface LiveDiscoveryPayload {
  readonly streams: ReadonlyArray<LiveStream>;
  readonly categories: ReadonlyArray<Category>;
  readonly totalViewers: number;
  readonly totalChannels: number;
  readonly activeCategory: Genre | null;
  readonly activeSort: "viewers" | "newest";
}

export interface CatalogPageOptions {
  readonly genre?: Genre | "All";
  readonly originalsOnly?: boolean;
  readonly sort?: "trending" | "newest" | "score" | "title";
  readonly limit?: number;
  readonly offset?: number;
}

export interface CatalogPagePayload<T> {
  readonly items: ReadonlyArray<T>;
  readonly total: number;
  readonly limit: number;
  readonly offset: number;
  readonly hasMore: boolean;
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

function mergeCatalogCache(
  current: RepositoryState,
  incoming: ReadonlyArray<Series | Film>,
): RepositoryState {
  const seriesById = new Map(current.series.map((item) => [item.id, item]));
  const filmsById = new Map(current.films.map((item) => [item.id, item]));

  for (const item of incoming) {
    if (item.kind === "series") {
      seriesById.set(item.id, item);
    } else {
      filmsById.set(item.id, item);
    }
  }

  return {
    ...current,
    series: Array.from(seriesById.values()),
    films: Array.from(filmsById.values()),
  };
}

function rememberCatalogItems(items: ReadonlyArray<Series | Film>): void {
  if (!state || items.length === 0) return;
  state = mergeCatalogCache(state, items);
}

function rememberContentCredits(
  contentKind: "series" | "film",
  contentId: string,
  credits: ReadonlyArray<Credit>,
): void {
  if (!state) return;
  if (contentKind === "series") {
    state = {
      ...state,
      series: state.series.map((item) => (item.id === contentId ? { ...item, credits } : item)),
    };
    return;
  }
  state = {
    ...state,
    films: state.films.map((item) => (item.id === contentId ? { ...item, credits } : item)),
  };
}

function rememberLiveDiscovery(payload: LiveDiscoveryPayload): void {
  if (!state) return;
  const streamsById = new Map(state.liveStreams.map((item) => [item.id, item]));
  for (const stream of payload.streams) streamsById.set(stream.id, stream);
  const categoriesBySlug = new Map(state.categories.map((item) => [item.slug, item]));
  for (const category of payload.categories) categoriesBySlug.set(category.slug, category);
  state = {
    ...state,
    liveStreams: Array.from(streamsById.values()),
    categories: Array.from(categoriesBySlug.values()),
  };
}

function rememberLiveStream(stream: LiveStream): void {
  if (!state) return;
  const streamsById = new Map(state.liveStreams.map((item) => [item.id, item]));
  streamsById.set(stream.id, stream);
  state = {
    ...state,
    liveStreams: Array.from(streamsById.values()),
  };
}

function buildCatalogPagePath(
  basePath: string,
  options: CatalogPageOptions = {},
): string {
  const params = new URLSearchParams();
  if (options.genre !== undefined && options.genre !== "All") {
    params.set("genre", options.genre);
  }
  if (options.originalsOnly !== undefined) {
    params.set("originalsOnly", String(options.originalsOnly));
  }
  if (options.sort !== undefined) {
    params.set("sort", options.sort);
  }
  if (options.limit !== undefined) {
    params.set("limit", String(options.limit));
  }
  if (options.offset !== undefined) {
    params.set("offset", String(options.offset));
  }
  const query = params.toString();
  return query ? `${basePath}?${query}` : basePath;
}

function buildLiveDiscoveryPath(options: LiveDiscoveryOptions = {}): string {
  const params = new URLSearchParams();
  if (options.category !== undefined) {
    params.set("category", options.category);
  }
  if (options.sort !== undefined) {
    params.set("sort", options.sort);
  }
  if (options.limit !== undefined) {
    params.set("limit", String(options.limit));
  }
  const query = params.toString();
  return query ? `/api/v1/live/discovery?${query}` : "/api/v1/live/discovery";
}

export const repository = {
  hasState(): boolean {
    return state !== null;
  },

  getViewerStateOrNull(): ViewerAppState | null {
    return state?.viewerState ?? null;
  },

  async hydrate(): Promise<void> {
    if (getAccessToken() === null) {
      await createGuestSession();
    }
    let bootstrap = await requestJson<BootstrapPayload>("/api/v1/bootstrap", {
      auth: getAccessToken() !== null,
    });
    if (!bootstrap.viewer && getAccessToken() !== null) {
      clearAccessToken();
      await createGuestSession();
      bootstrap = await requestJson<BootstrapPayload>("/api/v1/bootstrap", {
        auth: true,
      });
    }
    if (!bootstrap.viewer) {
      throw new Error("Bootstrap did not return viewer state.");
    }

    const creator = bootstrap.creator;
    const viewerState = bootstrap.viewer;
    state = {
      series: bootstrap.home.trendingSeries,
      films: bootstrap.home.trendingFilms,
      liveStreams: bootstrap.home.featuredLive,
      streamers: [],
      categories: bootstrap.home.categories,
      currentUser: viewerState.user,
      viewerState,
      creatorProfile: creator?.profile ?? null,
      broadcasts: dedupeBroadcasts([
        ...(creator?.currentBroadcast ? [creator.currentBroadcast] : []),
        ...(creator?.scheduledBroadcasts ?? []),
        ...(creator?.recentBroadcasts ?? []),
      ]),
      uploads: creator?.uploads.map(normalizeUpload) ?? [],
      analytics: creator?.analytics ?? [],
      trafficSources: creator?.trafficSources ?? [],
      topContent: creator?.topContent ?? [],
      revenue: creator?.revenue ?? [],
      creatorNotifications: creator?.notifications ?? [],
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
  async fetchSeriesPage(
    options: CatalogPageOptions = {},
    signal?: AbortSignal,
  ): Promise<CatalogPagePayload<Series>> {
    const payload = await requestJson<CatalogPagePayload<Series>>(
      buildCatalogPagePath("/api/v1/catalog/series/page", options),
      { auth: false, signal },
    );
    rememberCatalogItems(payload.items);
    return payload;
  },
  async fetchSeriesBySlug(slug: string, signal?: AbortSignal): Promise<Series> {
    const existing = state?.series.find((item) => item.slug === slug);
    if (existing) return existing;
    const series = await requestJson<Series>(
      `/api/v1/catalog/series/${encodeURIComponent(slug)}`,
      { auth: getAccessToken() !== null, signal },
    );
    rememberCatalogItems([series]);
    return series;
  },
  async fetchSeriesForEpisode(episodeId: string, signal?: AbortSignal): Promise<Series> {
    const existing = state?.series.find((item) =>
      item.seasons.some((season) => season.episodes.some((episode) => episode.id === episodeId)),
    );
    if (existing) return existing;
    const series = await requestJson<Series>(
      `/api/v1/catalog/episodes/${encodeURIComponent(episodeId)}/series`,
      { auth: getAccessToken() !== null, signal },
    );
    rememberCatalogItems([series]);
    return series;
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
  async fetchFilmsPage(
    options: CatalogPageOptions = {},
    signal?: AbortSignal,
  ): Promise<CatalogPagePayload<Film>> {
    const payload = await requestJson<CatalogPagePayload<Film>>(
      buildCatalogPagePath("/api/v1/catalog/films/page", options),
      { auth: false, signal },
    );
    rememberCatalogItems(payload.items);
    return payload;
  },
  async fetchFilmBySlug(slug: string, signal?: AbortSignal): Promise<Film> {
    const existing = state?.films.find((item) => item.slug === slug);
    if (existing) return existing;
    const film = await requestJson<Film>(
      `/api/v1/catalog/films/${encodeURIComponent(slug)}`,
      { auth: getAccessToken() !== null, signal },
    );
    rememberCatalogItems([film]);
    return film;
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
  async fetchLiveDiscovery(
    options: LiveDiscoveryOptions = {},
    signal?: AbortSignal,
  ): Promise<LiveDiscoveryPayload> {
    const payload = await requestJson<LiveDiscoveryPayload>(
      buildLiveDiscoveryPath(options),
      { auth: false, signal },
    );
    rememberLiveDiscovery(payload);
    return payload;
  },
  async fetchLiveStreamBySlug(slug: string, signal?: AbortSignal): Promise<LiveStream> {
    const existing = state?.liveStreams.find((item) => item.slug === slug);
    if (existing) return existing;
    const stream = await requestJson<LiveStream>(
      `/api/v1/live/streams/${encodeURIComponent(slug)}`,
      { auth: false, signal },
    );
    rememberLiveStream(stream);
    return stream;
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

  async fetchPersonProfile(slug: string, signal?: AbortSignal): Promise<PersonProfile> {
    return requestJson<PersonProfile>(
      `/api/v1/people/${encodeURIComponent(slug)}`,
      { auth: false, signal },
    );
  },

  async fetchMyPersonProfile(signal?: AbortSignal): Promise<PersonProfile> {
    return requestJson<PersonProfile>(
      "/api/v1/me/person-profile",
      { auth: true, signal },
    );
  },

  async updateMyPersonProfile(input: UpdatePersonProfileRequest): Promise<PersonProfile> {
    return requestJson<PersonProfile>("/api/v1/me/person-profile", {
      method: "PATCH",
      body: input,
      auth: true,
    });
  },

  async replaceProjectCredits(
    contentKind: "series" | "film",
    contentId: string,
    input: UpdateProjectCreditsRequest,
  ): Promise<ReadonlyArray<Credit>> {
    const credits = await requestJson<ReadonlyArray<Credit>>(
      `/api/v1/creator/me/content/${contentKind}/${encodeURIComponent(contentId)}/credits`,
      {
        method: "PUT",
        body: input,
        auth: true,
      },
    );
    rememberContentCredits(contentKind, contentId, credits);
    return credits;
  },

  listUserNotifications(): ReadonlyArray<UserNotification> {
    return requireState().viewerState.notifications;
  },

  hasCreatorWorkspace(): boolean {
    return requireState().creatorProfile !== null;
  },

  // ---------- aggregation helpers ----------
  async fetchHome(signal?: AbortSignal): Promise<HomePayload> {
    const payload = await requestJson<HomePayload>(
      "/api/v1/home",
      { auth: getAccessToken() !== null, signal },
    );
    rememberCatalogItems([...payload.trendingSeries, ...payload.trendingFilms]);
    return payload;
  },
  async fetchCategoryBrowse(
    slug: string,
    options: { readonly limit?: number; readonly offset?: number } = {},
    signal?: AbortSignal,
  ): Promise<CategoryBrowsePayload> {
    const params = new URLSearchParams();
    if (options.limit !== undefined) params.set("limit", String(options.limit));
    if (options.offset !== undefined) params.set("offset", String(options.offset));
    const suffix = params.toString() ? `?${params.toString()}` : "";
    const payload = await requestJson<CategoryBrowsePayload>(
      `/api/v1/categories/${encodeURIComponent(slug)}/browse${suffix}`,
      { auth: false, signal },
    );
    rememberCatalogItems([...payload.series, ...payload.films]);
    return payload;
  },
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
  async fetchContentById(id: string, signal?: AbortSignal): Promise<ContentItem> {
    const existing = this.getByAnyId(id);
    if (existing) return existing;
    const content = await requestJson<ContentItem>(
      `/api/v1/catalog/content/${encodeURIComponent(id)}`,
      { auth: getAccessToken() !== null, signal },
    );
    if (content.kind === "series" || content.kind === "film") {
      rememberCatalogItems([content]);
    }
    return content;
  },

  // ---------- creator ----------
  getCreatorProfile(): CreatorProfile {
    const profile = requireState().creatorProfile;
    if (!profile) {
      throw new Error("Creator workspace is unavailable for this session.");
    }
    return profile;
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

  async listUploadJobs(signal?: AbortSignal): Promise<ReadonlyArray<UploadJob>> {
    return requestJson<ReadonlyArray<UploadJob>>("/api/v1/creator/me/upload-jobs", { signal });
  },

  async createUploadJob(input: CreateUploadJobInput): Promise<UploadJob> {
    return requestJson<UploadJob>("/api/v1/creator/me/upload-jobs", {
      method: "POST",
      body: input,
    });
  },

  async updateUploadJob(id: string, input: UpdateUploadJobInput): Promise<UploadJob> {
    return requestJson<UploadJob>(`/api/v1/creator/me/upload-jobs/${encodeURIComponent(id)}`, {
      method: "PATCH",
      body: input,
    });
  },

  async publishUploadJob(jobId: string, input: PublishUploadJobInput): Promise<Upload> {
    const upload = normalizeUpload(await requestJson<Upload>(
      `/api/v1/creator/me/upload-jobs/${encodeURIComponent(jobId)}/publish`,
      {
        method: "POST",
        body: input,
      },
    ));
    if (state) {
      state = {
        ...state,
        uploads: [upload, ...state.uploads.filter((item) => item.id !== upload.id)],
      };
    }
    return upload;
  },

  async startUploadIngest(jobId: string): Promise<UploadIngestTicket> {
    return requestJson<UploadIngestTicket>(
      `/api/v1/creator/me/upload-jobs/${encodeURIComponent(jobId)}/ingest`,
      { method: "POST" },
    );
  },

  async appendUploadChunk(
    jobId: string,
    uploadToken: string,
    offset: number,
    body: BodyInit,
  ): Promise<UploadIngestTicket["session"]> {
    return requestBytes<UploadIngestTicket["session"]>(
      `/api/v1/creator/me/upload-jobs/${encodeURIComponent(jobId)}/ingest/chunk?offset=${offset}`,
      {
        method: "PUT",
        body,
        headers: { "x-upload-token": uploadToken },
        timeoutMs: 120_000,
      },
    );
  },

  async completeUploadIngest(jobId: string, uploadToken: string): Promise<UploadJob> {
    return requestJson<UploadJob>(
      `/api/v1/creator/me/upload-jobs/${encodeURIComponent(jobId)}/ingest/complete`,
      {
        method: "POST",
        headers: { "x-upload-token": uploadToken },
        timeoutMs: 120_000,
      },
    );
  },

  async getMediaAssetForUploadJob(jobId: string): Promise<MediaAsset> {
    return requestJson<MediaAsset>(
      `/api/v1/creator/me/upload-jobs/${encodeURIComponent(jobId)}/media-asset`,
    );
  },

  async listMediaAssets(signal?: AbortSignal): Promise<ReadonlyArray<MediaAsset>> {
    return requestJson<ReadonlyArray<MediaAsset>>("/api/v1/creator/me/media-assets", { signal });
  },

  async fetchAdHub(signal?: AbortSignal): Promise<CreatorAdHubResponse> {
    return requestJson<CreatorAdHubResponse>("/api/v1/creator/me/ad-hub", { signal });
  },

  async acceptAdOffer(id: string): Promise<AdMarketplaceOffer> {
    return requestJson<AdMarketplaceOffer>(
      `/api/v1/creator/me/ad-offers/${encodeURIComponent(id)}/accept`,
      { method: "POST" },
    );
  },

  async declineAdOffer(id: string): Promise<AdMarketplaceOffer> {
    return requestJson<AdMarketplaceOffer>(
      `/api/v1/creator/me/ad-offers/${encodeURIComponent(id)}/decline`,
      { method: "POST" },
    );
  },

  async submitAdOfferReview(
    id: string,
    input: { readonly submissionUrl: string; readonly notes?: string },
  ): Promise<AdMarketplaceOffer> {
    return requestJson<AdMarketplaceOffer>(
      `/api/v1/creator/me/ad-offers/${encodeURIComponent(id)}/submissions`,
      { method: "POST", body: input },
    );
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

  async searchRemote(query: string, signal?: AbortSignal): Promise<ReadonlyArray<SearchResult>> {
    const q = query.trim();
    if (!q) return [];
    const payload = await this.searchRemotePage(q, { limit: 8, offset: 0 }, signal);
    return payload.items;
  },

  async searchRemotePage(
    query: string,
    options: { readonly limit?: number; readonly offset?: number } = {},
    signal?: AbortSignal,
  ): Promise<SearchPagePayload> {
    const q = query.trim();
    if (!q) {
      return { items: [], series: [], films: [], liveStreams: [], total: 0, limit: 0, offset: 0, hasMore: false };
    }
    const params = new URLSearchParams();
    params.set("q", q);
    if (options.limit !== undefined) params.set("limit", String(options.limit));
    if (options.offset !== undefined) params.set("offset", String(options.offset));
    return requestJson<SearchPagePayload>(
      `/api/v1/search?${params.toString()}`,
      { auth: false, signal },
    );
  },
} as const;
