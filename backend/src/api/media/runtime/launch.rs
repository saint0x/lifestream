use super::*;
use crate::api::control::{collaboration_launch_relative_path, collaboration_route_relative_path};
use crate::models::{
    CollaborationContributionAttachment, CollaborationMediaLaunchArtifactOutput,
    CollaborationMediaLaunchInput, CollaborationMediaLaunchReturn, CollaborationMediaLaunchRuntime,
    CollaborationMediaLaunchStep, CollaborationMediaRuntime, CollaborationOutputRoute,
};

pub(crate) fn build_collaboration_media_launch_runtime(
    session: &LiveIngestSession,
    contributions: &[CollaborationContributionAttachment],
    outputs: &[CollaborationOutputRoute],
    mix_minus_required: bool,
    media_runtime: &CollaborationMediaRuntime,
) -> AppResult<CollaborationMediaLaunchRuntime> {
    let mut unresolved_reasons = Vec::new();
    let mut unresolved_participant_ids = Vec::new();

    let inputs = contributions
        .iter()
        .enumerate()
        .filter_map(|(index, contribution)| {
            let Some(media_transport) = contribution.media_transport.clone() else {
                if contribution.transport_class == "collaboration_socket" {
                    unresolved_participant_ids.push(contribution.participant_id.clone());
                    unresolved_reasons.push(format!(
                        "participant {} missing media transport declaration",
                        contribution.participant_id
                    ));
                }
                return None;
            };
            let Some(input_url) = contribution.contribution_endpoint_url.clone() else {
                unresolved_participant_ids.push(contribution.participant_id.clone());
                unresolved_reasons.push(format!(
                    "participant {} missing contribution endpoint URL",
                    contribution.participant_id
                ));
                return None;
            };
            Some(CollaborationMediaLaunchInput {
                participant_id: contribution.participant_id.clone(),
                user_id: contribution.user_id.clone(),
                creator_id: contribution.creator_id.clone(),
                media_transport,
                input_url,
                input_index: index as i64,
                mix_minus_required: contribution.mix_minus_required,
            })
        })
        .collect::<Vec<_>>();

    let artifact_outputs = outputs
        .iter()
        .filter_map(|output| {
            collaboration_route_relative_path(session, output).map(|relative_path| {
                CollaborationMediaLaunchArtifactOutput {
                    output_id: output.id.clone(),
                    output_kind: output.output_kind.clone(),
                    relative_path,
                    route_state: output.route_state.clone(),
                    source_participant_ids: output.source_participant_ids.clone(),
                    playback_enabled: output.playback_enabled,
                    recording_enabled: output.recording_enabled,
                }
            })
        })
        .collect::<Vec<_>>();

    let returns = media_runtime
        .return_targets
        .iter()
        .filter_map(|route| {
            let Some(contribution) = contributions
                .iter()
                .find(|item| item.participant_id == route.participant_id)
            else {
                unresolved_participant_ids.push(route.participant_id.clone());
                unresolved_reasons.push(format!(
                    "return route {} missing contribution declaration",
                    route.participant_id
                ));
                return None;
            };
            let Some(media_transport) = contribution.media_transport.clone() else {
                unresolved_participant_ids.push(route.participant_id.clone());
                unresolved_reasons.push(format!(
                    "return route {} missing media transport declaration",
                    route.participant_id
                ));
                return None;
            };
            let Some(output_url) = contribution.return_endpoint_url.clone() else {
                unresolved_participant_ids.push(route.participant_id.clone());
                unresolved_reasons.push(format!(
                    "return route {} missing return endpoint URL",
                    route.participant_id
                ));
                return None;
            };
            let source_participant_ids = contributions
                .iter()
                .map(|item| item.participant_id.clone())
                .filter(|participant_id| !route.excluded_participant_ids.contains(participant_id))
                .collect::<Vec<_>>();
            Some(CollaborationMediaLaunchReturn {
                participant_id: route.participant_id.clone(),
                media_transport,
                output_url,
                source_participant_ids,
                mix_minus_required: route.mix_minus_required,
            })
        })
        .collect::<Vec<_>>();

    unresolved_participant_ids.sort();
    unresolved_participant_ids.dedup();
    unresolved_reasons.sort();
    unresolved_reasons.dedup();

    let mut steps = Vec::new();
    if !inputs.is_empty() {
        steps.push(build_ffmpeg_launch_step(
            session,
            mix_minus_required,
            &inputs,
            &returns,
            &artifact_outputs,
        )?);
    }

    Ok(CollaborationMediaLaunchRuntime {
        launch_mode: "ffmpeg_plan_v1".to_string(),
        worker_family: "ffmpeg".to_string(),
        audio_codec: "aac".to_string(),
        ready: !inputs.is_empty()
            && unresolved_participant_ids.is_empty()
            && (!media_runtime.mix_minus_participant_ids.is_empty() || returns.is_empty()),
        unresolved_participant_ids,
        unresolved_reasons,
        inputs,
        returns,
        artifact_outputs,
        steps,
    })
}

