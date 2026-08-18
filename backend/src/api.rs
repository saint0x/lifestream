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
        CollaborationRuntimeResponse, CollaborationRuntimeTopology, CollaborationSession,
        CollaborationSessionView, CollaborationSocketPresence,
        CollaborationSocketPresenceReconciliationAction,
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
        LiveNotifyPreference, LiveReportRequest, LiveStream, LiveStreamReportRecord, MediaAsset,
        MediaAssetVariant, MediaJobReconciliationAction, MediaJobReconciliationReport,
        MediaProcessingRun, ModerationAuditEntry, NotificationChannelSetting,
        NotificationDeliveryQuery, NotificationDeliveryReconciliationAction,
        NotificationDeliveryReconciliationReport, NotificationDeliveryRecord, NotificationSettings,
        ParentalControls, PlaybackAccessQuery, PlaybackAudioTrack, PlaybackCaptionTrack,
        PlaybackGrant, PlaybackPreviewTrack, PlaybackReconciliationAction,
        PlaybackReconciliationReport, PlaybackSession, PlaybackSettings, PrivacySettings,
        ProgressInput, PublishUploadJobRequest, ReleaseCreatorEnforcementActionRequest,
        ResolveLiveStreamReportRequest, RevenueEntry, Season, Series, SessionTokenResponse,
        StartBroadcastRequest, Streamer, TerminateLiveIngestRequest, TopContent, TrafficSource,
        UpdateCollaborationParticipantRequest, UpdateCreatorLiveSettingsRequest,
        UpdateCreatorOperationalStateRequest, UpdateCreatorSeriesRequest,
        UpdateCreatorSubscriberTierRequest, UpdateLiveRequest, UpdateProfileRequest,
        UpdateSettingsRequest, UpdateUploadJobRequest, UpdateUploadLifecycleRequest,
        UpdateUploadRequest, Upload, UploadIngestSession, UploadIngestTicket, UploadJob, User,
        UserEntitlementReconciliationAction, UserEntitlements, UserLibrary, UserNotification,
        UserProfileDetails, UserSettingsBundle, ViewerAppState, ViewerPreview, WatchHistoryEntry,
        WatchlistResponse, WsEvent,
    },
    state::AppState,
};

mod admin_ops;
mod collaboration;
mod collaboration_core;
mod collaboration_events;
mod collaboration_mirror;
mod collaboration_runtime;
mod creator_business;
mod creator_catalog;
mod creator_commerce;
mod creator_core;
mod creator_dashboard_data;
mod creator_data;
mod creator_live;
mod discovery;
mod live_ingest;
mod live_ingest_authority;
mod me;
mod media_access;
mod media_pipeline;
mod moderation;
mod notifications;
mod playback;
mod playback_authority;
mod presence;
mod public;
mod realtime;
mod reconciliation;
mod shared_helpers;
mod upload_jobs;
mod uploads;
mod validation;

