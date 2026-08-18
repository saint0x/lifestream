use super::*;

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
        "SELECT source, sessions, share FROM traffic_sources WHERE creator_id = ? ORDER BY sessions DESC",
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| TrafficSource {
            source: row.get("source"),
            sessions: row.get("sessions"),
            share: row.get("share"),
        })
        .collect())
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
