// Core domain types for the LIFESTREAM platform.
// Everything downstream (repository, components, pages) consumes these.

export type ID = string;

export type ContentKind = "series" | "film" | "live";

export type MaturityRating = "G" | "PG" | "PG-13" | "TV-14" | "R" | "TV-MA";

export type Genre =
  | "Drama"
  | "Thriller"
  | "Sci-Fi"
  | "Comedy"
  | "Documentary"
  | "Action"
  | "Horror"
  | "Fantasy"
  | "Crime"
  | "Romance"
  | "Animation"
  | "Sports"
  | "Music"
  | "Tech"
  | "Gaming"
  | "Talk"
  | "News";

export interface Credit {
  readonly id: ID;
  readonly name: string;
  readonly role: "creator" | "director" | "writer" | "cast" | "host";
  readonly character?: string;
}

export interface ImageSet {
  readonly poster: string;
  readonly backdrop: string;
  readonly thumbnail: string;
  readonly logo?: string;
}

interface BaseContent {
  readonly id: ID;
  readonly slug: string;
  readonly title: string;
  readonly tagline?: string;
  readonly synopsis: string;
  readonly year: number;
  readonly rating: MaturityRating;
  readonly genres: ReadonlyArray<Genre>;
  readonly images: ImageSet;
  readonly credits: ReadonlyArray<Credit>;
  readonly score: number; // 0–100, critic/user aggregate
  readonly isOriginal: boolean;
  readonly trending: boolean;
  readonly heroColor: string; // accent color derived from key art
}

export interface Episode {
  readonly id: ID;
  readonly seriesId: ID;
  readonly seasonNumber: number;
  readonly episodeNumber: number;
  readonly title: string;
  readonly synopsis: string;
  readonly durationSec: number;
  readonly airedAt: string; // ISO date
  readonly thumbnail: string;
  readonly progressSec?: number;
  readonly playbackSessionUrl?: string | null;
  readonly playbackReady?: boolean;
}

export interface Season {
  readonly seasonNumber: number;
  readonly title: string;
  readonly episodes: ReadonlyArray<Episode>;
}

export interface Series extends BaseContent {
  readonly kind: "series";
  readonly seasons: ReadonlyArray<Season>;
  readonly totalEpisodes: number;
  readonly status: "ongoing" | "ended" | "upcoming";
}

export interface Film extends BaseContent {
  readonly kind: "film";
  readonly durationSec: number;
  readonly progressSec?: number;
  readonly playbackSessionUrl?: string | null;
  readonly playbackReady?: boolean;
}

export interface LiveStream {
  readonly id: ID;
  readonly slug: string;
  readonly title: string;
  readonly category: Genre;
  readonly tags: ReadonlyArray<string>;
  readonly streamer: Streamer;
  readonly viewers: number;
  readonly startedAt: string; // ISO
  readonly thumbnail: string;
  readonly language: string;
  readonly isMature: boolean;
  readonly kind: "live";
  readonly playbackSessionUrl?: string | null;
  readonly playbackReady?: boolean;
}

export interface Streamer {
  readonly id: ID;
  readonly handle: string;
  readonly displayName: string;
  readonly avatar: string;
  readonly bio: string;
  readonly followers: number;
  readonly isPartner: boolean;
  readonly isLive: boolean;
}

export interface Category {
  readonly slug: string;
  readonly name: Genre;
  readonly coverImage: string;
  readonly liveViewers: number;
  readonly liveChannels: number;
  readonly tags: ReadonlyArray<string>;
}

export interface User {
  readonly id: ID;
  readonly handle: string;
  readonly displayName: string;
  readonly avatar: string;
  readonly tier: "free" | "standard" | "premium";
  readonly joinedAt: string;
  readonly watchlist: ReadonlyArray<ID>;
  readonly following: ReadonlyArray<ID>;
  readonly continueWatching: ReadonlyArray<ContinueWatchingEntry>;
}

export interface ConnectedAccount {
  readonly id: ID;
  readonly provider: string;
  readonly displayName: string;
  readonly connectedAt: string;
}

