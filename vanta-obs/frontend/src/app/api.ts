import { getApiWebSocketUrl, requestJson } from "@/lib/api";
import type {
  ObsBridgeConnection,
  ObsDashboard,
  ObsExportJob,
  ObsImportReport,
  NativeHelperPackage,
  NativeHelperSession,
  MediaCapabilities,
  MediaCaptureArtifact,
  MediaCaptureFrame,
  MediaCaptureInventory,
  MediaCaptureSession,
  MediaEncodeJob,
  MediaPackage,
  MediaSourceArtifact,
  ObsRow,
} from "@/types";
import type { GuestWebrtcOfferPayload } from "@/engine/guestWebrtc";
import type { ReplayDraftOptions } from "@/engine/replay";

function post<T>(path: string, body?: unknown): Promise<T> {
  return requestJson<T>(path, {
    method: "POST",
    body: body ? JSON.stringify(body) : undefined,
  });
}

export function getDashboard(): Promise<ObsDashboard> {
  return requestJson<ObsDashboard>("/api/v1/obs/me/dashboard");
}

export function patchBroadcast(
  broadcastId: string,
  patch: Record<string, unknown>,
): Promise<ObsDashboard> {
  return requestJson<ObsDashboard>(`/api/v1/obs/me/broadcasts/${broadcastId}`, {
    method: "PATCH",
    body: JSON.stringify(patch),
  });
}

export function startBroadcast(broadcastId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/start`);
}

const OPERATOR_GUARD = {
  operator_id: "creator_vanta_originals",
  operator_role: "creator_owner",
  acknowledged_risks: ["campaign_recording"],
};

export function endBroadcast(broadcastId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/end`, {
    ...OPERATOR_GUARD,
    confirmation_text: "END STREAM",
  });
}

export function runtimeStreamUrl(broadcastId: string): string {
  return getApiWebSocketUrl(`/api/v1/obs/me/broadcasts/${broadcastId}/runtime/stream`);
}

export function reportRuntimeError(
  broadcastId: string,
  message: string,
  severity = "error",
): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/runtime/errors`, {
    error_code: "browser_runtime_report",
    severity,
    message,
    source: "studio_browser",
    operator_id: "creator_vanta_originals",
    details_json: { surface: "studio" },
  });
}

export function reportRuntimeTelemetry(
  broadcastId: string,
  sample: {
    readonly bitrateKbps: number;
    readonly uploadMbps: number;
    readonly ingestLatencyMs: number;
    readonly droppedFrames: number;
    readonly cpuPercent: number;
    readonly reconnectCount?: number;
  },
): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/runtime/telemetry`, {
    sample_kind: "browser_runtime_sample",
    bitrate_kbps: sample.bitrateKbps,
    upload_mbps: sample.uploadMbps,
    ingest_latency_ms: sample.ingestLatencyMs,
    dropped_frames: sample.droppedFrames,
    cpu_percent: sample.cpuPercent,
    reconnect_count: sample.reconnectCount,
    details_json: { surface: "studio" },
  });
}

export function reportAudienceTelemetry(
  broadcastId: string,
  sample: {
    readonly viewerCount: number;
    readonly chatMessagesPerMinute: number;
    readonly tipsCents?: number;
    readonly subscriptions?: number;
    readonly revenueCents?: number;
    readonly discoverySource?: string;
    readonly discoveryScore?: number;
  },
): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/audience/telemetry`, {
    viewer_count: sample.viewerCount,
    chat_messages_per_minute: sample.chatMessagesPerMinute,
    tips_cents: sample.tipsCents,
    subscriptions: sample.subscriptions,
    revenue_cents: sample.revenueCents,
    discovery_source: sample.discoverySource,
    discovery_score: sample.discoveryScore,
    details_json: { surface: "studio", source: "operator_sample" },
  });
}

export function scheduleRaidRedirect(broadcastId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/audience/raid-redirects`, {
    target_channel_id: "creator_afterparty",
    target_channel_name: "Afterparty Studio",
    viewer_count: 1284,
    execute_after_seconds: 30,
    redirect_url: "https://streamvanta.tv/creator_afterparty/live",
    safety_json: { source: "studio", moderation_handoff: true },
  });
}

