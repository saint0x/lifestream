use super::*;

pub(crate) async fn record_viewer_event(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<ViewerEventInput>,
) -> AppResult<StatusCode> {
    let identity = optional_identity(&state.db, &headers).await?;
    let received_at = Utc::now().to_rfc3339();
    state
        .db
        .record_viewer_event(
            &format!("ve-{}", Uuid::new_v4().simple()),
            identity.as_ref().map(|item| item.user_id.as_str()),
            &input,
            &received_at,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
