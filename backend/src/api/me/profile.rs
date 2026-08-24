use super::*;

pub(crate) async fn get_my_profile(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<UserProfileDetails>> {
    let identity = require_identity(&state.db, &headers).await?;
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        return Ok(Json(
            super::state::build_postgres_viewer_app_state(
                &state.db,
                &identity.user_id,
                &identity.session_id,
            )
            .await?
            .profile,
        ));
    }
    Ok(Json(
        fetch_user_profile_details(state.db.sqlite_adapter(), &identity.user_id).await?,
    ))
}

pub(crate) async fn update_my_profile(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<UpdateProfileRequest>,
) -> AppResult<Json<UserProfileDetails>> {
    let identity = require_identity(&state.db, &headers).await?;
    validate_profile_update(&input)?;
    state
        .db
        .update_user_profile(&identity.user_id, &input)
        .await?;
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        return Ok(Json(
            super::state::build_postgres_viewer_app_state(
                &state.db,
                &identity.user_id,
                &identity.session_id,
            )
            .await?
            .profile,
        ));
    }

    Ok(Json(
        fetch_user_profile_details(state.db.sqlite_adapter(), &identity.user_id).await?,
    ))
}

pub(crate) async fn get_my_settings(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<UserSettingsBundle>> {
    let identity = require_identity(&state.db, &headers).await?;
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        return Ok(Json(
            super::state::build_postgres_viewer_app_state(
                &state.db,
                &identity.user_id,
                &identity.session_id,
            )
            .await?
            .settings,
        ));
    }
    Ok(Json(
        fetch_user_settings_bundle(state.db.sqlite_adapter(), &identity.user_id).await?,
    ))
}

pub(crate) async fn update_my_settings(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(input): Json<UpdateSettingsRequest>,
) -> AppResult<Json<UserSettingsBundle>> {
    let identity = require_identity(&state.db, &headers).await?;
    validate_settings_update(&input)?;
    state
        .db
        .update_user_settings(&identity.user_id, &input)
        .await?;
    if state.database_kind == crate::config::DatabaseKind::Postgres {
        return Ok(Json(
            super::state::build_postgres_viewer_app_state(
                &state.db,
                &identity.user_id,
                &identity.session_id,
            )
            .await?
            .settings,
        ));
    }
    Ok(Json(
        fetch_user_settings_bundle(state.db.sqlite_adapter(), &identity.user_id).await?,
    ))
}
