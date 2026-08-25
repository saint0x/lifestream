use axum::{
    Json, Router,
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use thiserror::Error;
use tokio::time::{Duration, interval};

use crate::{
    AppState,
    obs::{
        bridge::ObsBridgeProfileInput,
        domain::{
            ActionConfirmationInput, AudienceTelemetryInput, AudioChannelPatch, BlockedTermInput,
            BroadcastInput, BroadcastPatch, CueInput, EmergencyDisconnectInput,
            EngagementAlertInput, EngagementPollInput, EngagementVoteInput, GuestDeviceCheckInput,
            GuestInviteInput, GuestIsolatedRecordingInput, GuestMediaTelemetryInput,
            GuestModerationInput, GuestPatchInput, GuestReturnFeedInput, GuestRoomRoutingInput,
            GuestRtpPacketInput, GuestWebrtcAnswerInput, GuestWebrtcIceInput,
            GuestWebrtcOfferInput, HotkeyPatch, InstanceInput, InstancePatch, LiveOpsOverrideInput,
            ModerationQueueInput, ModerationResolveInput, ModeratorInput, PinnedMessageInput,
            PreflightInput, RaidRedirectInput, RecordingInput, ReplayInput, RuntimeErrorInput,
            RuntimeTelemetryInput, SceneGroupInput, SceneGroupPatch, SceneInput, ScenePatch,
            SceneReorderInput, SceneTemplateInput, ScheduleSlotInput, ScheduleSlotPatch,
            SourceFilterInput, SourceFilterPatch, SourceInput, SourcePatch, SponsorCampaignInput,
            SponsorInventoryInput, SponsorProofInput, SponsorReviewInput, TransitionPreviewInput,
        },
        export::ObsExportInput,
        import::ObsImportInput,
        runtime::stream_snapshot,
        service::ObsServiceError,
        store::ObsStoreError,
    },
};

#[derive(Debug, Error)]
pub enum ObsApiError {
    #[error(transparent)]
    Service(#[from] ObsServiceError),
}

impl IntoResponse for ObsApiError {
    fn into_response(self) -> Response {
        let status = match self {
            ObsApiError::Service(ObsServiceError::Store(ObsStoreError::NotFound)) => {
                StatusCode::NOT_FOUND
            }
            ObsApiError::Service(ObsServiceError::Store(ObsStoreError::SafetyBlocked(_))) => {
                StatusCode::CONFLICT
            }
            ObsApiError::Service(ObsServiceError::Store(ObsStoreError::Invalid(_))) => {
                StatusCode::BAD_REQUEST
            }
            ObsApiError::Service(ObsServiceError::Invalid { .. }) => StatusCode::BAD_REQUEST,
            ObsApiError::Service(ObsServiceError::Import(_)) => StatusCode::BAD_REQUEST,
            ObsApiError::Service(ObsServiceError::Export(_)) => StatusCode::BAD_REQUEST,
            ObsApiError::Service(ObsServiceError::ReplayMedia(_)) => StatusCode::BAD_GATEWAY,
            ObsApiError::Service(ObsServiceError::Bridge(_)) => StatusCode::BAD_GATEWAY,
            ObsApiError::Service(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}

type ObsResult<T> = Result<Json<T>, ObsApiError>;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/obs/me/dashboard", get(dashboard))
        .route("/api/v1/obs/me/scene-collections", get(collections))
        .route(
            "/api/v1/obs/me/scene-collections/:collection_id",
            get(collection),
        )
        .route("/api/v1/obs/me/scenes", post(create_scene))
        .route("/api/v1/obs/me/scene-templates", get(scene_templates))
        .route(
            "/api/v1/obs/me/scene-templates/:template_id/create",
            post(create_scene_from_template),
        )
        .route(
            "/api/v1/obs/me/scenes/:scene_id",
            patch(patch_scene).delete(delete_scene),
        )
        .route(
            "/api/v1/obs/me/scene-collections/:collection_id/scenes/reorder",
            post(reorder_scenes),
        )
        .route(
            "/api/v1/obs/me/scenes/:scene_id/duplicate",
            post(duplicate_scene),
        )
        .route(
            "/api/v1/obs/me/scenes/:scene_id/send-to-program",
            post(send_to_program),
        )
        .route(
            "/api/v1/obs/me/scenes/:scene_id/transition-preview",
            post(transition_preview),
        )
        .route("/api/v1/obs/me/sources", post(create_source))
        .route("/api/v1/obs/me/sources/:source_id", patch(patch_source))
        .route(
            "/api/v1/obs/me/sources/:source_id/filters",
            post(create_source_filter),
        )
        .route(
            "/api/v1/obs/me/source-filters/:filter_id",
            patch(patch_source_filter),
        )
        .route(
            "/api/v1/obs/me/source-filters/:filter_id/disable",
            post(disable_source_filter),
        )
        .route(
            "/api/v1/obs/me/audio/channels/:channel_id",
            patch(patch_audio_channel),
        )
        .route(
            "/api/v1/obs/me/scenes/:scene_id/source-instances",
            post(create_instance),
        )
        .route(
            "/api/v1/obs/me/scenes/:scene_id/groups",
            post(create_scene_group),
        )
        .route(
            "/api/v1/obs/me/scene-groups/:source_id",
            patch(patch_scene_group),
        )
        .route(
            "/api/v1/obs/me/source-instances/:instance_id",
            patch(patch_instance),
        )
        .route("/api/v1/obs/me/preflight", post(preflight))
        .route("/api/v1/obs/me/broadcasts", post(create_broadcast))
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id",
            patch(patch_broadcast),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/start",
            post(start_broadcast),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/end",
            post(end_broadcast),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/recording/start",
            post(start_recording),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/recording/pause",
            post(pause_recording),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/recording/resume",
            post(resume_recording),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/recording/stop",
            post(stop_recording),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/recording/discard",
            post(discard_recording),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/replay-buffer/save",
            post(save_replay),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/cues",
            post(create_cue),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/moderation/moderators",
            post(add_moderator),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/moderation/blocked-terms",
            post(add_blocked_term),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/moderation/queue",
            post(enqueue_moderation),
        )
        .route(
            "/api/v1/obs/me/moderation/queue/:item_id/resolve",
            post(resolve_moderation),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/moderation/pins",
            post(pin_message),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/audience/telemetry",
            post(audience_telemetry),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/audience/raid-redirects",
            post(schedule_raid_redirect),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/audience/raids/inbound",
            post(record_inbound_raid),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/schedule",
            post(create_schedule_slot),
        )
        .route(
            "/api/v1/obs/me/schedule/:slot_id",
            patch(patch_schedule_slot),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/engagement/polls",
            post(create_engagement_poll),
        )
        .route(
            "/api/v1/obs/me/engagement/polls/:poll_id/vote",
            post(vote_engagement_poll),
        )
        .route(
            "/api/v1/obs/me/engagement/polls/:poll_id/close",
            post(close_engagement_poll),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/engagement/alerts",
            post(create_engagement_alert),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/sponsor/campaigns",
            post(attach_sponsor_campaign),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/sponsor/inventory",
            post(create_sponsor_inventory),
        )
        .route(
            "/api/v1/obs/me/sponsor/inventory/:inventory_id/proof",
            post(capture_sponsor_proof),
        )
        .route(
            "/api/v1/obs/me/sponsor/proofs/:proof_id/review",
            post(review_sponsor_proof),
        )
        .route(
            "/api/v1/obs/me/moderation/pins/:message_id/unpin",
            post(unpin_message),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/guests/invite",
            post(invite_guest),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/guests/routing",
            post(configure_guest_room_routing),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/guests/relays/reconcile",
            post(reconcile_guest_media_relays),
        )
        .route(
            "/api/v1/obs/me/guests/relays/:relay_id/rtp",
            post(ingest_guest_relay_rtp_packet),
        )
        .route(
            "/api/v1/obs/me/guests/:participant_id/promote/:scene_id",
            post(promote_guest),
        )
        .route("/api/v1/obs/me/guests/:participant_id", patch(patch_guest))
        .route(
            "/api/v1/obs/me/guests/:participant_id/device-check",
            post(run_guest_device_check),
        )
        .route(
            "/api/v1/obs/me/guests/:participant_id/moderation",
            post(moderate_guest),
        )
        .route(
            "/api/v1/obs/me/guests/:participant_id/media-telemetry",
            post(report_guest_media_telemetry),
        )
        .route(
            "/api/v1/obs/me/guests/:participant_id/webrtc/offer",
            post(create_guest_webrtc_offer),
        )
        .route(
            "/api/v1/obs/me/guests/webrtc/:session_id/answer",
            post(apply_guest_webrtc_answer),
        )
        .route(
            "/api/v1/obs/me/guests/webrtc/:session_id/ice",
            post(add_guest_webrtc_ice_candidate),
        )
        .route(
            "/api/v1/obs/me/guests/:participant_id/return-feed",
            post(negotiate_guest_return_feed),
        )
        .route(
            "/api/v1/obs/me/guests/:participant_id/isolated-recording/start",
            post(start_guest_isolated_recording),
        )
        .route(
            "/api/v1/obs/me/guests/:participant_id/isolated-recording/stop",
            post(stop_guest_isolated_recording),
        )
        .route(
            "/api/v1/obs/me/guests/:participant_id/remove",
            post(remove_guest),
        )
        .route("/api/v1/obs/me/hotkeys/:hotkey_id", patch(patch_hotkey))
        .route(
            "/api/v1/obs/me/hotkeys/:hotkey_id/trigger",
            post(trigger_hotkey),
        )
        .route(
            "/api/v1/obs/me/live-cues/:cue_id/trigger",
            post(trigger_cue),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/runtime",
            get(runtime),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/runtime/stream",
            get(runtime_stream),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/runtime/errors",
            post(runtime_error),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/runtime/telemetry",
            post(runtime_telemetry),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/live-ops/override",
            post(live_ops_override),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/health",
            get(health),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/post-show",
            get(post_show),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/send-to-editor",
            post(send_to_editor),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/emergency-disconnect",
            post(emergency_disconnect),
        )
        .route(
            "/api/v1/obs/me/broadcasts/:broadcast_id/support-bundle",
            post(support_bundle),
        )
        .route(
            "/api/v1/obs/me/bridge/connections",
            get(bridge_connections).post(create_bridge_connection),
        )
        .route(
            "/api/v1/obs/me/bridge/connections/:connection_id",
            get(bridge_connection),
        )
        .route(
            "/api/v1/obs/me/bridge/connections/:connection_id/sync",
            post(sync_bridge_connection),
        )
        .route(
            "/api/v1/obs/me/bridge/connections/:connection_id/events",
            get(bridge_events),
        )
        .route(
            "/api/v1/obs/me/bridge/connections/:connection_id/program-scene",
            post(bridge_set_program_scene),
        )
        .route(
            "/api/v1/obs/me/bridge/connections/:connection_id/stream/start",
            post(bridge_start_stream),
        )
        .route(
            "/api/v1/obs/me/bridge/connections/:connection_id/stream/stop",
            post(bridge_stop_stream),
        )
        .route(
            "/api/v1/obs/me/bridge/connections/:connection_id/recording/start",
            post(bridge_start_recording),
        )
        .route(
            "/api/v1/obs/me/bridge/connections/:connection_id/recording/stop",
            post(bridge_stop_recording),
        )
        .route(
            "/api/v1/obs/me/bridge/connections/:connection_id/replay-buffer/save",
            post(bridge_save_replay_buffer),
        )
        .route(
            "/api/v1/obs/me/imports/scene-collections",
            get(import_reports).post(import_obs_scene_collection),
        )
        .route(
            "/api/v1/obs/me/imports/scene-collections/:report_id",
            get(import_report),
        )
        .route(
            "/api/v1/obs/me/exports/scene-collections",
            get(export_jobs).post(export_obs_scene_collection),
        )
        .route(
            "/api/v1/obs/me/exports/scene-collections/:job_id",
            get(export_job),
        )
}

async fn dashboard(State(state): State<AppState>) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.dashboard().await?))
}

