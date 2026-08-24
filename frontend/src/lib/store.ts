import { create } from "zustand";
import { repository } from "./repository";
import { clearAccessToken, getAccessToken, requestJson } from "./api";
import { trackViewerEvent } from "./analytics";
import {
  buildLocalLibrary,
  buildLocalWatchlistResponse,
  getLocalWatchlistIds,
  removeLocalProgress,
  setLocalWatchlistIds,
  upsertLocalProgress,
} from "./localLibrary";
import type {
  AuthSession,
  BillingPlan,
  ContinueWatchingEntry,
  FollowingFeedResponse,
  ID,
  User,
  UserLibrary,
  UserNotification,
  UserProfileDetails,
  UserSettingsBundle,
  ViewerAppState,
  WatchlistResponse,
} from "@/types";

interface AppState {
  readonly hydrationStatus: "idle" | "loading" | "ready" | "signed-out" | "error";
  readonly hydrationMessage: string | null;
  readonly user: User;
  readonly watchlist: ReadonlySet<ID>;
  readonly following: ReadonlySet<ID>;
  readonly continueWatching: ReadonlyArray<ContinueWatchingEntry>;
  readonly library: UserLibrary;
  readonly watchlistDetails: WatchlistResponse;
  readonly followingFeed: FollowingFeedResponse;
  readonly profile: UserProfileDetails;
  readonly settings: UserSettingsBundle;
  readonly plan: BillingPlan;
  readonly notifications: ReadonlyArray<UserNotification>;
  readonly sessions: ReadonlyArray<AuthSession>;
  readonly actionError: string | null;

  hydrate: () => Promise<void>;
  clearActionError: () => void;
  toggleWatchlist: (id: ID) => void;
  isInWatchlist: (id: ID) => boolean;

  toggleFollow: (streamerId: ID) => void;
  isFollowing: (streamerId: ID) => boolean;

  recordProgress: (entry: ContinueWatchingEntry) => void;
  removeFromContinueWatching: (contentId: ID) => void;
  refreshViewerState: () => Promise<void>;
  updateProfile: (body: Partial<UserProfileDetails["user"]> & Partial<UserProfileDetails>) => Promise<void>;
  updateSettings: (settings: UserSettingsBundle) => Promise<void>;
  markNotificationRead: (id: ID) => Promise<void>;
  revokeSession: (id: ID) => Promise<void>;
  signOut: () => void;
}

const emptyUser: User = {
  id: "",
  handle: "",
  displayName: "",
  avatar: "",
  tier: "free",
  joinedAt: "",
  watchlist: [],
  following: [],
  continueWatching: [],
};

const emptyViewerState: ViewerAppState = {
  user: emptyUser,
  library: {
    continueWatching: [],
    history: [],
    memberships: [],
    purchases: [],
  },
  watchlist: {
    totalTitles: 0,
    series: [],
    films: [],
  },
  following: {
    totalFollowedStreamers: 0,
    liveNowCount: 0,
    followedStreamers: [],
    liveStreams: [],
  },
  profile: {
    user: emptyUser,
    email: "",
    emailVerified: false,
    matureContentAllowed: false,
    defaultAudio: "English",
    subtitlePreset: "Off",
    autoplayTrailers: false,
    liveChatFilter: "Standard",
    hoursWatched: 0,
    connectedAccounts: [],
  },
  settings: {
    playback: {
      defaultQuality: "Auto",
      audioLanguage: "English",
      subtitleLanguage: "Off",
      subtitleStyle: "Default",
      autoplayNextEpisode: true,
      autoplayTrailers: false,
      reducedMotion: false,
      preferDubbed: false,
      playbackSpeed: "1x",
    },
    notifications: {
      seriesReleases: { label: "Series releases", push: false, email: false, lock: false },
      liveStreams: { label: "Live streams", push: false, email: false, lock: false },
      originals: { label: "Originals", push: false, email: false, lock: false },
      watchlistUpdates: { label: "Watchlist updates", push: false, email: false, lock: false },
      creatorUpdates: { label: "Creator updates", push: false, email: false, lock: false },
      securityAlerts: { label: "Security alerts", push: false, email: false, lock: true },
    },
    privacy: {
      showFriendActivity: false,
      improveRecommendations: false,
      personalizedAds: false,
      abTests: false,
      dataExportSizeMb: 0,
      deleteCooldownDays: 0,
    },
    parental: {
      maxRating: "TV-MA",
      requirePinForMature: false,
      hideLiveChatForKids: false,
      blockMatureLiveStreams: false,
      pinSet: false,
    },
    downloads: {
      videoQuality: "Auto",
      wifiOnly: true,
      smartDownloads: false,
      storageUsedGb: 0,
      storageLimitGb: 0,
      deviceLimit: 0,
      activeDevices: 0,
    },
    language: {
      interfaceLanguage: "English",
      subtitleLanguage: "Off",
      catalogRegion: "US",
      dateFormat: "MM/DD/YYYY",
      clockFormat: "12h",
    },
  },
  plan: {
    planName: "Free",
    monthlyPrice: 0,
    nextRenewalDate: "",
    paymentBrand: "",
    paymentLast4: "",
    billingCity: "",
    billingRegion: "",
    billingCountry: "",
    invoicesCount: 0,
    screens: 1,
    features: [],
    averageRevenuePerUser: 0,
  },
  notifications: [],
  sessions: [],
};

