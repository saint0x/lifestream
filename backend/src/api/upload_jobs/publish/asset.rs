use super::*;

pub(crate) async fn list_media_assets(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<MediaAsset>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(fetch_media_assets(&state.pool, creator_id).await?))
}

pub(crate) async fn get_media_asset_for_upload_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<MediaAsset>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        fetch_media_asset_by_upload_job(&state.pool, creator_id, &id).await?,
    ))
}
