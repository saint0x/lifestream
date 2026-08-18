use super::*;
use crate::api::creator::fetch_creator_live_settings;
use crate::models::LiveSourceProbe;

#[derive(Clone, Debug)]
pub(super) struct LiveRuntimeProfile {
    pub runtime_class: String,
    pub latency_profile: String,
    pub segment_format: String,
    pub partial_segments_enabled: bool,
    pub blocking_reload_enabled: bool,
    pub target_segment_duration_sec: i64,
    pub hold_back_segments: i64,
    pub discontinuity_sequence: i64,
    pub ladder_policy: String,
    pub content_class: String,
}

pub(super) async fn derive_live_runtime_profile(
    pool: &SqlitePool,
    session: &LiveIngestSession,
) -> AppResult<LiveRuntimeProfile> {
    let settings = fetch_creator_live_settings(pool, &session.creator_id).await?;
    let runtime_class = settings.delivery_class;
    let is_low_latency = runtime_class == "ll_hls";
    let session_ordinal =
        count_live_ingest_sessions_for_broadcast(pool, &session.creator_id, &session.broadcast_id)
            .await?;
    let (content_class, ladder_policy) =
        derive_content_class_and_ladder_policy(session.source_probe.as_ref());

    Ok(LiveRuntimeProfile {
        runtime_class,
        latency_profile: if is_low_latency {
            "low".to_string()
        } else {
            "standard".to_string()
        },
        segment_format: if is_low_latency {
            "fmp4".to_string()
        } else {
            "mpegts".to_string()
        },
        partial_segments_enabled: is_low_latency,
        blocking_reload_enabled: is_low_latency,
        target_segment_duration_sec: if is_low_latency { 2 } else { 6 },
        hold_back_segments: if is_low_latency { 2 } else { 3 },
        discontinuity_sequence: session_ordinal.saturating_sub(1),
        ladder_policy,
        content_class,
    })
}

fn derive_content_class_and_ladder_policy(
    source_probe: Option<&LiveSourceProbe>,
) -> (String, String) {
    let Some(source_probe) = source_probe else {
        return ("unknown".to_string(), "awaiting_probe".to_string());
    };

    let height = source_probe.height.unwrap_or(0);
    let frame_rate = source_probe.frame_rate.unwrap_or(0.0);

    if height >= 1080 && frame_rate >= 50.0 {
        return (
            "high_motion".to_string(),
            "probe_high_motion_1080p".to_string(),
        );
    }
    if height >= 1080 {
        return (
            "cinematic".to_string(),
            "probe_cinematic_1080p".to_string(),
        );
    }
    if height >= 720 {
        return ("general_hd".to_string(), "probe_general_hd".to_string());
    }
    if height > 0 {
        return ("general_sd".to_string(), "probe_general_sd".to_string());
    }

    ("unknown".to_string(), "awaiting_probe".to_string())
}