export interface ContinueWatchingEntry {
  readonly contentId: ID;
  readonly kind: "series" | "film";
  readonly episodeId?: ID;
  readonly progressSec: number;
  readonly durationSec: number;
  readonly lastWatchedAt: string;
}

export interface WatchHistoryEntry {
  readonly contentId: ID;
  readonly kind: "series" | "film";
  readonly episodeId?: ID;
  readonly watchedAt: string;
  readonly progressSec: number;
  readonly durationSec: number;
}

export interface CreatorMembership {
  readonly creatorId: ID;
  readonly creatorHandle: string;
  readonly creatorDisplayName: string;
  readonly tierId: ID;
  readonly tierName: string;
  readonly tierRank: number;
  readonly status: string;
  readonly startedAt: string;
  readonly renewsAt?: string | null;
  readonly endsAt?: string | null;
}

export interface ContentPurchase {
  readonly id: ID;
  readonly creatorId: ID;
  readonly creatorHandle: string;
  readonly creatorDisplayName: string;
  readonly uploadId: ID;
  readonly title: string;
  readonly accessPolicy: string;
  readonly amountCents: number;
  readonly currency: string;
  readonly status: string;
  readonly purchasedAt: string;
  readonly expiresAt?: string | null;
}

export interface UserLibrary {
  readonly continueWatching: ReadonlyArray<ContinueWatchingEntry>;
  readonly history: ReadonlyArray<WatchHistoryEntry>;
  readonly memberships: ReadonlyArray<CreatorMembership>;
  readonly purchases: ReadonlyArray<ContentPurchase>;
}

export interface WatchlistResponse {
  readonly totalTitles: number;
  readonly series: ReadonlyArray<Series>;
  readonly films: ReadonlyArray<Film>;
}

export interface FollowingFeedResponse {
  readonly totalFollowedStreamers: number;
  readonly liveNowCount: number;
  readonly followedStreamers: ReadonlyArray<Streamer>;
  readonly liveStreams: ReadonlyArray<LiveStream>;
}

export interface UserNotification {
  readonly id: ID;
  readonly kind: string;
  readonly body: string;
  readonly channel: string;
  readonly actor?: string | null;
  readonly sentAt: string;
  readonly deliveryState: string;
  readonly readAt?: string | null;
}

export interface UserProfileDetails {
  readonly user: User;
  readonly email: string;
  readonly emailVerified: boolean;
  readonly matureContentAllowed: boolean;
  readonly defaultAudio: string;
  readonly subtitlePreset: string;
  readonly autoplayTrailers: boolean;
  readonly liveChatFilter: string;
  readonly hoursWatched: number;
  readonly connectedAccounts: ReadonlyArray<ConnectedAccount>;
}

export interface PlaybackSettings {
  readonly defaultQuality: string;
  readonly audioLanguage: string;
  readonly subtitleLanguage: string;
  readonly subtitleStyle: string;
  readonly autoplayNextEpisode: boolean;
  readonly autoplayTrailers: boolean;
  readonly reducedMotion: boolean;
  readonly preferDubbed: boolean;
  readonly playbackSpeed: string;
}

export interface NotificationChannelSetting {
  readonly label: string;
  readonly push: boolean;
  readonly email: boolean;
  readonly lock: boolean;
}

export interface NotificationSettings {
  readonly seriesReleases: NotificationChannelSetting;
  readonly liveStreams: NotificationChannelSetting;
  readonly originals: NotificationChannelSetting;
  readonly watchlistUpdates: NotificationChannelSetting;
  readonly creatorUpdates: NotificationChannelSetting;
  readonly securityAlerts: NotificationChannelSetting;
}

export interface PrivacySettings {
  readonly showFriendActivity: boolean;
  readonly improveRecommendations: boolean;
  readonly personalizedAds: boolean;
  readonly abTests: boolean;
  readonly dataExportSizeMb: number;
  readonly deleteCooldownDays: number;
}

