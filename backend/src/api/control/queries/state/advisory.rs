use super::*;
use crate::models::{
    CollaborationRuntimeTopology, LiveRuntimeAdvisory, LiveRuntimeAdvisoryAction,
    LiveRuntimeTelemetrySummary, LiveSourceValidationIssue,
};

pub(crate) fn build_live_runtime_advisory(
    session: Option<&LiveIngestSession>,
    output: Option<&LiveRuntimeOutput>,
    telemetry: Option<&LiveRuntimeTelemetrySummary>,
) -> LiveRuntimeAdvisory {
    let Some(session) = session else {
        return LiveRuntimeAdvisory {
            status: "idle".to_string(),
            summary: "No active ingest session is attached to the live runtime.".to_string(),
            requires_operator_action: false,
            blocking_issue_count: 0,
            repairable_issue_count: 0,
            source_validation_state: None,
            runtime_failure_present: false,
            recommended_actions: Vec::new(),
        };
    };

    let mut actions = Vec::new();
    if let Some(report) = session.source_validation.as_ref() {
        for issue in &report.issues {
            push_validation_action(&mut actions, issue);
        }
    } else if session.status == "connected" {
        actions.push(LiveRuntimeAdvisoryAction {
            code: "source_probe_missing".to_string(),
            severity: "warn".to_string(),
            repairable: true,
            title: "Source probe missing".to_string(),
            detail: "The ingest session is connected but has not yet reported normalized source characteristics.".to_string(),
        });
    }

    if let Some(output) = output {
        if matches!(
            output.runtime_state.as_str(),
            "failed" | "packaging_degraded"
        ) || output.packaging_status == "failed"
        {
            actions.push(LiveRuntimeAdvisoryAction {
                code: "runtime_packaging_attention".to_string(),
                severity: if output.packaging_status == "failed" || output.runtime_state == "failed"
                {
                    "error".to_string()
                } else {
                    "warn".to_string()
                },
                repairable: true,
                title: "Packaging path needs attention".to_string(),
                detail: output.last_error.clone().unwrap_or_else(|| {
                    "The runtime reported degraded or failed packaging state.".to_string()
                }),
            });
        }
        if output.archive_status == "failed" {
            actions.push(LiveRuntimeAdvisoryAction {
                code: "archive_failed".to_string(),
                severity: "error".to_string(),
                repairable: true,
                title: "Archive finalization failed".to_string(),
                detail: output.last_error.clone().unwrap_or_else(|| {
                    "The archive output failed and needs operator repair.".to_string()
                }),
            });
        }
        if matches!(output.packaging_status.as_str(), "ready" | "complete")
            && output.manifest_relative_path.is_none()
        {
            actions.push(LiveRuntimeAdvisoryAction {
                code: "manifest_missing".to_string(),
                severity: "error".to_string(),
                repairable: true,
                title: "Manifest path missing".to_string(),
                detail: "Packaging is marked ready but no manifest path is persisted.".to_string(),
            });
        }
    }

    if session.status == "stale" {
        actions.push(LiveRuntimeAdvisoryAction {
            code: "session_stale".to_string(),
            severity: "warn".to_string(),
            repairable: true,
            title: "Ingest heartbeat is stale".to_string(),
            detail:
                "The ingest session has stopped heartbeating and should reconnect or be terminated."
                    .to_string(),
        });
    }

    let runtime_failure_present = output.is_some_and(|item| {
        item.runtime_state == "failed"
            || item.packaging_status == "failed"
            || item.archive_status == "failed"
    }) || telemetry.is_some_and(|item| item.failure_samples > 0);

    finalize_advisory(session, runtime_failure_present, actions, None, telemetry)
}

pub(crate) fn apply_collaboration_transport_gap(
    session: &LiveIngestSession,
    mut advisory: LiveRuntimeAdvisory,
    transport_gap_present: bool,
) -> LiveRuntimeAdvisory {
    if !transport_gap_present
        || advisory
            .recommended_actions
            .iter()
            .any(|action| action.code == "collaboration_transport_gap")
    {
        return advisory;
    }

    advisory.recommended_actions.push(LiveRuntimeAdvisoryAction {
        code: "collaboration_transport_gap".to_string(),
        severity: "error".to_string(),
        repairable: false,
        title: "Collaboration return transport is not executable".to_string(),
        detail: "This collaboration session requires participant return audio over collaboration socket transport, but no concrete media transport endpoint is declared for executable guest audio ingress or egress.".to_string(),
    });

    finalize_advisory(
        session,
        advisory.runtime_failure_present,
        advisory.recommended_actions,
        advisory.source_validation_state,
        None,
    )
}

