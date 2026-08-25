pub(super) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS obs_scene_collections (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, name TEXT NOT NULL, description TEXT NOT NULL,
  canvas_width INTEGER NOT NULL, canvas_height INTEGER NOT NULL, frame_rate INTEGER NOT NULL,
  default_transition TEXT NOT NULL, active_scene_id TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_scenes (
  id TEXT PRIMARY KEY, collection_id TEXT NOT NULL, creator_id TEXT NOT NULL, name TEXT NOT NULL,
  order_index INTEGER NOT NULL, transition_kind TEXT NOT NULL, transition_duration_ms INTEGER NOT NULL,
  hotkey TEXT, locked INTEGER NOT NULL, validation_state TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_scene_templates (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, label TEXT NOT NULL, template_kind TEXT NOT NULL,
  description TEXT NOT NULL, transition_kind TEXT NOT NULL, transition_duration_ms INTEGER NOT NULL,
  layout_json TEXT NOT NULL, requirements_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_sources (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, source_kind TEXT NOT NULL, display_name TEXT NOT NULL,
  device_id TEXT, media_asset_id TEXT, browser_url TEXT, default_settings_json TEXT NOT NULL,
  permission_state TEXT NOT NULL, health_state TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_source_filters (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, source_id TEXT NOT NULL, filter_kind TEXT NOT NULL,
  label TEXT NOT NULL, enabled INTEGER NOT NULL, order_index INTEGER NOT NULL,
  settings_json TEXT NOT NULL, obs_mapping_json TEXT NOT NULL, validation_json TEXT NOT NULL,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_source_instances (
  id TEXT PRIMARY KEY, scene_id TEXT NOT NULL, source_id TEXT NOT NULL, order_index INTEGER NOT NULL,
  visible INTEGER NOT NULL, locked INTEGER NOT NULL, x REAL NOT NULL, y REAL NOT NULL, width REAL NOT NULL,
  height REAL NOT NULL, crop_json TEXT NOT NULL, transform_json TEXT NOT NULL, opacity REAL NOT NULL,
  settings_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_audio_channels (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, source_id TEXT, broadcast_id TEXT, label TEXT NOT NULL,
  channel_kind TEXT NOT NULL, muted INTEGER NOT NULL, solo INTEGER NOT NULL, gain_db REAL NOT NULL,
  monitor_enabled INTEGER NOT NULL, program_enabled INTEGER NOT NULL, delay_ms INTEGER NOT NULL,
  filters_json TEXT NOT NULL, route_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_guest_rooms (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, status TEXT NOT NULL, room_mode TEXT NOT NULL,
  max_participants INTEGER NOT NULL, shared_program_context_json TEXT NOT NULL,
  routing_policy_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_guest_participants (
  id TEXT PRIMARY KEY, room_id TEXT NOT NULL, broadcast_id TEXT NOT NULL, display_name TEXT NOT NULL,
  role TEXT NOT NULL, source_id TEXT, status TEXT NOT NULL, muted INTEGER NOT NULL, solo INTEGER NOT NULL,
  safety_disabled INTEGER NOT NULL, invite_url TEXT NOT NULL, scene_id TEXT,
  return_feed_json TEXT NOT NULL, connection_health_json TEXT NOT NULL, isolated_recording_json TEXT NOT NULL,
  device_check_json TEXT NOT NULL DEFAULT '{}', moderator_control_json TEXT NOT NULL DEFAULT '{}',
  media_state_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_guest_media_telemetry (
  id TEXT PRIMARY KEY, participant_id TEXT NOT NULL, broadcast_id TEXT NOT NULL,
  audio_level_db REAL NOT NULL, speaking INTEGER NOT NULL, video_active INTEGER NOT NULL,
  round_trip_ms INTEGER NOT NULL, packet_loss_percent REAL NOT NULL, jitter_ms INTEGER NOT NULL,
  dropped_frames INTEGER NOT NULL, active_speaker_score REAL NOT NULL,
  telemetry_json TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_guest_webrtc_sessions (
  id TEXT PRIMARY KEY, participant_id TEXT NOT NULL, broadcast_id TEXT NOT NULL,
  session_role TEXT NOT NULL, direction TEXT NOT NULL, status TEXT NOT NULL,
  audio_enabled INTEGER NOT NULL, video_enabled INTEGER NOT NULL,
  preferred_video_layer TEXT NOT NULL, selected_video_layer TEXT NOT NULL,
  offer_sdp TEXT NOT NULL, answer_sdp TEXT NOT NULL,
  ice_candidates_json TEXT NOT NULL, tracks_json TEXT NOT NULL,
  transport_json TEXT NOT NULL, health_json TEXT NOT NULL,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_guest_media_relays (
  id TEXT PRIMARY KEY, session_id TEXT NOT NULL, participant_id TEXT NOT NULL, broadcast_id TEXT NOT NULL,
  status TEXT NOT NULL, relay_kind TEXT NOT NULL, program_source_id TEXT,
  return_feed_session_id TEXT, runtime_output_id TEXT, archive_manifest_json TEXT NOT NULL,
  route_json TEXT NOT NULL, health_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_guest_rtp_packets (
  id TEXT PRIMARY KEY, relay_id TEXT NOT NULL, session_id TEXT NOT NULL,
  participant_id TEXT NOT NULL, broadcast_id TEXT NOT NULL, payload_kind TEXT NOT NULL,
  sequence_number INTEGER NOT NULL, rtp_timestamp INTEGER NOT NULL, ssrc INTEGER NOT NULL,
  marker INTEGER NOT NULL, payload_type INTEGER NOT NULL, byte_length INTEGER NOT NULL,
  packet_json TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_guest_media_worker_frames (
  id TEXT PRIMARY KEY, relay_id TEXT NOT NULL, session_id TEXT NOT NULL,
  participant_id TEXT NOT NULL, broadcast_id TEXT NOT NULL, payload_kind TEXT NOT NULL,
  status TEXT NOT NULL, start_sequence_number INTEGER NOT NULL, end_sequence_number INTEGER NOT NULL,
  rtp_timestamp INTEGER NOT NULL, ssrc INTEGER NOT NULL, packet_count INTEGER NOT NULL,
  byte_length INTEGER NOT NULL, playout_at_ms INTEGER NOT NULL, frame_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_guest_decoded_media_frames (
  id TEXT PRIMARY KEY, media_worker_frame_id TEXT NOT NULL, relay_id TEXT NOT NULL,
  session_id TEXT NOT NULL, participant_id TEXT NOT NULL, broadcast_id TEXT NOT NULL,
  payload_kind TEXT NOT NULL, codec TEXT NOT NULL, status TEXT NOT NULL,
  artifact_path TEXT NOT NULL, decoded_frame_json TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_guest_media_route_frames (
  id TEXT PRIMARY KEY, decoded_media_frame_id TEXT NOT NULL, media_worker_frame_id TEXT NOT NULL,
  relay_id TEXT NOT NULL, participant_id TEXT NOT NULL, broadcast_id TEXT NOT NULL,
  route_kind TEXT NOT NULL, status TEXT NOT NULL, route_frame_json TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_guest_media_sync_pairs (
  id TEXT PRIMARY KEY, relay_id TEXT NOT NULL, participant_id TEXT NOT NULL, broadcast_id TEXT NOT NULL,
  route_kind TEXT NOT NULL, audio_route_frame_id TEXT NOT NULL, video_route_frame_id TEXT NOT NULL,
  sync_status TEXT NOT NULL, drift_ms INTEGER NOT NULL, sync_pair_json TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_guest_compositor_frames (
  id TEXT PRIMARY KEY, relay_id TEXT NOT NULL, participant_id TEXT NOT NULL, broadcast_id TEXT NOT NULL,
  route_kind TEXT NOT NULL, sync_pair_id TEXT NOT NULL, audio_route_frame_id TEXT NOT NULL,
  video_route_frame_id TEXT NOT NULL, status TEXT NOT NULL, artifact_path TEXT NOT NULL,
  compositor_frame_json TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_guest_compositor_playout_frames (
  id TEXT PRIMARY KEY, relay_id TEXT NOT NULL, participant_id TEXT NOT NULL, broadcast_id TEXT NOT NULL,
  route_kind TEXT NOT NULL, compositor_frame_id TEXT NOT NULL, program_frame_sequence INTEGER NOT NULL,
  playout_status TEXT NOT NULL, dropped_frames INTEGER NOT NULL, playout_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_runtime_live_feed_sessions (
  id TEXT PRIMARY KEY, relay_id TEXT NOT NULL, participant_id TEXT NOT NULL, broadcast_id TEXT NOT NULL,
  transport TEXT NOT NULL, program_surface TEXT NOT NULL, status TEXT NOT NULL,
  first_program_frame_sequence INTEGER NOT NULL, last_program_frame_sequence INTEGER NOT NULL,
  delivered_chunks INTEGER NOT NULL, cumulative_dropped_frames INTEGER NOT NULL,
  average_lateness_ms REAL NOT NULL, max_lateness_ms INTEGER NOT NULL,
  pressure_level TEXT NOT NULL, delivery_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_guest_return_feed_sessions (
  id TEXT PRIMARY KEY, participant_id TEXT NOT NULL, broadcast_id TEXT NOT NULL,
  audio_mode TEXT NOT NULL, video_mode TEXT NOT NULL, transport TEXT NOT NULL,
  target_latency_ms INTEGER NOT NULL, status TEXT NOT NULL,
  audio_track_json TEXT NOT NULL, video_track_json TEXT NOT NULL, sync_json TEXT NOT NULL,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_guest_isolated_recordings (
  id TEXT PRIMARY KEY, participant_id TEXT NOT NULL, broadcast_id TEXT NOT NULL,
  source_id TEXT, status TEXT NOT NULL, recording_mode TEXT NOT NULL,
  started_at TEXT NOT NULL, ended_at TEXT,
  track_manifest_json TEXT NOT NULL, artifact_json TEXT NOT NULL, validation_json TEXT NOT NULL,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_guest_device_checks (
  id TEXT PRIMARY KEY, participant_id TEXT NOT NULL, broadcast_id TEXT NOT NULL, status TEXT NOT NULL,
  camera_status TEXT NOT NULL, microphone_status TEXT NOT NULL, network_status TEXT NOT NULL,
  browser_status TEXT NOT NULL, bitrate_kbps INTEGER NOT NULL, round_trip_ms INTEGER NOT NULL,
  packet_loss_percent REAL NOT NULL, checks_json TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_guest_moderation_actions (
  id TEXT PRIMARY KEY, participant_id TEXT NOT NULL, broadcast_id TEXT NOT NULL,
  moderator_id TEXT NOT NULL, action TEXT NOT NULL, reason TEXT NOT NULL, target_scene_id TEXT,
  status TEXT NOT NULL, result_json TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_hotkeys (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, scope TEXT NOT NULL, action TEXT NOT NULL,
  target_id TEXT, binding TEXT NOT NULL, enabled INTEGER NOT NULL, guard_json TEXT NOT NULL,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_broadcast_profiles (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, title TEXT NOT NULL, category TEXT NOT NULL,
  tags_json TEXT NOT NULL, thumbnail TEXT NOT NULL, mature_content INTEGER NOT NULL, language TEXT NOT NULL,
  scheduled_start TEXT, visibility TEXT NOT NULL, follower_notification INTEGER NOT NULL, chat_mode TEXT NOT NULL,
  recording_policy TEXT NOT NULL, archive_policy TEXT NOT NULL, latency_profile TEXT NOT NULL,
  output_quality_target TEXT NOT NULL, sponsor_campaign_id TEXT, collaboration_settings_json TEXT NOT NULL,
  status TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_moderator_roles (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, user_id TEXT NOT NULL, display_name TEXT NOT NULL,
  role TEXT NOT NULL, permissions_json TEXT NOT NULL, status TEXT NOT NULL, created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_blocked_terms (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, term TEXT NOT NULL, action TEXT NOT NULL,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_moderation_queue (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, author_id TEXT NOT NULL, author_name TEXT NOT NULL,
  message TEXT NOT NULL, reason TEXT NOT NULL, status TEXT NOT NULL, moderator_id TEXT,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_pinned_messages (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, author_name TEXT NOT NULL, message TEXT NOT NULL,
  status TEXT NOT NULL, pinned_at TEXT NOT NULL, unpinned_at TEXT, created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_audience_snapshots (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, viewer_count INTEGER NOT NULL,
  chat_messages_per_minute INTEGER NOT NULL, tips_cents INTEGER NOT NULL,
  subscriptions INTEGER NOT NULL, revenue_cents INTEGER NOT NULL, discovery_source TEXT NOT NULL,
  discovery_score REAL NOT NULL, discovery_json TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_raids (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, direction TEXT NOT NULL, target_channel_id TEXT NOT NULL,
  target_channel_name TEXT NOT NULL, viewer_count INTEGER NOT NULL, status TEXT NOT NULL,
  execute_after_seconds INTEGER NOT NULL, redirect_url TEXT NOT NULL, safety_json TEXT NOT NULL,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_schedule_slots (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, title TEXT NOT NULL, starts_at TEXT NOT NULL,
  timezone TEXT NOT NULL, duration_minutes INTEGER NOT NULL, status TEXT NOT NULL,
  reminder_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_engagement_polls (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, poll_kind TEXT NOT NULL, question TEXT NOT NULL,
  options_json TEXT NOT NULL, status TEXT NOT NULL, duration_seconds INTEGER NOT NULL,
  opened_at TEXT NOT NULL, closed_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_engagement_votes (
  id TEXT PRIMARY KEY, poll_id TEXT NOT NULL, broadcast_id TEXT NOT NULL, option_id TEXT NOT NULL,
  voter_id TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_alert_events (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, alert_kind TEXT NOT NULL, title TEXT NOT NULL,
  message TEXT NOT NULL, severity TEXT NOT NULL, source_user TEXT, amount_cents INTEGER NOT NULL,
  status TEXT NOT NULL, metadata_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_sponsor_campaigns (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, campaign_id TEXT NOT NULL, advertiser TEXT NOT NULL,
  title TEXT NOT NULL, status TEXT NOT NULL, flight_json TEXT NOT NULL, claims_json TEXT NOT NULL,
  performance_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_sponsor_inventory (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, campaign_id TEXT NOT NULL, creative_kind TEXT NOT NULL,
  label TEXT NOT NULL, source_kind TEXT NOT NULL, source_id TEXT NOT NULL, cue_id TEXT NOT NULL,
  scheduled_at_seconds REAL NOT NULL, required_duration_seconds REAL NOT NULL, status TEXT NOT NULL,
  requirements_json TEXT NOT NULL, renderer_json TEXT NOT NULL, proof_marker_id TEXT,
  review_status TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_sponsor_proofs (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, inventory_id TEXT NOT NULL, cue_id TEXT NOT NULL,
  proof_kind TEXT NOT NULL, status TEXT NOT NULL, media_time_seconds REAL NOT NULL,
  artifact_json TEXT NOT NULL, review_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_preflight_checks (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, broadcast_id TEXT NOT NULL, collection_id TEXT NOT NULL,
  ready INTEGER NOT NULL, checks_json TEXT NOT NULL, blockers_json TEXT NOT NULL, warnings_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_live_cues (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, broadcast_id TEXT NOT NULL, campaign_id TEXT, offer_id TEXT,
  cue_kind TEXT NOT NULL, label TEXT NOT NULL, scheduled_at_seconds REAL, required_duration_seconds REAL,
  status TEXT NOT NULL, scene_id TEXT, source_id TEXT, proof_marker_id TEXT, requirements_json TEXT NOT NULL,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_replay_markers (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, broadcast_id TEXT NOT NULL, label TEXT NOT NULL,
  duration_seconds INTEGER NOT NULL, sponsor_proof INTEGER NOT NULL, status TEXT NOT NULL, clip_media_asset_id TEXT,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_replay_clip_drafts (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, replay_marker_id TEXT NOT NULL, broadcast_id TEXT NOT NULL,
  clip_media_asset_id TEXT NOT NULL, status TEXT NOT NULL, output_path TEXT NOT NULL,
  manifest_json TEXT NOT NULL, pressure_json TEXT NOT NULL, buffer_json TEXT NOT NULL DEFAULT '{}',
  upload_queue_json TEXT NOT NULL,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_replay_buffer_segments (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, broadcast_id TEXT NOT NULL, segment_index INTEGER NOT NULL,
  duration_seconds INTEGER NOT NULL, status TEXT NOT NULL, artifact_path TEXT NOT NULL,
  validation_json TEXT NOT NULL, pressure_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS vanta_media_assets (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, broadcast_id TEXT NOT NULL, asset_kind TEXT NOT NULL,
  status TEXT NOT NULL, source_path TEXT NOT NULL, asset_path TEXT NOT NULL, manifest_path TEXT NOT NULL,
  metadata_json TEXT NOT NULL, validation_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_recording_jobs (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, broadcast_id TEXT NOT NULL, live_ingest_session_id TEXT,
  recording_mode TEXT NOT NULL, status TEXT NOT NULL, started_at TEXT NOT NULL, ended_at TEXT,
  output_media_asset_id TEXT, output_paths_json TEXT NOT NULL, error_message TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_runtime_bindings (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, broadcast_id TEXT NOT NULL, live_ingest_session_id TEXT NOT NULL,
  scene_collection_id TEXT NOT NULL, active_scene_id TEXT NOT NULL, program_scene_id TEXT NOT NULL, preview_scene_id TEXT,
  runtime_state TEXT NOT NULL, stream_state TEXT NOT NULL, recording_state TEXT NOT NULL, last_heartbeat_at TEXT NOT NULL,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_scene_transition_runs (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, broadcast_id TEXT NOT NULL, collection_id TEXT NOT NULL,
  from_scene_id TEXT, to_scene_id TEXT NOT NULL, transition_kind TEXT NOT NULL,
  duration_ms INTEGER NOT NULL, status TEXT NOT NULL, interruption_policy_json TEXT NOT NULL,
  preview_json TEXT NOT NULL, started_at TEXT NOT NULL, completed_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS vanta_live_ingest_sessions (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, broadcast_id TEXT NOT NULL, status TEXT NOT NULL,
  ingest_protocol TEXT NOT NULL, stream_key_hash TEXT NOT NULL, stream_key_hint TEXT NOT NULL,
  ingest_url TEXT NOT NULL, backup_ingest_url TEXT NOT NULL, started_at TEXT, ended_at TEXT,
  reconnect_policy_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS vanta_live_runtime_targets (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, target_kind TEXT NOT NULL, status TEXT NOT NULL,
  protocol TEXT NOT NULL, endpoint_url TEXT NOT NULL, latency_profile TEXT NOT NULL,
  negotiation_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS vanta_live_runtime_outputs (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, ingest_session_id TEXT NOT NULL, output_kind TEXT NOT NULL,
  status TEXT NOT NULL, target_id TEXT NOT NULL, health_json TEXT NOT NULL, started_at TEXT,
  ended_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS vanta_live_playback_readiness (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, ingest_session_id TEXT NOT NULL, status TEXT NOT NULL,
  grant_id TEXT NOT NULL, playback_url TEXT NOT NULL, checks_json TEXT NOT NULL, created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS vanta_live_runtime_telemetry (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, ingest_session_id TEXT NOT NULL, sample_kind TEXT NOT NULL,
  bitrate_kbps INTEGER NOT NULL, upload_mbps REAL NOT NULL, ingest_latency_ms INTEGER NOT NULL,
  dropped_frames INTEGER NOT NULL, cpu_percent INTEGER NOT NULL, reconnect_count INTEGER NOT NULL,
  health_json TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS vanta_live_authoritative_bindings (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, broadcast_id TEXT NOT NULL, obs_runtime_binding_id TEXT NOT NULL,
  live_ingest_session_id TEXT NOT NULL, external_broadcast_id TEXT NOT NULL, authority TEXT NOT NULL,
  status TEXT NOT NULL, version INTEGER NOT NULL, binding_json TEXT NOT NULL, last_snapshot_json TEXT NOT NULL,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS vanta_live_authoritative_events (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, binding_id TEXT NOT NULL, event_kind TEXT NOT NULL,
  status TEXT NOT NULL, payload_json TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_post_show_packages (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, broadcast_id TEXT NOT NULL, status TEXT NOT NULL,
  output_paths_json TEXT NOT NULL, metrics_json TEXT NOT NULL, sponsor_proofs_json TEXT NOT NULL,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_runtime_events (
  id TEXT PRIMARY KEY, broadcast_id TEXT, event_kind TEXT NOT NULL, severity TEXT NOT NULL,
  message TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_runtime_incidents (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, incident_kind TEXT NOT NULL, severity TEXT NOT NULL,
  status TEXT NOT NULL, operator_id TEXT, reason TEXT NOT NULL, holding_scene_id TEXT,
  details_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_support_bundles (
  id TEXT PRIMARY KEY, broadcast_id TEXT NOT NULL, status TEXT NOT NULL, bundle_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_bridge_connections (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, label TEXT NOT NULL, websocket_url TEXT NOT NULL,
  password_json TEXT, auto_sync INTEGER NOT NULL, sync_status TEXT NOT NULL, last_error TEXT,
  last_snapshot_json TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, last_synced_at TEXT
);
CREATE TABLE IF NOT EXISTS obs_bridge_events (
  id TEXT PRIMARY KEY, connection_id TEXT NOT NULL, event_kind TEXT NOT NULL, payload_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_import_reports (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, label TEXT NOT NULL, collection_id TEXT,
  status TEXT NOT NULL, report_json TEXT NOT NULL, original_metadata_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS obs_export_jobs (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, collection_id TEXT NOT NULL, label TEXT NOT NULL,
  status TEXT NOT NULL, scene_collection_json TEXT NOT NULL, asset_manifest_json TEXT NOT NULL,
  warnings_json TEXT NOT NULL, setup_instructions_json TEXT NOT NULL, created_at TEXT NOT NULL
);
"#;
