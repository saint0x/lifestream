use super::*;

pub(crate) async fn publish_due_scheduled_upload_releases(
    pool: &SqlitePool,
    creator_filter: Option<&str>,
    upload_filter: Option<&str>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"
        SELECT id, creator_id, title, visibility, release_at
        FROM uploads
        WHERE status = 'scheduled'
          AND visibility IN ('public', 'unlisted')
          AND release_at IS NOT NULL
          AND release_at <= ?
          AND (? IS NULL OR creator_id = ?)
          AND (? IS NULL OR id = ?)
        "#,
    )
    .bind(&now)
    .bind(creator_filter)
    .bind(creator_filter)
    .bind(upload_filter)
    .bind(upload_filter)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let upload_id: String = row.get("id");
        let creator_id: String = row.get("creator_id");
        let title: String = row.get("title");
        let visibility: String = row.get("visibility");
        let updated = sqlx::query(
            "UPDATE uploads SET status = 'published', published_at = COALESCE(published_at, ?) WHERE id = ? AND status = 'scheduled'",
        )
        .bind(&now)
        .bind(&upload_id)
        .execute(pool)
        .await?;
        if updated.rows_affected() == 0 {
            continue;
        }
        sqlx::query(
            "UPDATE media_assets SET status = 'published', visibility = ?, updated_at = ? WHERE upload_id = ? AND creator_id = ?",
        )
        .bind(&visibility)
        .bind(&now)
        .bind(&upload_id)
        .bind(&creator_id)
        .execute(pool)
        .await?;
        let _ = enqueue_notification_event(
            pool,
            "scheduled_release_published",
            &format!("{title} is now live."),
            None,
            Some("scheduler"),
            Some(&creator_id),
            None,
            None,
            json!({
                "uploadId": upload_id,
                "publishedAt": now,
            }),
            &[],
            std::slice::from_ref(&creator_id),
        )
        .await;
    }

    Ok(())
}

pub(crate) async fn schedule_media_processing(
    state: SharedState,
    creator_id: String,
    job_id: String,
) {
    tokio::spawn(async move {
        let result = process_media_job(state.clone(), &creator_id, &job_id).await;
        if let Err((error, lease_updated_at)) = result {
            let (message, retryable) = classify_media_processing_error(&error);
            let _ = fail_media_job_for_lease_in_database(
                &state.db,
                &creator_id,
                &job_id,
                &message,
                retryable,
                Some(&lease_updated_at),
            )
            .await;
        }
    });
}
