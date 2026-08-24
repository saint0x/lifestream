// Core domain types for the VANTA platform.
// Everything downstream (repository, components, pages) consumes these.

export type ID = string;

export type ContentKind = "series" | "film" | "live";

export type SearchResultKind =
  | "series"
  | "film"
  | "live"
  | "episode"
  | "creator"
  | "profile"
  | "category";

export type MaturityRating = "G" | "PG" | "PG-13" | "TV-14" | "R" | "TV-MA";

export type Genre =
  | "Drama"
  | "Thriller"
  | "Science Fiction"
  | "Cinematic Tech"
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
  readonly personId?: ID | null;
  readonly personSlug?: string | null;
  readonly name: string;
  readonly role: string;
  readonly character?: string | null;
  readonly avatar?: string | null;
}

export interface ProjectCreditInput {
  readonly personId?: ID | null;
  readonly personSlug?: string | null;
  readonly role: string;
  readonly character?: string | null;
}

export interface UpdateProjectCreditsRequest {
  readonly credits: ReadonlyArray<ProjectCreditInput>;
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

export interface SearchResult {
  readonly id: ID;
  readonly kind: SearchResultKind;
  readonly slug: string;
  readonly title: string;
  readonly subtitle: string;
  readonly image?: string | null;
  readonly href: string;
  readonly metadata: Record<string, unknown>;
  readonly score: number;
}

export interface PersonCredit {
  readonly contentId: ID;
  readonly contentSlug: string;
  readonly contentKind: "series" | "film";
  readonly title: string;
  readonly year: number;
  readonly role: string;
  readonly character?: string | null;
  readonly poster: string;
}

export interface PersonProfile {
  readonly id: ID;
  readonly userId?: ID | null;
  readonly slug: string;
  readonly profileUrlPath: string;
  readonly displayName: string;
  readonly avatar: string;
  readonly heroImage: string;
  readonly headline: string;
  readonly location: string;
  readonly about: string;
  readonly knownFor: ReadonlyArray<string>;
  readonly websiteUrl?: string | null;
  readonly instagramUrl?: string | null;
  readonly xUrl?: string | null;
  readonly imdbUrl?: string | null;
  readonly linkedinUrl?: string | null;
  readonly facebookUrl?: string | null;
  readonly publicLinks: ReadonlyArray<PersonProfileLink>;
  readonly createdAt: string;
  readonly updatedAt: string;
  readonly credits: ReadonlyArray<PersonCredit>;
}

export interface PersonProfileLink {
  readonly id?: ID;
  readonly platform: string;
  readonly label: string;
  readonly url: string;
  readonly position?: number;
}

export type PublicAlertTargetKind = "profile" | "series" | "episode";
export type PublicAlertContactChannel = "email" | "sms" | "social_dm";

export interface CreatePublicAlertSubscriptionInput {
  readonly targetKind: PublicAlertTargetKind;
  readonly targetId: ID;
  readonly targetSlug?: string | null;
  readonly targetTitle: string;
  readonly visitorId?: string | null;
  readonly contactChannel: PublicAlertContactChannel;
  readonly contactValue: string;
  readonly socialPlatform?: string | null;
  readonly alertTypes: ReadonlyArray<string>;
  readonly sourcePath?: string | null;
}

export interface PublicAlertSubscriptionResponse {
  readonly id: ID;
  readonly targetKind: PublicAlertTargetKind;
  readonly targetId: ID;
  readonly targetTitle: string;
  readonly contactChannel: PublicAlertContactChannel;
  readonly socialPlatform?: string | null;
  readonly alertTypes: ReadonlyArray<string>;
  readonly status: string;
  readonly updatedAt: string;
}

export interface UpdatePersonProfileRequest {
  readonly slug?: string;
  readonly displayName?: string;
  readonly avatar?: string;
  readonly heroImage?: string;
  readonly headline?: string;
  readonly location?: string;
  readonly about?: string;
  readonly knownFor?: ReadonlyArray<string>;
  readonly websiteUrl?: string | null;
  readonly instagramUrl?: string | null;
  readonly xUrl?: string | null;
  readonly imdbUrl?: string | null;
  readonly linkedinUrl?: string | null;
  readonly facebookUrl?: string | null;
  readonly publicLinks?: ReadonlyArray<PersonProfileLink>;
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

export interface CreatorAttentionScore {
  readonly algorithmVersion: string;
  readonly qualifiedViewers: number;
  readonly verifiedViewerScore: number;
  readonly creatorAttentionValue: number;
  readonly baselineValuePerQualifiedViewer: number;
  readonly averageWatchMinutes: number;
  readonly attentionMultiplier: number;
  readonly engagementMultiplier: number;
  readonly retentionMultiplier: number;
  readonly audienceQualityMultiplier: number;
  readonly dataConfidenceMultiplier: number;
  readonly qualifiedViewerRate: number;
  readonly returningViewerRate: number;
  readonly measuredSessions: number;
  readonly measuredViewers: number;
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

export interface PlaybackMediaAuthorization {
  readonly strategy: string;
  readonly manifestAuthorization: string;
  readonly assetAuthorization: string;
  readonly cacheStrategy: string;
  readonly cdnCookieUrl?: string | null;
  readonly cdnCookieName?: string | null;
  readonly cdnCookieDomain?: string | null;
}

export interface PlaybackGrant {
  readonly session: PlaybackSession;
  readonly playbackToken: string;
  readonly manifestUrl: string;
  readonly posterUrl?: string | null;
  readonly thumbnailUrl?: string | null;
  readonly mediaAuthorization: PlaybackMediaAuthorization;
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

export interface MediaAssetVariant {
  readonly id: ID;
  readonly variantType: string;
  readonly label: string;
  readonly relativePath: string;
  readonly url: string;
  readonly mimeType: string;
  readonly width?: number | null;
  readonly height?: number | null;
  readonly bitrateBps?: number | null;
  readonly fileSizeBytes: number;
  readonly isDefault: boolean;
  readonly createdAt: string;
}

export interface MediaProcessingRun {
  readonly id: ID;
  readonly stage: string;
  readonly status: string;
  readonly details: unknown;
  readonly startedAt: string;
  readonly completedAt?: string | null;
}

export interface MediaAsset {
  readonly id: ID;
  readonly uploadJobId: ID;
  readonly uploadId?: ID | null;
  readonly seriesId?: ID | null;
  readonly kind: string;
  readonly title: string;
  readonly status: string;
  readonly visibility: string;
  readonly sourcePath: string;
  readonly sourceUrl: string;
  readonly posterPath?: string | null;
  readonly posterUrl?: string | null;
  readonly playbackPath?: string | null;
  readonly playbackUrl?: string | null;
  readonly mimeType: string;
  readonly checksumSha256?: string | null;
  readonly containerFormat?: string | null;
  readonly fileSizeBytes: number;
  readonly durationSec: number;
  readonly width?: number | null;
  readonly height?: number | null;
  readonly frameRate?: number | null;
  readonly videoCodec?: string | null;
  readonly audioCodec?: string | null;
  readonly hasVideo: boolean;
  readonly hasAudio: boolean;
  readonly createdAt: string;
  readonly updatedAt: string;
  readonly processedAt?: string | null;
  readonly publishedContentId?: ID | null;
  readonly variants: ReadonlyArray<MediaAssetVariant>;
  readonly audioTracks: ReadonlyArray<PlaybackAudioTrack>;
  readonly captionTracks: ReadonlyArray<PlaybackCaptionTrack>;
  readonly previewTracks: ReadonlyArray<PlaybackPreviewTrack>;
  readonly defaultAudioTrackId?: ID | null;
  readonly defaultCaptionTrackId?: ID | null;
  readonly defaultPreviewTrackId?: ID | null;
  readonly processingRuns: ReadonlyArray<MediaProcessingRun>;
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
  readonly deliveryState?: string | null;
  readonly readAt?: string | null;
}

export interface CreatorRevenueBreakdownEntry {
  readonly source: string;
  readonly amount: number;
  readonly share: number;
}

export interface CreatorRevenueSummary {
  readonly totalEarnings30d: number;
  readonly totalSubscribers: number;
  readonly blendedMonthlyPrice: number;
  readonly estimatedNextPayout: number;
  readonly breakdown: ReadonlyArray<CreatorRevenueBreakdownEntry>;
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

export interface CreatorDashboardPayload {
  readonly profile: CreatorProfile;
  readonly currentBroadcast: Broadcast | null;
  readonly scheduledBroadcasts: ReadonlyArray<Broadcast>;
  readonly recentBroadcasts: ReadonlyArray<Broadcast>;
  readonly analytics: ReadonlyArray<AnalyticsPoint>;
  readonly trafficSources: ReadonlyArray<TrafficSource>;
  readonly attentionScore: CreatorAttentionScore;
  readonly topContent: ReadonlyArray<TopContent>;
  readonly revenue: ReadonlyArray<RevenueEntry>;
  readonly revenueSummary: CreatorRevenueSummary;
  readonly subscriberTiers: ReadonlyArray<CreatorSubscriberTier>;
  readonly notifications: ReadonlyArray<CreatorNotification>;
  readonly uploads: ReadonlyArray<Upload>;
}

export interface AdMarketplacePackage {
  readonly id: ID;
  readonly code: string;
  readonly title: string;
  readonly description: string;
  readonly placementKind: string;
  readonly spotLengthSeconds?: number | null;
  readonly deliverables: ReadonlyArray<string>;
  readonly basePriceCents: number;
  readonly currency: string;
  readonly status: string;
}

export interface AdMarketplaceAdvertiser {
  readonly id: ID;
  readonly name: string;
  readonly industry: string;
  readonly websiteUrl?: string | null;
}

export interface AdMarketplaceCampaign {
  readonly id: ID;
  readonly name: string;
  readonly objective: string;
  readonly startsAt?: string | null;
  readonly endsAt?: string | null;
  readonly budgetCents: number;
  readonly currency: string;
  readonly status: string;
}

export interface AdMarketplaceSubmission {
  readonly id: ID;
  readonly offerId: ID;
  readonly submissionUrl: string;
  readonly notes: string;
  readonly status: string;
  readonly submittedAt: string;
  readonly reviewedAt?: string | null;
  readonly advertiserFeedback?: string | null;
  readonly revisionDueAt?: string | null;
}

export interface AdMarketplaceOffer {
  readonly id: ID;
  readonly title: string;
  readonly brief: string;
  readonly requirements: ReadonlyArray<string>;
  readonly offerAmountCents: number;
  readonly creatorPayoutCents: number;
  readonly platformFeeCents: number;
  readonly currency: string;
  readonly status: string;
  readonly advertiserReviewStatus: string;
  readonly dueAt?: string | null;
  readonly createdAt: string;
  readonly updatedAt: string;
  readonly acceptedAt?: string | null;
  readonly declinedAt?: string | null;
  readonly package: AdMarketplacePackage;
  readonly advertiser: AdMarketplaceAdvertiser;
  readonly campaign: AdMarketplaceCampaign;
  readonly submissions: ReadonlyArray<AdMarketplaceSubmission>;
}

export interface AdMarketplaceSummary {
  readonly pendingOffers: number;
  readonly activeOffers: number;
  readonly inReviewOffers: number;
  readonly approvedOffers: number;
  readonly declinedOffers: number;
  readonly totalOfferAmountCents: number;
  readonly totalCreatorPayoutCents: number;
  readonly currency: string;
}

export interface AdMarketplacePaymentProvider {
  readonly providerKey: string;
  readonly displayName: string;
  readonly enabled: boolean;
  readonly mode: string;
  readonly status: string;
}

export interface CreatorAdHubResponse {
  readonly summary: AdMarketplaceSummary;
  readonly offers: ReadonlyArray<AdMarketplaceOffer>;
  readonly packages: ReadonlyArray<AdMarketplacePackage>;
  readonly paymentProvider: AdMarketplacePaymentProvider;
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

export interface UploadJob {
  readonly id: ID;
  readonly uploadId?: ID | null;
  readonly seriesId?: ID | null;
  readonly kind: string;
  readonly sourceType: string;
  readonly status: string;
  readonly title: string;
  readonly intendedVisibility: Visibility;
  readonly bytesExpected: number;
  readonly bytesReceived: number;
  readonly storageKey: string;
  readonly createdAt: string;
  readonly updatedAt: string;
  readonly publishedContentId?: ID | null;
  readonly mimeType: string;
  readonly checksumSha256?: string | null;
  readonly completedAt?: string | null;
  readonly processingAttemptCount: number;
  readonly lastProcessingError?: string | null;
  readonly lastFailedAt?: string | null;
}

export interface UploadIngestSession {
  readonly jobId: ID;
  readonly relativePath: string;
  readonly status: string;
  readonly mimeType: string;
  readonly bytesReceived: number;
  readonly createdAt: string;
  readonly updatedAt: string;
  readonly completedAt?: string | null;
}

export interface UploadIngestTicket {
  readonly session: UploadIngestSession;
  readonly uploadToken: string;
}

export interface CreatorSeriesProject {
  readonly id: ID;
  readonly slug: string;
  readonly title: string;
  readonly synopsis: string;
  readonly rating: string;
  readonly genres: ReadonlyArray<string>;
  readonly heroColor: string;
  readonly posterUrl: string;
  readonly backdropUrl: string;
  readonly status: string;
  readonly createdAt: string;
  readonly updatedAt: string;
}

export interface CreatorUploadOperationRecord {
  readonly uploadJob: UploadJob;
  readonly ingestSession?: UploadIngestSession | null;
  readonly mediaAsset?: unknown | null;
  readonly publishedUpload?: Upload | null;
}

export interface CreatorUploadOperationsSummary {
  readonly totalJobs: number;
  readonly createdJobs: number;
  readonly uploadedJobs: number;
  readonly processingJobs: number;
  readonly readyJobs: number;
  readonly failedJobs: number;
  readonly publishedJobs: number;
  readonly activeIngestSessions: number;
  readonly completedIngestSessions: number;
  readonly readyAssets: number;
  readonly processingAssets: number;
  readonly failedAssets: number;
  readonly publishedAssets: number;
  readonly totalBytesExpected: number;
  readonly totalBytesReceived: number;
  readonly totalAssetBytes: number;
}

export interface CreatorUploadOperationsResponse {
  readonly summary: CreatorUploadOperationsSummary;
  readonly records: ReadonlyArray<CreatorUploadOperationRecord>;
}