async fn collections(State(state): State<AppState>) -> ObsResult<Vec<serde_json::Value>> {
    Ok(Json(state.obs.collections().await?))
}

async fn collection(
    State(state): State<AppState>,
    Path(collection_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.collection_bundle(&collection_id).await?))
}

async fn create_scene(
    State(state): State<AppState>,
    Json(input): Json<SceneInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.create_scene(input).await?))
}

async fn scene_templates(State(state): State<AppState>) -> ObsResult<Vec<serde_json::Value>> {
    Ok(Json(state.obs.scene_templates().await?))
}

async fn create_scene_from_template(
    State(state): State<AppState>,
    Path(template_id): Path<String>,
    Json(input): Json<SceneTemplateInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .create_scene_from_template(&template_id, input)
            .await?,
    ))
}

async fn patch_scene(
    State(state): State<AppState>,
    Path(scene_id): Path<String>,
    Json(input): Json<ScenePatch>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.patch_scene(&scene_id, input).await?))
}

async fn delete_scene(
    State(state): State<AppState>,
    Path(scene_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.delete_scene(&scene_id).await?))
}

async fn reorder_scenes(
    State(state): State<AppState>,
    Path(collection_id): Path<String>,
    Json(input): Json<SceneReorderInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.reorder_scenes(&collection_id, input).await?))
}

