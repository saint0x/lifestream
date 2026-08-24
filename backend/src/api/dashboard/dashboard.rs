use super::analytics::{
    empty_creator_attention_score, fetch_analytics, fetch_creator_attention_score,
    fetch_revenue_entries, fetch_top_content, fetch_traffic_sources, summarize_creator_analytics,
    summarize_creator_revenue,
};
use super::content::{
    fetch_creator_content_summary, fetch_filtered_uploads_unreconciled, fetch_uploads,
    fetch_uploads_for_database,
};
use super::*;
use crate::api::control::{
    build_live_runtime_advisory, describe_declared_live_runtime_artifact_health,
    fetch_live_runtime_output_for_session,
};
use crate::api::creator::fetch_creator_profile_persisted;
use crate::api::notifications::fetch_notifications_rows_limited;
use crate::models::{CreatorAttentionScore, LiveRuntimeTelemetrySummary};
use sqlx::postgres::PgRow;

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
        attention_score: empty_creator_attention_score(),
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
    let attention_score = fetch_creator_attention_score(pool, creator_id).await?;
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
        attention_score,
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

pub(crate) async fn creator_dashboard_payload_for_database(
    database: &crate::db::Database,
    identity: &RequestIdentity,
) -> AppResult<CreatorDashboard> {
    let creator_id = identity.require_creator_scope()?;
    if let Ok(pool) = database.try_postgres_adapter() {
        return creator_dashboard_payload_postgres(pool, database, creator_id).await;
    }
    creator_dashboard_payload(database.try_sqlite_adapter()?, identity).await
}

async fn creator_dashboard_payload_postgres(
    pool: &sqlx::PgPool,
    database: &crate::db::Database,
    creator_id: &str,
) -> AppResult<CreatorDashboard> {
    let (
        profile,
        broadcasts,
        analytics,
        traffic_sources,
        top_content,
        revenue,
        subscriber_tiers,
        notifications,
        uploads,
    ) = tokio::try_join!(
        fetch_postgres_creator_profile(pool, creator_id),
        fetch_postgres_broadcasts(pool, creator_id),
        fetch_postgres_analytics(pool, creator_id),
        fetch_postgres_traffic_sources(pool, creator_id),
        fetch_postgres_top_content(pool, creator_id),
        fetch_postgres_revenue_entries(pool, creator_id),
        fetch_postgres_creator_subscriber_tiers(pool, creator_id),
        fetch_postgres_notifications_rows_limited(
            pool,
            creator_id,
            Some(CREATOR_DASHBOARD_NOTIFICATIONS_LIMIT)
        ),
        fetch_uploads_for_database(database, creator_id),
    )?;
    let operational_state = fetch_postgres_creator_operational_state(pool, &profile).await?;
    let attention_score = fetch_postgres_creator_attention_score(pool, creator_id).await?;
    let analytics_summary = summarize_creator_analytics(&analytics);
    let revenue_summary = summarize_creator_revenue(&analytics, &revenue, &subscriber_tiers);

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
        attention_score,
        top_content,
        revenue: revenue
            .into_iter()
            .take(CREATOR_DASHBOARD_REVENUE_LIMIT)
            .collect(),
        analytics_summary,
        revenue_summary,
        subscriber_tiers,
        operational_state,
        notifications,
        uploads: uploads
            .into_iter()
            .take(CREATOR_DASHBOARD_UPLOADS_LIMIT)
            .collect(),
    })
}

