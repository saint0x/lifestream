use super::*;

mod host;
mod member;

pub(crate) use host::{
    apply_collaboration_participant_update, create_collaboration_invite,
    create_collaboration_session, end_collaboration_session, get_creator_collaboration_control,
    get_creator_collaboration_runtime, get_creator_collaboration_session,
    get_creator_collaboration_socket_session, list_creator_collaboration_events,
    reconcile_creator_collaboration_socket_session, remove_collaboration_participant,
    revoke_collaboration_invite, revoke_collaboration_invite_internal,
    update_collaboration_participant,
};
pub(crate) use member::{
    accept_collaboration_invite, get_my_collaboration_runtime, get_my_collaboration_session,
    list_my_collaboration_events, list_my_collaboration_invites,
};

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/api/v1/creator/me/live/collabs",
            get(host::list_creator_collaboration_sessions),
        )
        .route(
            "/api/v1/creator/me/live/collabs/sessions",
            post(host::create_collaboration_session),
        )
        .route(
            "/api/v1/creator/me/live/collabs/sessions/:session_id",
            get(host::get_creator_collaboration_session),
        )
        .route(
            "/api/v1/creator/me/live/collabs/sessions/:session_id/events",
            get(host::list_creator_collaboration_events),
        )
        .route(
            "/api/v1/creator/me/live/collabs/sessions/:session_id/control",
            get(host::get_creator_collaboration_control),
        )
        .route(
            "/api/v1/creator/me/live/collabs/sessions/:session_id/socket-sessions/:socket_id",
            get(host::get_creator_collaboration_socket_session),
        )
        .route(
            "/api/v1/creator/me/live/collabs/sessions/:session_id/socket-sessions/:socket_id/reconcile",
            post(host::reconcile_creator_collaboration_socket_session),
        )
        .route(
            "/api/v1/creator/me/live/collabs/sessions/:session_id/runtime",
            get(host::get_creator_collaboration_runtime),
        )
        .route(
            "/api/v1/creator/me/live/collabs/sessions/:session_id/reconcile",
            post(host::reconcile_creator_collaboration_session),
        )
        .route(
            "/api/v1/creator/me/live/collabs/sessions/:session_id/end",
            post(host::end_collaboration_session),
        )
        .route(
            "/api/v1/creator/me/live/collabs/sessions/:session_id/invites",
            post(host::create_collaboration_invite),
        )
        .route(
            "/api/v1/creator/me/live/collabs/sessions/:session_id/invites/:invite_id/revoke",
            post(host::revoke_collaboration_invite),
        )
        .route(
            "/api/v1/creator/me/live/collabs/sessions/:session_id/participants/:participant_id",
            patch(host::update_collaboration_participant),
        )
        .route(
            "/api/v1/creator/me/live/collabs/sessions/:session_id/participants/:participant_id/remove",
            post(host::remove_collaboration_participant),
        )
        .route(
            "/api/v1/creator/me/live/collabs/sessions/:session_id/participants/:participant_id/grants/mirror",
            post(host::issue_collaboration_mirror_grant),
        )
        .route(
            "/api/v1/me/live/collabs/invites",
            get(member::list_my_collaboration_invites),
        )
        .route(
            "/api/v1/me/live/collabs/sessions",
            get(member::list_my_collaboration_sessions),
        )
        .route(
            "/api/v1/me/live/collabs/sessions/:session_id",
            get(member::get_my_collaboration_session),
        )
        .route(
            "/api/v1/me/live/collabs/sessions/:session_id/leave",
            post(member::leave_my_collaboration_session),
        )
        .route(
            "/api/v1/me/live/collabs/sessions/:session_id/events",
            get(member::list_my_collaboration_events),
        )
        .route(
            "/api/v1/me/live/collabs/sessions/:session_id/runtime",
            get(member::get_my_collaboration_runtime),
        )
        .route(
            "/api/v1/me/live/collabs/sessions/:session_id/grants",
            get(member::list_my_collaboration_mirror_grants),
        )
        .route(
            "/api/v1/live/collabs/invites/:invite_id/accept",
            post(member::accept_collaboration_invite),
        )
        .route(
            "/api/v1/live/collabs/invites/:invite_id/decline",
            post(member::decline_collaboration_invite),
        )
        .route(
            "/api/v1/live/collabs/grants/:grant_id/redeem",
            post(member::redeem_collaboration_mirror_grant),
        )
}
