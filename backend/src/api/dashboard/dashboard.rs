use super::analytics::{
    fetch_analytics, fetch_revenue_entries, fetch_top_content, fetch_traffic_sources,
    summarize_creator_analytics, summarize_creator_revenue,
};
use super::content::{
    fetch_creator_content_summary, fetch_filtered_uploads_unreconciled, fetch_uploads,
};
use super::*;
use crate::api::creator::fetch_creator_profile_persisted;
use crate::api::control::{
    build_live_runtime_advisory, describe_declared_live_runtime_artifact_health,
    fetch_live_runtime_output_for_session,
};
use crate::api::notifications::fetch_notifications_rows_limited;
use crate::models::LiveRuntimeTelemetrySummary;

const CREATOR_DASHBOARD_ANALYTICS_LIMIT: usize = 14;
const CREATOR_DASHBOARD_REVENUE_LIMIT: usize = 14;
const CREATOR_DASHBOARD_RECENT_BROADCAST_LIMIT: usize = 12;
const CREATOR_DASHBOARD_NOTIFICATIONS_LIMIT: usize = 20;
const CREATOR_DASHBOARD_UPLOADS_LIMIT: usize = 20;
const CREATOR_APP_STATE_DASHBOARD_NOTIFICATIONS_LIMIT: usize = 10;
const CREATOR_APP_STATE_UPLOADS_LIMIT: usize = 20;
const CREATOR_APP_STATE_RECENT_ENDED_BROADCAST_LIMIT: i64 = 12;