async fn fetch_postgres_creator_profile(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<CreatorProfile> {
    let row = sqlx::query(
        r#"
        SELECT id, user_id, handle, display_name, avatar, banner, tagline, bio, partner_status,
               joined_at, stream_key, rtmp_url, default_category, default_tags_json,
               followers::BIGINT AS followers, subscribers::BIGINT AS subscribers,
               monthly_viewers::BIGINT AS monthly_viewers,
               total_watch_hours::BIGINT AS total_watch_hours, live_status, current_broadcast_id
        FROM creator_profiles
        WHERE id = $1
        "#,
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let subscriber_tiers = fetch_postgres_creator_subscriber_tiers(pool, creator_id).await?;
    let subscribers = subscriber_tiers
        .iter()
        .map(|tier| tier.subscriber_count)
        .sum::<i64>();
    let analytics = fetch_postgres_analytics(pool, creator_id).await?;
    let analytics_summary = summarize_creator_analytics(&analytics);
    let vod_watch_hours = sqlx::query(
        "SELECT COALESCE(SUM(watch_hours), 0)::BIGINT AS total FROM uploads WHERE creator_id = $1",
    )
    .bind(creator_id)
    .fetch_one(pool)
    .await?
    .get::<i64, _>("total");
    let total_watch_hours = row
        .get::<i64, _>("total_watch_hours")
        .max(vod_watch_hours)
        .max(analytics_summary.total_watch_minutes / 60);
    let monthly_viewers = analytics_summary
        .total_viewers
        .max(row.get("monthly_viewers"));

    Ok(CreatorProfile {
        id: row.get("id"),
        user_id: row.get("user_id"),
        handle: row.get("handle"),
        display_name: row.get("display_name"),
        avatar: row.get("avatar"),
        banner: row.get("banner"),
        tagline: row.get("tagline"),
        bio: row.get("bio"),
        partner_status: row.get("partner_status"),
        joined_at: row.get("joined_at"),
        stream_key: row.get("stream_key"),
        rtmp_url: row.get("rtmp_url"),
        default_category: row.get("default_category"),
        default_tags: from_json(row.get::<String, _>("default_tags_json"))?,
        followers: row.get("followers"),
        subscribers,
        monthly_viewers,
        total_watch_hours,
        live_status: row.get("live_status"),
        current_broadcast_id: row.get("current_broadcast_id"),
    })
}

async fn fetch_postgres_broadcasts(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<Vec<Broadcast>> {
    let rows = sqlx::query(
        r#"
        SELECT id, title, category, tags_json, status, started_at, ended_at,
               duration_sec::BIGINT AS duration_sec, peak_viewers::BIGINT AS peak_viewers,
               average_viewers::BIGINT AS average_viewers,
               chat_messages::BIGINT AS chat_messages, new_followers::BIGINT AS new_followers,
               new_subscribers::BIGINT AS new_subscribers, revenue::DOUBLE PRECISION AS revenue,
               thumbnail, (is_mature != 0) AS is_mature
        FROM broadcasts
        WHERE creator_id = $1
        ORDER BY started_at DESC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(postgres_broadcast_from_row).collect())
}

fn postgres_broadcast_from_row(row: PgRow) -> Broadcast {
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
        is_mature: row.get("is_mature"),
    }
}

async fn fetch_postgres_analytics(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<Vec<AnalyticsPoint>> {
    let rows = sqlx::query(
        r#"
        SELECT date, viewers::BIGINT AS viewers, watch_minutes::BIGINT AS watch_minutes,
               revenue::DOUBLE PRECISION AS revenue, new_followers::BIGINT AS new_followers
        FROM analytics_points
        WHERE creator_id = $1
        ORDER BY date ASC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| AnalyticsPoint {
            date: row.get("date"),
            viewers: row.get("viewers"),
            watch_minutes: row.get("watch_minutes"),
            revenue: row.get("revenue"),
            new_followers: row.get("new_followers"),
        })
        .collect())
}

async fn fetch_postgres_traffic_sources(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<Vec<TrafficSource>> {
    let rows = sqlx::query(
        r#"
        SELECT
            COALESCE(lvs.attribution_source, 'direct') AS source,
            COUNT(*)::BIGINT AS sessions
        FROM live_viewer_sessions lvs
        JOIN live_streams ls ON ls.id = lvs.stream_id
        JOIN streamers s ON s.id = ls.streamer_id
        JOIN creator_profiles cp ON cp.handle = s.handle
        WHERE cp.id = $1
        GROUP BY COALESCE(lvs.attribution_source, 'direct')
        ORDER BY sessions DESC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;
    let total_sessions = rows
        .iter()
        .map(|row| row.get::<i64, _>("sessions"))
        .sum::<i64>()
        .max(1);
    Ok(rows
        .into_iter()
        .map(|row| {
            let sessions = row.get::<i64, _>("sessions");
            TrafficSource {
                source: row.get("source"),
                sessions,
                share: sessions as f64 / total_sessions as f64,
            }
        })
        .collect())
}

async fn fetch_postgres_creator_attention_score(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<CreatorAttentionScore> {
    let day = Utc::now().date_naive().format("%Y-%m-%d").to_string();
    let row = sqlx::query(
        r#"
        SELECT algorithm_version, qualified_viewers::BIGINT AS qualified_viewers,
               verified_viewer_score::DOUBLE PRECISION AS verified_viewer_score,
               creator_attention_value::DOUBLE PRECISION AS creator_attention_value,
               baseline_value_per_qualified_viewer::DOUBLE PRECISION AS baseline_value_per_qualified_viewer,
               average_watch_minutes::DOUBLE PRECISION AS average_watch_minutes,
               attention_multiplier::DOUBLE PRECISION AS attention_multiplier,
               engagement_multiplier::DOUBLE PRECISION AS engagement_multiplier,
               retention_multiplier::DOUBLE PRECISION AS retention_multiplier,
               audience_quality_multiplier::DOUBLE PRECISION AS audience_quality_multiplier,
               data_confidence_multiplier::DOUBLE PRECISION AS data_confidence_multiplier,
               qualified_viewer_rate::DOUBLE PRECISION AS qualified_viewer_rate,
               returning_viewer_rate::DOUBLE PRECISION AS returning_viewer_rate,
               measured_sessions::BIGINT AS measured_sessions,
               measured_viewers::BIGINT AS measured_viewers
        FROM creator_attention_daily
        WHERE creator_id = $1 AND day = $2
        "#,
    )
    .bind(creator_id)
    .bind(&day)
    .fetch_optional(pool)
    .await?;
    Ok(row
        .map(postgres_creator_attention_score_from_row)
        .unwrap_or_else(empty_creator_attention_score))
}

fn postgres_creator_attention_score_from_row(row: PgRow) -> CreatorAttentionScore {
    CreatorAttentionScore {
        algorithm_version: row.get("algorithm_version"),
        qualified_viewers: row.get("qualified_viewers"),
        verified_viewer_score: row.get("verified_viewer_score"),
        creator_attention_value: row.get("creator_attention_value"),
        baseline_value_per_qualified_viewer: row.get("baseline_value_per_qualified_viewer"),
        average_watch_minutes: row.get("average_watch_minutes"),
        attention_multiplier: row.get("attention_multiplier"),
        engagement_multiplier: row.get("engagement_multiplier"),
        retention_multiplier: row.get("retention_multiplier"),
        audience_quality_multiplier: row.get("audience_quality_multiplier"),
        data_confidence_multiplier: row.get("data_confidence_multiplier"),
        qualified_viewer_rate: row.get("qualified_viewer_rate"),
        returning_viewer_rate: row.get("returning_viewer_rate"),
        measured_sessions: row.get("measured_sessions"),
        measured_viewers: row.get("measured_viewers"),
    }
}

async fn fetch_postgres_top_content(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<Vec<TopContent>> {
    let rows = sqlx::query(
        r#"
        SELECT id, title, kind, views::BIGINT AS views, watch_hours::BIGINT AS watch_hours,
               trend::DOUBLE PRECISION AS trend, thumbnail
        FROM top_content
        WHERE creator_id = $1
        ORDER BY views DESC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| TopContent {
            id: row.get("id"),
            title: row.get("title"),
            kind: row.get("kind"),
            views: row.get("views"),
            watch_hours: row.get("watch_hours"),
            trend: row.get("trend"),
            thumbnail: row.get("thumbnail"),
        })
        .collect())
}

async fn fetch_postgres_revenue_entries(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<Vec<RevenueEntry>> {
    let rows = sqlx::query(
        r#"
        SELECT id, date, source, description, amount::DOUBLE PRECISION AS amount
        FROM revenue_entries
        WHERE creator_id = $1
        ORDER BY date DESC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| RevenueEntry {
            id: row.get("id"),
            date: row.get("date"),
            source: row.get("source"),
            description: row.get("description"),
            amount: row.get("amount"),
        })
        .collect())
}

async fn fetch_postgres_creator_subscriber_tiers(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<Vec<CreatorSubscriberTier>> {
    let rows = sqlx::query(
        r#"
        SELECT id, tier_name, rank::BIGINT AS rank, monthly_price::DOUBLE PRECISION AS monthly_price,
               subscriber_count::BIGINT AS subscriber_count, accent_color, status, retired_at
        FROM creator_subscriber_tiers
        WHERE creator_id = $1
        ORDER BY CASE WHEN status = 'active' THEN 0 ELSE 1 END ASC, rank ASC, monthly_price ASC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CreatorSubscriberTier {
            id: row.get("id"),
            tier_name: row.get("tier_name"),
            rank: row.get("rank"),
            monthly_price: row.get("monthly_price"),
            subscriber_count: row.get("subscriber_count"),
            accent_color: row.get("accent_color"),
            status: row.get("status"),
            retired_at: row.get("retired_at"),
        })
        .collect())
}

async fn fetch_postgres_notifications_rows_limited(
    pool: &sqlx::PgPool,
    creator_id: &str,
    limit: Option<usize>,
) -> AppResult<Vec<CreatorNotification>> {
    let effective_limit = limit.unwrap_or(i64::MAX as usize).max(1) as i64;
    let rows = sqlx::query(
        r#"
        SELECT id, kind, body, sent_at, amount::DOUBLE PRECISION AS amount, actor,
               NULL::TEXT AS delivery_state, NULL::TEXT AS read_at
        FROM creator_notifications
        WHERE creator_id = $1
        ORDER BY sent_at DESC
        LIMIT $2
        "#,
    )
    .bind(creator_id)
    .bind(effective_limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CreatorNotification {
            id: row.get("id"),
            kind: row.get("kind"),
            body: row.get("body"),
            sent_at: row.get("sent_at"),
            amount: row.get("amount"),
            actor: row.get("actor"),
            delivery_state: row.get("delivery_state"),
            read_at: row.get("read_at"),
        })
        .collect())
}

async fn fetch_postgres_creator_operational_state(
    pool: &sqlx::PgPool,
    profile: &CreatorProfile,
) -> AppResult<CreatorOperationalState> {
    expire_postgres_creator_enforcement_actions(pool, Some(&profile.id), None).await?;
    let row = sqlx::query(
        r#"
        SELECT legal_name, support_email, business_type, payout_country, payout_provider,
               onboarding_status, identity_status, tax_status, payout_status, hold_reasons_json,
               created_at, updated_at, last_reviewed_at
        FROM creator_operational_state
        WHERE creator_id = $1
        "#,
    )
    .bind(&profile.id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let legal_name: String = row.get("legal_name");
    let support_email: String = row.get("support_email");
    let business_type: String = row.get("business_type");
    let payout_country: String = row.get("payout_country");
    let payout_provider: String = row.get("payout_provider");
    let onboarding_status: String = row.get("onboarding_status");
    let identity_status: String = row.get("identity_status");
    let tax_status: String = row.get("tax_status");
    let payout_status: String = row.get("payout_status");
    let hold_reasons: Vec<String> = from_json(row.get::<String, _>("hold_reasons_json"))?;
    let active_enforcement_actions =
        fetch_postgres_active_creator_enforcement_actions(pool, &profile.id).await?;
    let live_streaming_enabled = !active_enforcement_actions
        .iter()
        .any(|action| action.scope == "live_streaming");
    let upload_ingest_enabled = !active_enforcement_actions
        .iter()
        .any(|action| action.scope == "uploads");
    let collaboration_enabled = !active_enforcement_actions
        .iter()
        .any(|action| action.scope == "collaboration");
    let monetization_enabled = !active_enforcement_actions
        .iter()
        .any(|action| action.scope == "monetization");
    let payouts_enabled = !active_enforcement_actions
        .iter()
        .any(|action| action.scope == "payouts");

    let profile_complete = !legal_name.trim().is_empty()
        && !support_email.trim().is_empty()
        && support_email.contains('@')
        && !business_type.trim().is_empty()
        && !payout_country.trim().is_empty()
        && !payout_provider.trim().is_empty();
    let onboarding_complete = onboarding_status == "approved";
    let identity_verified = identity_status == "verified";
    let tax_verified = tax_status == "verified";
    let payout_ready = payout_status == "active";
    let holds_clear = hold_reasons.is_empty();
    let can_monetize =
        onboarding_complete && identity_verified && tax_verified && monetization_enabled;
    let can_receive_payouts = can_monetize && payout_ready && holds_clear && payouts_enabled;

    let checklist = vec![
        CreatorOperationalChecklistItem {
            key: "profileComplete".to_string(),
            label: "Profile complete".to_string(),
            complete: profile_complete,
            detail: if profile_complete {
                "Legal and support contact details are present.".to_string()
            } else {
                "Complete legal name, support email, payout country, provider, and business type."
                    .to_string()
            },
        },
        CreatorOperationalChecklistItem {
            key: "onboardingApproved".to_string(),
            label: "Onboarding approved".to_string(),
            complete: onboarding_complete,
            detail: format!("Current onboarding status: {onboarding_status}."),
        },
        CreatorOperationalChecklistItem {
            key: "identityVerified".to_string(),
            label: "Identity verified".to_string(),
            complete: identity_verified,
            detail: format!("Current identity status: {identity_status}."),
        },
        CreatorOperationalChecklistItem {
            key: "taxProfileReady".to_string(),
            label: "Tax profile ready".to_string(),
            complete: tax_verified,
            detail: format!("Current tax status: {tax_status}."),
        },
        CreatorOperationalChecklistItem {
            key: "payoutMethodReady".to_string(),
            label: "Payout method active".to_string(),
            complete: payout_ready,
            detail: format!("Current payout status: {payout_status}."),
        },
        CreatorOperationalChecklistItem {
            key: "holdsClear".to_string(),
            label: "No active payout holds".to_string(),
            complete: holds_clear,
            detail: if holds_clear {
                "No manual or compliance holds are blocking monetization.".to_string()
            } else {
                format!("Active holds: {}.", hold_reasons.join(", "))
            },
        },
        CreatorOperationalChecklistItem {
            key: "enforcementClear".to_string(),
            label: "No active creator enforcement".to_string(),
            complete: active_enforcement_actions.is_empty(),
            detail: if active_enforcement_actions.is_empty() {
                "No operator-enforced restrictions are active.".to_string()
            } else {
                format!(
                    "Active enforcement scopes: {}.",
                    active_enforcement_actions
                        .iter()
                        .map(|action| action.scope.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
        },
    ];

    Ok(CreatorOperationalState {
        creator_id: profile.id.clone(),
        legal_name,
        support_email,
        business_type,
        payout_country,
        payout_provider,
        onboarding_status,
        identity_status,
        tax_status,
        payout_status,
        hold_reasons,
        active_enforcement_actions,
        live_streaming_enabled,
        upload_ingest_enabled,
        collaboration_enabled,
        monetization_enabled,
        payouts_enabled,
        can_receive_payouts,
        can_monetize,
        can_publish_paid_content: can_monetize,
        requires_action: !can_receive_payouts || !live_streaming_enabled || !upload_ingest_enabled,
        checklist,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        last_reviewed_at: row.get("last_reviewed_at"),
    })
}

async fn expire_postgres_creator_enforcement_actions(
    pool: &sqlx::PgPool,
    creator_id: Option<&str>,
    action_id: Option<&str>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "UPDATE creator_enforcement_actions SET state = 'expired', released_at = COALESCE(released_at, ",
    );
    builder.push_bind(now.clone());
    builder.push(") WHERE state = 'active' AND expires_at IS NOT NULL AND expires_at <= ");
    builder.push_bind(now);
    if let Some(creator_id) = creator_id {
        builder.push(" AND creator_id = ");
        builder.push_bind(creator_id);
    }
    if let Some(action_id) = action_id {
        builder.push(" AND id = ");
        builder.push_bind(action_id);
    }
    builder.build().execute(pool).await?;
    Ok(())
}

async fn fetch_postgres_active_creator_enforcement_actions(
    pool: &sqlx::PgPool,
    creator_id: &str,
) -> AppResult<Vec<CreatorEnforcementAction>> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"
        SELECT id, creator_id, scope, state, reason, resolution_note, created_by_user_id,
               released_by_user_id, created_at, released_at, expires_at
        FROM creator_enforcement_actions
        WHERE creator_id = $1
          AND state = 'active'
          AND (expires_at IS NULL OR expires_at > $2)
        ORDER BY created_at DESC
        "#,
    )
    .bind(creator_id)
    .bind(&now)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CreatorEnforcementAction {
            id: row.get("id"),
            creator_id: row.get("creator_id"),
            scope: row.get("scope"),
            state: row.get("state"),
            reason: row.get("reason"),
            resolution_note: row.get("resolution_note"),
            created_by_user_id: row.get("created_by_user_id"),
            released_by_user_id: row.get("released_by_user_id"),
            created_at: row.get("created_at"),
            released_at: row.get("released_at"),
            expires_at: row.get("expires_at"),
        })
        .collect())
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
    let pool = state.db.try_sqlite_adapter()?;
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
        fetch_filtered_uploads_unreconciled(
            pool,
            creator_id,
            content_query,
            Some(CREATOR_APP_STATE_UPLOADS_LIMIT)
        ),
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
    let snapshot =
        build_creator_live_snapshot_for_app_state_from_parts(profile, &broadcasts, active_session);
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
        (Some(session), Some(output)) => Some(describe_declared_live_runtime_artifact_health(
            session, output,
        )),
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