fn build_ffmpeg_launch_step(
    session: &LiveIngestSession,
    mix_minus_required: bool,
    inputs: &[CollaborationMediaLaunchInput],
    returns: &[CollaborationMediaLaunchReturn],
    artifact_outputs: &[CollaborationMediaLaunchArtifactOutput],
) -> AppResult<CollaborationMediaLaunchStep> {
    let input_participant_ids = inputs
        .iter()
        .map(|item| item.participant_id.clone())
        .collect::<Vec<_>>();
    let return_participant_ids = returns
        .iter()
        .map(|item| item.participant_id.clone())
        .collect::<Vec<_>>();
    let artifact_output_ids = artifact_outputs
        .iter()
        .map(|item| item.output_id.clone())
        .collect::<Vec<_>>();

    let filter_complex = build_filter_complex(inputs, returns)?;
    let mut args = vec![
        "-nostdin".to_string(),
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "warning".to_string(),
    ];

    for input in inputs {
        args.push("-i".to_string());
        args.push(input.input_url.clone());
    }

    args.push("-filter_complex".to_string());
    args.push(filter_complex.clone());

    for return_route in returns {
        args.push("-map".to_string());
        args.push(format!("[ret_{}]", return_route.participant_id));
        args.push("-c:a".to_string());
        args.push("aac".to_string());
        args.push("-f".to_string());
        args.push(output_format_for_transport(&return_route.media_transport).to_string());
        args.push(return_route.output_url.clone());
    }

    for output in artifact_outputs.iter().filter(|item| {
        item.playback_enabled || item.recording_enabled || item.output_kind == "host_channel"
    }) {
        args.push("-map".to_string());
        args.push("[program_main]".to_string());
        args.push("-c:a".to_string());
        args.push("aac".to_string());
        args.push("-f".to_string());
        args.push(output_format_for_artifact(&output.output_kind).to_string());
        args.push(output.relative_path.clone());
    }

    Ok(CollaborationMediaLaunchStep {
        step_kind: if mix_minus_required {
            "mix_minus_publish".to_string()
        } else {
            "program_publish".to_string()
        },
        command: "ffmpeg".to_string(),
        args: prepend_media_root_placeholder(session, args),
        filter_complex: Some(filter_complex),
        input_participant_ids,
        return_participant_ids,
        artifact_output_ids,
    })
}

fn build_filter_complex(
    inputs: &[CollaborationMediaLaunchInput],
    returns: &[CollaborationMediaLaunchReturn],
) -> AppResult<String> {
    if inputs.is_empty() {
        return Err(AppError::Internal(
            "collaboration launch plan requires at least one input".to_string(),
        ));
    }

    let mut filters = Vec::new();
    let input_labels = inputs
        .iter()
        .map(|input| format!("[{}:a]", input.input_index))
        .collect::<Vec<_>>();

    if input_labels.len() == 1 {
        filters.push(format!("{}anull[program_main]", input_labels[0]));
    } else {
        filters.push(format!(
            "{}amix=inputs={}:dropout_transition=0:normalize=0[program_main]",
            input_labels.join(""),
            input_labels.len()
        ));
    }

    for return_route in returns {
        let return_labels = inputs
            .iter()
            .filter(|input| {
                return_route
                    .source_participant_ids
                    .contains(&input.participant_id)
            })
            .map(|input| format!("[{}:a]", input.input_index))
            .collect::<Vec<_>>();
        if return_labels.is_empty() {
            return Err(AppError::Internal(format!(
                "return launch for participant {} missing source inputs",
                return_route.participant_id
            )));
        }
        if return_labels.len() == 1 {
            filters.push(format!(
                "{}anull[ret_{}]",
                return_labels[0], return_route.participant_id
            ));
        } else {
            filters.push(format!(
                "{}amix=inputs={}:dropout_transition=0:normalize=0[ret_{}]",
                return_labels.join(""),
                return_labels.len(),
                return_route.participant_id
            ));
        }
    }

    Ok(filters.join(";"))
}

fn output_format_for_transport(transport: &str) -> &'static str {
    match transport {
        "srt" => "mpegts",
        "rtmp" | "rtmps" => "flv",
        _ => "mpegts",
    }
}

fn output_format_for_artifact(output_kind: &str) -> &'static str {
    match output_kind {
        "archive" => "mp4",
        "host_channel" | "mirror_channel" => "hls",
        _ => "data",
    }
}

fn prepend_media_root_placeholder(session: &LiveIngestSession, args: Vec<String>) -> Vec<String> {
    let launch_relative_path = collaboration_launch_relative_path(session);
    let workspace_root = FsPath::new(&launch_relative_path)
        .parent()
        .map(|path| format!("${{VANTA_MEDIA_ROOT}}/{}", path.to_string_lossy()))
        .unwrap_or_else(|| "${VANTA_MEDIA_ROOT}".to_string());

    args.into_iter()
        .map(|arg| {
            if arg.starts_with("live/")
                || arg.starts_with("archive/")
                || arg.starts_with("runtime/")
            {
                format!("{workspace_root}/{arg}")
            } else {
                arg
            }
        })
        .collect()
}