export interface ParentalControls {
  readonly maxRating: string;
  readonly requirePinForMature: boolean;
  readonly hideLiveChatForKids: boolean;
  readonly blockMatureLiveStreams: boolean;
  readonly pinSet: boolean;
}

export interface DownloadSettings {
  readonly videoQuality: string;
  readonly wifiOnly: boolean;
  readonly smartDownloads: boolean;
  readonly storageUsedGb: number;
  readonly storageLimitGb: number;
  readonly deviceLimit: number;
  readonly activeDevices: number;
}

export interface LanguageSettings {
  readonly interfaceLanguage: string;
  readonly subtitleLanguage: string;
  readonly catalogRegion: string;
  readonly dateFormat: string;
  readonly clockFormat: string;
}

export interface UserSettingsBundle {
  readonly playback: PlaybackSettings;
  readonly notifications: NotificationSettings;
  readonly privacy: PrivacySettings;
  readonly parental: ParentalControls;
  readonly downloads: DownloadSettings;
  readonly language: LanguageSettings;
}

export interface BillingPlan {
  readonly planName: string;
  readonly monthlyPrice: number;
  readonly nextRenewalDate: string;
  readonly paymentBrand: string;
  readonly paymentLast4: string;
  readonly billingCity: string;
  readonly billingRegion: string;
  readonly billingCountry: string;
  readonly invoicesCount: number;
  readonly screens: number;
  readonly features: ReadonlyArray<string>;
  readonly averageRevenuePerUser: number;
}

export interface AuthSession {
  readonly id: ID;
  readonly label: string;
  readonly scopes: ReadonlyArray<string>;
  readonly createdAt: string;
  readonly expiresAt?: string | null;
  readonly revokedAt?: string | null;
  readonly lastUsedAt?: string | null;
  readonly isCurrent: boolean;
}

export interface ViewerAppState {
  readonly user: User;
  readonly library: UserLibrary;
  readonly watchlist: WatchlistResponse;
  readonly following: FollowingFeedResponse;
  readonly profile: UserProfileDetails;
  readonly settings: UserSettingsBundle;
  readonly plan: BillingPlan;
  readonly notifications: ReadonlyArray<UserNotification>;
  readonly sessions: ReadonlyArray<AuthSession>;
}

export interface ChatMessage {
  readonly id: ID;
  readonly sequence: number;
  readonly userHandle: string;
  readonly displayName: string;
  readonly color: string;
  readonly badges: ReadonlyArray<ChatBadge>;
  readonly body: string;
  readonly sentAt: string;
}

export type ChatBadge = "mod" | "partner" | "subscriber" | "vip" | "staff";

export type ContentItem = Series | Film | LiveStream;

export type Unionize<T> = T extends ContentItem ? T : never;

// ----------------------------------------------------------------------
// Creator-side types
// ----------------------------------------------------------------------

export type PartnerStatus = "creator" | "affiliate" | "partner";

export interface CreatorProfile {
  readonly id: ID;
  readonly userId: ID;
  readonly handle: string;
  readonly displayName: string;
  readonly avatar: string;
  readonly banner: string;
  readonly tagline: string;
  readonly bio: string;
  readonly partnerStatus: PartnerStatus;
  readonly joinedAt: string;
  readonly streamKey: string;
  readonly rtmpUrl: string;
  readonly defaultCategory: Genre;
  readonly defaultTags: ReadonlyArray<string>;
  readonly followers: number;
  readonly subscribers: number;
  readonly monthlyViewers: number;
  readonly totalWatchHours: number;
  readonly liveStatus: "live" | "offline" | "starting";
  readonly currentBroadcastId?: ID;
}

export type BroadcastStatus = "live" | "scheduled" | "ended";

export interface Broadcast {
  readonly id: ID;
  readonly title: string;
  readonly category: Genre;
  readonly tags: ReadonlyArray<string>;
  readonly status: BroadcastStatus;
  readonly startedAt: string;
  readonly endedAt?: string;
  readonly durationSec?: number;
  readonly peakViewers: number;
  readonly averageViewers: number;
  readonly chatMessages: number;
  readonly newFollowers: number;
  readonly newSubscribers: number;
  readonly revenue: number;
  readonly thumbnail: string;
  readonly isMature: boolean;
}

