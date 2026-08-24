use super::*;
use crate::models::CreatorAttentionScore;
use chrono::{DateTime, Days, NaiveDate, Utc};
use std::collections::HashMap;

const CREATOR_ATTENTION_ALGORITHM_VERSION: &str = "cav-v1.0.0";
const QUALIFIED_VIEW_SECONDS: i64 = 60;
const BASELINE_VALUE_PER_QUALIFIED_VIEWER: f64 = 0.05;
const ATTENTION_ALPHA: f64 = 0.25;
const CREATOR_ATTENTION_RECONCILE_DAYS: u64 = 2;

#[derive(Default)]
struct AttentionViewerAggregate {
    watch_seconds: i64,
    session_count: i64,
    authenticated: bool,
    attributed: bool,
}

pub(crate) async fn fetch_analytics(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<AnalyticsPoint>> {
    let rows = sqlx::query(
        "SELECT date, viewers, watch_minutes, revenue, new_followers FROM analytics_points WHERE creator_id = ? ORDER BY date ASC",
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| AnalyticsPoint {
            date: row.get("date"),
            viewers: row.get("viewers"),
            watch_minutes: row.get("watch_minutes"),
            revenue: row.get("revenue"),
            new_followers: row.get("new_followers"),
        })
        .collect())
}

