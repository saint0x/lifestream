use super::*;
use crate::models::{
    LiveSourceProbe, LiveSourceProbeInput, LiveSourceValidationIssue, LiveSourceValidationReport,
};

pub(super) fn merge_source_probe(
    current: Option<&LiveSourceProbe>,
    incoming: Option<&LiveSourceProbeInput>,
    probed_at: &str,
) -> AppResult<Option<LiveSourceProbe>> {
    let Some(incoming) = incoming else {
        return Ok(current.cloned());
    };
    if incoming.is_empty() {
        return Ok(current.cloned());
    }

    let container_format = normalize_optional_text(incoming.container_format.as_deref());
    let video_codec = normalize_optional_text(incoming.video_codec.as_deref());
    let audio_codec = normalize_optional_text(incoming.audio_codec.as_deref());
    let width = normalize_optional_positive_i64("sourceProbe.width", incoming.width)?;
    let height = normalize_optional_positive_i64("sourceProbe.height", incoming.height)?;
    let frame_rate = normalize_optional_positive_f64("sourceProbe.frameRate", incoming.frame_rate)?;
    let audio_sample_rate_hz = normalize_optional_positive_i64(
        "sourceProbe.audioSampleRateHz",
        incoming.audio_sample_rate_hz,
    )?;
    let audio_channels =
        normalize_optional_positive_i64("sourceProbe.audioChannels", incoming.audio_channels)?;

    if container_format.is_none()
        && video_codec.is_none()
        && audio_codec.is_none()
        && width.is_none()
        && height.is_none()
        && frame_rate.is_none()
        && audio_sample_rate_hz.is_none()
        && audio_channels.is_none()
    {
        return Ok(current.cloned());
    }

    Ok(Some(LiveSourceProbe {
        container_format,
        video_codec,
        audio_codec,
        width,
        height,
        frame_rate,
        audio_sample_rate_hz,
        audio_channels,
        probed_at: probed_at.to_string(),
    }))
}

pub(super) fn determine_contribution_state(
    session: &LiveIngestSession,
    input: &IngestHeartbeatRequest,
    has_source_probe: bool,
    validation: Option<&LiveSourceValidationReport>,
) -> String {
    if session.status != "connected" {
        return session.contribution_state.clone();
    }
    if input.bitrate_kbps == 0
        || input.ingest_latency_ms.unwrap_or(0) >= 12_000
        || input.dropped_frames >= 120
    {
        return "degraded".to_string();
    }
    if validation.is_some_and(|report| report.state == "unsupported") {
        return "degraded".to_string();
    }
    if has_source_probe {
        return "healthy".to_string();
    }
    "attached".to_string()
}

pub(super) fn assess_source_validation(
    source_probe: Option<&LiveSourceProbe>,
    validated_at: &str,
) -> Option<LiveSourceValidationReport> {
    let source_probe = source_probe?;
    let mut issues = Vec::new();

    validate_allowed_text(
        &mut issues,
        "container_format",
        source_probe.container_format.as_deref(),
        &["mpegts", "flv"],
        "unsupported source container format",
        false,
    );
    validate_allowed_text(
        &mut issues,
        "video_codec",
        source_probe.video_codec.as_deref(),
        &["h264", "hevc", "av1"],
        "unsupported source video codec",
        false,
    );
    validate_allowed_text(
        &mut issues,
        "audio_codec",
        source_probe.audio_codec.as_deref(),
        &["aac", "opus", "mp3"],
        "unsupported source audio codec",
        false,
    );

    if source_probe.width.is_none() || source_probe.height.is_none() {
        issues.push(LiveSourceValidationIssue {
            code: "missing_dimensions".to_string(),
            message: "source probe is missing video dimensions".to_string(),
            severity: "warn".to_string(),
            repairable: true,
        });
    }
    if let Some(frame_rate) = source_probe.frame_rate {
        if !(15.0..=60.0).contains(&frame_rate) {
            issues.push(LiveSourceValidationIssue {
                code: "frame_rate_out_of_range".to_string(),
                message: format!(
                    "source frame rate {frame_rate:.2}fps is outside the preferred live range"
                ),
                severity: "warn".to_string(),
                repairable: true,
            });
        }
    } else {
        issues.push(LiveSourceValidationIssue {
            code: "missing_frame_rate".to_string(),
            message: "source probe is missing frame rate".to_string(),
            severity: "warn".to_string(),
            repairable: true,
        });
    }
    if let Some(sample_rate) = source_probe.audio_sample_rate_hz {
        if !matches!(sample_rate, 44_100 | 48_000) {
            issues.push(LiveSourceValidationIssue {
                code: "audio_sample_rate_nonstandard".to_string(),
                message: format!(
                    "source audio sample rate {sample_rate}Hz is outside the preferred live set"
                ),
                severity: "warn".to_string(),
                repairable: true,
            });
        }
    }
    if let Some(audio_channels) = source_probe.audio_channels {
        if audio_channels > 2 {
            issues.push(LiveSourceValidationIssue {
                code: "audio_channels_excessive".to_string(),
                message: format!(
                    "source audio channel count {audio_channels} exceeds the supported stereo live path"
                ),
                severity: "warn".to_string(),
                repairable: true,
            });
        }
    }

    let state = if issues.is_empty() {
        "valid"
    } else if issues.iter().any(|issue| !issue.repairable) {
        "unsupported"
    } else {
        "repairable"
    };

    Some(LiveSourceValidationReport {
        state: state.to_string(),
        issues,
        validated_at: validated_at.to_string(),
    })
}

fn validate_allowed_text(
    issues: &mut Vec<LiveSourceValidationIssue>,
    code: &str,
    value: Option<&str>,
    allowed: &[&str],
    message_prefix: &str,
    repairable: bool,
) {
    let Some(value) = value else {
        return;
    };
    if allowed.contains(&value) {
        return;
    }
    issues.push(LiveSourceValidationIssue {
        code: code.to_string(),
        message: format!("{message_prefix}: {value}"),
        severity: if repairable { "warn" } else { "error" }.to_string(),
        repairable,
    });
}

fn normalize_optional_positive_i64(field: &str, value: Option<i64>) -> AppResult<Option<i64>> {
    match value {
        Some(value) if value <= 0 => Err(AppError::BadRequest(format!(
            "{field} must be positive when provided"
        ))),
        Some(value) => Ok(Some(value)),
        None => Ok(None),
    }
}

fn normalize_optional_positive_f64(field: &str, value: Option<f64>) -> AppResult<Option<f64>> {
    match value {
        Some(value) if !value.is_finite() || value <= 0.0 => Err(AppError::BadRequest(format!(
            "{field} must be positive when provided"
        ))),
        Some(value) => Ok(Some(value)),
        None => Ok(None),
    }
}

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

trait LiveSourceProbeInputExt {
    fn is_empty(&self) -> bool;
}

impl LiveSourceProbeInputExt for LiveSourceProbeInput {
    fn is_empty(&self) -> bool {
        self.container_format
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
            && self.video_codec.as_deref().unwrap_or("").trim().is_empty()
            && self.audio_codec.as_deref().unwrap_or("").trim().is_empty()
            && self.width.is_none()
            && self.height.is_none()
            && self.frame_rate.is_none()
            && self.audio_sample_rate_hz.is_none()
            && self.audio_channels.is_none()
    }
}
