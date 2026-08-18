use super::*;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::control::artifacts) struct LiveRuntimeVariantSpec {
    pub(in crate::api::control::artifacts) label: String,
    pub(in crate::api::control::artifacts) width: i64,
    pub(in crate::api::control::artifacts) height: i64,
    pub(in crate::api::control::artifacts) video_bitrate_bps: i64,
    pub(in crate::api::control::artifacts) bandwidth_bps: i64,
    pub(in crate::api::control::artifacts) output_relative_dir: String,
    pub(in crate::api::control::artifacts) relative_playlist_path: String,
    pub(in crate::api::control::artifacts) segment_relative_pattern: String,
}

pub(in crate::api::control::artifacts) fn build_live_runtime_variant_specs(
    session: &LiveIngestSession,
    output: &LiveRuntimeOutput,
) -> AppResult<Vec<LiveRuntimeVariantSpec>> {
    let Some(source_probe) = session.source_probe.as_ref() else {
        return Ok(Vec::new());
    };
    let Some(width) = source_probe.width else {
        return Ok(Vec::new());
    };
    let Some(height) = source_probe.height else {
        return Ok(Vec::new());
    };

    let probed = ProbedMedia {
        container_format: source_probe.container_format.clone(),
        duration_sec: 0.0,
        width: Some(width),
        height: Some(height),
        frame_rate: source_probe.frame_rate,
        video_codec: source_probe.video_codec.clone(),
        audio_codec: source_probe.audio_codec.clone(),
        audio_sample_rate_hz: source_probe.audio_sample_rate_hz,
        audio_channels: source_probe.audio_channels,
        has_video: true,
        has_audio: source_probe.audio_codec.is_some(),
        bitrate_bps: session.bitrate_kbps.checked_mul(1000),
        audio_streams: if source_probe.audio_codec.is_some() {
            vec![ProbedAudioStream {
                stream_index: 1,
                codec: source_probe.audio_codec.clone(),
                language: Some("und".to_string()),
                sample_rate_hz: source_probe.audio_sample_rate_hz,
                channels: source_probe.audio_channels,
            }]
        } else {
            Vec::new()
        },
        subtitle_streams: Vec::new(),
    };

    let refined = refine_live_variant_plans(
        plan_hls_variants(&probed)?,
        session,
        output.runtime_class.as_str(),
        output.ladder_policy.as_str(),
    );
    Ok(refined
        .into_iter()
        .map(|plan| live_runtime_variant_spec_from_plan(session, plan, &output.segment_format))
        .collect())
}

fn refine_live_variant_plans(
    mut plans: Vec<HlsVariantPlan>,
    session: &LiveIngestSession,
    runtime_class: &str,
    ladder_policy: &str,
) -> Vec<HlsVariantPlan> {
    let handheld_profile = runtime_class == "ll_hls";
    if handheld_profile {
        plans.retain(|plan| plan.height <= 720);
    }
    if ladder_policy.contains("general_sd") {
        plans.retain(|plan| plan.height <= 480);
    }
    if ladder_policy.contains("cinematic") {
        plans.retain(|plan| plan.height >= 360);
    }
    if plans.is_empty() {
        return plans;
    }

    let source_bitrate_bps = session.bitrate_kbps.saturating_mul(1000);
    let device_multiplier = if handheld_profile { 0.82 } else { 1.0 };

    for plan in &mut plans {
        let content_multiplier = if ladder_policy.contains("high_motion") {
            match plan.height {
                0..=240 => 0.90,
                241..=360 => 0.96,
                361..=480 => 1.00,
                481..=720 => 1.10,
                _ => 1.18,
            }
        } else if ladder_policy.contains("cinematic") {
            match plan.height {
                0..=360 => 0.82,
                361..=480 => 0.90,
                481..=720 => 1.00,
                _ => 1.08,
            }
        } else if ladder_policy.contains("general_sd") {
            match plan.height {
                0..=240 => 0.72,
                241..=360 => 0.82,
                _ => 0.90,
            }
        } else {
            match plan.height {
                0..=240 => 0.76,
                241..=360 => 0.86,
                361..=480 => 0.94,
                481..=720 => 1.00,
                _ => 1.04,
            }
        };

        let tuned_video_bitrate =
            ((plan.video_bitrate_bps as f64) * content_multiplier * device_multiplier).round()
                as i64;
        let bounded_video_bitrate = if source_bitrate_bps > 0 {
            tuned_video_bitrate.min((source_bitrate_bps as f64 * 0.92).round() as i64)
        } else {
            tuned_video_bitrate
        };
        let floor_video_bitrate = match plan.height {
            0..=240 => 350_000,
            241..=360 => 600_000,
            361..=480 => 1_000_000,
            481..=720 => 2_200_000,
            _ => 3_500_000,
        };
        plan.video_bitrate_bps = bounded_video_bitrate.max(floor_video_bitrate);
        let audio_bitrate_bps = match plan.height {
            0..=360 => 96_000,
            361..=720 => 128_000,
            _ => 192_000,
        };
        plan.bandwidth_bps = plan.video_bitrate_bps + audio_bitrate_bps;
    }

    plans
}

fn live_runtime_variant_spec_from_plan(
    session: &LiveIngestSession,
    plan: HlsVariantPlan,
    segment_format: &str,
) -> LiveRuntimeVariantSpec {
    let segment_extension = if segment_format == "fmp4" { "m4s" } else { "ts" };
    LiveRuntimeVariantSpec {
        label: plan.label.clone(),
        width: plan.width,
        height: plan.height,
        video_bitrate_bps: plan.video_bitrate_bps,
        bandwidth_bps: plan.bandwidth_bps,
        output_relative_dir: format!(
            "live/{}/{}/{}/{}",
            session.creator_id, session.broadcast_id, session.id, plan.label
        ),
        relative_playlist_path: format!(
            "live/{}/{}/{}/{}/playlist.m3u8",
            session.creator_id, session.broadcast_id, session.id, plan.label
        ),
        segment_relative_pattern: format!(
            "live/{}/{}/{}/{}/segment_%03d.{}",
            session.creator_id, session.broadcast_id, session.id, plan.label, segment_extension
        ),
    }
}