async fn fetch_creator_live_health_for_app_state(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorLiveHealth> {
    let row = sqlx::query(
        "SELECT bitrate_kbps, cpu_percent, dropped_frames, free_disk_gb FROM creator_live_settings WHERE creator_id = ?",
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(CreatorLiveHealth {
        current_bitrate_kbps: row.get("bitrate_kbps"),
        current_cpu_percent: row.get("cpu_percent"),
        current_dropped_frames: row.get("dropped_frames"),
        current_free_disk_gb: row.get("free_disk_gb"),
        samples: Vec::new(),
    })
}

fn build_creator_live_snapshot_for_app_state_from_parts(
    mut profile: CreatorProfile,
    broadcasts: &[Broadcast],
    ingest_session: Option<LiveIngestSession>,
) -> CreatorLiveSnapshot {
    let current_broadcast = broadcasts
        .iter()
        .find(|item| item.status == "live")
        .cloned();
    let pending_broadcast = broadcasts
        .iter()
        .find(|item| item.status == "ready")
        .cloned();

    profile.current_broadcast_id = current_broadcast
        .as_ref()
        .map(|item| item.id.clone())
        .or_else(|| pending_broadcast.as_ref().map(|item| item.id.clone()));
    profile.live_status = if current_broadcast.is_some() {
        "live".to_string()
    } else if pending_broadcast.is_some() {
        "ready".to_string()
    } else {
        "offline".to_string()
    };

    CreatorLiveSnapshot {
        profile: contract_creator_profile(profile),
        current_broadcast: current_broadcast.map(contract_broadcast),
        pending_broadcast: pending_broadcast.map(contract_broadcast),
        ingest_session,
    }
}

fn creator_dashboard_payload_for_app_state_from_parts(
    profile: CreatorProfile,
    operational_state: CreatorOperationalState,
    broadcasts: &[Broadcast],
    notifications: Vec<CreatorNotification>,
) -> CreatorDashboard {
    let current_broadcast = broadcasts
        .iter()
        .find(|item| item.status == "live")
        .cloned();
    let scheduled_broadcasts = broadcasts
        .iter()
        .filter(|item| item.status == "scheduled" || item.status == "ready")
        .cloned()
        .collect();
    let recent_broadcasts = broadcasts
        .iter()
        .filter(|item| item.status == "ended")
        .cloned()
        .take(CREATOR_DASHBOARD_RECENT_BROADCAST_LIMIT)
        .collect();

    CreatorDashboard {
        profile: contract_creator_profile(profile),
        current_broadcast: current_broadcast.map(contract_broadcast),
        scheduled_broadcasts: contract_broadcasts(scheduled_broadcasts),
        recent_broadcasts: contract_broadcasts(recent_broadcasts),
        analytics: Vec::new(),
        traffic_sources: Vec::new(),
        top_content: Vec::new(),
        revenue: Vec::new(),
        analytics_summary: CreatorAnalyticsSummary {
            window_days: 0,
            total_viewers: 0,
            total_watch_minutes: 0,
            total_revenue: 0.0,
            total_new_followers: 0,
        },
        revenue_summary: CreatorRevenueSummary {
            total_earnings_30d: 0.0,
            total_subscribers: 0,
            blended_monthly_price: 0.0,
            estimated_next_payout: 0.0,
            breakdown: Vec::new(),
        },
        subscriber_tiers: Vec::new(),
        operational_state,
        notifications: notifications
            .into_iter()
            .take(CREATOR_APP_STATE_DASHBOARD_NOTIFICATIONS_LIMIT)
            .collect(),
        uploads: Vec::new(),
    }
}

async fn fetch_creator_live_collaboration_summary_for_app_state(
    pool: &SqlitePool,
    creator_id: &str,
    snapshot: &CreatorLiveSnapshot,
) -> AppResult<CreatorLiveCollaborationSummary> {
    let active_session = if let Some(current_broadcast) = snapshot.current_broadcast.as_ref() {
        fetch_active_collaboration_session_for_broadcast(pool, &current_broadcast.id).await?
    } else if let Some(pending_broadcast) = snapshot.pending_broadcast.as_ref() {
        fetch_active_collaboration_session_for_broadcast(pool, &pending_broadcast.id).await?
    } else {
        None
    };

    let counts = sqlx::query(
        r#"
        SELECT
            (SELECT COUNT(*) FROM collaboration_sessions WHERE host_creator_id = ?) AS total_sessions,
            (SELECT COUNT(*) FROM collaboration_sessions WHERE host_creator_id = ? AND status IN ('active', 'pending')) AS active_session_count,
            (
                SELECT COUNT(*)
                FROM collaboration_invites invites
                JOIN collaboration_sessions sessions
                  ON sessions.id = invites.session_id
                WHERE sessions.host_creator_id = ?
                  AND invites.state = 'pending'
            ) AS pending_invite_count,
            (
                SELECT COUNT(*)
                FROM collaboration_mirror_grants grants
                JOIN collaboration_sessions sessions
                  ON sessions.id = grants.session_id
                WHERE sessions.host_creator_id = ?
                  AND grants.state = 'active'
            ) AS active_grant_count,
            (
                SELECT COUNT(*)
                FROM collaboration_mirror_grants grants
                JOIN collaboration_sessions sessions
                  ON sessions.id = grants.session_id
                WHERE sessions.host_creator_id = ?
                  AND grants.state = 'issued'
            ) AS issued_grant_count
        "#,
    )
    .bind(creator_id)
    .bind(creator_id)
    .bind(creator_id)
    .bind(creator_id)
    .bind(creator_id)
    .fetch_one(pool)
    .await?;

    Ok(CreatorLiveCollaborationSummary {
        active_session,
        active_control: None,
        recent_sessions: Vec::new(),
        total_sessions: counts.get("total_sessions"),
        active_session_count: counts.get("active_session_count"),
        pending_invite_count: counts.get("pending_invite_count"),
        active_grant_count: counts.get("active_grant_count"),
        issued_grant_count: counts.get("issued_grant_count"),
    })
}

async fn fetch_broadcasts_for_app_state(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<Broadcast>> {
    let (active_rows, ended_rows) = tokio::try_join!(
        sqlx::query(
            r#"
            SELECT id, title, category, tags_json, status, started_at, ended_at, duration_sec,
                   peak_viewers, average_viewers, chat_messages, new_followers, new_subscribers,
                   revenue, thumbnail, is_mature
            FROM broadcasts
            WHERE creator_id = ?
              AND status IN ('live', 'ready', 'scheduled')
            ORDER BY
                CASE status
                    WHEN 'live' THEN 0
                    WHEN 'ready' THEN 1
                    WHEN 'scheduled' THEN 2
                    ELSE 3
                END,
                started_at DESC
            "#
        )
        .bind(creator_id)
        .fetch_all(pool),
        sqlx::query(
            r#"
            SELECT id, title, category, tags_json, status, started_at, ended_at, duration_sec,
                   peak_viewers, average_viewers, chat_messages, new_followers, new_subscribers,
                   revenue, thumbnail, is_mature
            FROM broadcasts
            WHERE creator_id = ?
              AND status = 'ended'
            ORDER BY started_at DESC
            LIMIT ?
            "#
        )
        .bind(creator_id)
        .bind(CREATOR_APP_STATE_RECENT_ENDED_BROADCAST_LIMIT)
        .fetch_all(pool),
    )?;

    let mut broadcasts = Vec::with_capacity(active_rows.len() + ended_rows.len());
    broadcasts.extend(active_rows.into_iter().map(row_to_broadcast));
    broadcasts.extend(ended_rows.into_iter().map(row_to_broadcast));
    Ok(broadcasts)
}

fn row_to_broadcast(row: sqlx::sqlite::SqliteRow) -> Broadcast {
    Broadcast {
        id: row.get("id"),
        title: row.get("title"),
        category: row.get("category"),
        tags: from_json(row.get::<String, _>("tags_json")).unwrap_or_default(),
        status: row.get("status"),
        started_at: row.get("started_at"),
        ended_at: row.get("ended_at"),
        duration_sec: row.get("duration_sec"),
        peak_viewers: row.get("peak_viewers"),
        average_viewers: row.get("average_viewers"),
        chat_messages: row.get("chat_messages"),
        new_followers: row.get("new_followers"),
        new_subscribers: row.get("new_subscribers"),
        revenue: row.get("revenue"),
        thumbnail: row.get("thumbnail"),
        is_mature: row.get::<i64, _>("is_mature") == 1,
    }
}

fn empty_live_runtime_telemetry_summary() -> LiveRuntimeTelemetrySummary {
    LiveRuntimeTelemetrySummary {
        total_samples: 0,
        degraded_samples: 0,
        packaging_degraded_samples: 0,
        failure_samples: 0,
        archive_failure_samples: 0,
        reconnect_events: 0,
        probe_samples: 0,
        validation_issue_samples: 0,
        repairable_validation_samples: 0,
        advisory_critical_samples: 0,
        advisory_repairable_samples: 0,
        runtime_artifact_reconciliation_samples: 0,
        runtime_archive_completion_samples: 0,
        artifact_attention_samples: 0,
        manifest_path_missing_samples: 0,
        archive_path_missing_samples: 0,
        collaboration_samples: 0,
        mix_minus_samples: 0,
        collaboration_transport_gap_samples: 0,
        packaging_ready_samples: 0,
        archive_complete_samples: 0,
        avg_bitrate_kbps: None,
        peak_bitrate_kbps: None,
        avg_viewers: None,
        peak_viewers: None,
        total_dropped_frames: 0,
        peak_collaboration_participants: 0,
        peak_active_output_routes: 0,
        peak_engine_node_count: 0,
        peak_engine_edge_count: 0,
        peak_mix_minus_edge_count: 0,
        peak_mirror_fanout_edge_count: 0,
        peak_bundle_attachment_count: 0,
        peak_bundle_mixer_count: 0,
        peak_bundle_fanout_count: 0,
        peak_bundle_return_count: 0,
        peak_media_stage_count: 0,
        peak_media_output_target_count: 0,
        peak_media_return_target_count: 0,
        peak_media_input_participant_count: 0,
        peak_media_mix_minus_participant_count: 0,
        peak_runtime_target_count: 0,
        peak_playback_target_count: 0,
        peak_recording_target_count: 0,
        peak_variant_target_count: 0,
        peak_collaboration_target_count: 0,
        peak_program_target_count: 0,
        peak_audio_target_count: 0,
        peak_engine_target_count: 0,
        peak_host_channel_count: 0,
        peak_mirror_channel_count: 0,
        peak_shared_program_mirror_channel_count: 0,
        peak_guest_isolated_mirror_channel_count: 0,
        peak_archive_target_count: 0,
        peak_active_target_count: 0,
        peak_degraded_target_count: 0,
        peak_armed_target_count: 0,
        peak_pending_source_target_count: 0,
        ll_hls_samples: 0,
        peak_discontinuity_sequence: 0,
        last_collected_at: None,
        last_runtime_state: None,
        last_packaging_status: None,
        last_archive_status: None,
        last_contribution_state: None,
        last_ingest_latency_ms: None,
        last_source_probe_present: false,
        last_source_validation_state: None,
        last_advisory_status: None,
        last_manifest_artifact_state: None,
        last_archive_artifact_state: None,
        last_collaboration_session_id: None,
        last_collaboration_participant_count: None,
        last_collaboration_transport_gap_present: false,
        last_active_output_routes: None,
        last_audio_mix_mode: None,
        last_engine_node_count: None,
        last_engine_edge_count: None,
        last_mix_minus_edge_count: None,
        last_mirror_fanout_edge_count: None,
        last_bundle_attachment_count: None,
        last_bundle_mixer_count: None,
        last_bundle_fanout_count: None,
        last_bundle_return_count: None,
        last_media_stage_count: None,
        last_media_output_target_count: None,
        last_media_return_target_count: None,
        last_media_input_participant_count: None,
        last_media_mix_minus_participant_count: None,
        last_runtime_target_count: None,
        last_playback_target_count: None,
        last_recording_target_count: None,
        last_variant_target_count: None,
        last_collaboration_target_count: None,
        last_program_target_count: None,
        last_audio_target_count: None,
        last_engine_target_count: None,
        last_host_channel_count: None,
        last_mirror_channel_count: None,
        last_shared_program_mirror_channel_count: None,
        last_guest_isolated_mirror_channel_count: None,
        last_archive_target_count: None,
        last_active_target_count: None,
        last_degraded_target_count: None,
        last_armed_target_count: None,
        last_pending_source_target_count: None,
        last_runtime_class: None,
        last_latency_profile: None,
        last_ladder_policy: None,
        last_content_class: None,
        last_failure_at: None,
        last_failure_state: None,
        last_error: None,
    }
}

fn trim_creator_live_collaboration_summary_for_app_state(
    collaboration: &mut CreatorLiveCollaborationSummary,
) {
    collaboration.active_control = None;
    collaboration.recent_sessions.clear();
}

fn trim_creator_live_control_for_app_state(response: &mut CreatorLiveControlResponse) {
    trim_creator_live_collaboration_summary_for_app_state(&mut response.collaboration);
    response.subscriber_tiers.clear();
    response.health.samples.clear();
    response.viewer_history.clear();
    response.bitrate_history.clear();
}

fn trim_creator_live_runtime_for_app_state(response: &mut CreatorLiveRuntimeResponse) {
    trim_creator_live_collaboration_summary_for_app_state(&mut response.collaboration);
    response.health.samples.clear();
    response.active_runtime_targets.clear();
    response.recent_sessions.clear();
    response.recent_runtime_outputs.clear();
    response.recent_runtime_targets.clear();
    response.recent_telemetry.clear();
    response.recent_events.clear();
}

pub(crate) async fn creator_dashboard_payload(
    pool: &SqlitePool,
    identity: &RequestIdentity,
) -> AppResult<CreatorDashboard> {
    let creator_id = identity.require_creator_scope()?;
    let profile = fetch_creator_profile(pool, creator_id).await?;
    let operational_state = fetch_creator_operational_state(pool, &profile).await?;
    let broadcasts = fetch_broadcasts(pool, creator_id).await?;
    let analytics = fetch_analytics(pool, creator_id).await?;
    let analytics_summary = summarize_creator_analytics(&analytics);
    let traffic_sources = fetch_traffic_sources(pool, creator_id).await?;
    let top_content = fetch_top_content(pool, creator_id).await?;
    let revenue = fetch_revenue_entries(pool, creator_id).await?;
    let subscriber_tiers = fetch_creator_subscriber_tiers(pool, creator_id).await?;
    let revenue_summary = summarize_creator_revenue(&analytics, &revenue, &subscriber_tiers);
    let notifications = fetch_notifications_rows(pool, creator_id).await?;
    let uploads = fetch_uploads(pool, creator_id).await?;

    let current_broadcast = broadcasts
        .iter()
        .find(|item| item.status == "live")
        .cloned();
    let scheduled_broadcasts = broadcasts
        .iter()
        .filter(|item| item.status == "scheduled" || item.status == "ready")
        .cloned()
        .collect();
    let recent_broadcasts = broadcasts
        .iter()
        .filter(|item| item.status == "ended")
        .cloned()
        .take(CREATOR_DASHBOARD_RECENT_BROADCAST_LIMIT)
        .collect();

    Ok(CreatorDashboard {
        profile: contract_creator_profile(profile),
        current_broadcast: current_broadcast.map(contract_broadcast),
        scheduled_broadcasts: contract_broadcasts(scheduled_broadcasts),
        recent_broadcasts: contract_broadcasts(recent_broadcasts),
        analytics: analytics
            .into_iter()
            .rev()
            .take(CREATOR_DASHBOARD_ANALYTICS_LIMIT)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect(),
        traffic_sources,
        top_content,
        revenue: revenue
            .into_iter()
            .take(CREATOR_DASHBOARD_REVENUE_LIMIT)
            .collect(),
        analytics_summary,
        revenue_summary,
        subscriber_tiers,
        operational_state,
        notifications: notifications
            .into_iter()
            .take(CREATOR_DASHBOARD_NOTIFICATIONS_LIMIT)
            .collect(),
        uploads: uploads
            .into_iter()
            .take(CREATOR_DASHBOARD_UPLOADS_LIMIT)
            .collect(),
    })
}

pub(crate) async fn fetch_creator_dashboard_shell(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorDashboard> {
    let (profile, broadcasts) = tokio::try_join!(
        fetch_creator_profile_persisted(pool, creator_id),
        fetch_broadcasts_for_app_state(pool, creator_id),
    )?;
    let operational_state = fetch_creator_operational_state(pool, &profile).await?;
    Ok(creator_dashboard_payload_for_app_state_from_parts(
        profile,
        operational_state,
        &broadcasts,
        Vec::new(),
    ))
}

pub(crate) async fn fetch_creator_app_state(
    state: &SharedState,
    identity: &RequestIdentity,
    content_query: &CreatorContentQuery,
) -> AppResult<CreatorAppState> {
    let creator_id = identity.require_creator_scope()?;
    let pool = &state.pool;
    let (
        profile,
        broadcasts,
        notifications,
        settings,
        content_summary,
        filtered_uploads,
        upload_operations_summary,
        active_session,
    ) = tokio::try_join!(
        fetch_creator_profile(pool, creator_id),
        fetch_broadcasts_for_app_state(pool, creator_id),
        fetch_notifications_rows_limited(
            pool,
            creator_id,
            Some(CREATOR_APP_STATE_DASHBOARD_NOTIFICATIONS_LIMIT),
        ),
        fetch_creator_live_settings(pool, creator_id),
        fetch_creator_content_summary(pool, creator_id, content_query),
        fetch_filtered_uploads_unreconciled(pool, creator_id, content_query, Some(CREATOR_APP_STATE_UPLOADS_LIMIT)),
        fetch_creator_upload_operations_summary(pool, creator_id),
        fetch_active_live_ingest_session_unreconciled(pool, creator_id),
    )?;
    let operational_state = fetch_creator_operational_state(pool, &profile).await?;
    let dashboard = creator_dashboard_payload_for_app_state_from_parts(
        profile.clone(),
        operational_state,
        &broadcasts,
        notifications,
    );
    let snapshot = build_creator_live_snapshot_for_app_state_from_parts(
        profile,
        &broadcasts,
        active_session,
    );
    let health = fetch_creator_live_health_for_app_state(pool, creator_id).await?;
    let collaboration =
        fetch_creator_live_collaboration_summary_for_app_state(pool, creator_id, &snapshot).await?;
    let current_viewers = snapshot
        .ingest_session
        .as_ref()
        .map(|session| session.viewers)
        .unwrap_or(0);
    let mut live_control = CreatorLiveControlResponse {
        snapshot: snapshot.clone(),
        settings,
        health: health.clone(),
        collaboration: collaboration.clone(),
        subscriber_tiers: Vec::new(),
        is_live: snapshot.current_broadcast.is_some(),
        current_viewers,
        bitrate_history: Vec::new(),
        viewer_history: Vec::new(),
    };
    trim_creator_live_control_for_app_state(&mut live_control);
    let active_session = snapshot.ingest_session.clone();
    let active_runtime_output = if let Some(session) = active_session.as_ref() {
        fetch_live_runtime_output_for_session(pool, &session.id).await?
    } else {
        None
    };
    let telemetry_summary = empty_live_runtime_telemetry_summary();
    let runtime_advisory = build_live_runtime_advisory(
        active_session.as_ref(),
        active_runtime_output.as_ref(),
        Some(&telemetry_summary),
    );
    let artifact_health = match (active_session.as_ref(), active_runtime_output.as_ref()) {
        (Some(session), Some(output)) => {
            Some(describe_declared_live_runtime_artifact_health(session, output))
        }
        _ => None,
    };
    let mut live_runtime = CreatorLiveRuntimeResponse {
        snapshot,
        health,
        collaboration,
        active_session,
        active_runtime_output,
        active_runtime_targets: Vec::new(),
        telemetry_summary,
        runtime_advisory,
        artifact_health,
        recent_sessions: Vec::new(),
        recent_runtime_outputs: Vec::new(),
        recent_runtime_targets: Vec::new(),
        recent_telemetry: Vec::new(),
        recent_events: Vec::new(),
    };
    trim_creator_live_runtime_for_app_state(&mut live_runtime);
    let content = CreatorContentResponse {
        summary: content_summary,
        uploads: filtered_uploads,
    };
    let upload_operations = CreatorUploadOperationsResponse {
        summary: upload_operations_summary,
        records: Vec::new(),
    };

    Ok(CreatorAppState {
        dashboard,
        live_control,
        live_runtime,
        content,
        upload_operations,
    })
}
