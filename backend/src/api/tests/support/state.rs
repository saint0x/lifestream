use super::*;
use crate::{db::Database, storage::Storage};

pub(crate) async fn setup_test_state() -> AppResult<(SharedState, CreatorProfile)> {
    let test_id = Uuid::new_v4().to_string();
    let db_path = std::env::temp_dir().join(format!("vanta-test-{test_id}.db"));
    let media_root = std::env::temp_dir().join(format!("vanta-media-{test_id}"));
    let source_db_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    copy_sqlite_fixture(source_db_dir.join("vanta.db"), &db_path).await?;
    let database_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;

    sqlx::raw_sql(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    tokio::fs::create_dir_all(&media_root)
        .await
        .map_err(AppError::Io)?;

    let config = Config {
        bind_addr: "127.0.0.1:0".parse().expect("test bind"),
        database_kind: DatabaseKind::Sqlite,
        database_url,
        max_db_connections: 1,
        storage_kind: StorageKind::Local,
        media_scratch_root: std::env::temp_dir().join(format!("vanta-scratch-{test_id}")),
        media_root: PathBuf::from(&media_root),
        object_storage_bucket: None,
        object_storage_cdn_base_url: None,
        cdn_cookie_domain: None,
        admin_api_enabled: true,
        token_hash_secret: None,
        allowed_origins: vec!["http://localhost:3000".to_string()],
        environment: RuntimeEnvironment::Development,
    };
    let storage = Storage::from_config(&config)?;
    let state = Arc::new(AppState::new(
        Database::from_sqlite(pool.clone()),
        storage,
        config,
        vec![HeaderValue::from_static("http://localhost:3000")],
    ));
    let creator = fetch_creator_profile(&pool, "crt-deepsaint").await?;
    reset_creator_live_state(&pool, &creator).await?;
    Ok((state, creator))
}
