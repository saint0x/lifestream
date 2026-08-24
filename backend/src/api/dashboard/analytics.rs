use super::*;
use crate::db::Database;
use crate::models::CreatorAttentionScore;
use chrono::{DateTime, Days, NaiveDate, Utc};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};

const CREATOR_ATTENTION_ALGORITHM_VERSION: &str = "cav-v2.0.0";
const QUALIFIED_VIEW_SECONDS: i64 = 90;
const BASELINE_VALUE_PER_QUALIFIED_VIEWER: f64 = 0.05;
const ATTENTION_ALPHA: f64 = 0.25;
const CREATOR_ATTENTION_RECONCILE_DAYS: u64 = 2;
const MAX_SINGLE_CONTENT_PROGRESS_SECONDS: i64 = 21_600;

#[derive(Default)]
struct AttentionViewerAggregate {
    watch_seconds: i64,
    session_count: i64,
    event_count: i64,
    playback_progress_events: i64,
    watchlist_actions: i64,
    content_keys: HashSet<String>,
    authenticated: bool,
    attributed: bool,
    has_stable_visitor_id: bool,
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

pub(crate) async fn reconcile_creator_attention_rollups(database: &Database) -> AppResult<usize> {
    if let Ok(pool) = database.try_postgres_adapter() {
        return reconcile_postgres_creator_attention_rollups(pool).await;
    }
    reconcile_sqlite_creator_attention_rollups(database.try_sqlite_adapter()?).await
}

async fn reconcile_sqlite_creator_attention_rollups(pool: &SqlitePool) -> AppResult<usize> {
    let creator_ids = fetch_sqlite_creator_ids_for_attention_rollup(pool).await?;
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

async fn reconcile_postgres_creator_attention_rollups(pool: &PgPool) -> AppResult<usize> {
    let creator_ids = fetch_postgres_creator_ids_for_attention_rollup(pool).await?;
    let today = Utc::now().date_naive();
    let mut updated = 0;
    for creator_id in creator_ids {
        for offset in 0..CREATOR_ATTENTION_RECONCILE_DAYS {
            let Some(day) = today.checked_sub_days(Days::new(offset)) else {
                continue;
            };
            reconcile_postgres_creator_attention_rollup_for_day(pool, &creator_id, day).await?;
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
    let mut viewers = HashMap::<String, AttentionViewerAggregate>::new();
    collect_sqlite_live_attention(pool, creator_id, day_filter.as_deref(), &mut viewers).await?;
    collect_sqlite_viewer_event_attention(pool, creator_id, day_filter.as_deref(), &mut viewers)
        .await?;

    build_creator_attention_score(pool, creator_id, day_filter.as_deref(), viewers).await
}

async fn reconcile_postgres_creator_attention_rollup_for_day(
    pool: &PgPool,
    creator_id: &str,
    day: NaiveDate,
) -> AppResult<CreatorAttentionScore> {
    let score = compute_postgres_creator_attention_score(pool, creator_id, Some(day)).await?;
    persist_postgres_creator_attention_rollup(pool, creator_id, day, &score).await?;
    Ok(score)
}

async fn compute_postgres_creator_attention_score(
    pool: &PgPool,
    creator_id: &str,
    day: Option<NaiveDate>,
) -> AppResult<CreatorAttentionScore> {
    let day_filter = day.map(|value| value.format("%Y-%m-%d").to_string());
    let mut viewers = HashMap::<String, AttentionViewerAggregate>::new();
    collect_postgres_live_attention(pool, creator_id, day_filter.as_deref(), &mut viewers).await?;
    collect_postgres_viewer_event_attention(pool, creator_id, day_filter.as_deref(), &mut viewers)
        .await?;

    build_postgres_creator_attention_score(pool, creator_id, day_filter.as_deref(), viewers).await
}

async fn collect_sqlite_live_attention(
    pool: &SqlitePool,
    creator_id: &str,
    day_filter: Option<&str>,
    viewers: &mut HashMap<String, AttentionViewerAggregate>,
) -> AppResult<()> {
    let rows = sqlx::query(
        r#"
        SELECT
            lvs.user_id,
            lvs.visitor_id,
            lvs.session_token_hash,
            lvs.connected_at,
            lvs.last_seen_at,
            lvs.disconnected_at,
            lvs.attribution_source,
            lvs.stream_id
        FROM live_viewer_sessions lvs
        JOIN live_streams ls ON ls.id = lvs.stream_id
        JOIN streamers s ON s.id = ls.streamer_id
        JOIN creator_profiles cp ON cp.handle = s.handle
        WHERE cp.id = ?
          AND (? IS NULL OR substr(lvs.connected_at, 1, 10) = ?)
        "#,
    )
    .bind(creator_id)
    .bind(day_filter)
    .bind(day_filter)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let user_id = row.get::<Option<String>, _>("user_id");
        let visitor_id = row.get::<Option<String>, _>("visitor_id");
        let session_token_hash = row.get::<String, _>("session_token_hash");
        let viewer_key = resolve_viewer_key(
            user_id.as_deref(),
            visitor_id.as_deref(),
            &session_token_hash,
        );
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
        aggregate
            .content_keys
            .insert(format!("live:{}", row.get::<String, _>("stream_id")));
        aggregate.authenticated |= user_id.is_some();
        aggregate.has_stable_visitor_id |= visitor_id.is_some();
        aggregate.attributed |= attribution_source
            .as_deref()
            .is_some_and(|source| !source.trim().is_empty());
    }
    Ok(())
}

async fn collect_postgres_live_attention(
    pool: &PgPool,
    creator_id: &str,
    day_filter: Option<&str>,
    viewers: &mut HashMap<String, AttentionViewerAggregate>,
) -> AppResult<()> {
    let rows = sqlx::query(
        r#"
        SELECT
            lvs.user_id,
            lvs.visitor_id,
            lvs.session_token_hash,
            lvs.connected_at,
            lvs.last_seen_at,
            lvs.disconnected_at,
            lvs.attribution_source,
            lvs.stream_id
        FROM live_viewer_sessions lvs
        JOIN live_streams ls ON ls.id = lvs.stream_id
        JOIN streamers s ON s.id = ls.streamer_id
        JOIN creator_profiles cp ON cp.handle = s.handle
        WHERE cp.id = $1
          AND ($2 IS NULL OR substr(lvs.connected_at, 1, 10) = $2)
        "#,
    )
    .bind(creator_id)
    .bind(day_filter)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let user_id = row.get::<Option<String>, _>("user_id");
        let visitor_id = row.get::<Option<String>, _>("visitor_id");
        let session_token_hash = row.get::<String, _>("session_token_hash");
        let viewer_key = resolve_viewer_key(
            user_id.as_deref(),
            visitor_id.as_deref(),
            &session_token_hash,
        );
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
        aggregate
            .content_keys
            .insert(format!("live:{}", row.get::<String, _>("stream_id")));
        aggregate.authenticated |= user_id.is_some();
        aggregate.has_stable_visitor_id |= visitor_id.is_some();
        aggregate.attributed |= attribution_source
            .as_deref()
            .is_some_and(|source| !source.trim().is_empty());
    }
    Ok(())
}

async fn collect_sqlite_viewer_event_attention(
    pool: &SqlitePool,
    creator_id: &str,
    day_filter: Option<&str>,
    viewers: &mut HashMap<String, AttentionViewerAggregate>,
) -> AppResult<()> {
    let rows = sqlx::query(
        r#"
        WITH event_creator AS (
            SELECT DISTINCT cp.id AS creator_id, ve.id AS event_id, ve.user_id, ve.visitor_id,
                   ve.event_type, COALESCE(ve.episode_id, ve.content_id, ve.stream_id, ve.path, ve.id) AS content_key,
                   ve.progress_sec, ve.duration_sec, ve.watch_time_ms,
                   COALESCE(ve.utm_source, ve.utm_campaign, ve.referrer_url, ve.landing_url, ve.initial_referrer_url) AS attribution_marker,
                   ve.occurred_at
            FROM viewer_events ve
            JOIN live_streams ls ON ls.id = ve.stream_id
            JOIN streamers st ON st.id = ls.streamer_id
            JOIN creator_profiles cp ON cp.handle = st.handle

            UNION

            SELECT DISTINCT cp.id AS creator_id, ve.id AS event_id, ve.user_id, ve.visitor_id,
                   ve.event_type, COALESCE(ve.episode_id, ve.content_id, e.id, ve.path, ve.id) AS content_key,
                   ve.progress_sec, ve.duration_sec, ve.watch_time_ms,
                   COALESCE(ve.utm_source, ve.utm_campaign, ve.referrer_url, ve.landing_url, ve.initial_referrer_url) AS attribution_marker,
                   ve.occurred_at
            FROM viewer_events ve
            JOIN episodes e ON e.id = ve.episode_id OR (ve.content_kind = 'episode' AND e.id = ve.content_id)
            JOIN content_credits cc ON cc.content_kind = 'series' AND cc.content_id = e.series_id
            JOIN person_profiles pp ON pp.id = cc.person_id
            JOIN creator_profiles cp ON lower(replace(cp.handle, '-', '')) = lower(replace(pp.slug, '-', ''))

            UNION

            SELECT DISTINCT cp.id AS creator_id, ve.id AS event_id, ve.user_id, ve.visitor_id,
                   ve.event_type, COALESCE(ve.content_id, ve.path, ve.id) AS content_key,
                   ve.progress_sec, ve.duration_sec, ve.watch_time_ms,
                   COALESCE(ve.utm_source, ve.utm_campaign, ve.referrer_url, ve.landing_url, ve.initial_referrer_url) AS attribution_marker,
                   ve.occurred_at
            FROM viewer_events ve
            JOIN content_credits cc ON cc.content_kind = ve.content_kind AND cc.content_id = ve.content_id
            JOIN person_profiles pp ON pp.id = cc.person_id
            JOIN creator_profiles cp ON lower(replace(cp.handle, '-', '')) = lower(replace(pp.slug, '-', ''))
            WHERE ve.content_kind IN ('series', 'film')

            UNION

            SELECT DISTINCT cp.id AS creator_id, ve.id AS event_id, ve.user_id, ve.visitor_id,
                   ve.event_type, COALESCE(ve.content_id, ve.path, ve.id) AS content_key,
                   ve.progress_sec, ve.duration_sec, ve.watch_time_ms,
                   COALESCE(ve.utm_source, ve.utm_campaign, ve.referrer_url, ve.landing_url, ve.initial_referrer_url) AS attribution_marker,
                   ve.occurred_at
            FROM viewer_events ve
            JOIN content_credits cc ON cc.content_kind IN ('series', 'film') AND cc.content_id = ve.content_id
            JOIN person_profiles pp ON pp.id = cc.person_id
            JOIN creator_profiles cp ON lower(replace(cp.handle, '-', '')) = lower(replace(pp.slug, '-', ''))
            WHERE ve.content_id IS NOT NULL
              AND (ve.content_kind IS NULL OR ve.content_kind NOT IN ('series', 'film', 'episode'))

            UNION

            SELECT DISTINCT cp.id AS creator_id, ve.id AS event_id, ve.user_id, ve.visitor_id,
                   ve.event_type, COALESCE(ve.episode_id, ve.content_id, ve.path, ve.id) AS content_key,
                   ve.progress_sec, ve.duration_sec, ve.watch_time_ms,
                   COALESCE(ve.utm_source, ve.utm_campaign, ve.referrer_url, ve.landing_url, ve.initial_referrer_url) AS attribution_marker,
                   ve.occurred_at
            FROM viewer_events ve
            JOIN uploads u ON u.id = ve.content_id
            JOIN creator_profiles cp ON cp.id = u.creator_id
        )
        SELECT event_id, user_id, visitor_id, event_type, content_key, progress_sec,
               duration_sec, watch_time_ms, attribution_marker
        FROM event_creator
        WHERE creator_id = ?
          AND (? IS NULL OR substr(occurred_at, 1, 10) = ?)
        "#,
    )
    .bind(creator_id)
    .bind(day_filter)
    .bind(day_filter)
    .fetch_all(pool)
    .await?;

    let mut progress_by_viewer_content = HashMap::<(String, String), i64>::new();
    for row in rows {
        record_event_attention(
            row.get::<String, _>("event_id"),
            row.get::<Option<String>, _>("user_id"),
            row.get::<String, _>("visitor_id"),
            row.get::<String, _>("event_type"),
            row.get::<String, _>("content_key"),
            row.get::<Option<i64>, _>("progress_sec"),
            row.get::<Option<i64>, _>("duration_sec"),
            row.get::<Option<i64>, _>("watch_time_ms"),
            row.get::<Option<String>, _>("attribution_marker"),
            viewers,
            &mut progress_by_viewer_content,
        );
    }
    apply_progress_attention(viewers, progress_by_viewer_content);
    Ok(())
}

async fn collect_postgres_viewer_event_attention(
    pool: &PgPool,
    creator_id: &str,
    day_filter: Option<&str>,
    viewers: &mut HashMap<String, AttentionViewerAggregate>,
) -> AppResult<()> {
    let rows = sqlx::query(
        r#"
        WITH event_creator AS (
            SELECT DISTINCT cp.id AS creator_id, ve.id AS event_id, ve.user_id, ve.visitor_id,
                   ve.event_type, COALESCE(ve.episode_id, ve.content_id, ve.stream_id, ve.path, ve.id) AS content_key,
                   ve.progress_sec, ve.duration_sec, ve.watch_time_ms,
                   COALESCE(ve.utm_source, ve.utm_campaign, ve.referrer_url, ve.landing_url, ve.initial_referrer_url) AS attribution_marker,
                   ve.occurred_at
            FROM viewer_events ve
            JOIN live_streams ls ON ls.id = ve.stream_id
            JOIN streamers st ON st.id = ls.streamer_id
            JOIN creator_profiles cp ON cp.handle = st.handle

            UNION

            SELECT DISTINCT cp.id AS creator_id, ve.id AS event_id, ve.user_id, ve.visitor_id,
                   ve.event_type, COALESCE(ve.episode_id, ve.content_id, e.id, ve.path, ve.id) AS content_key,
                   ve.progress_sec, ve.duration_sec, ve.watch_time_ms,
                   COALESCE(ve.utm_source, ve.utm_campaign, ve.referrer_url, ve.landing_url, ve.initial_referrer_url) AS attribution_marker,
                   ve.occurred_at
            FROM viewer_events ve
            JOIN episodes e ON e.id = ve.episode_id OR (ve.content_kind = 'episode' AND e.id = ve.content_id)
            JOIN content_credits cc ON cc.content_kind = 'series' AND cc.content_id = e.series_id
            JOIN person_profiles pp ON pp.id = cc.person_id
            JOIN creator_profiles cp ON lower(replace(cp.handle, '-', '')) = lower(replace(pp.slug, '-', ''))

            UNION

            SELECT DISTINCT cp.id AS creator_id, ve.id AS event_id, ve.user_id, ve.visitor_id,
                   ve.event_type, COALESCE(ve.content_id, ve.path, ve.id) AS content_key,
                   ve.progress_sec, ve.duration_sec, ve.watch_time_ms,
                   COALESCE(ve.utm_source, ve.utm_campaign, ve.referrer_url, ve.landing_url, ve.initial_referrer_url) AS attribution_marker,
                   ve.occurred_at
            FROM viewer_events ve
            JOIN content_credits cc ON cc.content_kind = ve.content_kind AND cc.content_id = ve.content_id
            JOIN person_profiles pp ON pp.id = cc.person_id
            JOIN creator_profiles cp ON lower(replace(cp.handle, '-', '')) = lower(replace(pp.slug, '-', ''))
            WHERE ve.content_kind IN ('series', 'film')

            UNION

            SELECT DISTINCT cp.id AS creator_id, ve.id AS event_id, ve.user_id, ve.visitor_id,
                   ve.event_type, COALESCE(ve.content_id, ve.path, ve.id) AS content_key,
                   ve.progress_sec, ve.duration_sec, ve.watch_time_ms,
                   COALESCE(ve.utm_source, ve.utm_campaign, ve.referrer_url, ve.landing_url, ve.initial_referrer_url) AS attribution_marker,
                   ve.occurred_at
            FROM viewer_events ve
            JOIN content_credits cc ON cc.content_kind IN ('series', 'film') AND cc.content_id = ve.content_id
            JOIN person_profiles pp ON pp.id = cc.person_id
            JOIN creator_profiles cp ON lower(replace(cp.handle, '-', '')) = lower(replace(pp.slug, '-', ''))
            WHERE ve.content_id IS NOT NULL
              AND (ve.content_kind IS NULL OR ve.content_kind NOT IN ('series', 'film', 'episode'))

            UNION

            SELECT DISTINCT cp.id AS creator_id, ve.id AS event_id, ve.user_id, ve.visitor_id,
                   ve.event_type, COALESCE(ve.episode_id, ve.content_id, ve.path, ve.id) AS content_key,
                   ve.progress_sec, ve.duration_sec, ve.watch_time_ms,
                   COALESCE(ve.utm_source, ve.utm_campaign, ve.referrer_url, ve.landing_url, ve.initial_referrer_url) AS attribution_marker,
                   ve.occurred_at
            FROM viewer_events ve
            JOIN uploads u ON u.id = ve.content_id
            JOIN creator_profiles cp ON cp.id = u.creator_id
        )
        SELECT event_id, user_id, visitor_id, event_type, content_key, progress_sec,
               duration_sec, watch_time_ms, attribution_marker
        FROM event_creator
        WHERE creator_id = $1
          AND ($2 IS NULL OR substr(occurred_at, 1, 10) = $2)
        "#,
    )
    .bind(creator_id)
    .bind(day_filter)
    .fetch_all(pool)
    .await?;

    let mut progress_by_viewer_content = HashMap::<(String, String), i64>::new();
    for row in rows {
        record_event_attention(
            row.get::<String, _>("event_id"),
            row.get::<Option<String>, _>("user_id"),
            row.get::<String, _>("visitor_id"),
            row.get::<String, _>("event_type"),
            row.get::<String, _>("content_key"),
            row.get::<Option<i64>, _>("progress_sec"),
            row.get::<Option<i64>, _>("duration_sec"),
            row.get::<Option<i64>, _>("watch_time_ms"),
            row.get::<Option<String>, _>("attribution_marker"),
            viewers,
            &mut progress_by_viewer_content,
        );
    }
    apply_progress_attention(viewers, progress_by_viewer_content);
    Ok(())
}

async fn build_creator_attention_score(
    pool: &SqlitePool,
    creator_id: &str,
    day_filter: Option<&str>,
    viewers: HashMap<String, AttentionViewerAggregate>,
) -> AppResult<CreatorAttentionScore> {
    let measured_viewers = viewers.len() as i64;
    let measured_sessions = viewers
        .values()
        .map(|viewer| viewer.session_count + (viewer.event_count > 0) as i64)
        .sum::<i64>();
    let qualified = viewers
        .values()
        .filter(|viewer| qualifies_viewer(viewer))
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

    let chat_participants = count_creator_chat_participants(pool, creator_id, day_filter).await?;
    let clip_requesters = count_creator_clip_requesters(pool, creator_id, day_filter).await?;
    let notified_users = count_creator_live_notify_users(pool, creator_id, day_filter).await?;
    let engagement_multiplier = engagement_multiplier(
        qualified_viewers,
        chat_participants,
        clip_requesters,
        notified_users,
        qualified
            .iter()
            .filter(|viewer| viewer.watchlist_actions > 0)
            .count() as i64,
        qualified
            .iter()
            .filter(|viewer| {
                viewer.content_keys.len() >= 2 || viewer.playback_progress_events >= 30
            })
            .count() as i64,
    );

    let returning_viewers = qualified
        .iter()
        .filter(|viewer| viewer.session_count >= 2 || viewer.content_keys.len() >= 2)
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

async fn build_postgres_creator_attention_score(
    pool: &PgPool,
    creator_id: &str,
    day_filter: Option<&str>,
    viewers: HashMap<String, AttentionViewerAggregate>,
) -> AppResult<CreatorAttentionScore> {
    let measured_viewers = viewers.len() as i64;
    let measured_sessions = viewers
        .values()
        .map(|viewer| viewer.session_count + (viewer.event_count > 0) as i64)
        .sum::<i64>();
    let qualified = viewers
        .values()
        .filter(|viewer| qualifies_viewer(viewer))
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
        count_postgres_creator_chat_participants(pool, creator_id, day_filter).await?;
    let clip_requesters =
        count_postgres_creator_clip_requesters(pool, creator_id, day_filter).await?;
    let notified_users =
        count_postgres_creator_live_notify_users(pool, creator_id, day_filter).await?;
    let engagement_multiplier = engagement_multiplier(
        qualified_viewers,
        chat_participants,
        clip_requesters,
        notified_users,
        qualified
            .iter()
            .filter(|viewer| viewer.watchlist_actions > 0)
            .count() as i64,
        qualified
            .iter()
            .filter(|viewer| {
                viewer.content_keys.len() >= 2 || viewer.playback_progress_events >= 30
            })
            .count() as i64,
    );

    let returning_viewers = qualified
        .iter()
        .filter(|viewer| viewer.session_count >= 2 || viewer.content_keys.len() >= 2)
        .count() as i64;
    let returning_viewer_rate = ratio(returning_viewers, qualified_viewers);
    let retention_multiplier = 0.8 + 0.8 * returning_viewer_rate;
    let audience_quality_multiplier =
        postgres_audience_quality_multiplier(pool, creator_id).await?;
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

async fn persist_postgres_creator_attention_rollup(
    pool: &PgPool,
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
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
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

async fn fetch_sqlite_creator_ids_for_attention_rollup(
    pool: &SqlitePool,
) -> AppResult<Vec<String>> {
    let rows = sqlx::query("SELECT id AS creator_id FROM creator_profiles ORDER BY id ASC")
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| row.get::<String, _>("creator_id"))
        .collect())
}

async fn fetch_postgres_creator_ids_for_attention_rollup(pool: &PgPool) -> AppResult<Vec<String>> {
    let rows = sqlx::query("SELECT id AS creator_id FROM creator_profiles ORDER BY id ASC")
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

fn resolve_viewer_key(user_id: Option<&str>, visitor_id: Option<&str>, fallback: &str) -> String {
    user_id
        .map(|id| format!("u:{id}"))
        .or_else(|| visitor_id.map(|id| format!("v:{id}")))
        .unwrap_or_else(|| format!("s:{fallback}"))
}

#[allow(clippy::too_many_arguments)]
fn record_event_attention(
    event_id: String,
    user_id: Option<String>,
    visitor_id: String,
    event_type: String,
    content_key: String,
    progress_sec: Option<i64>,
    duration_sec: Option<i64>,
    watch_time_ms: Option<i64>,
    attribution_marker: Option<String>,
    viewers: &mut HashMap<String, AttentionViewerAggregate>,
    progress_by_viewer_content: &mut HashMap<(String, String), i64>,
) {
    let viewer_key = resolve_viewer_key(user_id.as_deref(), Some(&visitor_id), &event_id);
    let aggregate = viewers.entry(viewer_key.clone()).or_default();
    aggregate.event_count += 1;
    aggregate.authenticated |= user_id.is_some();
    aggregate.has_stable_visitor_id = true;
    aggregate.attributed |= attribution_marker
        .as_deref()
        .is_some_and(|marker| !marker.trim().is_empty());
    aggregate.content_keys.insert(content_key.clone());

    match event_type.as_str() {
        "playback_progress" => {
            aggregate.playback_progress_events += 1;
            let bounded_progress = progress_sec
                .unwrap_or_default()
                .clamp(
                    0,
                    duration_sec.unwrap_or(MAX_SINGLE_CONTENT_PROGRESS_SECONDS),
                )
                .min(MAX_SINGLE_CONTENT_PROGRESS_SECONDS);
            let key = (viewer_key, content_key);
            let current = progress_by_viewer_content.entry(key).or_default();
            *current = (*current).max(bounded_progress);
        }
        "watchlist_add" | "watchlist_remove" => {
            aggregate.watchlist_actions += 1;
        }
        "page_leave" => {
            if let Some(ms) = watch_time_ms {
                aggregate.watch_seconds += (ms / 1000).clamp(0, 1_800);
            }
        }
        _ => {}
    }
}

fn apply_progress_attention(
    viewers: &mut HashMap<String, AttentionViewerAggregate>,
    progress_by_viewer_content: HashMap<(String, String), i64>,
) {
    for ((viewer_key, _content_key), seconds) in progress_by_viewer_content {
        if let Some(viewer) = viewers.get_mut(&viewer_key) {
            viewer.watch_seconds += seconds;
        }
    }
}

fn qualifies_viewer(viewer: &AttentionViewerAggregate) -> bool {
    viewer.watch_seconds >= QUALIFIED_VIEW_SECONDS
        && viewer.event_count <= 2_000
        && viewer.watch_seconds <= 16 * 60 * 60
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
    watchlist_viewers: i64,
    deep_content_viewers: i64,
) -> f64 {
    if qualified_viewers <= 0 {
        return 1.0;
    }
    let chat = (chat_participants as f64 / qualified_viewers as f64 / 0.20).min(1.0);
    let clips = (clip_requesters as f64 / qualified_viewers as f64 / 0.04).min(1.0);
    let notify = (notified_users as f64 / qualified_viewers as f64 / 0.12).min(1.0);
    let watchlist = (watchlist_viewers as f64 / qualified_viewers as f64 / 0.18).min(1.0);
    let depth = (deep_content_viewers as f64 / qualified_viewers as f64 / 0.25).min(1.0);
    let intensity = 0.25 * chat + 0.12 * clips + 0.15 * notify + 0.25 * watchlist + 0.23 * depth;
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
    let stable = qualified
        .iter()
        .filter(|viewer| viewer.has_stable_visitor_id)
        .count() as i64;
    let behavior_depth = qualified
        .iter()
        .filter(|viewer| {
            viewer.session_count >= 2
                || viewer.content_keys.len() >= 2
                || viewer.watchlist_actions > 0
                || viewer.playback_progress_events >= 30
        })
        .count() as i64;
    let suspicious = qualified
        .iter()
        .filter(|viewer| viewer.event_count > 1_000 || viewer.watch_seconds > 12 * 60 * 60)
        .count() as i64;
    let identity_confidence = 0.50
        + 0.30 * ratio(authenticated, qualified.len() as i64)
        + 0.20 * ratio(stable, qualified.len() as i64);
    let attribution_confidence = 0.70 + 0.30 * ratio(attributed, qualified.len() as i64);
    let behavior_confidence = 0.70 + 0.30 * ratio(behavior_depth, qualified.len() as i64);
    let invalid_traffic_penalty = 1.0 - 0.50 * ratio(suspicious, qualified.len() as i64);
    (identity_confidence * attribution_confidence * behavior_confidence * invalid_traffic_penalty)
        .clamp(0.0, 1.0)
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

async fn postgres_audience_quality_multiplier(pool: &PgPool, creator_id: &str) -> AppResult<f64> {
    let row = sqlx::query("SELECT default_category FROM creator_profiles WHERE id = $1")
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

async fn count_postgres_creator_chat_participants(
    pool: &PgPool,
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
        WHERE cp.id = $1
          AND cm.user_id IS NOT NULL
          AND ($2 IS NULL OR substr(cm.sent_at, 1, 10) = $2)
        "#,
    )
    .bind(creator_id)
    .bind(day)
    .fetch_all(pool)
    .await?;
    Ok(rows.len() as i64)
}

async fn count_postgres_creator_clip_requesters(
    pool: &PgPool,
    creator_id: &str,
    day: Option<&str>,
) -> AppResult<i64> {
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT req.user_id
        FROM live_stream_clip_requests req
        JOIN live_streams ls ON ls.id = req.stream_id
        JOIN streamers s ON s.id = ls.streamer_id
        JOIN creator_profiles cp ON cp.handle = s.handle
        WHERE cp.id = $1
          AND req.user_id IS NOT NULL
          AND ($2 IS NULL OR substr(req.created_at, 1, 10) = $2)
        "#,
    )
    .bind(creator_id)
    .bind(day)
    .fetch_all(pool)
    .await?;
    Ok(rows.len() as i64)
}

async fn count_postgres_creator_live_notify_users(
    pool: &PgPool,
    creator_id: &str,
    day: Option<&str>,
) -> AppResult<i64> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(DISTINCT pref.user_id)::BIGINT AS count
        FROM live_stream_notification_preferences pref
        JOIN streamers s ON s.id = pref.streamer_id
        JOIN creator_profiles cp ON cp.handle = s.handle
        WHERE cp.id = $1
          AND ($2 IS NULL OR substr(pref.created_at, 1, 10) = $2)
        "#,
    )
    .bind(creator_id)
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

#[cfg(test)]
mod attention_tests {
    use super::*;

    #[test]
    fn v2_qualifies_only_meaningful_attention() {
        let short = AttentionViewerAggregate {
            watch_seconds: QUALIFIED_VIEW_SECONDS - 1,
            ..Default::default()
        };
        let qualified = AttentionViewerAggregate {
            watch_seconds: QUALIFIED_VIEW_SECONDS,
            ..Default::default()
        };
        let noisy = AttentionViewerAggregate {
            watch_seconds: QUALIFIED_VIEW_SECONDS,
            event_count: 2_001,
            ..Default::default()
        };

        assert!(!qualifies_viewer(&short));
        assert!(qualifies_viewer(&qualified));
        assert!(!qualifies_viewer(&noisy));
    }

    #[test]
    fn v2_progress_events_use_highest_progress_per_viewer_content() {
        let mut viewers = HashMap::new();
        let mut progress = HashMap::new();
        record_event_attention(
            "evt-1".to_string(),
            None,
            "visitor-1".to_string(),
            "playback_progress".to_string(),
            "episode-1".to_string(),
            Some(30),
            Some(600),
            Some(1000),
            Some("campaign".to_string()),
            &mut viewers,
            &mut progress,
        );
        record_event_attention(
            "evt-2".to_string(),
            None,
            "visitor-1".to_string(),
            "playback_progress".to_string(),
            "episode-1".to_string(),
            Some(120),
            Some(600),
            Some(1000),
            Some("campaign".to_string()),
            &mut viewers,
            &mut progress,
        );
        apply_progress_attention(&mut viewers, progress);

        let aggregate = viewers.get("v:visitor-1").expect("viewer");
        assert_eq!(aggregate.watch_seconds, 120);
        assert_eq!(aggregate.playback_progress_events, 2);
        assert!(aggregate.attributed);
        assert!(aggregate.has_stable_visitor_id);
    }

    #[test]
    fn v2_engagement_uses_saved_and_deep_content_intent() {
        let baseline = engagement_multiplier(10, 0, 0, 0, 0, 0);
        let with_intent = engagement_multiplier(10, 0, 0, 0, 4, 4);

        assert_eq!(baseline, 1.0);
        assert!(with_intent > baseline);
    }

    #[test]
    fn v2_confidence_penalizes_suspicious_event_volume() {
        let healthy = AttentionViewerAggregate {
            watch_seconds: 300,
            event_count: 20,
            has_stable_visitor_id: true,
            attributed: true,
            playback_progress_events: 30,
            ..Default::default()
        };
        let suspicious = AttentionViewerAggregate {
            watch_seconds: 300,
            event_count: 1_500,
            has_stable_visitor_id: true,
            attributed: true,
            playback_progress_events: 30,
            ..Default::default()
        };

        assert!(
            data_confidence_multiplier(&[&healthy]) > data_confidence_multiplier(&[&suspicious])
        );
    }
}
