use super::*;

pub(crate) async fn execute_collaboration_socket_command(
    state: &SharedState,
    session_id: &str,
    identity: &RequestIdentity,
    session: &CollaborationSessionView,
    command: CollaborationSocketCommand,
) -> AppResult<CollaborationSocketCommandOutcome> {
    match command {
        CollaborationSocketCommand::Heartbeat => Ok(CollaborationSocketCommandOutcome {
            command_type: "heartbeat",
            participant_id: None,
            state: None,
        }),
        CollaborationSocketCommand::RevokeInvite { invite_id } => {
            host_controls::execute_revoke_invite(state, session_id, identity, session, &invite_id)
                .await
        }
        CollaborationSocketCommand::RequestStateChange {
            state: requested_state,
        } => {
            requests::execute_request_state_change(
                state,
                session_id,
                identity,
                session,
                requested_state,
            )
            .await
        }
        CollaborationSocketCommand::UpdateParticipant {
            participant_id,
            state: requested_state,
            publish_to_host,
            mirror_to_guest_channel,
            can_speak_in_chat,
            media_transport,
            contribution_endpoint_url,
            return_endpoint_url,
        } => {
            host_controls::execute_update_participant(
                state,
                session_id,
                identity,
                session,
                participant_id,
                UpdateCollaborationParticipantRequest {
                    state: requested_state,
                    publish_to_host,
                    mirror_to_guest_channel,
                    can_speak_in_chat,
                    media_transport,
                    contribution_endpoint_url,
                    return_endpoint_url,
                },
            )
            .await
        }
        CollaborationSocketCommand::RemoveParticipant { participant_id } => {
            host_controls::execute_remove_participant(
                state,
                session_id,
                identity,
                session,
                &participant_id,
            )
            .await
        }
        CollaborationSocketCommand::IssueMirrorGrant { participant_id } => {
            mirror::execute_issue_mirror_grant(
                state,
                session_id,
                identity,
                session,
                &participant_id,
            )
            .await
        }
        CollaborationSocketCommand::RevokeMirrorGrants { participant_id } => {
            mirror::execute_revoke_mirror_grants(
                state,
                session_id,
                identity,
                session,
                &participant_id,
            )
            .await
        }
    }
}