export function recordInboundRaid(broadcastId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/audience/raids/inbound`, {
    target_channel_id: "creator_luna",
    target_channel_name: "Luna Live",
    viewer_count: 312,
    redirect_url: "https://streamvanta.tv/creator_luna/live",
    safety_json: { source: "studio", moderation_handoff: true },
  });
}

export function createScheduleSlot(broadcastId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/schedule`, {
    title: "Late-night sponsor Q&A",
    starts_at: "2026-08-27T22:00:00-04:00",
    timezone: "America/New_York",
    duration_minutes: 45,
    reminder_json: { notify_followers: true, reminder_minutes: [30, 5] },
  });
}

export function patchScheduleSlot(slotId: string, patch: Record<string, unknown>): Promise<ObsDashboard> {
  return requestJson<ObsDashboard>(`/api/v1/obs/me/schedule/${slotId}`, {
    method: "PATCH",
    body: JSON.stringify(patch),
  });
}

export function createEngagementPoll(
  broadcastId: string,
  pollKind: "poll" | "prediction",
): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/engagement/polls`, {
    poll_kind: pollKind,
    question: pollKind === "prediction"
      ? "Will the demo finish under five minutes?"
      : "Which segment should we replay?",
    options: pollKind === "prediction" ? ["Yes", "No"] : ["Product demo", "Sponsor read"],
    duration_seconds: pollKind === "prediction" ? 120 : 300,
  });
}

export function voteEngagementPoll(pollId: string, optionId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/engagement/polls/${pollId}/vote`, {
    option_id: optionId,
    voter_id: `viewer_${Date.now()}`,
  });
}

export function closeEngagementPoll(pollId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/engagement/polls/${pollId}/close`);
}

export function createEngagementAlert(broadcastId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/engagement/alerts`, {
    alert_kind: "tip",
    title: "Tip received",
    message: "Ari tipped during the sponsor read.",
    severity: "success",
    source_user: "Ari",
    amount_cents: 2500,
    metadata_json: { surface: "studio" },
  });
}

export function attachSponsorCampaign(broadcastId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/sponsor/campaigns`, {
    campaign_id: "campaign_live_nova",
    advertiser: "Nova",
    title: "Nova Launch Run",
    flight_json: { source: "vanta_backend", spots: 3 },
    claims_json: {
      required: ["Use code VANTA20"],
      prohibited: ["guaranteed results"],
    },
    performance_json: { handoff: "ad_ops_ready", deal_id: "deal_live_nova" },
  });
}

export function createSponsorInventory(
  broadcastId: string,
  creativeKind: "sponsor_card" | "lower_third" | "branded_bumper" | "pinned_cta" | "qr_code" | "promo_code",
): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/sponsor/inventory`, {
    campaign_id: "campaign_live_nova",
    creative_kind: creativeKind,
    label: sponsorLabel(creativeKind),
    scheduled_at_seconds: creativeKind === "branded_bumper" ? 300 : 45,
    required_duration_seconds: creativeKind === "branded_bumper" ? 10 : 20,
    scene_id: "scene_sponsor_read",
    required_claims: ["Use code VANTA20"],
    prohibited_claims: ["guaranteed results"],
    settings_json: {
      promo_code: "VANTA20",
      cta_text: "Open Nova offer",
      target_url: "https://streamvanta.tv/r/nova",
      tracking: "streamvanta.tv/r/nova",
      headline: sponsorLabel(creativeKind),
    },
  });
}

export function captureSponsorProof(inventoryId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/sponsor/inventory/${inventoryId}/proof`, {
    proof_kind: "media_segment",
    media_time_seconds: 1,
  });
}

export function reviewSponsorProof(proofId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/sponsor/proofs/${proofId}/review`, {
    status: "approved",
    reviewer_id: "ad_ops_live",
    notes: "Approved from studio proof queue.",
  });
}

