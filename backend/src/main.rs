mod api;
mod auth;
mod better_auth_runtime;
mod config;
mod db;
mod error;
mod models;
mod repositories;
mod runtime_command;
mod state;
mod storage;

use std::sync::Arc;

use crate::runtime_command::RuntimeCommand;
use crate::{
    config::{Config, DatabaseKind},
    db::Database,
    state::AppState,
    storage::Storage,
};
use axum::http::HeaderValue;
use sqlx::{
    PgPool, SqlitePool, migrate::Migrator, postgres::PgPoolOptions, sqlite::SqlitePoolOptions,
};
use tokio::net::TcpListener;
use tracing::info;

static SQLITE_MIGRATOR: Migrator = sqlx::migrate!("./migrations");
static POSTGRES_MIGRATOR: Migrator = sqlx::migrate!("./migrations/postgres");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let command = RuntimeCommand::from_args(std::env::args().skip(1))?;
    let config = Config::from_env()?;
    match command {
        RuntimeCommand::Serve => serve(config).await?,
        RuntimeCommand::ProvisionUser(command) => {
            let database = connect_database(&config).await?;
            command.execute(&database).await?;
        }
        RuntimeCommand::ProvisionCreator(command) => {
            let database = connect_database(&config).await?;
            command.execute(&database).await?;
        }
        RuntimeCommand::IssueSession(command) => {
            let database = connect_database(&config).await?;
            command.execute(&database).await?;
        }
        RuntimeCommand::RunCollaborationWorker(command) => {
            let database = connect_database(&config).await?;
            Storage::from_config(&config)?.prepare().await?;
            command.execute(&config, &database).await?;
        }
        RuntimeCommand::RunBackgroundWorker => {
            let state = build_app_state(config).await?;
            info!("starting standalone background worker");
            api::run_background_worker_loop(state).await;
        }
    }

    Ok(())
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
        PRAGMA trusted_schema = ON;
        PRAGMA busy_timeout = 5000;
        PRAGMA temp_store = MEMORY;
        PRAGMA cache_size = -32768;
        PRAGMA mmap_size = 268435456;
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn connect_database(config: &Config) -> Result<Database, sqlx::Error> {
    match config.database_kind {
        DatabaseKind::Sqlite => Ok(Database::from_sqlite(connect_sqlite_pool(config).await?)),
        DatabaseKind::Postgres => Ok(Database::from_postgres(
            connect_postgres_pool(config).await?,
        )),
    }
}

async fn connect_sqlite_pool(config: &Config) -> Result<SqlitePool, sqlx::Error> {
    if config.database_kind != DatabaseKind::Sqlite {
        return Err(sqlx::Error::Configuration(
            "sqlite database initializer called while another database provider is active".into(),
        ));
    }
    let pool = SqlitePoolOptions::new()
        .max_connections(config.max_db_connections)
        .connect(&config.database_url)
        .await?;
    apply_sqlite_pragmas(&pool).await?;
    SQLITE_MIGRATOR.run(&pool).await?;
    Ok(pool)
}

async fn connect_postgres_pool(config: &Config) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(config.max_db_connections)
        .connect(&config.database_url)
        .await?;
    POSTGRES_MIGRATOR.run(&pool).await?;
    validate_postgres_schema(&pool).await?;
    Ok(pool)
}

async fn validate_postgres_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT 1").fetch_one(pool).await?;
    Ok(())
}

async fn serve(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let bind_addr = config.bind_addr;
    let state = build_app_state(config).await?;
    api::start_background_workers(state.clone());
    let app = api::router(state);

    let listener = TcpListener::bind(bind_addr).await?;
    info!("listening on {}", bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn build_app_state(config: Config) -> Result<Arc<AppState>, Box<dyn std::error::Error>> {
    let database = connect_database(&config).await?;
    let storage = Storage::from_config(&config)?;
    storage.prepare().await?;
    let cors_allowed_origins = config
        .allowed_origins
        .iter()
        .map(|origin| HeaderValue::from_str(origin))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Arc::new(AppState::new(
        database,
        storage,
        config.clone(),
        cors_allowed_origins,
    )))
}