const initialViewerState = repository.getViewerStateOrNull() ?? emptyViewerState;

function patchFromViewerState(
  viewerState: ViewerAppState,
): Pick<
  AppState,
  | "user"
  | "watchlist"
  | "following"
  | "continueWatching"
  | "library"
  | "watchlistDetails"
  | "followingFeed"
  | "profile"
  | "settings"
  | "plan"
  | "notifications"
  | "sessions"
> {
  return {
    user: viewerState.user,
    watchlist: new Set([
      ...viewerState.watchlist.series.map((item) => item.id),
      ...viewerState.watchlist.films.map((item) => item.id),
    ]),
    following: new Set(viewerState.following.followedStreamers.map((item) => item.id)),
    continueWatching: viewerState.library.continueWatching,
    library: viewerState.library,
    watchlistDetails: viewerState.watchlist,
    followingFeed: viewerState.following,
    profile: viewerState.profile,
    settings: viewerState.settings,
    plan: viewerState.plan,
    notifications: viewerState.notifications,
    sessions: viewerState.sessions,
  };
}

function refreshLocalViewerPatch(): Pick<AppState, "watchlist" | "watchlistDetails" | "continueWatching" | "library"> {
  const ids = getLocalWatchlistIds();
  const catalog = repository.hasState()
    ? repository.listAllContent().filter((item) => item.kind === "series" || item.kind === "film")
    : [];
  const library = buildLocalLibrary();
  return {
    watchlist: new Set(ids),
    watchlistDetails: buildLocalWatchlistResponse(ids, catalog),
    continueWatching: library.continueWatching,
    library,
  };
}

