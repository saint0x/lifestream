export type PortalView =
  | "overview"
  | "creators"
  | "niches"
  | "stats"
  | "creator"
  | "cart"
  | "orders"
  | "approvals"
  | "review"
  | "reports"
  | "account";

export type Objective =
  | "awareness"
  | "consideration"
  | "traffic"
  | "conversion"
  | "sponsorship_association"
  | "category_ownership"
  | "launch";

export type ReviewStatus = "review_pending" | "approved" | "revision_requested" | "rejected" | "expired";

export type AdvertiserPermission =
  | "manage_account"
  | "manage_team"
  | "manage_billing"
  | "buy_media"
  | "approve_work"
  | "view_reports";

export type AdvertiserRole = "admin" | "buyer" | "analyst" | "reviewer";

export interface AdvertiserCompany {
  readonly id: string;
  readonly name: string;
  readonly industry: string;
  readonly websiteUrl?: string;
  readonly billingName: string;
  readonly billingEmail: string;
  readonly billingStatus: string;
}

export interface AdvertiserSeat {
  readonly userId: string;
  readonly name: string;
  readonly email: string;
  readonly role: AdvertiserRole;
  readonly permissions: ReadonlyArray<AdvertiserPermission>;
  readonly status: "active" | "suspended";
}

export interface AdvertiserInvite {
  readonly id: string;
  readonly email: string;
  readonly role: AdvertiserRole;
  readonly permissions: ReadonlyArray<AdvertiserPermission>;
  readonly status: "pending" | "accepted" | "revoked" | "expired";
  readonly invitedByUserId: string;
  readonly createdAt: string;
  readonly expiresAt: string;
}

export interface AdvertiserPermissionPreset {
  readonly role: AdvertiserRole;
  readonly label: string;
  readonly permissions: ReadonlyArray<AdvertiserPermission>;
}

export interface AdvertiserAccount {
  readonly company: AdvertiserCompany;
  readonly currentSeat: AdvertiserSeat;
  readonly seats: ReadonlyArray<AdvertiserSeat>;
  readonly invites: ReadonlyArray<AdvertiserInvite>;
  readonly permissionPresets: ReadonlyArray<AdvertiserPermissionPreset>;
}

export interface AttentionProof {
  readonly qualifiedViewers: number;
  readonly verifiedViewerScore: number;
  readonly averageWatchMinutes: number;
  readonly returningViewerRate: number;
  readonly measuredSessions: number;
  readonly dataConfidence: number;
  readonly algorithmVersion: string;
}

export interface InventoryItem {
  readonly id: string;
  readonly creator: string;
  readonly series: string;
  readonly category: string;
  readonly audience: string;
  readonly package: string;
  readonly placement: string;
  readonly availability: string;
  readonly basePriceCents: number;
  readonly objectiveFit: ReadonlyArray<Objective>;
  readonly brandSafety: "standard" | "sensitive_review" | "restricted";
  readonly promotion: string;
  readonly notes: string;
  readonly attention: AttentionProof;
  readonly deliverables: ReadonlyArray<string>;
  readonly minUnits: number;
  readonly maxUnits: number;
  readonly unitLabel: string;
  readonly salesNote: string;
  readonly image: string;
  readonly profileUrl: string;
  readonly episodes: ReadonlyArray<{
    readonly id: string;
    readonly title: string;
    readonly duration: string;
    readonly views: number;
    readonly image: string;
    readonly playbackUrl: string;
  }>;
}

export interface CartLine {
  readonly id: string;
  readonly inventoryId: string;
  readonly units: number;
  readonly objective: Objective;
  readonly flightStart: string;
  readonly flightEnd: string;
  readonly tracking: "none" | "links" | "codes" | "third_party";
  readonly usageRights: "none" | "organic_repost" | "paid_amplification";
  readonly categoryExclusivity: boolean;
  readonly approvalRounds: 0 | 1 | 2;
  readonly context: string;
}

export interface Order {
  readonly id: string;
  readonly createdAt: string;
  readonly advertiser: string;
  readonly lines: ReadonlyArray<CartLine>;
  readonly subtotalCents: number;
  readonly serviceCents: number;
  readonly totalCents: number;
  readonly paymentMethod: string;
  readonly status: "submitted" | "paid" | "sales_review";
}

export interface Campaign {
  readonly id: string;
  readonly name: string;
  readonly objective: Objective;
  readonly status: "draft" | "proposal" | "active" | "in_review" | "reporting" | "renewal";
  readonly creator: string;
  readonly package: string;
  readonly flight: string;
  readonly committedSpendCents: number;
  readonly forecastQualifiedViewers: number;
  readonly deliveredQualifiedViewers: number;
  readonly nextAction: string;
  readonly dueAt: string;
}

export interface Approval {
  readonly id: string;
  readonly campaignId: string;
  readonly campaign: string;
  readonly creator: string;
  readonly package: string;
  readonly submissionUrl: string;
  readonly status: ReviewStatus;
  readonly submittedAt: string;
  readonly decisionDueAt: string;
  readonly notes: string;
}

export interface ReviewComment {
  readonly id: string;
  readonly author: string;
  readonly visibility: "internal_advertiser" | "vanta_only" | "creator_visible";
  readonly body: string;
  readonly timestampSeconds?: number;
  readonly resolved: boolean;
}

export interface ReviewRoom {
  readonly campaignId: string;
  readonly brief: ReadonlyArray<{ readonly label: string; readonly value: string }>;
  readonly versions: ReadonlyArray<{ readonly label: string; readonly status: string; readonly url: string }>;
  readonly comments: ReadonlyArray<ReviewComment>;
  readonly requiredChanges: ReadonlyArray<string>;
  readonly audit: ReadonlyArray<string>;
}

export interface Renewal {
  readonly title: string;
  readonly reason: string;
  readonly priceCents: number;
}

export interface PortalData {
  readonly account: AdvertiserAccount;
  readonly campaigns: ReadonlyArray<Campaign>;
  readonly inventory: ReadonlyArray<InventoryItem>;
  readonly approvals: ReadonlyArray<Approval>;
  readonly reviewRoom: ReviewRoom;
  readonly renewals: ReadonlyArray<Renewal>;
}