export function emergencyDisconnect(broadcastId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/emergency-disconnect`, {
    reason: "Operator emergency disconnect",
    operator_id: "creator_vanta_originals",
    operator_role: "creator_owner",
  });
}

export function liveOpsOverride(
  broadcastId: string,
  action: "force_end" | "safe_mode" | "clear_incidents",
  reason: string,
): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/live-ops/override`, {
    action,
    reason,
    operator_id: "live_ops",
    operator_role: "live_ops",
    confirmation_text: action === "force_end" ? "FORCE END" : undefined,
    acknowledged_risks: ["campaign_recording"],
  });
}

export function createSupportBundle(broadcastId: string): Promise<ObsRow> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/support-bundle`);
}

export function startRecording(broadcastId: string): Promise<ObsRow> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/recording/start`, {
    recording_mode: "program_plus_isolated_audio",
    ...OPERATOR_GUARD,
  });
}

export function stopRecording(broadcastId: string): Promise<readonly ObsRow[]> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/recording/stop`, {
    ...OPERATOR_GUARD,
    confirmation_text: "STOP RECORDING",
  });
}

export function pauseRecording(broadcastId: string): Promise<ObsRow> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/recording/pause`);
}

export function resumeRecording(broadcastId: string): Promise<ObsRow> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/recording/resume`);
}

export function discardRecording(broadcastId: string): Promise<ObsRow> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/recording/discard`, {
    ...OPERATOR_GUARD,
    confirmation_text: "DISCARD RECORDING",
  });
}

export function saveReplay(broadcastId: string, options: ReplayDraftOptions): Promise<ObsRow> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/replay-buffer/save`, {
    duration_seconds: options.durationSeconds,
    label: "Live marker",
    sponsor_proof: options.sponsorProof,
  });
}

export function duplicateScene(sceneId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/scenes/${sceneId}/duplicate`);
}

export function deleteScene(sceneId: string): Promise<ObsDashboard> {
  return requestJson<ObsDashboard>(`/api/v1/obs/me/scenes/${sceneId}`, {
    method: "DELETE",
  });
}

export function reorderScenes(collectionId: string, sceneIds: readonly string[]): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/scene-collections/${collectionId}/scenes/reorder`, {
    scene_ids: sceneIds,
  });
}

export function createSceneFromTemplate(
  templateId: string,
  collectionId: string,
  name?: string,
): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/scene-templates/${templateId}/create`, {
    collection_id: collectionId,
    name,
  });
}

export function createSceneGroup(
  targetSceneId: string,
  childSceneId: string,
): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/scenes/${targetSceneId}/groups`, {
    child_scene_id: childSceneId,
    label: "Nested scene",
    x: 120,
    y: 120,
    width: 760,
    height: 428,
    opacity: 1,
  });
}

export function patchSceneGroup(
  sourceId: string,
  patch: Record<string, unknown>,
): Promise<ObsDashboard> {
  return requestJson<ObsDashboard>(`/api/v1/obs/me/scene-groups/${sourceId}`, {
    method: "PATCH",
    body: JSON.stringify(patch),
  });
}

export function patchSource(
  sourceId: string,
  patch: Record<string, unknown>,
): Promise<ObsDashboard> {
  return requestJson<ObsDashboard>(`/api/v1/obs/me/sources/${sourceId}`, {
    method: "PATCH",
    body: JSON.stringify(patch),
  });
}

export function sendSceneToProgram(sceneId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/scenes/${sceneId}/send-to-program`);
}

export function previewTransition(sceneId: string, fromSceneId?: string): Promise<ObsRow> {
  return post(`/api/v1/obs/me/scenes/${sceneId}/transition-preview`, {
    from_scene_id: fromSceneId,
  });
}

export function runPreflight(broadcastId: string, collectionId: string): Promise<ObsRow> {
  return post("/api/v1/obs/me/preflight", {
    broadcast_id: broadcastId,
    collection_id: collectionId,
  });
}

export function triggerCue(cueId: string): Promise<ObsRow> {
  return post(`/api/v1/obs/me/live-cues/${cueId}/trigger`);
}

