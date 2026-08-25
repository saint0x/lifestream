use std::{path::Path, sync::Arc};

use axum::{Json, Router, routing::get};
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{
    media::{self, service::MediaService, store::MediaStore},
    native::{self, service::NativeService, store::NativeStore},
    obs::{self, service::ObsService, store::ObsStore},
    release,
};

#[derive(Clone)]
pub struct AppState {
    pub obs: Arc<ObsService>,
    pub native: Arc<NativeService>,
    pub media: Arc<MediaService>,
}

pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .merge(obs::api::router())
        .merge(native::api::router())
        .merge(media::api::router())
        .merge(release::router())
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

pub async fn app_state(store: ObsStore) -> Result<AppState, sqlx::Error> {
    let native = NativeStore::connect(store.pool())
        .await
        .map_err(|error| match error {
            crate::native::store::NativeStoreError::Sqlx(error) => error,
            other => sqlx::Error::Protocol(other.to_string()),
        })?;
    let media = MediaStore::connect(store.pool())
        .await
        .map_err(|error| match error {
            crate::media::store::MediaStoreError::Sqlx(error) => error,
            other => sqlx::Error::Protocol(other.to_string()),
        })?;
    Ok(app_state_from_stores(store, native, media))
}

pub fn app_state_from_stores(obs: ObsStore, native: NativeStore, media: MediaStore) -> AppState {
    let native = Arc::new(NativeService::new(native));
    AppState {
        obs: Arc::new(ObsService::new(obs)),
        media: Arc::new(MediaService::new(media, native.clone())),
        native,
    }
}

pub async fn connect_stores(
    database_path: &Path,
) -> Result<(ObsStore, NativeStore, MediaStore), sqlx::Error> {
    let connect_options = SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(connect_options)
        .await?;
    let obs = ObsStore::connect(pool.clone())
        .await
        .map_err(|error| match error {
            crate::obs::store::ObsStoreError::Sqlx(error) => error,
            other => sqlx::Error::Protocol(other.to_string()),
        })?;
    let native = NativeStore::connect(pool)
        .await
        .map_err(|error| match error {
            crate::native::store::NativeStoreError::Sqlx(error) => error,
            other => sqlx::Error::Protocol(other.to_string()),
        })?;
    let media = MediaStore::connect(obs.pool())
        .await
        .map_err(|error| match error {
            crate::media::store::MediaStoreError::Sqlx(error) => error,
            other => sqlx::Error::Protocol(other.to_string()),
        })?;
    Ok((obs, native, media))
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok", "service": "vanta-obs" }))
}