use admin_ops::{
    get_admin_media_job, get_admin_notification_delivery, reconcile_admin_media_job,
    reconcile_admin_notification_delivery, retry_admin_media_job, routes as admin_ops_routes,
};
use collaboration::{
    accept_collaboration_invite, apply_collaboration_participant_update,
    create_collaboration_invite, create_collaboration_session, end_collaboration_session,
    get_creator_collaboration_control, get_creator_collaboration_runtime,
    get_creator_collaboration_session, get_creator_collaboration_socket_session,
    get_my_collaboration_runtime, get_my_collaboration_session, list_creator_collaboration_events,
    list_my_collaboration_events, list_my_collaboration_invites,
    reconcile_creator_collaboration_socket_session, remove_collaboration_participant,
    revoke_collaboration_invite, revoke_collaboration_invite_internal,
    routes as collaboration_routes, update_collaboration_participant,
};
use collaboration_core::{
    collaboration_event_is_visible_to_session, collaboration_session_view_for_host,
    end_collaboration_session_internal, end_collaboration_session_internal_raw,
    fetch_active_collaboration_session_for_broadcast, fetch_collaboration_events,
    fetch_collaboration_host_summary, fetch_collaboration_invite_by_id,
    fetch_collaboration_invites_for_session, fetch_collaboration_invites_for_user,
    fetch_collaboration_participant_by_id, fetch_collaboration_participant_for_user,
    fetch_collaboration_session_by_id, fetch_collaboration_session_for_host,
    fetch_collaboration_session_for_participant, fetch_collaboration_sessions_for_host,
    fetch_collaboration_sessions_for_participant, filter_visible_collaboration_events_for_session,
    has_pending_collaboration_invite_for_user, load_collaboration_socket_event_bootstrap,
    resolve_collaboration_broadcast, validate_collaboration_participant_access,
};
use collaboration_events::{
    collaboration_channel_id, publish_collaboration_event,
    publish_collaboration_invite_revoked_events, publish_collaboration_invite_revoked_events_raw,
    publish_collaboration_reconciliation_event,
};
use collaboration_mirror::{
    deactivate_collaboration_mirror_pickups_for_grants, fetch_collaboration_mirror_grant_by_id,
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
use collaboration_runtime::{
    build_collaboration_runtime_response_for_host,
    build_collaboration_runtime_response_for_participant, build_collaboration_runtime_topology,
    build_creator_collaboration_control_response_for_host,
    disconnect_stale_collaboration_socket_sessions_for_session,
    expire_collaboration_mirror_grants_for_session,
    expire_pending_collaboration_invites_for_session,
    fetch_collaboration_socket_presence_by_id_raw, fetch_creator_live_collaboration_summary,
    fetch_visible_collaboration_mirror_grants_for_session_view,
    fetch_visible_collaboration_mirror_pickups_for_session_view, publish_collaboration_topology,
    reconcile_single_collaboration_session, reconcile_single_collaboration_socket_session,
};
use creator_catalog::{
    fetch_creator_catalog_film_by_id, fetch_creator_catalog_film_by_slug,
    fetch_creator_catalog_films, fetch_creator_catalog_series, fetch_creator_catalog_series_by_id,
    fetch_creator_catalog_series_by_slug, fetch_creator_series, fetch_creator_series_by_id,
};
use creator_commerce::{
    fetch_content_purchase_by_id, fetch_creator_membership, fetch_current_content_purchase,
    fetch_user_entitlements, purchase_belongs_to_user, reconcile_single_membership_entitlement,
    reconcile_single_purchase_entitlement,
};
use creator_core::{
    get_admin_creator_enforcement_action, get_creator_state,
    reconcile_admin_creator_enforcement_action,
};
use creator_dashboard_data::{
    creator_dashboard_payload, derive_upload_lifecycle_status, ensure_creator_series_season,
    fetch_analytics, fetch_broadcast_by_id, fetch_broadcasts, fetch_creator_app_state,
    fetch_creator_series_title, fetch_creator_upload_operations_response, fetch_revenue_entries,
    fetch_upload_by_id, fetch_uploads, filter_creator_uploads, summarize_creator_analytics,
    summarize_creator_content, summarize_creator_revenue, validate_bulk_upload_action,
    validate_upload_job_kind, validate_upload_job_source_type, validate_upload_visibility,
};
use creator_data::{
    fetch_creator_enforcement_action_by_id, fetch_creator_enforcement_action_by_id_raw,
    fetch_creator_enforcement_state, fetch_creator_live_health, fetch_creator_live_settings,
    fetch_creator_operational_state, fetch_creator_profile, fetch_creator_profile_by_stream_key,
    fetch_creator_subscriber_tier_by_id, fetch_creator_subscriber_tiers,
    next_creator_subscriber_tier_rank, normalize_creator_subscriber_tier_ranks,
    validate_creator_subscriber_tier_input,
};
use creator_live::{
    build_creator_live_snapshot, contract_broadcast, contract_broadcasts, contract_creator_profile,
    contract_live_status, creator_live_channel_id,
    fetch_authoritative_creator_live_control_response,
    fetch_authoritative_creator_live_runtime_response, fetch_creator_live_control_response,
    fetch_creator_live_runtime_response, fetch_creator_live_socket_presence_by_id_raw,
    get_creator_live_socket_session, normalize_creator_live_profile,
    publish_authoritative_creator_live_state, publish_creator_live_state,
    publish_raw_creator_live_state, reconcile_creator_live_socket_session,
    routes as creator_live_routes,
};
use discovery::{
    fetch_categories, fetch_category_by_slug, fetch_creator_id_for_user, fetch_live_stream_by_id,
    fetch_live_streams, fetch_streamer_by_handle,
};
use live_ingest::{
    connect_live_ingest, end_broadcast, get_admin_live_ingest_session,
    get_creator_live_ingest_session_by_id, heartbeat_live_ingest, list_creator_live_ingest_events,
    reconcile_admin_live_ingest_session, reconcile_creator_live_ingest_session,
    routes as live_ingest_routes, terminate_creator_live_ingest,
};
use live_ingest_authority::{
    close_live_ingest_session, count_live_ingest_sessions_for_broadcast,
    enqueue_creator_broadcast_ended_notification, ensure_live_stream_row,
    fetch_active_live_ingest_session, fetch_active_live_ingest_session_unreconciled,
    fetch_admin_live_ingest_session_record, fetch_admin_live_ingest_sessions,
    fetch_creator_live_ingest_session_record, fetch_live_ingest_events_for_creator,
    fetch_live_ingest_events_for_session, fetch_live_ingest_session_by_id,
    fetch_live_ingest_session_by_id_global, fetch_live_ingest_session_by_id_global_unreconciled,
    fetch_live_ingest_session_by_id_unreconciled, fetch_recent_live_ingest_sessions,
    fetch_terminalizable_live_ingest_sessions_for_broadcast, mark_live_ingest_session_stale,
    mark_live_ingest_session_stale_in_db, reconcile_single_live_ingest_session,
    reconcile_stale_live_ingest_sessions, reset_creator_live_operational_metrics,
    transition_broadcast_to_live, validate_live_ingest_session, write_live_ingest_event,
};
use me::{
    get_my_membership_entitlement, get_my_purchase_entitlement,
    reconcile_my_membership_entitlement, reconcile_my_purchase_entitlement, revoke_session,
};
use media_access::{
    check_database, ensure_parent_dir, fetch_admin_playback_session_record,
    fetch_admin_playback_sessions, fetch_playback_session_by_id, media_api_url,
    media_path_for_relative, parse_ffprobe_ratio, path_allowed_for_paths,
    playback_path_allowed_for_asset, require_ingest_token, require_upload_token,
    rewrite_hls_manifest_media_uri_line, rewrite_hls_manifest_reference, sanitize_slug,
    sanitize_storage_key, serve_media_file, sha256_file, slugify, validate_playback_session,
    validate_playback_session_token_for_path, validate_upload_ingest_token,
};
use media_pipeline::{
    GeneratedHlsPackage, GeneratedHlsSubtitleTrack, GeneratedHlsVariant, HlsVariantPlan,
    MAX_MEDIA_PROCESSING_ATTEMPTS, ProbedAudioStream, ProbedMedia, StoredMediaPreviewTrack,
    ensure_media_asset_shell, fail_media_job_for_lease, fetch_admin_media_job_record,
    fetch_admin_media_jobs, fetch_media_asset_by_id_any_creator, fetch_media_asset_by_upload_id,
    fetch_media_asset_by_upload_job, fetch_media_asset_variants, fetch_media_assets,
    fetch_media_preview_track_rows, fetch_pending_media_jobs, fetch_upload_ingest_session,
    fetch_upload_ingest_sessions, fetch_upload_job_by_id, fetch_upload_job_by_id_global,
    fetch_upload_job_creator_id, fetch_upload_jobs, plan_hls_variants, probe_media,
    publish_due_scheduled_upload_releases, reconcile_single_media_job,
    reconcile_stale_media_processing_jobs, requeue_media_job_for_processing,
    schedule_media_processing, validate_generated_hls_package, validate_probed_media,
    verify_media_integrity, write_hls_master_manifest,
};
use moderation::{
    creator_enforcement_action_from_row, fetch_active_live_moderation_action,
    fetch_live_moderation_action_by_id, fetch_live_moderation_action_by_id_raw,
    fetch_live_stream_report_by_id, fetch_moderation_audit_log, live_moderation_action_from_row,
    write_moderation_audit_entry,
};
use notifications::{
    claim_notification_delivery_attempt, dispatch_notification_delivery,
    enqueue_notification_event, fetch_live_notification_recipient_user_ids,
    fetch_notification_deliveries, fetch_notification_delivery_by_id,
    fetch_notification_delivery_by_id_raw, fetch_notifications_rows, fetch_user_notifications,
    list_my_notifications, mark_my_notification_read, reconcile_single_notification_delivery,
};
use playback::{
    create_content_playback_session, create_live_playback_session, get_admin_playback_session,
    get_playback_manifest, get_playback_session, reconcile_admin_playback_session,
    refresh_playback_session, routes as playback_routes,
};
use playback_authority::{
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
use presence::{
    active_presence_cutoff, count_active_collaboration_socket_sessions,
    count_active_live_viewer_sessions, count_all_active_collaboration_socket_sessions,
    count_all_active_creator_live_socket_sessions, count_all_active_live_viewer_sessions,
    disconnect_collaboration_socket_session, disconnect_creator_live_socket_session,
    disconnect_live_viewer_session, effective_live_viewer_count, ensure_stream_exists,
    fetch_auth_sessions, fetch_chat_messages_for_viewer, fetch_continue_watching_entry,
    fetch_live_viewer_sample_users, next_chat_message_sequence,
    reconcile_single_creator_live_socket_session,
    reconcile_stale_creator_live_socket_sessions_for_read, reconcile_stale_presence_sessions,
    register_collaboration_socket_session, register_creator_live_socket_session,
    register_live_viewer_session, touch_collaboration_socket_session,
    touch_creator_live_socket_session, touch_live_viewer_session, upsert_watch_history_entry,
};
use public::{
    LimitQuery, PersistedChatMessage, bootstrap, check_binary_available, check_media_root_writable,
    check_runtime_dependencies_with_binaries, create_clip_request, create_live_moderation_action,
    get_live_moderation_action, get_live_viewer_preview, list_chat_messages,
    list_live_moderation_actions, list_live_streams, metrics, reconcile_live_moderation_action,
    remove_live_stream_moderator, resolve_live_stream_report, revoke_live_moderation_action,
};
use realtime::{
    CollaborationSocketCommand, execute_collaboration_socket_command,
    fetch_current_collaboration_socket_session_view, persist_chat_message,
    reconcile_collaboration_expiry_for_host_read, routes as realtime_routes,
};
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
use upload_jobs::{
    append_upload_chunk, complete_upload_ingest, create_upload_job, get_media_asset_for_upload_job,
    publish_upload_job, retry_upload_job_processing, routes as upload_jobs_routes,
    start_upload_ingest_session, update_upload_job,
};
use uploads::{routes as uploads_routes, takedown_upload, unpublish_upload, update_upload};
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
    Router::new()
        .merge(admin_ops_routes())
        .merge(public::routes())
        .merge(me::routes())
        .merge(creator_business::routes())
        .merge(creator_core::routes())
        .merge(creator_live_routes())
        .merge(collaboration_routes())
        .merge(live_ingest_routes())
        .merge(playback_routes())
        .merge(realtime_routes())
        .merge(uploads_routes())
        .merge(upload_jobs_routes())
        .route("/api/v1/media/*path", get(serve_media_file))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            request_context_middleware,
        ))
        .with_state(state.clone())
        .layer(build_cors_layer(state.as_ref()))
        .layer(TraceLayer::new_for_http())
}