async fn duplicate_scene(
    State(state): State<AppState>,
    Path(scene_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.duplicate_scene(&scene_id).await?))
}

async fn send_to_program(
    State(state): State<AppState>,
    Path(scene_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.send_to_program(&scene_id).await?))
}

async fn transition_preview(
    State(state): State<AppState>,
    Path(scene_id): Path<String>,
    Json(input): Json<TransitionPreviewInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.transition_preview(&scene_id, input).await?))
}

async fn create_source(
    State(state): State<AppState>,
    Json(input): Json<SourceInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.create_source(input).await?))
}

async fn patch_source(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    Json(input): Json<SourcePatch>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.patch_source(&source_id, input).await?))
}

async fn create_source_filter(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    Json(input): Json<SourceFilterInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state.obs.create_source_filter(&source_id, input).await?,
    ))
}

async fn patch_source_filter(
    State(state): State<AppState>,
    Path(filter_id): Path<String>,
    Json(input): Json<SourceFilterPatch>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state.obs.patch_source_filter(&filter_id, input).await?,
    ))
}

async fn disable_source_filter(
    State(state): State<AppState>,
    Path(filter_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.disable_source_filter(&filter_id).await?))
}

async fn patch_audio_channel(
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
    Json(input): Json<AudioChannelPatch>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state.obs.patch_audio_channel(&channel_id, input).await?,
    ))
}