pub(crate) fn collaboration_transport_gap_from_topology(
    topology: &CollaborationRuntimeTopology,
) -> bool {
    topology.audio.iter().any(|route| {
        route.receive_program_audio
            && route.mix_minus_required
            && topology.contributions.iter().any(|contribution| {
                contribution.participant_id == route.participant_id
                    && contribution.transport_class == "collaboration_socket"
                    && (contribution.media_transport.as_deref().is_none()
                        || contribution
                            .contribution_endpoint_url
                            .as_deref()
                            .is_none_or(str::is_empty)
                        || contribution
                            .return_endpoint_url
                            .as_deref()
                            .is_none_or(str::is_empty))
            })
    })
}

fn finalize_advisory(
    session: &LiveIngestSession,
    runtime_failure_present: bool,
    actions: Vec<LiveRuntimeAdvisoryAction>,
    source_validation_state: Option<String>,
    telemetry: Option<&LiveRuntimeTelemetrySummary>,
) -> LiveRuntimeAdvisory {
    let blocking_issue_count = actions
        .iter()
        .filter(|action| action.severity == "error")
        .count() as i64;
    let repairable_issue_count = actions.iter().filter(|action| action.repairable).count() as i64;
    let runtime_failure_present =
        runtime_failure_present || telemetry.is_some_and(|item| item.failure_samples > 0);
    let status = if runtime_failure_present || blocking_issue_count > 0 {
        "critical"
    } else if repairable_issue_count > 0 {
        "repairable"
    } else if session.status != "connected" {
        "observe"
    } else {
        "healthy"
    };

    let summary = match status {
        "critical" => {
            if let Some(validation) = session.source_validation.as_ref() {
                if validation.state == "unsupported" {
                    "The current source contribution is unsupported and must be corrected before stable delivery.".to_string()
                } else {
                    "The live runtime has blocking issues that require operator action.".to_string()
                }
            } else {
                "The live runtime has blocking issues that require operator action.".to_string()
            }
        }
        "repairable" => {
            "The live runtime is serviceable but has repairable ingest or packaging issues."
                .to_string()
        }
        "observe" => "The live runtime is not fully healthy and should be observed.".to_string(),
        _ => {
            "The live runtime is healthy and no operator repair is currently required.".to_string()
        }
    };

    LiveRuntimeAdvisory {
        status: status.to_string(),
        summary,
        requires_operator_action: blocking_issue_count > 0 || runtime_failure_present,
        blocking_issue_count,
        repairable_issue_count,
        source_validation_state: source_validation_state.or_else(|| {
            session
                .source_validation
                .as_ref()
                .map(|report| report.state.clone())
        }),
        runtime_failure_present,
        recommended_actions: actions,
    }
}

fn push_validation_action(
    actions: &mut Vec<LiveRuntimeAdvisoryAction>,
    issue: &LiveSourceValidationIssue,
) {
    let (title, detail): (String, String) = match issue.code.as_str() {
        "container_format" => (
            "Unsupported source container".to_string(),
            "Switch the live contribution container to an allowed ingest transport such as mpegts or flv."
                .to_string(),
        ),
        "video_codec" => (
            "Unsupported video codec".to_string(),
            "Reconfigure the live encoder to output h264, hevc, or av1 for this ingest path."
                .to_string(),
        ),
        "audio_codec" => (
            "Unsupported audio codec".to_string(),
            "Reconfigure the live encoder to output aac, opus, or mp3 audio.".to_string(),
        ),
        "missing_dimensions" => (
            "Missing video dimensions".to_string(),
            "Ensure the contribution encoder reports explicit width and height in probe data."
                .to_string(),
        ),
        "missing_frame_rate" => (
            "Missing frame rate".to_string(),
            "Ensure the contribution encoder reports an explicit frame rate in probe data."
                .to_string(),
        ),
        "frame_rate_out_of_range" => (
            "Frame rate outside preferred range".to_string(),
            "Adjust the contribution frame rate into the preferred 15-60fps live range.".to_string(),
        ),
        "audio_sample_rate_nonstandard" => (
            "Nonstandard audio sample rate".to_string(),
            "Adjust the contribution audio sample rate to 44100Hz or 48000Hz.".to_string(),
        ),
        "audio_channels_excessive" => (
            "Unsupported multichannel audio".to_string(),
            "Downmix the contribution audio path to stereo for the primary live ingest pipeline."
                .to_string(),
        ),
        _ => ("Source validation issue".to_string(), issue.message.clone()),
    };

    actions.push(LiveRuntimeAdvisoryAction {
        code: issue.code.clone(),
        severity: issue.severity.clone(),
        repairable: issue.repairable,
        title,
        detail,
    });
}