export const useAppStore = create<AppState>((set, get) => ({
  hydrationStatus: repository.hasState() ? "ready" : "idle",
  hydrationMessage: null,
  ...patchFromViewerState(initialViewerState),
  actionError: null,

  hydrate: async () => {
    set({ hydrationStatus: "loading", hydrationMessage: null });
    try {
      await repository.hydrate();
      const viewerState = repository.getViewerState();
      set({
        ...patchFromViewerState(viewerState),
        hydrationStatus: "ready",
        hydrationMessage: null,
        actionError: null,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unable to start VANTA.";
      set({
        hydrationStatus: message.toLowerCase().includes("sign in") ? "signed-out" : "error",
        hydrationMessage: message,
      });
    }
  },

  clearActionError: () => set({ actionError: null }),

  toggleWatchlist: (id) => {
    const previous = new Set(get().watchlist);
    const next = new Set(previous);
    const willAdd = !previous.has(id);
    if (willAdd) next.add(id);
    else next.delete(id);
    set({ watchlist: next });

    trackViewerEvent({
      eventType: willAdd ? "watchlist_add" : "watchlist_remove",
      contentId: id,
      metadata: { signedIn: Boolean(getAccessToken()) },
    });

    if (!getAccessToken()) {
      setLocalWatchlistIds(Array.from(next));
      set(refreshLocalViewerPatch());
      return;
    }

    void requestJson<User>("/api/v1/me/watchlist/" + id, {
      method: willAdd ? "POST" : "DELETE",
    })
      .then(() => requestJson<ViewerAppState>("/api/v1/me/state"))
      .then((viewerState) => {
        repository.replaceViewerState(viewerState);
        set(patchFromViewerState(viewerState));
      })
      .catch(() => {
        set({ watchlist: previous });
        set({ actionError: "Unable to update watchlist. Try again." });
      });
  },

  isInWatchlist: (id) => get().watchlist.has(id),

  toggleFollow: (streamerId) => {
    const previous = new Set(get().following);
    const next = new Set(previous);
    const willAdd = !previous.has(streamerId);
    if (willAdd) next.add(streamerId);
    else next.delete(streamerId);
    set({ following: next });

    void requestJson<User>("/api/v1/me/following/" + streamerId, {
      method: willAdd ? "POST" : "DELETE",
    })
      .then(() => requestJson<ViewerAppState>("/api/v1/me/state"))
      .then((viewerState) => {
        repository.replaceViewerState(viewerState);
        set(patchFromViewerState(viewerState));
      })
      .catch(() => {
        set({ following: previous });
        set({ actionError: "Unable to update following. Try again." });
      });
  },

  isFollowing: (streamerId) => get().following.has(streamerId),

  recordProgress: (entry) => {
    const previous = get().continueWatching;
    const filtered = previous.filter((item) => item.contentId !== entry.contentId);
    set({ continueWatching: [entry, ...filtered] });

    trackViewerEvent({
      eventType: "playback_progress",
      contentId: entry.contentId,
      contentKind: entry.kind,
      episodeId: entry.episodeId,
      progressSec: entry.progressSec,
      durationSec: entry.durationSec,
      watchTimeMs: 1000,
      metadata: { signedIn: Boolean(getAccessToken()) },
    });

    if (!getAccessToken()) {
      upsertLocalProgress(entry);
      set(refreshLocalViewerPatch());
      return;
    }

    void requestJson<User>("/api/v1/me/progress", {
      method: "PUT",
      body: entry,
    })
      .then(() => requestJson<ViewerAppState>("/api/v1/me/state"))
      .then((viewerState) => {
        repository.replaceViewerState(viewerState);
        set(patchFromViewerState(viewerState));
      })
      .catch(() => {
        set({ continueWatching: previous });
        set({ actionError: "Unable to save playback progress." });
      });
  },

  removeFromContinueWatching: (contentId) => {
    const previous = get().continueWatching;
    set({
      continueWatching: previous.filter((item) => item.contentId !== contentId),
    });

    trackViewerEvent({
      eventType: "library_progress_remove",
      contentId,
      metadata: { signedIn: Boolean(getAccessToken()) },
    });

    if (!getAccessToken()) {
      removeLocalProgress(contentId);
      set(refreshLocalViewerPatch());
      return;
    }

    void requestJson<User>("/api/v1/me/progress/" + contentId, {
      method: "DELETE",
    })
      .then(() => requestJson<ViewerAppState>("/api/v1/me/state"))
      .then((viewerState) => {
        repository.replaceViewerState(viewerState);
        set(patchFromViewerState(viewerState));
      })
      .catch(() => {
        set({ continueWatching: previous });
        set({ actionError: "Unable to remove playback progress." });
      });
  },

  refreshViewerState: async () => {
    const viewerState = await requestJson<ViewerAppState>("/api/v1/me/state");
    repository.replaceViewerState(viewerState);
    set({ ...patchFromViewerState(viewerState), actionError: null });
  },

  updateProfile: async (body) => {
    await requestJson<UserProfileDetails>("/api/v1/me/profile", {
      method: "PATCH",
      body,
    });
    await get().refreshViewerState();
  },

  updateSettings: async (settings) => {
    await requestJson<UserSettingsBundle>("/api/v1/me/settings", {
      method: "PATCH",
      body: settings,
    });
    await get().refreshViewerState();
  },

  markNotificationRead: async (id) => {
    await requestJson<UserNotification>(`/api/v1/me/notifications/${id}/read`, {
      method: "POST",
    });
    await get().refreshViewerState();
  },

  revokeSession: async (id) => {
    await requestJson<void>(`/api/v1/me/sessions/${id}`, {
      method: "DELETE",
    });
    await get().refreshViewerState();
  },

  signOut: () => {
    clearAccessToken();
    window.location.assign("/");
  },
}));