async fn create_instance(
    State(state): State<AppState>,
    Path(scene_id): Path<String>,
    Json(input): Json<InstanceInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.create_instance(&scene_id, input).await?))
}

async fn create_scene_group(
    State(state): State<AppState>,
    Path(scene_id): Path<String>,
    Json(input): Json<SceneGroupInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.create_scene_group(&scene_id, input).await?))
}

async fn patch_scene_group(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    Json(input): Json<SceneGroupPatch>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.patch_scene_group(&source_id, input).await?))
}

async fn patch_instance(
    State(state): State<AppState>,
    Path(instance_id): Path<String>,
    Json(input): Json<InstancePatch>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.patch_instance(&instance_id, input).await?))
}

async fn preflight(
    State(state): State<AppState>,
    Json(input): Json<PreflightInput>,
) -> ObsResult<super::domain::PreflightResult> {
    Ok(Json(state.obs.save_preflight(input).await?))
}

async fn create_broadcast(
    State(state): State<AppState>,
    Json(input): Json<BroadcastInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.create_broadcast(input).await?))
}

async fn patch_broadcast(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<BroadcastPatch>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.patch_broadcast(&broadcast_id, input).await?))
}

async fn start_broadcast(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.start_broadcast(&broadcast_id).await?))
}

async fn end_broadcast(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    input: Option<Json<ActionConfirmationInput>>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .end_broadcast(
                &broadcast_id,
                input.map(|Json(input)| input).unwrap_or_default(),
            )
            .await?,
    ))
}