pub(super) async fn fetch_traffic_sources(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<TrafficSource>> {
    let rows = sqlx::query(
        r#"
        SELECT
            COALESCE(lvs.attribution_source, 'direct') AS source,
            COUNT(*) AS sessions
        FROM live_viewer_sessions lvs
        JOIN live_streams ls ON ls.id = lvs.stream_id
        JOIN streamers s ON s.id = ls.streamer_id
        JOIN creator_profiles cp ON cp.handle = s.handle
        WHERE cp.id = ?
        GROUP BY COALESCE(lvs.attribution_source, 'direct')
        ORDER BY sessions DESC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;
    let total_sessions = rows
        .iter()
        .map(|row| row.get::<i64, _>("sessions"))
        .sum::<i64>()
        .max(1);
    Ok(rows
        .into_iter()
        .map(|row| {
            let sessions = row.get::<i64, _>("sessions");
            TrafficSource {
                source: row.get("source"),
                sessions,
                share: sessions as f64 / total_sessions as f64,
            }
        })
        .collect())
}

pub(crate) async fn fetch_creator_attention_score(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorAttentionScore> {
    let day = Utc::now().date_naive();
    reconcile_creator_attention_rollup_for_day(pool, creator_id, day).await?;
    fetch_creator_attention_rollup_for_day(pool, creator_id, day).await
}

pub(crate) async fn reconcile_creator_attention_rollups(pool: &SqlitePool) -> AppResult<usize> {
    let creator_ids = fetch_creator_ids_for_attention_rollup(pool).await?;
    let today = Utc::now().date_naive();
    let mut updated = 0;
    for creator_id in creator_ids {
        for offset in 0..CREATOR_ATTENTION_RECONCILE_DAYS {
            let Some(day) = today.checked_sub_days(Days::new(offset)) else {
                continue;
            };
            reconcile_creator_attention_rollup_for_day(pool, &creator_id, day).await?;
            updated += 1;
        }
    }
    Ok(updated)
}

pub(crate) async fn reconcile_creator_attention_rollup_for_day(
    pool: &SqlitePool,
    creator_id: &str,
    day: NaiveDate,
) -> AppResult<CreatorAttentionScore> {
    let score = compute_creator_attention_score(pool, creator_id, Some(day)).await?;
    persist_creator_attention_rollup(pool, creator_id, day, &score).await?;
    Ok(score)
}

async fn compute_creator_attention_score(
    pool: &SqlitePool,
    creator_id: &str,
    day: Option<NaiveDate>,
) -> AppResult<CreatorAttentionScore> {
    let day_filter = day.map(|value| value.format("%Y-%m-%d").to_string());
    let rows = sqlx::query(
        r#"
        SELECT
            lvs.user_id,
            lvs.visitor_id,
            lvs.session_token_hash,
            lvs.connected_at,
            lvs.last_seen_at,
            lvs.disconnected_at,
            lvs.attribution_source
        FROM live_viewer_sessions lvs
        JOIN live_streams ls ON ls.id = lvs.stream_id
        JOIN streamers s ON s.id = ls.streamer_id
        JOIN creator_profiles cp ON cp.handle = s.handle
        WHERE cp.id = ?
          AND (? IS NULL OR substr(lvs.connected_at, 1, 10) = ?)
        "#,
    )
    .bind(creator_id)
    .bind(day_filter.as_deref())
    .bind(day_filter.as_deref())
    .fetch_all(pool)
    .await?;

    let mut viewers = HashMap::<String, AttentionViewerAggregate>::new();
    for row in rows {
        let user_id = row.get::<Option<String>, _>("user_id");
        let visitor_id = row.get::<Option<String>, _>("visitor_id");
        let session_token_hash = row.get::<String, _>("session_token_hash");
        let viewer_key = user_id
            .as_ref()
            .map(|id| format!("u:{id}"))
            .or_else(|| visitor_id.as_ref().map(|id| format!("v:{id}")))
            .unwrap_or_else(|| format!("s:{session_token_hash}"));
        let connected_at = row.get::<String, _>("connected_at");
        let last_seen_at = row.get::<String, _>("last_seen_at");
        let disconnected_at = row.get::<Option<String>, _>("disconnected_at");
        let watch_seconds = session_duration_seconds(
            &connected_at,
            disconnected_at.as_deref().unwrap_or(&last_seen_at),
        );
        let attribution_source = row.get::<Option<String>, _>("attribution_source");
        let aggregate = viewers.entry(viewer_key).or_default();
        aggregate.watch_seconds += watch_seconds;
        aggregate.session_count += 1;
        aggregate.authenticated |= user_id.is_some();
        aggregate.attributed |= attribution_source
            .as_deref()
            .is_some_and(|source| !source.trim().is_empty());
    }

    let measured_viewers = viewers.len() as i64;
    let measured_sessions = viewers
        .values()
        .map(|viewer| viewer.session_count)
        .sum::<i64>();
    let qualified = viewers
        .values()
        .filter(|viewer| viewer.watch_seconds >= QUALIFIED_VIEW_SECONDS)
        .collect::<Vec<_>>();
    let qualified_viewers = qualified.len() as i64;
    let total_qualified_seconds = qualified
        .iter()
        .map(|viewer| viewer.watch_seconds)
        .sum::<i64>();
    let average_watch_minutes = if qualified_viewers > 0 {
        total_qualified_seconds as f64 / 60.0 / qualified_viewers as f64
    } else {
        0.0
    };
    let attention_multiplier = 1.0 + ATTENTION_ALPHA * (1.0 + average_watch_minutes / 10.0).ln();

    let chat_participants =
        count_creator_chat_participants(pool, creator_id, day_filter.as_deref()).await?;
    let clip_requesters =
        count_creator_clip_requesters(pool, creator_id, day_filter.as_deref()).await?;
    let notified_users =
        count_creator_live_notify_users(pool, creator_id, day_filter.as_deref()).await?;
    let engagement_multiplier = engagement_multiplier(
        qualified_viewers,
        chat_participants,
        clip_requesters,
        notified_users,
    );

    let returning_viewers = qualified
        .iter()
        .filter(|viewer| viewer.session_count >= 2)
        .count() as i64;
    let returning_viewer_rate = ratio(returning_viewers, qualified_viewers);
    let retention_multiplier = 0.8 + 0.8 * returning_viewer_rate;
    let audience_quality_multiplier = audience_quality_multiplier(pool, creator_id).await?;
    let data_confidence_multiplier = data_confidence_multiplier(&qualified);
    let qualified_viewer_rate = ratio(qualified_viewers, measured_viewers);
    let creator_attention_value = qualified_viewers as f64
        * BASELINE_VALUE_PER_QUALIFIED_VIEWER
        * attention_multiplier
        * engagement_multiplier
        * retention_multiplier
        * audience_quality_multiplier
        * data_confidence_multiplier;
    let verified_viewer_score = verified_viewer_score(
        attention_multiplier,
        engagement_multiplier,
        retention_multiplier,
        audience_quality_multiplier,
        data_confidence_multiplier,
        qualified_viewer_rate,
    );

    Ok(CreatorAttentionScore {
        algorithm_version: CREATOR_ATTENTION_ALGORITHM_VERSION.to_string(),
        qualified_viewers,
        verified_viewer_score,
        creator_attention_value,
        baseline_value_per_qualified_viewer: BASELINE_VALUE_PER_QUALIFIED_VIEWER,
        average_watch_minutes,
        attention_multiplier,
        engagement_multiplier,
        retention_multiplier,
        audience_quality_multiplier,
        data_confidence_multiplier,
        qualified_viewer_rate,
        returning_viewer_rate,
        measured_sessions,
        measured_viewers,
    })
}

async fn fetch_creator_attention_rollup_for_day(
    pool: &SqlitePool,
    creator_id: &str,
    day: NaiveDate,
) -> AppResult<CreatorAttentionScore> {
    let day = day.format("%Y-%m-%d").to_string();
    let row = sqlx::query(
        r#"
        SELECT
            algorithm_version,
            qualified_viewers,
            verified_viewer_score,
            creator_attention_value,
            baseline_value_per_qualified_viewer,
            average_watch_minutes,
            attention_multiplier,
            engagement_multiplier,
            retention_multiplier,
            audience_quality_multiplier,
            data_confidence_multiplier,
            qualified_viewer_rate,
            returning_viewer_rate,
            measured_sessions,
            measured_viewers
        FROM creator_attention_daily
        WHERE creator_id = ? AND day = ? AND algorithm_version = ?
        "#,
    )
    .bind(creator_id)
    .bind(day)
    .bind(CREATOR_ATTENTION_ALGORITHM_VERSION)
    .fetch_optional(pool)
    .await?;

    Ok(row
        .map(row_to_creator_attention_score)
        .unwrap_or_else(empty_creator_attention_score))
}

async fn persist_creator_attention_rollup(
    pool: &SqlitePool,
    creator_id: &str,
    day: NaiveDate,
    score: &CreatorAttentionScore,
) -> AppResult<()> {
    let day = day.format("%Y-%m-%d").to_string();
    let updated_at = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO creator_attention_daily (
            creator_id,
            day,
            algorithm_version,
            qualified_viewers,
            verified_viewer_score,
            creator_attention_value,
            average_watch_minutes,
            attention_multiplier,
            engagement_multiplier,
            retention_multiplier,
            audience_quality_multiplier,
            data_confidence_multiplier,
            qualified_viewer_rate,
            returning_viewer_rate,
            measured_sessions,
            updated_at,
            measured_viewers,
            baseline_value_per_qualified_viewer
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(creator_id, day, algorithm_version) DO UPDATE SET
            qualified_viewers = excluded.qualified_viewers,
            verified_viewer_score = excluded.verified_viewer_score,
            creator_attention_value = excluded.creator_attention_value,
            average_watch_minutes = excluded.average_watch_minutes,
            attention_multiplier = excluded.attention_multiplier,
            engagement_multiplier = excluded.engagement_multiplier,
            retention_multiplier = excluded.retention_multiplier,
            audience_quality_multiplier = excluded.audience_quality_multiplier,
            data_confidence_multiplier = excluded.data_confidence_multiplier,
            qualified_viewer_rate = excluded.qualified_viewer_rate,
            returning_viewer_rate = excluded.returning_viewer_rate,
            measured_sessions = excluded.measured_sessions,
            updated_at = excluded.updated_at,
            measured_viewers = excluded.measured_viewers,
            baseline_value_per_qualified_viewer = excluded.baseline_value_per_qualified_viewer
        "#,
    )
    .bind(creator_id)
    .bind(day)
    .bind(&score.algorithm_version)
    .bind(score.qualified_viewers)
    .bind(score.verified_viewer_score)
    .bind(score.creator_attention_value)
    .bind(score.average_watch_minutes)
    .bind(score.attention_multiplier)
    .bind(score.engagement_multiplier)
    .bind(score.retention_multiplier)
    .bind(score.audience_quality_multiplier)
    .bind(score.data_confidence_multiplier)
    .bind(score.qualified_viewer_rate)
    .bind(score.returning_viewer_rate)
    .bind(score.measured_sessions)
    .bind(updated_at)
    .bind(score.measured_viewers)
    .bind(score.baseline_value_per_qualified_viewer)
    .execute(pool)
    .await?;
    Ok(())
}

async fn fetch_creator_ids_for_attention_rollup(pool: &SqlitePool) -> AppResult<Vec<String>> {
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT cp.id AS creator_id
        FROM creator_profiles cp
        WHERE EXISTS (
            SELECT 1
            FROM live_viewer_sessions lvs
            JOIN live_streams ls ON ls.id = lvs.stream_id
            JOIN streamers s ON s.id = ls.streamer_id
            WHERE s.handle = cp.handle
        )
        OR EXISTS (
            SELECT 1
            FROM creator_attention_daily cad
            WHERE cad.creator_id = cp.id
        )
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| row.get::<String, _>("creator_id"))
        .collect())
}

fn row_to_creator_attention_score(row: sqlx::sqlite::SqliteRow) -> CreatorAttentionScore {
    CreatorAttentionScore {
        algorithm_version: row.get("algorithm_version"),
        qualified_viewers: row.get("qualified_viewers"),
        verified_viewer_score: row.get("verified_viewer_score"),
        creator_attention_value: row.get("creator_attention_value"),
        baseline_value_per_qualified_viewer: row.get("baseline_value_per_qualified_viewer"),
        average_watch_minutes: row.get("average_watch_minutes"),
        attention_multiplier: row.get("attention_multiplier"),
        engagement_multiplier: row.get("engagement_multiplier"),
        retention_multiplier: row.get("retention_multiplier"),
        audience_quality_multiplier: row.get("audience_quality_multiplier"),
        data_confidence_multiplier: row.get("data_confidence_multiplier"),
        qualified_viewer_rate: row.get("qualified_viewer_rate"),
        returning_viewer_rate: row.get("returning_viewer_rate"),
        measured_sessions: row.get("measured_sessions"),
        measured_viewers: row.get("measured_viewers"),
    }
}

pub(crate) fn empty_creator_attention_score() -> CreatorAttentionScore {
    CreatorAttentionScore {
        algorithm_version: CREATOR_ATTENTION_ALGORITHM_VERSION.to_string(),
        qualified_viewers: 0,
        verified_viewer_score: 0.0,
        creator_attention_value: 0.0,
        baseline_value_per_qualified_viewer: BASELINE_VALUE_PER_QUALIFIED_VIEWER,
        average_watch_minutes: 0.0,
        attention_multiplier: 1.0,
        engagement_multiplier: 1.0,
        retention_multiplier: 0.8,
        audience_quality_multiplier: 1.0,
        data_confidence_multiplier: 0.0,
        qualified_viewer_rate: 0.0,
        returning_viewer_rate: 0.0,
        measured_sessions: 0,
        measured_viewers: 0,
    }
}

pub(super) async fn fetch_top_content(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<TopContent>> {
    let rows = sqlx::query(
        "SELECT id, title, kind, views, watch_hours, trend, thumbnail FROM top_content WHERE creator_id = ? ORDER BY views DESC",
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| TopContent {
            id: row.get("id"),
            title: row.get("title"),
            kind: row.get("kind"),
            views: row.get("views"),
            watch_hours: row.get("watch_hours"),
            trend: row.get("trend"),
            thumbnail: row.get("thumbnail"),
        })
        .collect())
}

fn session_duration_seconds(start: &str, end: &str) -> i64 {
    let Ok(start) = DateTime::parse_from_rfc3339(start) else {
        return 0;
    };
    let Ok(end) = DateTime::parse_from_rfc3339(end) else {
        return 0;
    };
    (end - start).num_seconds().max(0)
}

fn ratio(numerator: i64, denominator: i64) -> f64 {
    if denominator <= 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn engagement_multiplier(
    qualified_viewers: i64,
    chat_participants: i64,
    clip_requesters: i64,
    notified_users: i64,
) -> f64 {
    if qualified_viewers <= 0 {
        return 1.0;
    }
    let chat = (chat_participants as f64 / qualified_viewers as f64 / 0.25).min(1.0);
    let clips = (clip_requesters as f64 / qualified_viewers as f64 / 0.05).min(1.0);
    let notify = (notified_users as f64 / qualified_viewers as f64 / 0.15).min(1.0);
    let intensity = 0.55 * chat + 0.20 * clips + 0.25 * notify;
    1.0 + 0.40 * intensity.clamp(0.0, 1.0)
}

fn data_confidence_multiplier(qualified: &[&AttentionViewerAggregate]) -> f64 {
    if qualified.is_empty() {
        return 0.0;
    }
    let authenticated = qualified
        .iter()
        .filter(|viewer| viewer.authenticated)
        .count() as i64;
    let attributed = qualified.iter().filter(|viewer| viewer.attributed).count() as i64;
    let identity_confidence = 0.55 + 0.45 * ratio(authenticated, qualified.len() as i64);
    let attribution_confidence = 0.75 + 0.25 * ratio(attributed, qualified.len() as i64);
    (identity_confidence * attribution_confidence).clamp(0.0, 1.0)
}

fn verified_viewer_score(
    attention: f64,
    engagement: f64,
    retention: f64,
    quality: f64,
    confidence: f64,
    qualified_rate: f64,
) -> f64 {
    if qualified_rate <= 0.0 || confidence <= 0.0 {
        return 0.0;
    }
    let attention_score = ((attention - 1.0) / 0.65).clamp(0.0, 1.0);
    let engagement_score = ((engagement - 1.0) / 0.40).clamp(0.0, 1.0);
    let retention_score = ((retention - 0.8) / 0.8).clamp(0.0, 1.0);
    let quality_score = ((quality - 0.8) / 0.7).clamp(0.0, 1.0);
    let confidence_score = confidence.clamp(0.0, 1.0);
    let score = 100.0
        * (0.25 * attention_score
            + 0.20 * engagement_score
            + 0.20 * retention_score
            + 0.15 * quality_score
            + 0.15 * confidence_score
            + 0.05 * qualified_rate.clamp(0.0, 1.0));
    score.clamp(0.0, 100.0)
}

async fn audience_quality_multiplier(pool: &SqlitePool, creator_id: &str) -> AppResult<f64> {
    let row = sqlx::query("SELECT default_category FROM creator_profiles WHERE id = ?")
        .bind(creator_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    let category = row
        .get::<String, _>("default_category")
        .trim()
        .to_ascii_lowercase();
    Ok(match category.as_str() {
        "tech" | "technology" => 1.35,
        "software" | "ai" | "developer" | "dev" => 1.50,
        "gaming" => 1.10,
        "sports" | "outdoor" | "lifestyle" => 1.15,
        _ => 1.00,
    })
}

async fn count_creator_chat_participants(
    pool: &SqlitePool,
    creator_id: &str,
    day: Option<&str>,
) -> AppResult<i64> {
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT cm.user_id
        FROM chat_messages cm
        JOIN live_streams ls ON ls.id = cm.stream_id
        JOIN streamers s ON s.id = ls.streamer_id
        JOIN creator_profiles cp ON cp.handle = s.handle
        WHERE cp.id = ?
          AND cm.user_id IS NOT NULL
          AND (? IS NULL OR substr(cm.sent_at, 1, 10) = ?)
        "#,
    )
    .bind(creator_id)
    .bind(day)
    .bind(day)
    .fetch_all(pool)
    .await?;
    Ok(rows.len() as i64)
}

async fn count_creator_clip_requesters(
    pool: &SqlitePool,
    creator_id: &str,
    day: Option<&str>,
) -> AppResult<i64> {
    count_distinct_live_request_users(pool, creator_id, "live_stream_clip_requests", day).await
}

async fn count_creator_live_notify_users(
    pool: &SqlitePool,
    creator_id: &str,
    day: Option<&str>,
) -> AppResult<i64> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(DISTINCT pref.user_id) AS count
        FROM live_stream_notification_preferences pref
        JOIN streamers s ON s.id = pref.streamer_id
        JOIN creator_profiles cp ON cp.handle = s.handle
        WHERE cp.id = ?
          AND (? IS NULL OR substr(pref.created_at, 1, 10) = ?)
        "#,
    )
    .bind(creator_id)
    .bind(day)
    .bind(day)
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

async fn count_distinct_live_request_users(
    pool: &SqlitePool,
    creator_id: &str,
    table_name: &str,
    day: Option<&str>,
) -> AppResult<i64> {
    let table_name = match table_name {
        "live_stream_clip_requests" => "live_stream_clip_requests",
        _ => return Ok(0),
    };
    let rows = sqlx::query(&format!(
        r#"
        SELECT DISTINCT req.user_id
        FROM {table_name} req
        JOIN live_streams ls ON ls.id = req.stream_id
        JOIN streamers s ON s.id = ls.streamer_id
        JOIN creator_profiles cp ON cp.handle = s.handle
        WHERE cp.id = ?
          AND req.user_id IS NOT NULL
          AND (? IS NULL OR substr(req.created_at, 1, 10) = ?)
        "#
    ))
    .bind(creator_id)
    .bind(day)
    .bind(day)
    .fetch_all(pool)
    .await?;
    Ok(rows.len() as i64)
}

pub(crate) async fn fetch_revenue_entries(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<RevenueEntry>> {
    let rows = sqlx::query(
        "SELECT id, date, source, description, amount FROM revenue_entries WHERE creator_id = ? ORDER BY date DESC",
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| RevenueEntry {
            id: row.get("id"),
            date: row.get("date"),
            source: row.get("source"),
            description: row.get("description"),
            amount: row.get("amount"),
        })
        .collect())
}

pub(crate) fn summarize_creator_analytics(analytics: &[AnalyticsPoint]) -> CreatorAnalyticsSummary {
    CreatorAnalyticsSummary {
        window_days: analytics.len() as i64,
        total_viewers: analytics.iter().map(|point| point.viewers).sum(),
        total_watch_minutes: analytics.iter().map(|point| point.watch_minutes).sum(),
        total_revenue: analytics.iter().map(|point| point.revenue).sum(),
        total_new_followers: analytics.iter().map(|point| point.new_followers).sum(),
    }
}

pub(crate) fn summarize_creator_revenue(
    analytics: &[AnalyticsPoint],
    revenue: &[RevenueEntry],
    subscriber_tiers: &[CreatorSubscriberTier],
) -> CreatorRevenueSummary {
    let total_subscribers = subscriber_tiers
        .iter()
        .map(|tier| tier.subscriber_count)
        .sum::<i64>();
    let weighted_price_total = subscriber_tiers
        .iter()
        .map(|tier| tier.monthly_price * tier.subscriber_count as f64)
        .sum::<f64>();
    let blended_monthly_price = if total_subscribers > 0 {
        weighted_price_total / total_subscribers as f64
    } else {
        0.0
    };

    let positive_total = revenue
        .iter()
        .filter(|entry| entry.amount > 0.0)
        .map(|entry| entry.amount)
        .sum::<f64>();
    let payout_total = revenue
        .iter()
        .filter(|entry| entry.source == "payout")
        .map(|entry| entry.amount.abs())
        .sum::<f64>();
    let total_earnings_30d = analytics.iter().map(|point| point.revenue).sum::<f64>();
    let estimated_next_payout = (total_earnings_30d - payout_total).max(0.0);

    let mut breakdown = Vec::new();
    for source in ["subscriptions", "ads", "tips", "clips", "payout"] {
        let amount = revenue
            .iter()
            .filter(|entry| entry.source == source && entry.amount > 0.0)
            .map(|entry| entry.amount)
            .sum::<f64>();
        let share = if positive_total > 0.0 {
            amount / positive_total
        } else {
            0.0
        };
        breakdown.push(CreatorRevenueBreakdownEntry {
            source: source.to_string(),
            amount,
            share,
        });
    }

    CreatorRevenueSummary {
        total_earnings_30d,
        total_subscribers,
        blended_monthly_price,
        estimated_next_payout,
        breakdown,
    }
}
