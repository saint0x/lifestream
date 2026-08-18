use crate::models::{
    CollaborationAudioRoute, CollaborationContributionAttachment, CollaborationSessionView,
};

pub(super) fn build_topology_audio(
    session: &CollaborationSessionView,
    host_output_participant_ids: &[String],
    live_participant_ids: &[String],
    backstage_participant_ids: &[String],
    contributions: &[CollaborationContributionAttachment],
) -> Vec<CollaborationAudioRoute> {
    session
        .participants
        .iter()
        .map(|participant| {
            let contribution = contributions
                .iter()
                .find(|item| item.participant_id == participant.id)
                .expect("collaboration contribution should exist for participant");
            let route_kind = if participant.role == "host" {
                "program_origin"
            } else if participant.publish_to_host {
                "mix_minus_return"
            } else if !contribution.attached_output_ids.is_empty() {
                "isolated_program"
            } else {
                "inactive"
            };
            let route_state = if live_participant_ids.contains(&participant.id) {
                "live"
            } else if backstage_participant_ids.contains(&participant.id) {
                "standby"
            } else {
                "inactive"
            };
            let receive_program_audio = participant.role != "host" && participant.publish_to_host;
            let mix_minus_required = participant.role != "host"
                && participant.publish_to_host
                && participant.state == "live";
            let upstream_participant_ids =
                if participant.role == "host" || !participant.publish_to_host {
                    if contribution.attached_output_ids.is_empty() {
                        Vec::new()
                    } else {
                        vec![participant.id.clone()]
                    }
                } else {
                    host_output_participant_ids.to_vec()
                };
            let excluded_participant_ids = if mix_minus_required {
                vec![participant.id.clone()]
            } else {
                Vec::new()
            };

            CollaborationAudioRoute {
                participant_id: participant.id.clone(),
                user_id: participant.user_id.clone(),
                creator_id: participant.creator_id.clone(),
                route_kind: route_kind.to_string(),
                route_state: route_state.to_string(),
                receive_program_audio,
                mix_minus_required,
                upstream_participant_ids,
                excluded_participant_ids,
                attached_output_ids: contribution.attached_output_ids.clone(),
            }
        })
        .collect()
}
