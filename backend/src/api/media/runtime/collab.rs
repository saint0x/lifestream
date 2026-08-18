use super::*;
use crate::models::{
    CollaborationMediaReturn, CollaborationMediaRuntime, CollaborationMediaStage,
    CollaborationMediaTarget, CollaborationRuntimeBundle,
};

pub(crate) fn build_collaboration_media_runtime(
    bundle: &CollaborationRuntimeBundle,
) -> AppResult<CollaborationMediaRuntime> {
    validate_runtime_bundle(bundle)?;

    let stages = vec![
        build_stage(
            "attach",
            bundle
                .attachments
                .iter()
                .map(|attachment| format!("attach:{}", attachment.participant_id))
                .collect(),
            bundle
                .attachments
                .iter()
                .flat_map(|attachment| {
                    [
                        attachment.contribution_bus_id.clone(),
                        attachment.program_bus_id.clone(),
                    ]
                })
                .collect(),
        ),
        build_stage(
            "mix",
            bundle
                .mixers
                .iter()
                .map(|mixer| format!("mix:{}", mixer.program_id))
                .collect(),
            bundle
                .mixers
                .iter()
                .flat_map(|mixer| {
                    let mut bus_ids = mixer.input_bus_ids.clone();
                    bus_ids.push(mixer.output_bus_id.clone());
                    bus_ids
                })
                .collect(),
        ),
        build_stage(
            "fanout",
            bundle
                .fanouts
                .iter()
                .map(|fanout| format!("fanout:{}", fanout.output_id))
                .collect(),
            bundle
                .fanouts
                .iter()
                .flat_map(|fanout| [fanout.input_bus_id.clone(), fanout.output_bus_id.clone()])
                .collect(),
        ),
        build_stage(
            "return",
            bundle
                .returns
                .iter()
                .map(|item| format!("return:{}", item.participant_id))
                .collect(),
            bundle
                .returns
                .iter()
                .flat_map(|item| [item.input_bus_id.clone(), item.output_bus_id.clone()])
                .collect(),
        ),
    ]
    .into_iter()
    .filter(|stage| !stage.operation_ids.is_empty())
    .collect::<Vec<_>>();

    let output_targets = bundle
        .fanouts
        .iter()
        .map(|fanout| CollaborationMediaTarget {
            output_id: fanout.output_id.clone(),
            output_kind: fanout.output_kind.clone(),
            relative_path: fanout.relative_path.clone(),
            route_state: fanout.route_state.clone(),
            playback_enabled: fanout.playback_enabled,
            recording_enabled: fanout.recording_enabled,
            mix_minus_required: fanout.mix_minus_required,
        })
        .collect::<Vec<_>>();
    let return_targets = bundle
        .returns
        .iter()
        .map(|item| CollaborationMediaReturn {
            participant_id: item.participant_id.clone(),
            input_bus_id: item.input_bus_id.clone(),
            output_bus_id: item.output_bus_id.clone(),
            excluded_participant_ids: item.excluded_participant_ids.clone(),
            attached_output_ids: item.attached_output_ids.clone(),
            route_state: item.route_state.clone(),
            mix_minus_required: item.mix_minus_required,
        })
        .collect::<Vec<_>>();

    let input_participant_ids = bundle
        .attachments
        .iter()
        .map(|attachment| attachment.participant_id.clone())
        .collect::<Vec<_>>();
    let mix_minus_participant_ids = bundle
        .returns
        .iter()
        .filter(|item| item.mix_minus_required)
        .map(|item| item.participant_id.clone())
        .collect::<Vec<_>>();

    Ok(CollaborationMediaRuntime {
        runtime_mode: "media_schedule_v1".to_string(),
        bundle_mode: bundle.bundle_mode.clone(),
        engine_execution_mode: bundle.engine_execution_mode.clone(),
        fanout_mode: bundle.fanout_mode.clone(),
        audio_mode: bundle.audio_mode.clone(),
        stage_count: stages.len() as i64,
        stages,
        output_targets,
        return_targets,
        input_participant_ids,
        mix_minus_participant_ids,
    })
}

fn build_stage(
    stage_kind: &str,
    operation_ids: Vec<String>,
    bus_ids: Vec<String>,
) -> CollaborationMediaStage {
    CollaborationMediaStage {
        stage_kind: stage_kind.to_string(),
        operation_ids,
        bus_ids,
    }
}

fn validate_runtime_bundle(bundle: &CollaborationRuntimeBundle) -> AppResult<()> {
    if bundle.mixers.is_empty() {
        return Err(AppError::Internal(
            "collaboration media runtime requires at least one mixer".to_string(),
        ));
    }
    if bundle.fanouts.is_empty() {
        return Err(AppError::Internal(
            "collaboration media runtime requires at least one fanout".to_string(),
        ));
    }
    if bundle
        .fanouts
        .iter()
        .any(|fanout| fanout.relative_path.as_deref().is_none_or(str::is_empty))
    {
        return Err(AppError::Internal(
            "collaboration media runtime fanout missing relative output path".to_string(),
        ));
    }
    if bundle.attachments.iter().any(|attachment| {
        !bundle
            .mixers
            .iter()
            .any(|mixer| mixer.output_bus_id == attachment.program_bus_id)
    }) {
        return Err(AppError::Internal(
            "collaboration media runtime attachment references missing mixer bus".to_string(),
        ));
    }
    if bundle
        .returns
        .iter()
        .any(|item| item.excluded_participant_ids.is_empty() && item.mix_minus_required)
    {
        return Err(AppError::Internal(
            "collaboration media runtime mix-minus return missing exclusion set".to_string(),
        ));
    }
    if bundle
        .returns
        .iter()
        .any(|item| item.attached_output_ids.is_empty())
    {
        return Err(AppError::Internal(
            "collaboration media runtime return missing attached output routing".to_string(),
        ));
    }
    Ok(())
}
