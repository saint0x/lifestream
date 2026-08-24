use super::*;

pub(crate) async fn list_media_assets(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<MediaAsset>>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        list_creator_media_assets(&state.db, creator_id).await?,
    ))
}

pub(crate) async fn get_media_asset_for_upload_job(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<MediaAsset>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(
        get_creator_media_asset_for_upload_job(&state.db, creator_id, &id).await?,
    ))
}

pub(crate) async fn list_creator_media_assets(
    database: &crate::db::Database,
    creator_id: &str,
) -> AppResult<Vec<MediaAsset>> {
    fetch_media_assets_for_database(database, creator_id).await
}

pub(crate) async fn get_creator_media_asset_for_upload_job(
    database: &crate::db::Database,
    creator_id: &str,
    job_id: &str,
) -> AppResult<MediaAsset> {
    fetch_media_asset_by_upload_job_for_database(database, creator_id, job_id).await
}
