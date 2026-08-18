use std::{
    collections::HashMap,
    fmt::Write,
    path::{Component, Path as FsPath, PathBuf},
    sync::Arc,
    time::Duration,
};

use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post, put},
};
use chrono::{Duration as ChronoDuration, Utc};
use serde_json::{Value, json};
use sha2::Digest;
use sqlx::{Row, SqlitePool};
use tokio::{
    process::Command,
    time::{interval, sleep},
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::warn;
use uuid::Uuid;

#[cfg(test)]
use crate::models::{AdminLiveIngestOverviewQuery, RepairLiveRuntimeOutputRequest};
use crate::{
    auth::{RequestIdentity, hash_token, lookup_identity, optional_identity, require_identity},
    error::{AppError, AppResult},
    models::{
        AdminLiveIngestQuery, AdminLiveIngestSessionRecord, AdminMediaJobQuery,
        AdminMediaJobRecord, AdminPlaybackSessionQuery, AdminPlaybackSessionRecord, AnalyticsPoint,
        AppendUploadChunkQuery, AuthSession, BillingPlan, Broadcast, BulkUploadRequest, Category,
        CategoryBrowseResponse, ChatInput, ChatMessage, CollaborationEvent,
        CollaborationEventsQuery, CollaborationHostSummary, CollaborationInvite,
        CollaborationMirrorGrant, CollaborationMirrorPickup, CollaborationParticipant,
        CollaborationReconciliationAction, CollaborationReconciliationReport,
        CollaborationRuntimeResponse, CollaborationSession, CollaborationSessionView,
        CollaborationSocketPresence, CollaborationSocketPresenceReconciliationAction,
        CollaborationSocketPresenceReconciliationReport, CollaborationTopologyMember,
        ConnectedAccount, ContentPurchase, ContentPurchaseReconciliationReport,
        ContinueWatchingEntry, CreateCollaborationInviteRequest, CreateCollaborationSessionRequest,
        CreateCreatorEnforcementActionRequest, CreateCreatorModeratorRequest,
        CreateCreatorSeriesRequest, CreateCreatorSubscriberTierRequest,
        CreateLiveModerationActionRequest, CreateSessionRequest, CreateUploadJobRequest,
        CreatorAnalyticsSummary, CreatorAppState, CreatorCatalogEpisode, CreatorCatalogFilm,
        CreatorCatalogSeason, CreatorCatalogSeries, CreatorCollaborationControlResponse,
        CreatorContentQuery, CreatorContentResponse, CreatorContentSummary, CreatorDashboard,
        CreatorEnforcementAction, CreatorEnforcementReconciliationAction,
        CreatorEnforcementReconciliationReport, CreatorEnforcementState, CreatorHealthSample,
        CreatorLiveCollaborationSummary, CreatorLiveControlResponse, CreatorLiveHealth,
        CreatorLiveRuntimeResponse, CreatorLiveSettings, CreatorLiveSnapshot,
        CreatorLiveSocketPresence, CreatorLiveSocketPresenceReconciliationAction,
        CreatorLiveSocketPresenceReconciliationReport, CreatorMembership,
        CreatorMembershipReconciliationReport, CreatorModerator, CreatorNotification,
        CreatorOperationalChecklistItem, CreatorOperationalState, CreatorProfile,
        CreatorRevenueBreakdownEntry, CreatorRevenueSummary, CreatorSeriesProject,
        CreatorSubscriberTier, CreatorUploadOperationRecord, CreatorUploadOperationsResponse,
        CreatorUploadOperationsSummary, DownloadSettings, Episode, Film, FollowingFeedResponse,
        HealthDependencies, HealthDependencyStatus, HealthResponse, HomeResponse,
        IngestConnectRequest, IngestConnectResponse, IngestHeartbeatRequest, LanguageSettings,
        LiveDiscoveryResponse, LiveIngestEvent, LiveIngestReconciliationAction,
        LiveIngestReconciliationReport, LiveIngestSession, LiveModerationAction,
        LiveModerationReconciliationAction, LiveModerationReconciliationReport,
        LiveNotifyPreference, LiveReportRequest, LiveRuntimeOutput, LiveStream,
        LiveStreamReportRecord, MediaAsset, MediaAssetVariant, MediaJobReconciliationAction,
        MediaJobReconciliationReport, MediaProcessingRun, ModerationAuditEntry,
        NotificationChannelSetting, NotificationDeliveryQuery,
        NotificationDeliveryReconciliationAction, NotificationDeliveryReconciliationReport,
        NotificationDeliveryRecord, NotificationSettings, ParentalControls, PlaybackAccessQuery,
        PlaybackAudioTrack, PlaybackCaptionTrack, PlaybackGrant, PlaybackPreviewTrack,
        PlaybackReconciliationAction, PlaybackReconciliationReport, PlaybackSession,
        PlaybackSettings, PrivacySettings, ProgressInput, PublishUploadJobRequest,
        ReleaseCreatorEnforcementActionRequest, ResolveLiveStreamReportRequest, RevenueEntry,
        Season, Series, SessionTokenResponse, StartBroadcastRequest, Streamer,
        TerminateLiveIngestRequest, TopContent, TrafficSource,
        UpdateCollaborationParticipantRequest, UpdateCreatorLiveSettingsRequest,
        UpdateCreatorOperationalStateRequest, UpdateCreatorSeriesRequest,
        UpdateCreatorSubscriberTierRequest, UpdateLiveRequest, UpdateLiveRuntimeStateRequest,
        UpdateProfileRequest, UpdateSettingsRequest, UpdateUploadJobRequest,
        UpdateUploadLifecycleRequest, UpdateUploadRequest, Upload, UploadIngestSession,
        UploadIngestTicket, UploadJob, User, UserEntitlementReconciliationAction, UserEntitlements,
        UserLibrary, UserNotification, UserProfileDetails, UserSettingsBundle, ViewerAppState,
        ViewerPreview, WatchHistoryEntry, WatchlistResponse, WsEvent,
    },
    state::AppState,
};

mod admin_ops;
mod api_runtime;
mod api_surface;
mod app_request;
mod collab;
mod collaboration_events;
mod collabs;
mod control;
mod creator;
mod dashboard;
mod discovery;
mod ingest;
mod me;
mod media;
mod mirror;
mod moderation;
mod notifications;
mod playauth;
mod playback;
mod presence;
mod public;
mod realtime;
mod reconciliation;
mod shared_helpers;
mod uploads;
mod validation;

pub(super) use app_request::{enforce_rate_limit, validate_request_origin};

use collab::{
    build_collaboration_runtime_response_for_host,
    build_collaboration_runtime_response_for_participant, build_collaboration_runtime_topology,
    build_creator_collaboration_control_response_for_host,
    collaboration_event_is_visible_to_session, collaboration_session_view_for_host,
    disconnect_stale_collaboration_socket_sessions_for_session, end_collaboration_session_internal,
    expire_collaboration_mirror_grants_for_session,
    expire_pending_collaboration_invites_for_session,
    fetch_active_collaboration_session_for_broadcast, fetch_collaboration_events,
    fetch_collaboration_host_summary, fetch_collaboration_invite_by_id,
    fetch_collaboration_invites_for_user, fetch_collaboration_participant_by_id,
    fetch_collaboration_participant_for_user, fetch_collaboration_session_by_id,
    fetch_collaboration_session_for_host, fetch_collaboration_session_for_participant,
    fetch_collaboration_sessions_for_host, fetch_collaboration_sessions_for_participant,
    fetch_collaboration_socket_presence_by_id_raw, fetch_creator_live_collaboration_summary,
    fetch_visible_collaboration_mirror_grants_for_session_view,
    fetch_visible_collaboration_mirror_pickups_for_session_view,
    filter_visible_collaboration_events_for_session, has_pending_collaboration_invite_for_user,
    load_collaboration_socket_event_bootstrap, publish_collaboration_topology,
    reconcile_single_collaboration_session, reconcile_single_collaboration_socket_session,
    resolve_collaboration_broadcast, validate_collaboration_participant_access,
};
use collaboration_events::{
    collaboration_channel_id, publish_collaboration_event,
    publish_collaboration_invite_revoked_events, publish_collaboration_invite_revoked_events_raw,
    publish_collaboration_reconciliation_event,
};
#[cfg(test)]
use collabs::{
    accept_collaboration_invite, create_collaboration_invite, create_collaboration_session,
    end_collaboration_session, get_creator_collaboration_control,
    get_creator_collaboration_runtime, get_creator_collaboration_session,
    get_creator_collaboration_socket_session, get_my_collaboration_runtime,
    get_my_collaboration_session, list_creator_collaboration_events, list_my_collaboration_events,
    list_my_collaboration_invites, reconcile_creator_collaboration_socket_session,
    remove_collaboration_participant, revoke_collaboration_invite,
    update_collaboration_participant,
};
use collabs::{apply_collaboration_participant_update, revoke_collaboration_invite_internal};
#[cfg(test)]
use control::{
    canonical_live_runtime_archive_relative_path, canonical_live_runtime_manifest_relative_path,
};
use control::{
    close_live_ingest_session, count_live_ingest_sessions_for_broadcast,
    enqueue_creator_broadcast_ended_notification, ensure_live_stream_row,
    fetch_active_live_ingest_session, fetch_active_live_ingest_session_unreconciled,
    fetch_admin_live_ingest_overview, fetch_admin_live_ingest_session_record,
    fetch_admin_live_ingest_sessions, fetch_creator_live_ingest_session_record,
    fetch_current_live_runtime_output, fetch_live_ingest_events_for_creator,
    fetch_live_ingest_events_for_session, fetch_live_ingest_session_by_id,
    fetch_live_ingest_session_by_id_global, fetch_live_ingest_session_by_id_global_unreconciled,
    fetch_live_ingest_session_by_id_unreconciled, fetch_recent_live_ingest_sessions,
    fetch_recent_live_runtime_outputs, fetch_terminalizable_live_ingest_sessions_for_broadcast,
    initialize_live_runtime_output, mark_live_ingest_session_stale,
    mark_live_ingest_session_stale_in_db, persist_live_runtime_spec,
    reconcile_single_live_ingest_session, reconcile_stale_live_ingest_sessions,
    reset_creator_live_operational_metrics, transition_broadcast_to_live,
    update_live_runtime_output, validate_live_ingest_session,
    validate_live_ingest_session_any_status, write_live_ingest_event,
};
pub(crate) use creator::{
    build_creator_live_snapshot, contract_broadcast, contract_broadcasts, contract_creator_profile,
    contract_live_status, creator_live_channel_id,
    fetch_authoritative_creator_live_control_response,
    fetch_authoritative_creator_live_runtime_response,
    fetch_creator_live_socket_presence_by_id_raw, normalize_creator_live_profile,
    publish_authoritative_creator_live_state, publish_creator_live_state,
    publish_current_creator_live_state,
};
pub(crate) use creator::{
    fetch_content_purchase_by_id, fetch_creator_catalog_film_by_id,
    fetch_creator_catalog_film_by_slug, fetch_creator_catalog_films, fetch_creator_catalog_series,
    fetch_creator_catalog_series_by_id, fetch_creator_catalog_series_by_slug,
    fetch_creator_membership, fetch_current_content_purchase, fetch_user_entitlements,
    purchase_belongs_to_user, reconcile_single_membership_entitlement,
    reconcile_single_purchase_entitlement,
};
pub(crate) use creator::{
    fetch_creator_enforcement_action_by_id_raw, fetch_creator_live_settings,
    fetch_creator_operational_state, fetch_creator_profile, fetch_creator_profile_by_stream_key,
    fetch_creator_subscriber_tier_by_id, fetch_creator_subscriber_tiers,
};
#[cfg(test)]
use creator::{fetch_creator_live_control_response, fetch_creator_live_runtime_response};
#[cfg(test)]
use creator::{get_creator_live_socket_session, reconcile_creator_live_socket_session};
use dashboard::{
    creator_dashboard_payload, derive_upload_lifecycle_status, ensure_creator_series_season,
    fetch_analytics, fetch_broadcast_by_id, fetch_broadcasts, fetch_creator_app_state,
    fetch_creator_series_title, fetch_creator_upload_operations_response, fetch_revenue_entries,
    fetch_upload_by_id, fetch_uploads, filter_creator_uploads, summarize_creator_analytics,
    summarize_creator_content, summarize_creator_revenue, validate_bulk_upload_action,
    validate_upload_job_kind, validate_upload_job_source_type, validate_upload_visibility,
};
use discovery::{
    fetch_creator_id_for_user, fetch_live_stream_by_id, fetch_live_streams,
    fetch_streamer_by_handle,
};
#[cfg(test)]
use ingest::{
    connect_live_ingest, disconnect_live_ingest, end_broadcast, get_admin_live_ingest_overview,
    get_admin_live_ingest_session, get_creator_live_ingest_session_by_id, heartbeat_live_ingest,
    list_creator_live_ingest_events, reconcile_admin_live_ingest_session,
    reconcile_creator_live_ingest_session, repair_admin_live_runtime_output,
    repair_creator_live_runtime_output, report_live_runtime, terminate_creator_live_ingest,
    terminate_live_ingest,
};
#[cfg(test)]
use me::{
    get_my_membership_entitlement, get_my_purchase_entitlement,
    reconcile_my_membership_entitlement, reconcile_my_purchase_entitlement, revoke_session,
};
use media::access::{
    check_database, ensure_parent_dir, fetch_admin_playback_session_record,
    fetch_admin_playback_sessions, fetch_playback_session_by_id, media_api_url,
    media_path_for_relative, parse_ffprobe_ratio, path_allowed_for_paths,
    playback_path_allowed_for_asset, require_ingest_token, require_upload_token,
    rewrite_hls_manifest_media_uri_line, rewrite_hls_manifest_reference, sanitize_slug,
    sanitize_storage_key, serve_media_file, sha256_file, slugify, validate_playback_session,
    validate_upload_ingest_token,
};
#[cfg(test)]
use media::pipeline::{
    GeneratedHlsVariant, MAX_MEDIA_PROCESSING_ATTEMPTS, fail_media_job_for_lease,
    validate_generated_hls_package, write_hls_master_manifest,
};
use media::pipeline::{
    HlsVariantPlan, ProbedAudioStream, ProbedMedia, StoredMediaPreviewTrack,
    ensure_media_asset_shell, fetch_admin_media_job_record, fetch_admin_media_jobs,
    fetch_media_asset_by_id_any_creator, fetch_media_asset_by_upload_id,
    fetch_media_asset_by_upload_job, fetch_media_asset_variants, fetch_media_assets,
    fetch_media_preview_track_rows, fetch_pending_media_jobs, fetch_upload_ingest_session,
    fetch_upload_ingest_sessions, fetch_upload_job_by_id, fetch_upload_job_by_id_global,
    fetch_upload_job_creator_id, fetch_upload_jobs, plan_hls_variants,
    publish_due_scheduled_upload_releases, reconcile_single_media_job,
    reconcile_stale_media_processing_jobs, requeue_media_job_for_processing,
    schedule_media_processing,
};
#[cfg(test)]
use mirror::fetch_collaboration_mirror_grant_by_id;
use mirror::{
    deactivate_collaboration_mirror_pickups_for_grants,
    fetch_collaboration_mirror_grants_for_participant,
    fetch_collaboration_mirror_grants_for_session,
    fetch_collaboration_mirror_pickups_for_participant,
    fetch_collaboration_mirror_pickups_for_session, issue_mirror_grant_for_participant,
    redeem_collaboration_mirror_grant_internal, revoke_collaboration_mirror_grants_for_participant,
    revoke_collaboration_mirror_grants_for_session,
    revoke_collaboration_mirror_grants_for_session_raw,
    sync_active_collaboration_mirror_pickups_for_session,
    sync_active_collaboration_mirror_pickups_for_session_and_publish,
};
use moderation::{
    creator_enforcement_action_from_row, fetch_active_live_moderation_action,
    fetch_live_moderation_action_by_id, fetch_live_moderation_action_by_id_raw,
    live_moderation_action_from_row, write_moderation_audit_entry,
};
#[cfg(test)]
use notifications::claim_notification_delivery_attempt;
use notifications::{
    dispatch_notification_delivery, enqueue_notification_event,
    fetch_live_notification_recipient_user_ids, fetch_notification_deliveries,
    fetch_notification_delivery_by_id, fetch_notification_delivery_by_id_raw,
    fetch_notifications_rows, fetch_user_notifications, reconcile_single_notification_delivery,
};
use playauth::{
    PlaybackSessionRecord, build_media_audio_tracks, build_media_caption_tracks,
    build_media_preview_tracks, default_audio_track_id, default_caption_track_id,
    default_preview_track_id, expire_playback_session_by_id,
    expire_playback_sessions_for_auth_session, expire_playback_sessions_for_upload,
    fetch_active_creator_membership, fetch_live_stream_playback_target,
    fetch_playback_session_record_by_id, fetch_upload_playback_target,
    fetch_user_audio_preferences, fetch_user_subtitle_preference, playback_session_from_record,
    reconcile_invalid_playback_sessions, reconcile_playback_sessions_for_read,
    reconcile_playback_sessions_for_user, reconcile_single_playback_session,
    resolve_upload_access_terms, resolve_upload_playback_access,
    validate_existing_playback_session_access, validate_playback_session_record,
    validate_playback_session_record_for_path,
};
#[cfg(test)]
use playback::{
    create_content_playback_session, create_live_playback_session, get_admin_playback_session,
    get_playback_manifest, get_playback_session, reconcile_admin_playback_session,
    refresh_playback_session,
};
#[cfg(test)]
use presence::count_active_live_viewer_sessions;
use presence::{
    active_presence_cutoff, count_active_collaboration_socket_sessions,
    count_all_active_collaboration_socket_sessions, count_all_active_creator_live_socket_sessions,
    count_all_active_live_viewer_sessions, disconnect_collaboration_socket_session,
    disconnect_creator_live_socket_session, disconnect_live_viewer_session,
    effective_live_viewer_count, ensure_stream_exists, fetch_auth_sessions,
    fetch_chat_messages_for_viewer, fetch_continue_watching_entry, fetch_live_viewer_sample_users,
    next_chat_message_sequence, reconcile_single_creator_live_socket_session,
    reconcile_stale_creator_live_socket_sessions_for_read, reconcile_stale_presence_sessions,
    register_collaboration_socket_session, register_creator_live_socket_session,
    register_live_viewer_session, touch_collaboration_socket_session,
    touch_creator_live_socket_session, touch_live_viewer_session, upsert_watch_history_entry,
};
use public::PersistedChatMessage;
#[cfg(test)]
use public::{
    LimitQuery, check_binary_available, check_media_root_writable,
    check_runtime_dependencies_with_binaries,
};
use realtime::reconcile_collaboration_expiry_for_host_read;
#[cfg(test)]
use realtime::{CollaborationSocketCommand, fetch_current_collaboration_socket_session_view};
use reconciliation::{
    ensure_creator_can_accept_paid_transactions, ensure_creator_can_manage_subscription_tiers,
    ensure_creator_can_publish_paid_content, ensure_creator_collaboration_enabled,
    ensure_creator_live_streaming_enabled, ensure_creator_upload_ingest_enabled,
    is_live_ingest_session_stale, is_upload_job_stale, reconcile_expired_collaboration_invites,
    reconcile_expired_collaboration_mirror_grants, reconcile_expired_creator_enforcement_actions,
    reconcile_expired_creator_enforcement_actions_for_read,
    reconcile_expired_live_moderation_actions, reconcile_expired_live_moderation_actions_for_read,
    reconcile_expired_user_entitlements, reconcile_expired_user_entitlements_for_read,
    reconcile_notification_deliveries, reconcile_scheduled_upload_releases,
    reconcile_single_creator_enforcement_action, reconcile_single_live_moderation_action,
    stale_live_ingest_cutoff, stale_media_processing_cutoff, validate_creator_access_tier,
};
use shared_helpers::{
    build_fts_query, from_json, live_stream_from_row, notification_delivery_record_from_row,
    playback_content_session_api_url, stream_channel_id, streamer_from_row, to_json,
};
use validation::{
    monetized_access_policy, parse_optional_future_timestamp,
    transition_creator_operational_status, validate_collaboration_chat_mode,
    validate_collaboration_participant_state, validate_collaboration_participant_transition,
    validate_collaboration_recording_policy, validate_collaboration_role,
    validate_pending_collaboration_invite, validate_profile_update,
    validate_redeemable_collaboration_mirror_grant, validate_settings_update,
};

type SharedState = Arc<AppState>;
const WS_PRESENCE_TTL_SECONDS: i64 = 45;
const WS_PRESENCE_TOUCH_INTERVAL_SECONDS: u64 = 15;
const MAX_NOTIFICATION_DELIVERY_ATTEMPTS: i64 = 3;
const BACKGROUND_WORKER_STALE_AFTER_SECONDS: u64 = 15;

pub fn router(state: SharedState) -> Router {
    api_surface::router(state)
}

pub fn start_background_workers(state: SharedState) {
    api_runtime::start_background_workers(state)
}

#[cfg(test)]
use admin_ops::*;
#[cfg(test)]
use creator::*;
#[cfg(test)]
use discovery::*;
#[cfg(test)]
use media::access::*;
#[cfg(test)]
use media::jobs::*;
#[cfg(test)]
use media::pipeline::*;
#[cfg(test)]
use moderation::*;
#[cfg(test)]
use public::*;
#[cfg(test)]
use realtime::*;
#[cfg(test)]
use uploads::*;

#[cfg(test)]
mod tests;
