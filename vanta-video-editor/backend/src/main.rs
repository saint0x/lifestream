mod api;
mod domain;
mod integrations;
mod media;
mod store;

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use api::router;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use store::EditorStore;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    store: Arc<EditorStore>,
    media: Arc<media::MediaProcessor>,
    integrations: Arc<integrations::VantaIntegrations>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vanta_video_editor_backend=info,tower_http=info".into()),
        )
        .compact()
        .init();

    let bind_addr = std::env::var("VANTA_EDITOR_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:4117".to_string())
        .parse::<SocketAddr>()?;
    let db_path = std::env::var("VANTA_EDITOR_DATABASE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("vanta-editor.db"));
    if let Some(parent) = db_path.parent().filter(|path| !path.as_os_str().is_empty()) {
        tokio::fs::create_dir_all(parent).await?;
    }

    let connect_options = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(connect_options)
        .await?;
    let store = EditorStore::connect(pool).await?;
    store.seed().await?;
    let media_root = std::env::var("VANTA_EDITOR_MEDIA_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("storage"));
    let media = media::MediaProcessor::new(media_root);
    media.prepare().await?;
    let media_pipeline_database = std::env::var("VANTA_MEDIA_PIPELINE_DATABASE")
        .ok()
        .map(PathBuf::from);
    let ad_hub_outbox = std::env::var("VANTA_AD_HUB_OUTBOX")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("storage/ad-hub"));
    let integrations =
        integrations::VantaIntegrations::new(media_pipeline_database, ad_hub_outbox).await?;

    let state = AppState {
        store: Arc::new(store),
        media: Arc::new(media),
        integrations: Arc::new(integrations),
    };
    let app = router(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(bind_addr).await?;
    info!("vanta editor api listening on {}", bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
