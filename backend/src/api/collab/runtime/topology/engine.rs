use crate::models::{
    CollaborationAudioRoute, CollaborationContributionAttachment, CollaborationExecutionBus,
    CollaborationExecutionEdge, CollaborationExecutionNode, CollaborationExecutionOperation,
    CollaborationExecutionPlan, CollaborationOutputRoute, CollaborationProgramRoute,
};

pub(super) fn build_topology_engine(
    programs: &[CollaborationProgramRoute],
    audio: &[CollaborationAudioRoute],
    outputs: &[CollaborationOutputRoute],
    contributions: &[CollaborationContributionAttachment],
    mirrored_creator_ids: &[String],
    mix_minus_required: bool,
) -> CollaborationExecutionPlan {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut buses = Vec::new();
    let mut operations = Vec::new();

    for contribution in contributions {
        nodes.push(CollaborationExecutionNode {
            id: format!("contrib-{}", contribution.participant_id),
            node_kind: "contribution".to_string(),
            route_state: contribution.contribution_state.clone(),
            participant_id: Some(contribution.participant_id.clone()),
            target_creator_id: contribution.creator_id.clone(),
            target_broadcast_id: contribution.source_broadcast_id.clone(),
            mix_minus_required: contribution.mix_minus_required,
        });
        buses.push(CollaborationExecutionBus {
            id: contribution_bus_id(&contribution.participant_id),
            bus_kind: "contribution_bus".to_string(),
            route_state: contribution.contribution_state.clone(),
            participant_id: Some(contribution.participant_id.clone()),
            target_creator_id: contribution.creator_id.clone(),
            target_broadcast_id: contribution.source_broadcast_id.clone(),
            mix_minus_required: contribution.mix_minus_required,
        });
    }
    for program in programs {
        nodes.push(CollaborationExecutionNode {
            id: program.id.clone(),
            node_kind: program.program_kind.clone(),
            route_state: program.route_state.clone(),
            participant_id: None,
            target_creator_id: program.target_creator_id.clone(),
            target_broadcast_id: program.target_broadcast_id.clone(),
            mix_minus_required: program.mix_minus_required,
        });
        buses.push(CollaborationExecutionBus {
            id: program_bus_id(&program.id),
            bus_kind: program.program_kind.clone(),
            route_state: program.route_state.clone(),
            participant_id: None,
            target_creator_id: program.target_creator_id.clone(),
            target_broadcast_id: program.target_broadcast_id.clone(),
            mix_minus_required: program.mix_minus_required,
        });
    }
    for route in audio {
        nodes.push(CollaborationExecutionNode {
            id: format!("audio-{}", route.participant_id),
            node_kind: route.route_kind.clone(),
            route_state: route.route_state.clone(),
            participant_id: Some(route.participant_id.clone()),
            target_creator_id: route.creator_id.clone(),
            target_broadcast_id: None,
            mix_minus_required: route.mix_minus_required,
        });
        buses.push(CollaborationExecutionBus {
            id: audio_bus_id(&route.participant_id),
            bus_kind: route.route_kind.clone(),
            route_state: route.route_state.clone(),
            participant_id: Some(route.participant_id.clone()),
            target_creator_id: route.creator_id.clone(),
            target_broadcast_id: None,
            mix_minus_required: route.mix_minus_required,
        });
    }
    for output in outputs {
        nodes.push(CollaborationExecutionNode {
            id: output.id.clone(),
            node_kind: output.output_kind.clone(),
            route_state: output.route_state.clone(),
            participant_id: None,
            target_creator_id: output.target_creator_id.clone(),
            target_broadcast_id: output.target_broadcast_id.clone(),
            mix_minus_required: output.mix_minus_required,
        });
        buses.push(CollaborationExecutionBus {
            id: output_bus_id(&output.id),
            bus_kind: output.output_kind.clone(),
            route_state: output.route_state.clone(),
            participant_id: None,
            target_creator_id: output.target_creator_id.clone(),
            target_broadcast_id: output.target_broadcast_id.clone(),
            mix_minus_required: output.mix_minus_required,
        });
    }

    for program in programs {
        push_program_edges(&mut edges, program);
        push_program_operations(&mut operations, program, outputs);
    }

    let host_program_id = programs
        .iter()
        .find(|program| program.program_kind == "host_program")
        .map(|program| program.id.clone());
    for route in audio {
        if route.receive_program_audio {
            push_audio_return_edge(&mut edges, host_program_id.as_ref(), route);
            push_audio_return_operation(&mut operations, host_program_id.as_ref(), route);
        }
    }

    CollaborationExecutionPlan {
        execution_mode: "topology_graph_v1".to_string(),
        fanout_mode: if mirrored_creator_ids.is_empty() {
            "host_only".to_string()
        } else {
            "mirrored_collaboration".to_string()
        },
        audio_mode: if mix_minus_required {
            "mix_minus".to_string()
        } else {
            "program_only".to_string()
        },
        nodes,
        edges,
        buses,
        operations,
    }
}