async fn start_recording(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<RecordingInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.start_recording(&broadcast_id, input).await?))
}

async fn stop_recording(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    input: Option<Json<ActionConfirmationInput>>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .stop_recording(
                &broadcast_id,
                input.map(|Json(input)| input).unwrap_or_default(),
            )
            .await?,
    ))
}

async fn pause_recording(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.pause_recording(&broadcast_id).await?))
}

async fn resume_recording(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.resume_recording(&broadcast_id).await?))
}

async fn discard_recording(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    input: Option<Json<ActionConfirmationInput>>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .discard_recording(
                &broadcast_id,
                input.map(|Json(input)| input).unwrap_or_default(),
            )
            .await?,
    ))
}

async fn save_replay(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<ReplayInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.save_replay(&broadcast_id, input).await?))
}

async fn create_cue(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<CueInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.create_cue(&broadcast_id, input).await?))
}

async fn add_moderator(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<ModeratorInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.add_moderator(&broadcast_id, input).await?))
}

async fn add_blocked_term(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<BlockedTermInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state.obs.add_blocked_term(&broadcast_id, input).await?,
    ))
}

async fn enqueue_moderation(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<ModerationQueueInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state.obs.enqueue_moderation(&broadcast_id, input).await?,
    ))
}

async fn resolve_moderation(
    State(state): State<AppState>,
    Path(item_id): Path<String>,
    Json(input): Json<ModerationResolveInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.resolve_moderation(&item_id, input).await?))
}

async fn pin_message(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<PinnedMessageInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.pin_message(&broadcast_id, input).await?))
}

async fn unpin_message(
    State(state): State<AppState>,
    Path(message_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.unpin_message(&message_id).await?))
}

async fn audience_telemetry(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<AudienceTelemetryInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .ingest_audience_telemetry(&broadcast_id, input)
            .await?,
    ))
}

async fn schedule_raid_redirect(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<RaidRedirectInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .schedule_raid_redirect(&broadcast_id, input)
            .await?,
    ))
}

async fn record_inbound_raid(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<RaidRedirectInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state.obs.record_inbound_raid(&broadcast_id, input).await?,
    ))
}

async fn create_schedule_slot(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<ScheduleSlotInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state.obs.create_schedule_slot(&broadcast_id, input).await?,
    ))
}

async fn patch_schedule_slot(
    State(state): State<AppState>,
    Path(slot_id): Path<String>,
    Json(input): Json<ScheduleSlotPatch>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.patch_schedule_slot(&slot_id, input).await?))
}

async fn create_engagement_poll(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<EngagementPollInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .create_engagement_poll(&broadcast_id, input)
            .await?,
    ))
}

async fn vote_engagement_poll(
    State(state): State<AppState>,
    Path(poll_id): Path<String>,
    Json(input): Json<EngagementVoteInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.vote_engagement_poll(&poll_id, input).await?))
}

async fn close_engagement_poll(
    State(state): State<AppState>,
    Path(poll_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.close_engagement_poll(&poll_id).await?))
}

async fn create_engagement_alert(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<EngagementAlertInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .create_engagement_alert(&broadcast_id, input)
            .await?,
    ))
}

async fn attach_sponsor_campaign(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<SponsorCampaignInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .attach_sponsor_campaign(&broadcast_id, input)
            .await?,
    ))
}

async fn create_sponsor_inventory(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<SponsorInventoryInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .create_sponsor_inventory(&broadcast_id, input)
            .await?,
    ))
}

async fn capture_sponsor_proof(
    State(state): State<AppState>,
    Path(inventory_id): Path<String>,
    Json(input): Json<SponsorProofInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .capture_sponsor_proof(&inventory_id, input)
            .await?,
    ))
}

async fn review_sponsor_proof(
    State(state): State<AppState>,
    Path(proof_id): Path<String>,
    Json(input): Json<SponsorReviewInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state.obs.review_sponsor_proof(&proof_id, input).await?,
    ))
}

async fn trigger_cue(
    State(state): State<AppState>,
    Path(cue_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.trigger_cue(&cue_id).await?))
}

