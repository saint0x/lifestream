use super::*;

fn creator_profile_from_row(
    row: sqlx::sqlite::SqliteRow,
    subscribers: i64,
    monthly_viewers: i64,
    total_watch_hours: i64,
) -> AppResult<CreatorProfile> {
    Ok(CreatorProfile {
        id: row.get("id"),
        user_id: row.get("user_id"),
        handle: row.get("handle"),
        display_name: row.get("display_name"),
        avatar: row.get("avatar"),
        banner: row.get("banner"),
        tagline: row.get("tagline"),
        bio: row.get("bio"),
        partner_status: row.get("partner_status"),
        joined_at: row.get("joined_at"),
        stream_key: row.get("stream_key"),
        rtmp_url: row.get("rtmp_url"),
        default_category: row.get("default_category"),
        default_tags: from_json(row.get::<String, _>("default_tags_json"))?,
        followers: row.get("followers"),
        subscribers,
        monthly_viewers,
        total_watch_hours,
        live_status: row.get("live_status"),
        current_broadcast_id: row.get("current_broadcast_id"),
    })
}

async fn fetch_creator_profile_row(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<sqlx::sqlite::SqliteRow> {
    sqlx::query(
        r#"
        SELECT id, user_id, handle, display_name, avatar, banner, tagline, bio, partner_status,
               joined_at, stream_key, rtmp_url, default_category, default_tags_json,
               followers, subscribers, monthly_viewers, total_watch_hours, live_status, current_broadcast_id
        FROM creator_profiles
        WHERE id = ?
        "#,
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn fetch_creator_profile(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorProfile> {
    let row = fetch_creator_profile_row(pool, creator_id).await?;
    let subscriber_tiers = fetch_creator_subscriber_tiers(pool, creator_id).await?;
    let subscribers = subscriber_tiers
        .iter()
        .map(|tier| tier.subscriber_count)
        .sum::<i64>();
    let analytics = fetch_analytics(pool, creator_id).await?;
    let analytics_summary = summarize_creator_analytics(&analytics);
    let vod_watch_hours = sqlx::query(
        "SELECT COALESCE(SUM(watch_hours), 0) AS total FROM uploads WHERE creator_id = ?",
    )
    .bind(creator_id)
    .fetch_one(pool)
    .await?
    .get::<i64, _>("total");
    let total_watch_hours = row
        .get::<i64, _>("total_watch_hours")
        .max(vod_watch_hours)
        .max(analytics_summary.total_watch_minutes / 60);
    let monthly_viewers = analytics_summary
        .total_viewers
        .max(row.get("monthly_viewers"));

    creator_profile_from_row(row, subscribers, monthly_viewers, total_watch_hours)
}

pub(crate) async fn fetch_creator_profile_persisted(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorProfile> {
    let row = fetch_creator_profile_row(pool, creator_id).await?;
    let subscribers = row.get("subscribers");
    let monthly_viewers = row.get("monthly_viewers");
    let total_watch_hours = row.get("total_watch_hours");
    creator_profile_from_row(row, subscribers, monthly_viewers, total_watch_hours)
}

pub(crate) async fn fetch_creator_profile_by_stream_key(
    pool: &SqlitePool,
    stream_key: &str,
) -> AppResult<CreatorProfile> {
    let row = sqlx::query(
        r#"
        SELECT id
        FROM creator_profiles
        WHERE stream_key = ?
        "#,
    )
    .bind(stream_key)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;
    let creator_id: String = row.get("id");
    fetch_creator_profile(pool, &creator_id).await
}
