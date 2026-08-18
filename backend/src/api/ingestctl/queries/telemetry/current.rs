use super::*;

pub(crate) async fn fetch_current_operational_telemetry(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<(Option<i64>, Option<f64>)> {
    let row = sqlx::query(
        r#"
        SELECT cpu_percent, free_disk_gb
        FROM creator_live_settings
        WHERE creator_id = ?
        "#,
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?;

    Ok(match row {
        Some(row) => (Some(row.get("cpu_percent")), Some(row.get("free_disk_gb"))),
        None => (None, None),
    })
}
