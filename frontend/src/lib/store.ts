import { create } from "zustand";
import { repository } from "./repository";
import { requestJson } from "./api";
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

  toggleWatchlist: (id: ID) => void;
  isInWatchlist: (id: ID) => boolean;

  toggleFollow: (streamerId: ID) => void;
  isFollowing: (streamerId: ID) => boolean;

  recordProgress: (entry: ContinueWatchingEntry) => void;
  removeFromContinueWatching: (contentId: ID) => void;
}

const initialViewerState = repository.getViewerState();

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

export const useAppStore = create<AppState>((set, get) => ({
  ...patchFromViewerState(initialViewerState),

  toggleWatchlist: (id) => {
    const previous = new Set(get().watchlist);
    const next = new Set(previous);
    const willAdd = !previous.has(id);
    if (willAdd) next.add(id);
    else next.delete(id);
    set({ watchlist: next });

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
      });
  },

  isFollowing: (streamerId) => get().following.has(streamerId),

  recordProgress: (entry) => {
    const previous = get().continueWatching;
    const filtered = previous.filter((item) => item.contentId !== entry.contentId);
    set({ continueWatching: [entry, ...filtered] });

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
      });
  },

  removeFromContinueWatching: (contentId) => {
    const previous = get().continueWatching;
    set({
      continueWatching: previous.filter((item) => item.contentId !== contentId),
    });

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
      });
  },
}));