async fn invite_guest(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<GuestInviteInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.invite_guest(&broadcast_id, input).await?))
}

async fn configure_guest_room_routing(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<GuestRoomRoutingInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .configure_guest_room_routing(&broadcast_id, input)
            .await?,
    ))
}

async fn reconcile_guest_media_relays(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .reconcile_guest_media_relays(&broadcast_id)
            .await?,
    ))
}

async fn ingest_guest_relay_rtp_packet(
    State(state): State<AppState>,
    Path(relay_id): Path<String>,
    Json(input): Json<GuestRtpPacketInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .ingest_guest_relay_rtp_packet(&relay_id, input)
            .await?,
    ))
}

async fn promote_guest(
    State(state): State<AppState>,
    Path((participant_id, scene_id)): Path<(String, String)>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state.obs.promote_guest(&participant_id, &scene_id).await?,
    ))
}

async fn patch_guest(
    State(state): State<AppState>,
    Path(participant_id): Path<String>,
    Json(input): Json<GuestPatchInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.patch_guest(&participant_id, input).await?))
}

async fn run_guest_device_check(
    State(state): State<AppState>,
    Path(participant_id): Path<String>,
    Json(input): Json<GuestDeviceCheckInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .run_guest_device_check(&participant_id, input)
            .await?,
    ))
}

async fn moderate_guest(
    State(state): State<AppState>,
    Path(participant_id): Path<String>,
    Json(input): Json<GuestModerationInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state.obs.moderate_guest(&participant_id, input).await?,
    ))
}

async fn report_guest_media_telemetry(
    State(state): State<AppState>,
    Path(participant_id): Path<String>,
    Json(input): Json<GuestMediaTelemetryInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .report_guest_media_telemetry(&participant_id, input)
            .await?,
    ))
}

async fn create_guest_webrtc_offer(
    State(state): State<AppState>,
    Path(participant_id): Path<String>,
    Json(input): Json<GuestWebrtcOfferInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .create_guest_webrtc_offer(&participant_id, input)
            .await?,
    ))
}

async fn apply_guest_webrtc_answer(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(input): Json<GuestWebrtcAnswerInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .apply_guest_webrtc_answer(&session_id, input)
            .await?,
    ))
}

async fn add_guest_webrtc_ice_candidate(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(input): Json<GuestWebrtcIceInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .add_guest_webrtc_ice_candidate(&session_id, input)
            .await?,
    ))
}

async fn negotiate_guest_return_feed(
    State(state): State<AppState>,
    Path(participant_id): Path<String>,
    Json(input): Json<GuestReturnFeedInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .negotiate_guest_return_feed(&participant_id, input)
            .await?,
    ))
}

async fn start_guest_isolated_recording(
    State(state): State<AppState>,
    Path(participant_id): Path<String>,
    Json(input): Json<GuestIsolatedRecordingInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .start_guest_isolated_recording(&participant_id, input)
            .await?,
    ))
}

async fn stop_guest_isolated_recording(
    State(state): State<AppState>,
    Path(participant_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .stop_guest_isolated_recording(&participant_id)
            .await?,
    ))
}

async fn remove_guest(
    State(state): State<AppState>,
    Path(participant_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.remove_guest(&participant_id).await?))
}

async fn patch_hotkey(
    State(state): State<AppState>,
    Path(hotkey_id): Path<String>,
    Json(input): Json<HotkeyPatch>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.patch_hotkey(&hotkey_id, input).await?))
}

async fn trigger_hotkey(
    State(state): State<AppState>,
    Path(hotkey_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.trigger_hotkey(&hotkey_id).await?))
}

async fn runtime(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.runtime(&broadcast_id).await?))
}

async fn runtime_error(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<RuntimeErrorInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state.obs.ingest_runtime_error(&broadcast_id, input).await?,
    ))
}

async fn runtime_telemetry(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<RuntimeTelemetryInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .ingest_runtime_telemetry(&broadcast_id, input)
            .await?,
    ))
}

async fn live_ops_override(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<LiveOpsOverrideInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state.obs.live_ops_override(&broadcast_id, input).await?,
    ))
}

