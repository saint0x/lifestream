mod api;
mod auth;
mod config;
mod error;
mod models;
mod seed;
mod state;

use std::sync::Arc;

use crate::{config::Config, state::AppState};
use axum::http::HeaderValue;
use sqlx::{SqlitePool, migrate::Migrator, sqlite::SqlitePoolOptions};
use tokio::net::TcpListener;
use tracing::info;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let command = RuntimeCommand::from_args(std::env::args().skip(1))?;
    let config = Config::from_env()?;
    match command {
        RuntimeCommand::Serve => serve(config).await?,
        RuntimeCommand::Seed => seed_database(config).await?,
    }

    Ok(())
}

enum RuntimeCommand {
    Serve,
    Seed,
}

impl RuntimeCommand {
    fn from_args(
        mut args: impl Iterator<Item = String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        match args.next().as_deref() {
            None | Some("serve") => Ok(Self::Serve),
            Some("seed") => Ok(Self::Seed),
            Some(flag) => {
                Err(format!("unknown command `{flag}`; supported commands: `serve`, `seed`").into())
            }
        }
    }
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend=info,tower_http=info".into()),
        )
        .compact()
        .init();
}

async fn apply_sqlite_pragmas(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn connect_pool(config: &Config) -> Result<SqlitePool, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(config.max_db_connections)
        .connect(&config.database_url)
        .await?;
    apply_sqlite_pragmas(&pool).await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

async fn serve(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let pool = connect_pool(&config).await?;
    tokio::fs::create_dir_all(&config.media_root).await?;
    let cors_allowed_origins = config
        .allowed_origins
        .iter()
        .map(|origin| HeaderValue::from_str(origin))
        .collect::<Result<Vec<_>, _>>()?;

    let state = Arc::new(AppState::new(
        pool,
        config.media_root.clone(),
        cors_allowed_origins,
    ));
    api::start_background_workers(state.clone());
    let app = api::router(state);

    let listener = TcpListener::bind(config.bind_addr).await?;
    info!("listening on {}", config.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn seed_database(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let pool = connect_pool(&config).await?;
    seed::seed_if_empty(&pool, &config).await?;
    info!("seed command completed against {}", config.database_url);
    Ok(())
}