fn build_cors_layer(state: &AppState) -> CorsLayer {
    CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::ACCEPT,
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ORIGIN,
            header::HeaderName::from_static("x-request-id"),
        ])
        .allow_origin(state.cors_allowed_origins.clone())
        .expose_headers([header::HeaderName::from_static("x-request-id")])
}

pub fn start_background_workers(state: SharedState) {
    tokio::spawn(async move {
        loop {
            state.background_worker.mark_tick().await;
            let mut errors = Vec::new();

            match fetch_pending_media_jobs(&state.pool).await {
                Ok(pending_jobs) => {
                    for (creator_id, job_id) in pending_jobs {
                        schedule_media_processing(state.clone(), creator_id, job_id).await;
                    }
                }
                Err(error) => {
                    errors.push(format!("pending media jobs fetch failed: {error}"));
                }
            }

            if let Err(error) = reconcile_stale_live_ingest_sessions(state.clone()).await {
                errors.push(format!("stale live ingest reconciliation failed: {error}"));
            }
            if let Err(error) = reconcile_expired_collaboration_invites(state.clone()).await {
                errors.push(format!(
                    "expired collaboration invite reconciliation failed: {error}"
                ));
            }
            if let Err(error) = reconcile_expired_collaboration_mirror_grants(state.clone()).await {
                errors.push(format!(
                    "expired collaboration mirror grant reconciliation failed: {error}"
                ));
            }
            if let Err(error) = reconcile_expired_user_entitlements(state.clone()).await {
                errors.push(format!(
                    "expired entitlement reconciliation failed: {error}"
                ));
            }
            if let Err(error) = reconcile_expired_live_moderation_actions(state.clone()).await {
                errors.push(format!(
                    "expired live moderation reconciliation failed: {error}"
                ));
            }
            if let Err(error) = reconcile_expired_creator_enforcement_actions(state.clone()).await {
                errors.push(format!(
                    "expired creator enforcement reconciliation failed: {error}"
                ));
            }
            if let Err(error) = reconcile_notification_deliveries(state.clone()).await {
                errors.push(format!(
                    "notification delivery reconciliation failed: {error}"
                ));
            }
            if let Err(error) = reconcile_stale_media_processing_jobs(state.clone()).await {
                errors.push(format!(
                    "stale media processing reconciliation failed: {error}"
                ));
            }
            if let Err(error) = reconcile_scheduled_upload_releases(state.clone()).await {
                errors.push(format!("scheduled release reconciliation failed: {error}"));
            }
            if let Err(error) = reconcile_stale_presence_sessions(state.clone()).await {
                errors.push(format!("stale presence reconciliation failed: {error}"));
            }
            if let Err(error) = reconcile_invalid_playback_sessions(state.clone()).await {
                errors.push(format!(
                    "invalid playback session reconciliation failed: {error}"
                ));
            }

            if errors.is_empty() {
                state.background_worker.mark_success().await;
            } else {
                state
                    .background_worker
                    .mark_failure(errors.join("; "))
                    .await;
            }

            sleep(Duration::from_secs(5)).await;
        }
    });
}

async fn request_context_middleware(
    State(state): State<SharedState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    state.metrics.begin_request();

    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    if let Ok(value) = HeaderValue::from_str(&request_id) {
        request.headers_mut().insert("x-request-id", value);
    }

    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-request-id", value);
    }
    state
        .metrics
        .finish_request(response.status().as_u16())
        .await;
    response
}

fn validate_request_origin(state: &SharedState, headers: &HeaderMap) -> AppResult<()> {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return Ok(());
    };
    if state.allows_origin(origin) {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

async fn enforce_rate_limit(
    state: &SharedState,
    key: &str,
    limit: usize,
    window: Duration,
) -> AppResult<()> {
    state
        .rate_limits
        .check(key, limit, window)
        .await
        .map_err(|_| {
            state.metrics.increment_rate_limit();
            AppError::RateLimited
        })
}

#[cfg(test)]
#[cfg(test)]
mod tests;