export type UploadKind = "episode" | "vod" | "clip" | "trailer" | "film";
export type UploadStatus =
  | "draft"
  | "processing"
  | "scheduled"
  | "published"
  | "archived"
  | "taken_down";
export type Visibility = "public" | "unlisted" | "private";
export type Resolution = string;

export interface Upload {
  readonly id: ID;
  readonly title: string;
  readonly description: string;
  readonly kind: UploadKind;
  readonly durationSec: number;
  readonly uploadedAt: string;
  readonly publishedAt?: string;
  readonly status: UploadStatus;
  readonly visibility: Visibility;
  readonly views: number;
  readonly likes: number;
  readonly comments: number;
  readonly watchHours: number;
  readonly thumbnail: string;
  readonly seriesTitle?: string;
  readonly seasonNumber?: number;
  readonly episodeNumber?: number;
  readonly sizeBytes: number;
  readonly resolution: Resolution;
  readonly transcodeProgress?: number;
}

export interface AnalyticsPoint {
  readonly date: string; // ISO date
  readonly viewers: number;
  readonly watchMinutes: number;
  readonly revenue: number;
  readonly newFollowers: number;
}

export interface TrafficSource {
  readonly source: string;
  readonly sessions: number;
  readonly share: number; // 0..1
}

export interface ViewerPreview {
  readonly totalViewers: number;
  readonly sampleUsers: ReadonlyArray<string>;
}

export interface LiveNotifyPreference {
  readonly streamerId: ID;
  readonly enabled: boolean;
}

export interface LiveModerationAction {
  readonly id: ID;
  readonly streamId: ID;
  readonly creatorId: ID;
  readonly subjectUserId: ID;
  readonly actorUserId: ID;
  readonly actionType: string;
  readonly reason: string;
  readonly state: string;
  readonly expiresAt?: string | null;
  readonly createdAt: string;
  readonly revokedAt?: string | null;
}

export interface LiveStreamReportRecord {
  readonly id: ID;
  readonly streamId: ID;
  readonly userId: ID;
  readonly reason: string;
  readonly details?: string | null;
  readonly status: string;
  readonly resolvedByUserId?: ID | null;
  readonly resolutionNote?: string | null;
  readonly createdAt: string;
  readonly resolvedAt?: string | null;
}

export interface CreatorModerator {
  readonly creatorId: ID;
  readonly userId: ID;
  readonly role: string;
  readonly createdAt: string;
}

export interface ModerationAuditEntry {
  readonly id: ID;
  readonly creatorId: ID;
  readonly streamId?: ID | null;
  readonly actorUserId: ID;
  readonly subjectUserId?: ID | null;
  readonly eventType: string;
  readonly payload: unknown;
  readonly createdAt: string;
}

export interface PlaybackSession {
  readonly id: ID;
  readonly contentId: ID;
  readonly contentKind: string;
  readonly accessScope: string;
  readonly createdAt: string;
  readonly expiresAt: string;
  readonly lastUsedAt: string;
}

export interface PlaybackAudioTrack {
  readonly id: ID;
  readonly label: string;
  readonly language: string;
  readonly codec?: string | null;
  readonly playlistPath?: string | null;
  readonly playlistUrl?: string | null;
  readonly source: string;
  readonly isDubbed: boolean;
  readonly isDefault: boolean;
  readonly published: boolean;
}

export interface PlaybackCaptionTrack {
  readonly id: ID;
  readonly label: string;
  readonly language: string;
  readonly role: string;
  readonly source: string;
  readonly mimeType: string;
  readonly url: string;
  readonly isDefault: boolean;
  readonly published: boolean;
}

export interface PlaybackPreviewTrack {
  readonly id: ID;
  readonly label: string;
  readonly imagePath: string;
  readonly imageUrl: string;
  readonly vttPath: string;
  readonly vttUrl: string;
  readonly tileWidth: number;
  readonly tileHeight: number;
  readonly columnsCount: number;
  readonly rowsCount: number;
  readonly intervalSec: number;
  readonly frameCount: number;
  readonly isDefault: boolean;
  readonly published: boolean;
}

