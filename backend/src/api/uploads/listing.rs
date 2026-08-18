use super::*;

pub(super) async fn list_uploads(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<Upload>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    identity.require_creator_scope()?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(fetch_uploads(&state.pool, creator_id).await?))
}

pub(super) async fn get_creator_content(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<CreatorContentQuery>,
) -> AppResult<Json<CreatorContentResponse>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let uploads = fetch_uploads(&state.pool, creator_id).await?;
    let filtered_uploads = filter_creator_uploads(uploads.clone(), &query)?;

    Ok(Json(CreatorContentResponse {
        summary: summarize_creator_content(&uploads, filtered_uploads.len() as i64),
        uploads: filtered_uploads,
    }))
}
