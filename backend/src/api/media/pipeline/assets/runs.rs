use super::*;

pub(crate) async fn fetch_media_processing_runs(
    pool: &SqlitePool,
    creator_id: &str,
    asset_id: &str,
) -> AppResult<Vec<MediaProcessingRun>> {
    let rows = sqlx::query(
        r#"
        SELECT id, stage, status, details_json, started_at, completed_at
        FROM media_processing_runs
        WHERE creator_id = ? AND asset_id = ?
        ORDER BY started_at DESC
        "#,
    )
    .bind(creator_id)
    .bind(asset_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| MediaProcessingRun {
            id: row.get("id"),
            stage: row.get("stage"),
            status: row.get("status"),
            details: serde_json::from_str(&row.get::<String, _>("details_json"))
                .unwrap_or(json!({})),
            started_at: row.get("started_at"),
            completed_at: row.get("completed_at"),
        })
        .collect())
}

pub(crate) async fn start_media_processing_run(
    pool: &SqlitePool,
    creator_id: &str,
    job_id: &str,
    asset_id: &str,
    stage: &str,
    details: Value,
) -> AppResult<String> {
    let id = format!("mpr-{}", Uuid::new_v4().simple());
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO media_processing_runs (
            id, creator_id, upload_job_id, asset_id, stage, status, details_json, started_at, completed_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(creator_id)
    .bind(job_id)
    .bind(asset_id)
    .bind(stage)
    .bind("running")
    .bind(details.to_string())
    .bind(&now)
    .bind(Option::<String>::None)
    .execute(pool)
    .await?;
    Ok(id)
}

pub(crate) async fn start_media_processing_run_for_database(
    database: &crate::db::Database,
    creator_id: &str,
    job_id: &str,
    asset_id: &str,
    stage: &str,
    details: Value,
) -> AppResult<String> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return start_postgres_media_processing_run(
            pool, creator_id, job_id, asset_id, stage, details,
        )
        .await;
    }
    start_media_processing_run(
        database.try_sqlite_adapter()?,
        creator_id,
        job_id,
        asset_id,
        stage,
        details,
    )
    .await
}

async fn start_postgres_media_processing_run(
    pool: &sqlx::PgPool,
    creator_id: &str,
    job_id: &str,
    asset_id: &str,
    stage: &str,
    details: Value,
) -> AppResult<String> {
    let id = format!("mpr-{}", Uuid::new_v4().simple());
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO media_processing_runs (
            id, creator_id, upload_job_id, asset_id, stage, status, details_json, started_at, completed_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(&id)
    .bind(creator_id)
    .bind(job_id)
    .bind(asset_id)
    .bind(stage)
    .bind("running")
    .bind(details.to_string())
    .bind(&now)
    .bind(Option::<String>::None)
    .execute(pool)
    .await?;
    Ok(id)
}

pub(crate) async fn finish_media_processing_run(
    pool: &SqlitePool,
    run_id: &str,
    status: &str,
    details: Value,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE media_processing_runs SET status = ?, details_json = ?, completed_at = ? WHERE id = ?",
    )
    .bind(status)
    .bind(details.to_string())
    .bind(&now)
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn finish_media_processing_run_for_database(
    database: &crate::db::Database,
    run_id: &str,
    status: &str,
    details: Value,
) -> AppResult<()> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return finish_postgres_media_processing_run(pool, run_id, status, details).await;
    }
    finish_media_processing_run(database.try_sqlite_adapter()?, run_id, status, details).await
}

async fn finish_postgres_media_processing_run(
    pool: &sqlx::PgPool,
    run_id: &str,
    status: &str,
    details: Value,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE media_processing_runs SET status = $1, details_json = $2, completed_at = $3 WHERE id = $4",
    )
    .bind(status)
    .bind(details.to_string())
    .bind(&now)
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}