export interface PlaybackGrant {
  readonly session: PlaybackSession;
  readonly playbackToken: string;
  readonly manifestUrl: string;
  readonly posterUrl?: string | null;
  readonly contentTitle: string;
  readonly contentKind: string;
  readonly visibility: string;
  readonly accessPolicy: string;
  readonly accessTierId?: string | null;
  readonly priceCents?: number | null;
  readonly currency?: string | null;
  readonly rentalWindowHours?: number | null;
  readonly audioTracks: ReadonlyArray<PlaybackAudioTrack>;
  readonly captionTracks: ReadonlyArray<PlaybackCaptionTrack>;
  readonly previewTracks: ReadonlyArray<PlaybackPreviewTrack>;
  readonly defaultAudioTrackId?: ID | null;
  readonly defaultCaptionTrackId?: ID | null;
  readonly defaultPreviewTrackId?: ID | null;
}

export interface TopContent {
  readonly id: ID;
  readonly title: string;
  readonly kind: UploadKind | "live";
  readonly views: number;
  readonly watchHours: number;
  readonly trend: number; // percentage change
  readonly thumbnail: string;
}

export interface RevenueEntry {
  readonly id: ID;
  readonly date: string;
  readonly source: string;
  readonly description: string;
  readonly amount: number; // positive for income, negative for payout
}

export interface CreatorNotification {
  readonly id: ID;
  readonly kind: string;
  readonly body: string;
  readonly sentAt: string;
  readonly amount?: number;
  readonly actor?: string;
}

export interface CreatorScene {
  readonly id: string;
  readonly label: string;
  readonly active: boolean;
}

export interface CreatorLiveSettings {
  readonly subscriberOnly: boolean;
  readonly slowModeSeconds: number;
  readonly autoModLevel: string;
  readonly notifyFollowersDefault: boolean;
  readonly activeSceneId: string;
  readonly scenes: ReadonlyArray<CreatorScene>;
}

export interface CreatorHealthSample {
  readonly collectedAt: string;
  readonly bitrateKbps: number;
  readonly viewers: number;
  readonly cpuPercent: number;
  readonly droppedFrames: number;
  readonly freeDiskGb: number;
}

export interface CreatorLiveHealth {
  readonly currentBitrateKbps: number;
  readonly currentCpuPercent: number;
  readonly currentDroppedFrames: number;
  readonly currentFreeDiskGb: number;
  readonly samples: ReadonlyArray<CreatorHealthSample>;
}

export interface LiveIngestSession {
  readonly id: ID;
  readonly creatorId: ID;
  readonly broadcastId: ID;
  readonly protocol: string;
  readonly ingestServer: string;
  readonly status: string;
  readonly bitrateKbps: number;
  readonly viewers: number;
  readonly droppedFrames: number;
  readonly connectedAt: string;
  readonly lastHeartbeatAt: string;
  readonly disconnectedAt?: string | null;
}

export interface CollaborationSessionSummary {
  readonly id: ID;
  readonly title: string;
  readonly status: string;
  readonly chatMode: string;
  readonly recordingPolicy: string;
  readonly createdAt: string;
  readonly updatedAt: string;
  readonly activatedAt?: string | null;
  readonly endedAt?: string | null;
}

export interface CollaborationInvite {
  readonly id: ID;
  readonly sessionId: ID;
  readonly hostCreatorId: ID;
  readonly inviteeUserId: ID;
  readonly inviteeCreatorId?: ID | null;
  readonly role: string;
  readonly state: string;
  readonly mirrorToGuestChannel: boolean;
  readonly message?: string | null;
  readonly createdAt: string;
  readonly respondedAt?: string | null;
  readonly expiresAt: string;
}

