use super::chat::{persist_chat_message, send_chat_message_rejected};
use super::collab::{
    CollaborationSocketCommand, collaboration_socket_command_name,
    fetch_current_collaboration_socket_session_view, send_collaboration_command_accepted,
    send_collaboration_command_rejected,
};
use super::*;
use crate::api::presence::LiveViewerAttribution;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use serde_json::Value;

mod collab;
mod creator_live;
mod shared;
mod viewer_live;

#[derive(Deserialize)]
pub(crate) struct WsAuthQuery {
    #[serde(alias = "accessToken")]
    access_token: Option<String>,
    #[serde(alias = "afterSeq")]
    after_seq: Option<i64>,
    #[serde(alias = "sessionToken")]
    session_token: Option<String>,
    #[serde(alias = "visitorId")]
    visitor_id: Option<String>,
    #[serde(alias = "landingUrl")]
    landing_url: Option<String>,
    #[serde(alias = "initialReferrerUrl")]
    initial_referrer_url: Option<String>,
    #[serde(alias = "currentUrl")]
    current_url: Option<String>,
    #[serde(alias = "currentReferrerUrl")]
    current_referrer_url: Option<String>,
    #[serde(alias = "utmSource")]
    utm_source: Option<String>,
    #[serde(alias = "utmMedium")]
    utm_medium: Option<String>,
    #[serde(alias = "utmCampaign")]
    utm_campaign: Option<String>,
    #[serde(alias = "utmTerm")]
    utm_term: Option<String>,
    #[serde(alias = "utmContent")]
    utm_content: Option<String>,
}

pub(crate) use shared::{auth_session_channel_id, close_websocket, ensure_identity_session_active};

pub(crate) async fn ws_live(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<WsAuthQuery>,
    Path(stream_id): Path<String>,
) -> AppResult<impl axum::response::IntoResponse> {
    validate_request_origin(&state, &headers)?;
    ensure_stream_exists(state.db.sqlite_adapter(), &stream_id).await?;
    let viewer_identity = match query.access_token {
        Some(token) => Some(lookup_identity(&state.db, &token).await?),
        None => None,
    };
    let attribution = LiveViewerAttribution {
        visitor_id: query.visitor_id,
        landing_url: query.landing_url,
        initial_referrer_url: query.initial_referrer_url,
        current_url: query.current_url,
        current_referrer_url: query.current_referrer_url,
        utm_source: query.utm_source,
        utm_medium: query.utm_medium,
        utm_campaign: query.utm_campaign,
        utm_term: query.utm_term,
        utm_content: query.utm_content,
    };
    Ok(ws.on_upgrade(move |socket| {
        viewer_live::handle_socket(
            socket,
            state,
            stream_id,
            viewer_identity,
            query.after_seq.unwrap_or(0),
            query.session_token,
            attribution,
        )
    }))
}

pub(crate) async fn ws_creator_live(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<WsAuthQuery>,
) -> AppResult<impl axum::response::IntoResponse> {
    validate_request_origin(&state, &headers)?;
    let token = query.access_token.ok_or(AppError::Unauthorized)?;
    let identity = lookup_identity(&state.db, &token).await?;
    let creator_id = identity.creator_id.clone().ok_or(AppError::Forbidden)?;
    Ok(ws.on_upgrade(move |socket| {
        creator_live::handle_creator_live_socket(
            socket,
            state,
            creator_id,
            identity,
            query.session_token,
        )
    }))
}

pub(crate) async fn ws_collaboration(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<WsAuthQuery>,
    Path(session_id): Path<String>,
) -> AppResult<impl axum::response::IntoResponse> {
    validate_request_origin(&state, &headers)?;
    let token = query.access_token.ok_or(AppError::Unauthorized)?;
    let identity = lookup_identity(&state.db, &token).await?;
    let participant_access = fetch_collaboration_session_for_participant(
        state.db.sqlite_adapter(),
        &identity.user_id,
        &session_id,
    )
    .await;
    if participant_access.is_err() {
        let creator_id = identity.creator_id.as_deref().ok_or(AppError::Forbidden)?;
        let host_session = fetch_collaboration_session_for_host(
            state.db.sqlite_adapter(),
            creator_id,
            &session_id,
        )
        .await?;
        let host = fetch_collaboration_host_summary(
            state.db.sqlite_adapter(),
            &host_session.host_creator_id,
        )
        .await?;
        let host_view = collaboration_session_view_for_host(host_session, host)?;
        super::collab::validate_collaboration_socket_access(&host_view)?;
    } else if let Ok(session) = &participant_access {
        super::collab::validate_collaboration_socket_access(session)?;
    }
    Ok(ws.on_upgrade(move |socket| {
        collab::handle_collaboration_socket(
            socket,
            state,
            session_id,
            identity,
            query.after_seq.unwrap_or(0),
            query.session_token,
        )
    }))
}