fn contribution_bus_id(participant_id: &str) -> String {
    format!("bus-contrib-{participant_id}")
}

fn program_bus_id(program_id: &str) -> String {
    format!("bus-program-{program_id}")
}

fn audio_bus_id(participant_id: &str) -> String {
    format!("bus-audio-{participant_id}")
}

fn output_bus_id(output_id: &str) -> String {
    format!("bus-output-{output_id}")
}

fn push_program_edges(
    edges: &mut Vec<CollaborationExecutionEdge>,
    program: &CollaborationProgramRoute,
) {
    for participant_id in &program.source_participant_ids {
        edges.push(CollaborationExecutionEdge {
            id: format!("edge-contrib-{participant_id}-{}", program.id),
            edge_kind: "contribution_to_program".to_string(),
            from_node_id: format!("contrib-{participant_id}"),
            to_node_id: program.id.clone(),
            route_state: program.route_state.clone(),
            excluded_participant_ids: Vec::new(),
        });
    }
    for output_id in &program.output_ids {
        edges.push(CollaborationExecutionEdge {
            id: format!("edge-program-{}-{output_id}", program.id),
            edge_kind: "program_to_output".to_string(),
            from_node_id: program.id.clone(),
            to_node_id: output_id.clone(),
            route_state: program.route_state.clone(),
            excluded_participant_ids: Vec::new(),
        });
    }
}

fn push_program_operations(
    operations: &mut Vec<CollaborationExecutionOperation>,
    program: &CollaborationProgramRoute,
    outputs: &[CollaborationOutputRoute],
) {
    for participant_id in &program.source_participant_ids {
        operations.push(CollaborationExecutionOperation {
            id: format!("op-attach-{participant_id}-{}", program.id),
            operation_kind: "attach_contribution".to_string(),
            input_bus_ids: vec![contribution_bus_id(participant_id)],
            output_bus_id: program_bus_id(&program.id),
            route_state: program.route_state.clone(),
            excluded_participant_ids: Vec::new(),
            mix_minus_required: program.mix_minus_required,
        });
    }
    for output_id in &program.output_ids {
        let operation_kind = outputs
            .iter()
            .find(|output| output.id == *output_id)
            .map(|output| match output.output_kind.as_str() {
                "mirror_channel" => "fanout_mirror",
                "archive" => "fanout_archive",
                "host_channel" => "fanout_host",
                _ => "fanout_output",
            })
            .unwrap_or("fanout_output");
        operations.push(CollaborationExecutionOperation {
            id: format!("op-fanout-{}-{output_id}", program.id),
            operation_kind: operation_kind.to_string(),
            input_bus_ids: vec![program_bus_id(&program.id)],
            output_bus_id: output_bus_id(output_id),
            route_state: program.route_state.clone(),
            excluded_participant_ids: Vec::new(),
            mix_minus_required: program.mix_minus_required,
        });
    }
}

fn push_audio_return_edge(
    edges: &mut Vec<CollaborationExecutionEdge>,
    host_program_id: Option<&String>,
    route: &CollaborationAudioRoute,
) {
    if let Some(host_program_id) = host_program_id {
        edges.push(CollaborationExecutionEdge {
            id: format!(
                "edge-program-{host_program_id}-audio-{}",
                route.participant_id
            ),
            edge_kind: "program_to_audio_return".to_string(),
            from_node_id: host_program_id.clone(),
            to_node_id: format!("audio-{}", route.participant_id),
            route_state: route.route_state.clone(),
            excluded_participant_ids: route.excluded_participant_ids.clone(),
        });
    }
}

fn push_audio_return_operation(
    operations: &mut Vec<CollaborationExecutionOperation>,
    host_program_id: Option<&String>,
    route: &CollaborationAudioRoute,
) {
    if let Some(host_program_id) = host_program_id {
        operations.push(CollaborationExecutionOperation {
            id: format!("op-return-{host_program_id}-{}", route.participant_id),
            operation_kind: "return_audio".to_string(),
            input_bus_ids: vec![program_bus_id(host_program_id)],
            output_bus_id: audio_bus_id(&route.participant_id),
            route_state: route.route_state.clone(),
            excluded_participant_ids: route.excluded_participant_ids.clone(),
            mix_minus_required: route.mix_minus_required,
        });
    }
}