async fn runtime_stream(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
) -> Response {
    ws.on_upgrade(move |socket| stream_runtime(socket, state, broadcast_id))
}

async fn stream_runtime(socket: WebSocket, state: AppState, broadcast_id: String) {
    let (mut sender, mut receiver) = socket.split();
    let mut ticks = interval(Duration::from_millis(1000));
    let mut sequence = 0_u64;
    loop {
        sequence += 1;
        if let Some(payload) = runtime_stream_payload(&state, &broadcast_id, sequence).await
            && sender
                .send(Message::Text(payload.to_string()))
                .await
                .is_err()
        {
            break;
        }
        tokio::select! {
            message = receiver.next() => {
                match message {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(payload))) => {
                        if sender.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            _ = ticks.tick() => {}
        }
    }
}

async fn runtime_stream_payload(
    state: &AppState,
    broadcast_id: &str,
    sequence: u64,
) -> Option<Value> {
    let dashboard = state.obs.dashboard().await.ok()?;
    if dashboard["broadcast"]["id"].as_str()? != broadcast_id {
        return None;
    }
    Some(stream_snapshot(sequence, dashboard))
}

async fn health(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.health(&broadcast_id).await?))
}

async fn post_show(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.post_show(&broadcast_id).await?))
}

async fn send_to_editor(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.send_to_editor(&broadcast_id).await?))
}

async fn emergency_disconnect(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
    Json(input): Json<EmergencyDisconnectInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state.obs.emergency_disconnect(&broadcast_id, input).await?,
    ))
}

async fn support_bundle(
    State(state): State<AppState>,
    Path(broadcast_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.create_support_bundle(&broadcast_id).await?))
}

async fn create_bridge_connection(
    State(state): State<AppState>,
    Json(input): Json<ObsBridgeProfileInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.create_bridge_connection(input).await?))
}

async fn bridge_connections(State(state): State<AppState>) -> ObsResult<Vec<serde_json::Value>> {
    Ok(Json(state.obs.bridge_connections().await?))
}

async fn bridge_connection(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.bridge_connection(&connection_id).await?))
}

async fn sync_bridge_connection(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state.obs.sync_bridge_connection(&connection_id).await?,
    ))
}

async fn bridge_events(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
) -> ObsResult<Vec<serde_json::Value>> {
    Ok(Json(state.obs.bridge_events(&connection_id).await?))
}

async fn bridge_set_program_scene(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
    Json(input): Json<BridgeSceneInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state
            .obs
            .bridge_set_program_scene(&connection_id, input.scene_name)
            .await?,
    ))
}

async fn bridge_start_stream(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.bridge_start_stream(&connection_id).await?))
}

async fn bridge_stop_stream(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.bridge_stop_stream(&connection_id).await?))
}

async fn bridge_start_recording(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state.obs.bridge_start_recording(&connection_id).await?,
    ))
}

async fn bridge_stop_recording(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.bridge_stop_recording(&connection_id).await?))
}

async fn bridge_save_replay_buffer(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(
        state.obs.bridge_save_replay_buffer(&connection_id).await?,
    ))
}

#[derive(serde::Deserialize)]
struct BridgeSceneInput {
    scene_name: String,
}

async fn import_obs_scene_collection(
    State(state): State<AppState>,
    Json(input): Json<ObsImportInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.import_obs_scene_collection(input).await?))
}

async fn import_reports(State(state): State<AppState>) -> ObsResult<Vec<serde_json::Value>> {
    Ok(Json(state.obs.import_reports().await?))
}

async fn import_report(
    State(state): State<AppState>,
    Path(report_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.import_report(&report_id).await?))
}

async fn export_obs_scene_collection(
    State(state): State<AppState>,
    Json(input): Json<ObsExportInput>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.export_obs_scene_collection(input).await?))
}

async fn export_jobs(State(state): State<AppState>) -> ObsResult<Vec<serde_json::Value>> {
    Ok(Json(state.obs.export_jobs().await?))
}

async fn export_job(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> ObsResult<serde_json::Value> {
    Ok(Json(state.obs.export_job(&job_id).await?))
}
