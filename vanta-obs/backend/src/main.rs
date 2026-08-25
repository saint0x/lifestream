use std::{net::SocketAddr, path::PathBuf};

use tokio::net::TcpListener;
use tracing::info;
use vanta_obs_backend::app::{app_state_from_stores, build_app, connect_stores};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vanta_obs_backend=info,tower_http=info".into()),
        )
        .compact()
        .init();

    let bind_addr = std::env::var("VANTA_OBS_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:4127".to_string())
        .parse::<SocketAddr>()?;
    let db_path = std::env::var("VANTA_OBS_DATABASE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("vanta-obs.db"));

    if let Some(parent) = db_path.parent().filter(|path| !path.as_os_str().is_empty()) {
        tokio::fs::create_dir_all(parent).await?;
    }

    let (obs, native, media) = connect_stores(&db_path).await?;
    obs.seed().await?;

    let app = build_app(app_state_from_stores(obs, native, media));
    let listener = TcpListener::bind(bind_addr).await?;
    info!("vanta obs api listening on {}", bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
