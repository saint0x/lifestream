use super::*;

pub(crate) async fn fetch_creator_subscriber_tiers(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<CreatorSubscriberTier>> {
    let rows = sqlx::query(
        r#"
        SELECT id, tier_name, rank, monthly_price, subscriber_count, accent_color, status, retired_at
        FROM creator_subscriber_tiers
        WHERE creator_id = ?
        ORDER BY CASE WHEN status = 'active' THEN 0 ELSE 1 END ASC, rank ASC, monthly_price ASC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| CreatorSubscriberTier {
            id: row.get("id"),
            tier_name: row.get("tier_name"),
            rank: row.get("rank"),
            monthly_price: row.get("monthly_price"),
            subscriber_count: row.get("subscriber_count"),
            accent_color: row.get("accent_color"),
            status: row.get("status"),
            retired_at: row.get("retired_at"),
        })
        .collect())
}

pub(crate) async fn fetch_creator_subscriber_tier_by_id(
    pool: &SqlitePool,
    creator_id: &str,
    tier_id: &str,
) -> AppResult<CreatorSubscriberTier> {
    let row = sqlx::query(
        r#"
        SELECT id, tier_name, rank, monthly_price, subscriber_count, accent_color, status, retired_at
        FROM creator_subscriber_tiers
        WHERE creator_id = ? AND id = ?
        "#,
    )
    .bind(creator_id)
    .bind(tier_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(CreatorSubscriberTier {
        id: row.get("id"),
        tier_name: row.get("tier_name"),
        rank: row.get("rank"),
        monthly_price: row.get("monthly_price"),
        subscriber_count: row.get("subscriber_count"),
        accent_color: row.get("accent_color"),
        status: row.get("status"),
        retired_at: row.get("retired_at"),
    })
}

pub(crate) async fn next_creator_subscriber_tier_rank(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<i64> {
    let row = sqlx::query(
        "SELECT COALESCE(MAX(rank), 0) AS max_rank FROM creator_subscriber_tiers WHERE creator_id = ?",
    )
    .bind(creator_id)
    .fetch_one(pool)
    .await?;
    let max_rank: i64 = row.get("max_rank");
    Ok(max_rank + 1)
}

pub(crate) async fn normalize_creator_subscriber_tier_ranks(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    let rows = sqlx::query(
        "SELECT id FROM creator_subscriber_tiers WHERE creator_id = ? ORDER BY rank ASC, monthly_price ASC, rowid ASC",
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;
    for (index, row) in rows.into_iter().enumerate() {
        let tier_id: String = row.get("id");
        sqlx::query("UPDATE creator_subscriber_tiers SET rank = ? WHERE id = ? AND creator_id = ?")
            .bind((index + 1) as i64)
            .bind(&tier_id)
            .bind(creator_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub(crate) fn validate_creator_subscriber_tier_input(
    tier_name: &str,
    rank: Option<i64>,
    monthly_price: f64,
    accent_color: &str,
) -> AppResult<()> {
    if tier_name.trim().is_empty() {
        return Err(AppError::BadRequest("tierName is required".to_string()));
    }
    if tier_name.len() > 64 {
        return Err(AppError::BadRequest(
            "tierName must be 64 characters or fewer".to_string(),
        ));
    }
    if rank.is_some_and(|value| value <= 0) {
        return Err(AppError::BadRequest(
            "rank must be greater than zero".to_string(),
        ));
    }
    if monthly_price <= 0.0 {
        return Err(AppError::BadRequest(
            "monthlyPrice must be greater than zero".to_string(),
        ));
    }
    if !accent_color.starts_with('#') || accent_color.len() != 7 {
        return Err(AppError::BadRequest(
            "accentColor must be a 7-character hex color".to_string(),
        ));
    }
    Ok(())
}
