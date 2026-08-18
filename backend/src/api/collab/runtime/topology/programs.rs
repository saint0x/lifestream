use super::{outputs::planned_output_ids_for_participant, push_unique};
use crate::models::{
    CollaborationOutputRoute, CollaborationParticipant, CollaborationProgramRoute,
    CollaborationSessionView,
};

pub(super) fn build_topology_programs(
    session: &CollaborationSessionView,
    outputs: &[CollaborationOutputRoute],
    host_output_participant_ids: &[String],
    live_participant_ids: &[String],
) -> Vec<CollaborationProgramRoute> {
    let mut programs = Vec::new();
    let host_participant_ids = host_output_participant_ids.to_vec();
    let host_output_ids = session
        .participants
        .iter()
        .filter(|participant| participant.role == "host" || participant.publish_to_host)
        .flat_map(|participant| {
            planned_output_ids_for_participant(
                session,
                participant,
                outputs,
                host_output_participant_ids,
            )
        })
        .fold(Vec::new(), |mut ids, output_id| {
            push_unique(&mut ids, output_id);
            ids
        });
    let host_route_state = program_route_state(outputs, &host_output_ids);
    programs.push(CollaborationProgramRoute {
        id: format!("col-program-host-{}", session.id),
        program_kind: "host_program".to_string(),
        route_state: host_route_state,
        source_participant_ids: host_participant_ids.clone(),
        output_ids: host_output_ids,
        target_creator_id: Some(session.host_creator_id.clone()),
        target_broadcast_id: Some(session.source_broadcast_id.clone()),
        playback_enabled: outputs.iter().any(|output| {
            output.output_kind == "host_channel"
                && matches!(
                    output.route_state.as_str(),
                    "active" | "degraded" | "issued"
                )
        }),
        recording_enabled: outputs.iter().any(|output| {
            output.recording_enabled
                && host_participant_ids
                    .iter()
                    .all(|participant_id| output.source_participant_ids.contains(participant_id))
        }),
        mix_minus_required: session.participants.iter().any(|participant| {
            participant.role != "host" && participant.publish_to_host && participant.state == "live"
        }),
    });

    for participant in &session.participants {
        if participant.role == "host" || participant.publish_to_host {
            continue;
        }
        let output_ids = planned_output_ids_for_participant(
            session,
            participant,
            outputs,
            host_output_participant_ids,
        );
        if output_ids.is_empty() {
            continue;
        }
        let source_participant_ids = guest_program_sources(participant, live_participant_ids);
        programs.push(CollaborationProgramRoute {
            id: format!("col-program-{}", participant.id),
            program_kind: "guest_program".to_string(),
            route_state: program_route_state(outputs, &output_ids),
            source_participant_ids,
            output_ids,
            target_creator_id: participant.creator_id.clone(),
            target_broadcast_id: outputs
                .iter()
                .find(|output| output.id == format!("col-out-mirror-{}", participant.id))
                .and_then(|output| output.target_broadcast_id.clone()),
            playback_enabled: outputs.iter().any(|output| {
                output.id == format!("col-out-mirror-{}", participant.id)
                    && matches!(
                        output.route_state.as_str(),
                        "active" | "degraded" | "issued"
                    )
            }),
            recording_enabled: outputs.iter().any(|output| {
                output.id == format!("col-out-archive-{}", participant.id)
                    && output.recording_enabled
            }),
            mix_minus_required: false,
        });
    }

    programs
}

fn guest_program_sources(
    participant: &CollaborationParticipant,
    live_participant_ids: &[String],
) -> Vec<String> {
    if live_participant_ids.contains(&participant.id) {
        vec![participant.id.clone()]
    } else {
        Vec::new()
    }
}

fn program_route_state(outputs: &[CollaborationOutputRoute], output_ids: &[String]) -> String {
    let selected = outputs
        .iter()
        .filter(|output| output_ids.contains(&output.id))
        .collect::<Vec<_>>();
    if selected
        .iter()
        .any(|output| matches!(output.route_state.as_str(), "active" | "degraded"))
    {
        if selected
            .iter()
            .any(|output| output.route_state == "degraded")
        {
            "degraded".to_string()
        } else {
            "active".to_string()
        }
    } else if selected
        .iter()
        .any(|output| matches!(output.route_state.as_str(), "issued" | "armed"))
    {
        "armed".to_string()
    } else if selected
        .iter()
        .any(|output| output.route_state == "pending_source")
    {
        "pending_source".to_string()
    } else {
        "inactive".to_string()
    }
}