export function addBlockedTerm(broadcastId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/moderation/blocked-terms`, {
    term: "spoiler",
    action: "hold",
  });
}

export function addModerator(broadcastId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/moderation/moderators`, {
    user_id: "user_mod_live",
    display_name: "Live Mod",
    role: "moderator",
  });
}

export function enqueueModeration(broadcastId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/moderation/queue`, {
    author_id: "viewer_sample",
    author_name: "Sample Viewer",
    message: "Can we talk about the sponsor code?",
    reason: "operator sample",
  });
}

export function resolveModeration(itemId: string, status: "approved" | "hidden" | "banned"): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/moderation/queue/${itemId}/resolve`, {
    status,
    moderator_id: "user_producer_ike",
  });
}

export function pinMessage(broadcastId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/moderation/pins`, {
    author_name: "Vanta",
    message: "Sponsor segment starts soon. Keep chat on-topic.",
  });
}

export function unpinMessage(messageId: string): Promise<ObsDashboard> {
  return post(`/api/v1/obs/me/moderation/pins/${messageId}/unpin`);
}

export function inviteGuest(broadcastId: string): Promise<ObsRow> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/guests/invite`, {
    display_name: "Guest",
    role: "guest",
  });
}

export function configureGuestRoomRouting(
  broadcastId: string,
  roomMode: "dual" | "group" | "shared_game",
): Promise<ObsRow> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/guests/routing`, {
    room_mode: roomMode,
    max_participants: roomMode === "dual" ? 2 : 8,
    shared_feed_source_id: roomMode === "shared_game" ? "source_screen" : undefined,
    mirrored_channels: roomMode !== "dual",
    latency_target_ms: roomMode === "shared_game" ? 120 : 140,
  });
}

export function promoteGuest(participantId: string, sceneId: string): Promise<ObsRow> {
  return post(`/api/v1/obs/me/guests/${participantId}/promote/${sceneId}`);
}

export function runGuestDeviceCheck(participantId: string): Promise<ObsRow> {
  return post(`/api/v1/obs/me/guests/${participantId}/device-check`, {
    camera_status: "ready",
    microphone_status: "ready",
    network_status: "ready",
    browser_status: "ready",
    bitrate_kbps: 2400,
    round_trip_ms: 118,
    packet_loss_percent: 0.4,
    checks_json: { surface: "studio", device_picker: "browser_prejoin" },
  });
}

export function reportGuestMediaTelemetry(
  participantId: string,
  speaking = true,
): Promise<ObsRow> {
  return post(`/api/v1/obs/me/guests/${participantId}/media-telemetry`, {
    audio_level_db: speaking ? -28 : -78,
    speaking,
    video_active: true,
    round_trip_ms: speaking ? 92 : 120,
    packet_loss_percent: speaking ? 0.2 : 0.6,
    jitter_ms: speaking ? 8 : 16,
    dropped_frames: 0,
    media_json: { surface: "studio", source: "browser_guest_tile" },
  });
}

export function createGuestWebrtcOffer(
  participantId: string,
  payload: GuestWebrtcOfferPayload,
): Promise<ObsRow> {
  return post(`/api/v1/obs/me/guests/${participantId}/webrtc/offer`, payload);
}

export function reconcileGuestMediaRelays(broadcastId: string): Promise<ObsRow> {
  return post(`/api/v1/obs/me/broadcasts/${broadcastId}/guests/relays/reconcile`);
}

export function applyGuestWebrtcAnswer(
  sessionId: string,
  answerSdp: string,
  selectedVideoLayer = "720p30",
): Promise<ObsRow> {
  return post(`/api/v1/obs/me/guests/webrtc/${sessionId}/answer`, {
    answer_sdp: answerSdp,
    selected_video_layer: selectedVideoLayer,
    media_json: { surface: "vanta_realtime_runtime" },
  });
}

export function addGuestWebrtcIceCandidate(
  sessionId: string,
  candidate: string,
): Promise<ObsRow> {
  return post(`/api/v1/obs/me/guests/webrtc/${sessionId}/ice`, {
    candidate,
    sdp_mid: "0",
    sdp_mline_index: 0,
    candidate_json: { surface: "studio_browser" },
  });
}

export function negotiateGuestReturnFeed(
  participantId: string,
  sharedFeedSourceId?: string,
): Promise<ObsRow> {
  return post(`/api/v1/obs/me/guests/${participantId}/return-feed`, {
    audio_mode: "mix_minus",
    video_mode: sharedFeedSourceId ? "shared_game" : "program_return",
    transport: "vanta_realtime_sfu",
    shared_feed_source_id: sharedFeedSourceId,
    target_latency_ms: sharedFeedSourceId ? 110 : 140,
    audio_bitrate_kbps: 96,
    video_bitrate_kbps: sharedFeedSourceId ? 3200 : 1800,
  });
}

export function startGuestIsolatedRecording(participantId: string): Promise<ObsRow> {
  return post(`/api/v1/obs/me/guests/${participantId}/isolated-recording/start`, {
    recording_mode: "audio_video",
    include_audio: true,
    include_video: true,
  });
}

export function stopGuestIsolatedRecording(participantId: string): Promise<ObsRow> {
  return post(`/api/v1/obs/me/guests/${participantId}/isolated-recording/stop`);
}

export function moderateGuest(
  participantId: string,
  action: "hold_backstage" | "release_backstage" | "approve_live",
  targetSceneId?: string,
): Promise<ObsRow> {
  return post(`/api/v1/obs/me/guests/${participantId}/moderation`, {
    action,
    moderator_id: "producer_live",
    reason: action === "hold_backstage" ? "Producer hold from studio" : "Producer approved from studio",
    target_scene_id: targetSceneId,
  });
}

export function patchGuest(participantId: string, patch: Record<string, unknown>): Promise<ObsRow> {
  return requestJson<ObsRow>(`/api/v1/obs/me/guests/${participantId}`, {
    method: "PATCH",
    body: JSON.stringify(patch),
  });
}

export function removeGuest(participantId: string): Promise<ObsRow> {
  return post(`/api/v1/obs/me/guests/${participantId}/remove`);
}

export function triggerHotkey(hotkeyId: string): Promise<ObsDashboard> {
  return post<{ readonly dashboard: ObsDashboard }>(`/api/v1/obs/me/hotkeys/${hotkeyId}/trigger`)
    .then((result) => result.dashboard);
}

export function patchHotkey(hotkeyId: string, patch: Record<string, unknown>): Promise<ObsRow> {
  return requestJson<ObsRow>(`/api/v1/obs/me/hotkeys/${hotkeyId}`, {
    method: "PATCH",
    body: JSON.stringify(patch),
  });
}

export function createSourceFilter(sourceId: string): Promise<ObsRow> {
  return post(`/api/v1/obs/me/sources/${sourceId}/filters`, {
    filter_kind: "color_correction",
    label: "Color correction",
    settings_json: { exposure: 0, contrast: 1, saturation: 1 },
  });
}

export function patchSourceFilter(filterId: string, patch: Record<string, unknown>): Promise<ObsRow> {
  return requestJson<ObsRow>(`/api/v1/obs/me/source-filters/${filterId}`, {
    method: "PATCH",
    body: JSON.stringify(patch),
  });
}

export function disableSourceFilter(filterId: string): Promise<ObsRow> {
  return post(`/api/v1/obs/me/source-filters/${filterId}/disable`);
}

export function importObsCollection(collectionJson: unknown): Promise<ObsImportReport> {
  return post("/api/v1/obs/me/imports/scene-collections", {
    label: "OBS scene collection",
    collection_json: collectionJson,
    allow_partial: true,
  });
}

export function exportObsCollection(collectionId: string, label: string): Promise<ObsExportJob> {
  return post("/api/v1/obs/me/exports/scene-collections", {
    label,
    collection_id: collectionId,
    include_setup_instructions: true,
  });
}

export async function syncLocalObs(): Promise<ObsBridgeConnection> {
  const connection = await post<ObsBridgeConnection>("/api/v1/obs/me/bridge/connections", {
    label: "Local OBS",
    websocket_url: "ws://127.0.0.1:4455",
    auto_sync: true,
  });
  return post(`/api/v1/obs/me/bridge/connections/${connection.id}/sync`);
}

export function startNativeHelper(helperKind: string): Promise<NativeHelperSession> {
  return post("/api/v1/native/helpers/sessions", {
    helper_kind: helperKind,
    launch_mode: "managed",
  });
}

export function getNativeHelperPackages(): Promise<readonly NativeHelperPackage[]> {
  return requestJson<readonly NativeHelperPackage[]>("/api/v1/native/helpers/packages");
}

export function heartbeatNativeHelper(sessionId: string): Promise<NativeHelperSession> {
  return post(`/api/v1/native/helpers/sessions/${sessionId}/heartbeat`);
}

export function recoverNativeHelper(sessionId: string): Promise<NativeHelperSession> {
  return post(`/api/v1/native/helpers/sessions/${sessionId}/recover`, {
    reason: "operator_requested",
  });
}

export function reportNativeHelperCrash(sessionId: string): Promise<NativeHelperSession> {
  return post(`/api/v1/native/helpers/sessions/${sessionId}/command`, {
    command_kind: "report_crash",
    payload_json: {
      reason: "operator_test",
      trace_event: `native.helper.${sessionId}.operator_test_crash`,
    },
  });
}

export function shutdownNativeHelper(sessionId: string): Promise<NativeHelperSession> {
  return post(`/api/v1/native/helpers/sessions/${sessionId}/shutdown`);
}

export function getMediaCapabilities(): Promise<MediaCapabilities> {
  return requestJson<MediaCapabilities>("/api/v1/media/capabilities");
}

export function getMediaCaptureInventory(): Promise<MediaCaptureInventory> {
  return requestJson<MediaCaptureInventory>("/api/v1/media/capture/devices");
}

export function startMediaCapture(source: ObsRow): Promise<MediaCaptureSession> {
  return post("/api/v1/media/capture/sessions", {
    source_id: source.id,
    capture_kind: mediaCaptureKind(source),
    width: 1920,
    height: 1080,
    frame_rate: 60,
    audio: source.source_kind === "microphone" || source.source_kind === "desktop_audio",
    duration_seconds: 5,
  });
}

export function stopMediaCapture(sessionId: string): Promise<MediaCaptureSession> {
  return post(`/api/v1/media/capture/sessions/${sessionId}/stop`);
}

export function reconcileMediaCapture(sessionId: string): Promise<MediaCaptureSession> {
  return post(`/api/v1/media/capture/sessions/${sessionId}/reconcile`);
}

export function captureMediaPreviewFrame(sessionId: string): Promise<MediaCaptureFrame> {
  return post(`/api/v1/media/capture/sessions/${sessionId}/preview-frame`);
}

export function ingestRuntimeProgramFrame(
  sessionId: string,
  imageDataUrl: string,
  compositorBackend: "webgl_gpu" | "canvas_2d",
  frameSequence: number,
): Promise<MediaCaptureFrame> {
  return post(`/api/v1/media/capture/sessions/${sessionId}/runtime-frame`, {
    image_data_url: imageDataUrl,
    compositor_backend: compositorBackend,
    frame_sequence: frameSequence,
    captured_at_ms: Date.now(),
  });
}

export function ingestRuntimeSourceFrame(
  sessionId: string,
  imageDataUrl: string,
  compositorBackend: "webgl_gpu" | "canvas_2d" | "runtime_headless_browser",
  frameSequence: number,
  surfaceKind: "browser_source" | "remote_web_surface",
  health: {
    readonly droppedFrames?: number;
    readonly reconnectCount?: number;
    readonly ingestLatencyMs?: number;
  } = {},
): Promise<MediaCaptureFrame> {
  return post(`/api/v1/media/capture/sessions/${sessionId}/source-frame`, {
    image_data_url: imageDataUrl,
    compositor_backend: compositorBackend,
    frame_sequence: frameSequence,
    captured_at_ms: Date.now(),
    surface_kind: surfaceKind,
    dropped_frames: health.droppedFrames ?? 0,
    reconnect_count: health.reconnectCount ?? 0,
    ingest_latency_ms: health.ingestLatencyMs ?? 0,
  });
}

export function createRuntimeSourcePlayout(
  sessionId: string,
  frameCount = 8,
  targetFrameRate = 30,
): Promise<MediaCaptureArtifact> {
  return post(`/api/v1/media/capture/sessions/${sessionId}/source-playout`, {
    frame_count: frameCount,
    target_frame_rate: targetFrameRate,
  });
}

export function getMediaCaptureFrames(sessionId: string): Promise<readonly MediaCaptureFrame[]> {
  return requestJson<readonly MediaCaptureFrame[]>(`/api/v1/media/capture/sessions/${sessionId}/frames`);
}

export function captureMediaSegment(sessionId: string): Promise<MediaCaptureArtifact> {
  return post(`/api/v1/media/capture/sessions/${sessionId}/segment`);
}

export function getMediaCaptureArtifacts(sessionId: string): Promise<readonly MediaCaptureArtifact[]> {
  return requestJson<readonly MediaCaptureArtifact[]>(`/api/v1/media/capture/sessions/${sessionId}/artifacts`);
}

export function ingestMediaSourceAudio(
  sourceId: string,
  inputPath: string,
): Promise<MediaSourceArtifact> {
  return post("/api/v1/media/sources/audio", {
    source_id: sourceId,
    input_path: inputPath,
  });
}

export function getMediaSourceArtifacts(sourceId: string): Promise<readonly MediaSourceArtifact[]> {
  return requestJson<readonly MediaSourceArtifact[]>(`/api/v1/media/sources/${sourceId}/artifacts`);
}

export function startMediaEncode(
  broadcastId: string,
  captureSessionId: string,
): Promise<MediaEncodeJob> {
  return post("/api/v1/media/encode/jobs", {
    broadcast_id: broadcastId,
    capture_session_id: captureSessionId,
    codec: "h264",
    audio_codec: "aac",
    container: "fragmented_mp4",
    bitrate_kbps: 6200,
    keyframe_interval_seconds: 2,
    latency_profile: "low",
  });
}

export function stopMediaEncode(jobId: string): Promise<MediaEncodeJob> {
  return post(`/api/v1/media/encode/jobs/${jobId}/stop`);
}

export function renderMediaEncode(jobId: string): Promise<MediaEncodeJob> {
  return post(`/api/v1/media/encode/jobs/${jobId}/render`);
}

export function packageMediaEncode(jobId: string): Promise<MediaPackage> {
  return post(`/api/v1/media/encode/jobs/${jobId}/package`);
}

export function patchAudioChannel(channelId: string, patch: Record<string, unknown>): Promise<ObsRow> {
  return requestJson<ObsRow>(`/api/v1/obs/me/audio/channels/${channelId}`, {
    method: "PATCH",
    body: JSON.stringify(patch),
  });
}

export function mediaCaptureKind(source: ObsRow): string {
  if (source.source_kind === "microphone") return "microphone";
  if (source.source_kind === "desktop_audio") return "desktop_audio";
  if (source.source_kind === "system_audio") return "system_audio";
  if (source.source_kind === "application_audio") return "application_audio";
  if (source.source_kind === "display_capture" || source.source_kind === "screen_capture") return "display";
  if (source.source_kind === "window_capture") return "window";
  if (source.source_kind === "browser_capture" || source.source_kind === "remote_contribution") return "browser_surface";
  return "camera";
}

function sponsorLabel(creativeKind: string): string {
  if (creativeKind === "qr_code") return "Nova QR CTA";
  if (creativeKind === "promo_code") return "Nova promo code";
  if (creativeKind === "pinned_cta") return "Nova pinned CTA";
  if (creativeKind === "branded_bumper") return "Nova bumper";
  if (creativeKind === "lower_third") return "Nova lower third";
  return "Nova sponsor card";
}