export interface CollaborationParticipant {
  readonly id: ID;
  readonly sessionId: ID;
  readonly inviteId?: ID | null;
  readonly userId: ID;
  readonly creatorId?: ID | null;
  readonly role: string;
  readonly state: string;
  readonly publishToHost: boolean;
  readonly mirrorToGuestChannel: boolean;
  readonly canSpeakInChat: boolean;
  readonly joinedAt?: string | null;
  readonly leftAt?: string | null;
  readonly createdAt: string;
  readonly updatedAt: string;
}

export interface CollaborationSession {
  readonly id: ID;
  readonly hostCreatorId: ID;
  readonly sourceBroadcastId: ID;
  readonly title: string;
  readonly status: string;
  readonly chatMode: string;
  readonly recordingPolicy: string;
  readonly lastEventSeq: number;
  readonly createdAt: string;
  readonly updatedAt: string;
  readonly activatedAt?: string | null;
  readonly endedAt?: string | null;
  readonly invites: ReadonlyArray<CollaborationInvite>;
  readonly participants: ReadonlyArray<CollaborationParticipant>;
}

export interface CollaborationHostSummary {
  readonly creatorId: ID;
  readonly userId: ID;
  readonly handle: string;
  readonly displayName: string;
  readonly avatar: string;
  readonly partnerStatus: string;
  readonly liveStatus: string;
  readonly currentBroadcastId?: ID | null;
}

export interface CollaborationSessionView {
  readonly id: ID;
  readonly hostCreatorId: ID;
  readonly sourceBroadcastId: ID;
  readonly title: string;
  readonly status: string;
  readonly chatMode: string;
  readonly recordingPolicy: string;
  readonly lastEventSeq: number;
  readonly createdAt: string;
  readonly updatedAt: string;
  readonly activatedAt?: string | null;
  readonly endedAt?: string | null;
  readonly host: CollaborationHostSummary;
  readonly participant: CollaborationParticipant;
  readonly participants: ReadonlyArray<CollaborationParticipant>;
}

export interface CollaborationEvent {
  readonly id: ID;
  readonly sessionId: ID;
  readonly sequence: number;
  readonly actorUserId?: ID | null;
  readonly participantId?: ID | null;
  readonly eventType: string;
  readonly payload: unknown;
  readonly createdAt: string;
}

export interface CollaborationMirrorGrant {
  readonly id: ID;
  readonly sessionId: ID;
  readonly participantId: ID;
  readonly hostCreatorId: ID;
  readonly guestCreatorId: ID;
  readonly scope: string;
  readonly state: string;
  readonly publishToHost: boolean;
  readonly mirrorToGuestChannel: boolean;
  readonly issuedAt: string;
  readonly activatedAt?: string | null;
  readonly revokedAt?: string | null;
  readonly expiresAt: string;
}

export interface CollaborationMirrorPickup {
  readonly id: ID;
  readonly sessionId: ID;
  readonly participantId: ID;
  readonly grantId: ID;
  readonly hostCreatorId: ID;
  readonly guestCreatorId: ID;
  readonly sourceBroadcastId: ID;
  readonly guestBroadcastId: ID;
  readonly state: string;
  readonly activatedAt: string;
  readonly updatedAt: string;
  readonly endedAt?: string | null;
}

export interface CollaborationTopologyMember {
  readonly participantId: ID;
  readonly userId: ID;
  readonly creatorId?: ID | null;
  readonly role: string;
  readonly state: string;
  readonly publishToHost: boolean;
  readonly mirrorToGuestChannel: boolean;
  readonly canSpeakInChat: boolean;
  readonly hostOutputState: string;
  readonly mirrorPickupState: string;
  readonly mirrorPickupBroadcastId?: ID | null;
  readonly mirrorPickupActivatedAt?: string | null;
}

export interface CollaborationRuntimeTopology {
  readonly sessionId: ID;
  readonly sourceBroadcastId: ID;
  readonly chatMode: string;
  readonly recordingPolicy: string;
  readonly sharedChat: boolean;
  readonly recordingOwnerCreatorId?: ID | null;
  readonly connectedParticipants: number;
  readonly hostOutputParticipantIds: ReadonlyArray<ID>;
  readonly backstageParticipantIds: ReadonlyArray<ID>;
  readonly liveParticipantIds: ReadonlyArray<ID>;
  readonly mirroredCreatorIds: ReadonlyArray<ID>;
  readonly members: ReadonlyArray<CollaborationTopologyMember>;
}

