use super::*;

pub(crate) async fn update_creator_live(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<UpdateLiveRequest>,
) -> AppResult<Json<CreatorLiveSnapshot>> {
    let identity = require_identity(&state.db, &headers).await?;
    enforce_rate_limit(
        &state,
        &format!("creator-live-update:{}", identity.user_id),
        30,
        Duration::from_secs(60),
    )
    .await?;
    let creator_id = identity.require_creator_scope()?;
    let profile = fetch_creator_profile(state.db.try_sqlite_adapter()?, creator_id).await?;
    let next_category = input
        .category
        .clone()
        .unwrap_or_else(|| profile.default_category.clone());
    let next_tags = input.tags.clone().unwrap_or(profile.default_tags.clone());

    sqlx::query(
        "UPDATE creator_profiles SET default_category = ?, default_tags_json = ? WHERE id = ?",
    )
    .bind(&next_category)
    .bind(to_json(&next_tags)?)
    .bind(creator_id)
    .execute(state.db.try_sqlite_adapter()?)
    .await?;

    if let Some(current_id) = profile.current_broadcast_id {
        let current =
            fetch_broadcast_by_id(state.db.try_sqlite_adapter()?, creator_id, &current_id).await?;
        sqlx::query(
            "UPDATE broadcasts SET title = ?, category = ?, tags_json = ?, is_mature = ? WHERE id = ?",
        )
        .bind(input.title.unwrap_or(current.title))
        .bind(next_category)
        .bind(to_json(&next_tags)?)
        .bind(input.is_mature.unwrap_or(current.is_mature) as i64)
        .bind(current_id)
        .execute(state.db.try_sqlite_adapter()?)
        .await?;
    }

    get_creator_live(State(state), headers).await
}
