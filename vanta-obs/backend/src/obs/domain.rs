use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct BroadcastInput {
    pub title: String,
    pub category: String,
    pub visibility: String,
    pub latency_profile: String,
    pub recording_policy: String,
    pub archive_policy: String,
    pub scheduled_start: Option<String>,
    pub sponsor_campaign_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BroadcastPatch {
    pub title: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub mature_content: Option<bool>,
    pub language: Option<String>,
    pub scheduled_start: Option<String>,
    pub visibility: Option<String>,
    pub follower_notification: Option<bool>,
    pub chat_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SceneInput {
    pub collection_id: String,
    pub name: String,
    pub transition_kind: Option<String>,
    pub transition_duration_ms: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ScenePatch {
    pub name: Option<String>,
    pub locked: Option<bool>,
    pub validation_state: Option<String>,
    pub transition_kind: Option<String>,
    pub transition_duration_ms: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SceneReorderInput {
    pub scene_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SceneTemplateInput {
    pub collection_id: String,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SceneGroupInput {
    pub child_scene_id: String,
    pub label: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub opacity: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct SceneGroupPatch {
    pub child_scene_id: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TransitionPreviewInput {
    pub from_scene_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SourceInput {
    pub source_kind: String,
    pub display_name: String,
    pub device_id: Option<String>,
    pub browser_url: Option<String>,
    pub media_asset_id: Option<String>,
    pub settings_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct SourcePatch {
    pub display_name: Option<String>,
    pub permission_state: Option<String>,
    pub health_state: Option<String>,
    pub settings_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct SourceFilterInput {
    pub filter_kind: String,
    pub label: String,
    pub order_index: Option<i64>,
    pub settings_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct SourceFilterPatch {
    pub label: Option<String>,
    pub enabled: Option<bool>,
    pub order_index: Option<i64>,
    pub settings_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct AudioChannelPatch {
    pub muted: Option<bool>,
    pub solo: Option<bool>,
    pub gain_db: Option<f64>,
    pub monitor_enabled: Option<bool>,
    pub program_enabled: Option<bool>,
    pub delay_ms: Option<i64>,
    pub filters_json: Option<Value>,
    pub route_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct InstanceInput {
    pub source_id: String,
    pub order_index: i64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Deserialize)]
pub struct InstancePatch {
    pub visible: Option<bool>,
    pub locked: Option<bool>,
    pub order_index: Option<i64>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub opacity: Option<f64>,
    pub settings_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct CueInput {
    pub cue_kind: String,
    pub label: String,
    pub scheduled_at_seconds: Option<f64>,
    pub required_duration_seconds: Option<f64>,
    pub campaign_id: Option<String>,
    pub scene_id: Option<String>,
    pub source_id: Option<String>,
    pub requirements_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct PreflightInput {
    pub broadcast_id: String,
    pub collection_id: String,
}

#[derive(Debug, Deserialize)]
pub struct RecordingInput {
    pub recording_mode: String,
    pub operator_id: Option<String>,
    pub operator_role: Option<String>,
    pub confirmation_text: Option<String>,
    pub acknowledged_risks: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct ReplayInput {
    pub duration_seconds: i64,
    pub label: Option<String>,
    pub sponsor_proof: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct EmergencyDisconnectInput {
    pub reason: Option<String>,
    pub operator_id: Option<String>,
    pub operator_role: Option<String>,
    pub confirmation_text: Option<String>,
    pub acknowledged_risks: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ActionConfirmationInput {
    pub operator_id: Option<String>,
    pub operator_role: Option<String>,
    pub confirmation_text: Option<String>,
    pub acknowledged_risks: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct RuntimeErrorInput {
    pub error_code: Option<String>,
    pub severity: Option<String>,
    pub message: String,
    pub source: Option<String>,
    pub operator_id: Option<String>,
    pub details_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct RuntimeTelemetryInput {
    pub sample_kind: Option<String>,
    pub bitrate_kbps: i64,
    pub upload_mbps: f64,
    pub ingest_latency_ms: i64,
    pub dropped_frames: i64,
    pub cpu_percent: i64,
    pub reconnect_count: Option<i64>,
    pub details_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct LiveOpsOverrideInput {
    pub action: String,
    pub reason: String,
    pub operator_id: Option<String>,
    pub operator_role: Option<String>,
    pub confirmation_text: Option<String>,
    pub acknowledged_risks: Option<Vec<String>>,
    pub target_scene_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GuestInviteInput {
    pub display_name: String,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GuestPatchInput {
    pub muted: Option<bool>,
    pub solo: Option<bool>,
    pub safety_disabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct GuestDeviceCheckInput {
    pub camera_status: String,
    pub microphone_status: String,
    pub network_status: String,
    pub browser_status: String,
    pub bitrate_kbps: i64,
    pub round_trip_ms: i64,
    pub packet_loss_percent: f64,
    pub checks_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct GuestModerationInput {
    pub action: String,
    pub moderator_id: String,
    pub reason: String,
    pub target_scene_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GuestMediaTelemetryInput {
    pub audio_level_db: f64,
    pub speaking: bool,
    pub video_active: bool,
    pub round_trip_ms: i64,
    pub packet_loss_percent: f64,
    pub jitter_ms: Option<i64>,
    pub dropped_frames: Option<i64>,
    pub media_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct GuestWebrtcOfferInput {
    pub session_role: String,
    pub direction: String,
    pub offer_sdp: String,
    pub audio: bool,
    pub video: bool,
    pub preferred_video_layer: Option<String>,
    pub tracks_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct GuestWebrtcAnswerInput {
    pub answer_sdp: String,
    pub selected_video_layer: Option<String>,
    pub media_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct GuestWebrtcIceInput {
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_mline_index: Option<i64>,
    pub candidate_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct GuestRtpPacketInput {
    pub payload_kind: String,
    pub packet_base64: String,
    pub received_at_ms: Option<i64>,
    pub metadata_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct GuestRoomRoutingInput {
    pub room_mode: String,
    pub max_participants: Option<i64>,
    pub shared_feed_source_id: Option<String>,
    pub mirrored_channels: Option<bool>,
    pub latency_target_ms: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct GuestReturnFeedInput {
    pub audio_mode: String,
    pub video_mode: String,
    pub transport: Option<String>,
    pub shared_feed_source_id: Option<String>,
    pub target_latency_ms: Option<i64>,
    pub audio_bitrate_kbps: Option<i64>,
    pub video_bitrate_kbps: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct GuestIsolatedRecordingInput {
    pub recording_mode: Option<String>,
    pub include_video: Option<bool>,
    pub include_audio: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ModeratorInput {
    pub user_id: String,
    pub display_name: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct BlockedTermInput {
    pub term: String,
    pub action: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModerationQueueInput {
    pub author_id: String,
    pub author_name: String,
    pub message: String,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModerationResolveInput {
    pub status: String,
    pub moderator_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PinnedMessageInput {
    pub author_name: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct AudienceTelemetryInput {
    pub viewer_count: i64,
    pub chat_messages_per_minute: Option<i64>,
    pub tips_cents: Option<i64>,
    pub subscriptions: Option<i64>,
    pub revenue_cents: Option<i64>,
    pub discovery_source: Option<String>,
    pub discovery_score: Option<f64>,
    pub details_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct RaidRedirectInput {
    pub target_channel_id: String,
    pub target_channel_name: String,
    pub viewer_count: Option<i64>,
    pub execute_after_seconds: Option<i64>,
    pub redirect_url: Option<String>,
    pub safety_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct ScheduleSlotInput {
    pub title: String,
    pub starts_at: String,
    pub timezone: Option<String>,
    pub duration_minutes: i64,
    pub reminder_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct ScheduleSlotPatch {
    pub title: Option<String>,
    pub starts_at: Option<String>,
    pub timezone: Option<String>,
    pub duration_minutes: Option<i64>,
    pub status: Option<String>,
    pub reminder_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct EngagementPollInput {
    pub poll_kind: String,
    pub question: String,
    pub options: Vec<String>,
    pub duration_seconds: i64,
}

#[derive(Debug, Deserialize)]
pub struct EngagementVoteInput {
    pub option_id: String,
    pub voter_id: String,
}

#[derive(Debug, Deserialize)]
pub struct EngagementAlertInput {
    pub alert_kind: String,
    pub title: String,
    pub message: String,
    pub severity: Option<String>,
    pub source_user: Option<String>,
    pub amount_cents: Option<i64>,
    pub metadata_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct SponsorCampaignInput {
    pub campaign_id: String,
    pub advertiser: String,
    pub title: String,
    pub flight_json: Option<Value>,
    pub claims_json: Option<Value>,
    pub performance_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct SponsorInventoryInput {
    pub campaign_id: String,
    pub creative_kind: String,
    pub label: String,
    pub scheduled_at_seconds: f64,
    pub required_duration_seconds: f64,
    pub scene_id: Option<String>,
    pub required_claims: Option<Vec<String>>,
    pub prohibited_claims: Option<Vec<String>>,
    pub settings_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct SponsorProofInput {
    pub proof_kind: String,
    pub media_time_seconds: f64,
    pub artifact_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct SponsorReviewInput {
    pub status: String,
    pub reviewer_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HotkeyPatch {
    pub binding: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct CheckResult {
    pub key: String,
    pub label: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct PreflightResult {
    pub ready: bool,
    pub checks: Vec<CheckResult>,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn bool_int(value: bool) -> i64 {
    if value { 1 } else { 0 }
}