export interface CollaborationRuntimeResponse {
  readonly session: CollaborationSessionView;
  readonly topology: CollaborationRuntimeTopology;
  readonly grants: ReadonlyArray<CollaborationMirrorGrant>;
  readonly pickups: ReadonlyArray<CollaborationMirrorPickup>;
  readonly recentEvents: ReadonlyArray<CollaborationEvent>;
}

export interface CollaborationSocketPresence {
  readonly id: ID;
  readonly sessionId: ID;
  readonly userId: ID;
  readonly creatorId?: ID | null;
  readonly participantId?: ID | null;
  readonly connectedAt: string;
  readonly lastSeenAt: string;
  readonly disconnectedAt?: string | null;
  readonly isStale: boolean;
}

export interface CreatorCollaborationControlResponse {
  readonly runtime: CollaborationRuntimeResponse;
  readonly socketSessions: ReadonlyArray<CollaborationSocketPresence>;
  readonly pendingInviteCount: number;
  readonly activeGrantCount: number;
  readonly issuedGrantCount: number;
  readonly staleSocketCount: number;
}

export interface CreatorLiveCollaborationSummary {
  readonly activeSession?: CollaborationSession | null;
  readonly activeControl?: CreatorCollaborationControlResponse | null;
  readonly recentSessions: ReadonlyArray<CollaborationSession>;
  readonly totalSessions: number;
  readonly activeSessionCount: number;
  readonly pendingInviteCount: number;
  readonly activeGrantCount: number;
  readonly issuedGrantCount: number;
}

export interface CreatorSubscriberTier {
  readonly id: ID;
  readonly tierName: string;
  readonly rank: number;
  readonly monthlyPrice: number;
  readonly subscriberCount: number;
  readonly accentColor: string;
  readonly status: string;
  readonly retiredAt?: string | null;
}

export interface CreatorLiveSnapshot {
  readonly profile: CreatorProfile;
  readonly currentBroadcast?: Broadcast | null;
  readonly pendingBroadcast?: Broadcast | null;
  readonly ingestSession?: LiveIngestSession | null;
}

export interface CreatorLiveControlResponse {
  readonly snapshot: CreatorLiveSnapshot;
  readonly settings: CreatorLiveSettings;
  readonly health: CreatorLiveHealth;
  readonly collaboration: CreatorLiveCollaborationSummary;
  readonly subscriberTiers: ReadonlyArray<CreatorSubscriberTier>;
  readonly isLive: boolean;
  readonly currentViewers: number;
  readonly bitrateHistory: ReadonlyArray<number>;
  readonly viewerHistory: ReadonlyArray<number>;
}

export interface LiveIngestEvent {
  readonly id: ID;
  readonly sessionId: ID;
  readonly creatorId: ID;
  readonly broadcastId: ID;
  readonly eventType: string;
  readonly payload: unknown;
  readonly createdAt: string;
}

export interface CreatorLiveRuntimeResponse {
  readonly snapshot: CreatorLiveSnapshot;
  readonly health: CreatorLiveHealth;
  readonly collaboration: CreatorLiveCollaborationSummary;
  readonly activeSession?: LiveIngestSession | null;
  readonly recentSessions: ReadonlyArray<LiveIngestSession>;
  readonly recentEvents: ReadonlyArray<LiveIngestEvent>;
}

export interface CreatorContentSummary {
  readonly totalUploads: number;
  readonly publishedUploads: number;
  readonly scheduledUploads: number;
  readonly processingUploads: number;
  readonly draftUploads: number;
  readonly archivedUploads: number;
  readonly totalViews: number;
  readonly totalWatchHours: number;
  readonly totalStorageBytes: number;
  readonly filteredCount: number;
}

export interface CreatorContentResponse {
  readonly summary: CreatorContentSummary;
  readonly uploads: ReadonlyArray<Upload>;
}
