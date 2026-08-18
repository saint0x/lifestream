use super::*;

pub(super) async fn creator_dashboard_payload(
    pool: &SqlitePool,
    identity: &RequestIdentity,
) -> AppResult<CreatorDashboard> {
    let creator_id = identity.require_creator_scope()?;
    let profile = fetch_creator_profile(pool, creator_id).await?;
    let operational_state = fetch_creator_operational_state(pool, &profile).await?;
    let broadcasts = fetch_broadcasts(pool, creator_id).await?;
    let analytics = fetch_analytics(pool, creator_id).await?;
    let analytics_summary = summarize_creator_analytics(&analytics);
    let traffic_sources = fetch_traffic_sources(pool, creator_id).await?;
    let top_content = fetch_top_content(pool, creator_id).await?;
    let revenue = fetch_revenue_entries(pool, creator_id).await?;
    let subscriber_tiers = fetch_creator_subscriber_tiers(pool, creator_id).await?;
    let revenue_summary = summarize_creator_revenue(&analytics, &revenue, &subscriber_tiers);
    let notifications = fetch_notifications_rows(pool, creator_id).await?;
    let uploads = fetch_uploads(pool, creator_id).await?;

    let current_broadcast = broadcasts
        .iter()
        .find(|item| item.status == "live")
        .cloned();
    let scheduled_broadcasts = broadcasts
        .iter()
        .filter(|item| item.status == "scheduled" || item.status == "ready")
        .cloned()
        .collect();
    let recent_broadcasts = broadcasts
        .iter()
        .filter(|item| item.status == "ended")
        .cloned()
        .collect();

    Ok(CreatorDashboard {
        profile: contract_creator_profile(profile),
        current_broadcast: current_broadcast.map(contract_broadcast),
        scheduled_broadcasts: contract_broadcasts(scheduled_broadcasts),
        recent_broadcasts: contract_broadcasts(recent_broadcasts),
        analytics,
        traffic_sources,
        top_content,
        revenue,
        analytics_summary,
        revenue_summary,
        subscriber_tiers,
        operational_state,
        notifications,
        uploads,
    })
}

pub(super) async fn fetch_creator_app_state(
    pool: &SqlitePool,
    identity: &RequestIdentity,
    content_query: &CreatorContentQuery,
) -> AppResult<CreatorAppState> {
    let creator_id = identity.require_creator_scope()?;
    let dashboard = creator_dashboard_payload(pool, identity).await?;
    let live_control = fetch_creator_live_control_response(pool, creator_id).await?;
    let live_runtime = fetch_creator_live_runtime_response(pool, creator_id).await?;
    let uploads = fetch_uploads(pool, creator_id).await?;
    let filtered_uploads = filter_creator_uploads(uploads.clone(), content_query)?;
    let content = CreatorContentResponse {
        summary: summarize_creator_content(&uploads, filtered_uploads.len() as i64),
        uploads: filtered_uploads,
    };
    let upload_operations = fetch_creator_upload_operations_response(pool, creator_id).await?;

    Ok(CreatorAppState {
        dashboard,
        live_control,
        live_runtime,
        content,
        upload_operations,
    })
}

pub(super) fn validate_upload_visibility(visibility: &str) -> AppResult<()> {
    match visibility {
        "public" | "unlisted" | "private" => Ok(()),
        _ => Err(AppError::BadRequest(
            "unsupported upload visibility".to_string(),
        )),
    }
}

pub(super) fn validate_upload_job_kind(kind: &str) -> AppResult<()> {
    match kind {
        "film" | "episode" | "clip" | "trailer" | "video" | "vod" | "live_archive" => Ok(()),
        other => Err(AppError::BadRequest(format!(
            "unsupported upload job kind: {other}"
        ))),
    }
}

pub(super) fn validate_upload_job_source_type(source_type: &str) -> AppResult<()> {
    match source_type {
        "resumable-upload" | "live-archive" | "studio-export" => Ok(()),
        other => Err(AppError::BadRequest(format!(
            "unsupported upload job source type: {other}"
        ))),
    }
}

pub(super) fn derive_upload_lifecycle_status(
    current_status: &str,
    visibility: &str,
    release_at: Option<&str>,
    now: &str,
) -> AppResult<String> {
    if current_status == "taken_down" {
        return Ok("taken_down".to_string());
    }
    match visibility {
        "private" => Ok("draft".to_string()),
        "public" | "unlisted" => {
            if release_at.is_some_and(|release_at| release_at > now) {
                Ok("scheduled".to_string())
            } else {
                Ok("published".to_string())
            }
        }
        _ => Err(AppError::BadRequest(
            "unsupported upload visibility".to_string(),
        )),
    }
}

pub(super) fn validate_bulk_upload_action(upload: &Upload, action: &str) -> AppResult<()> {
    match action {
        "archive" => {
            if upload.status == "processing" {
                return Err(AppError::BadRequest(
                    "processing uploads cannot be archived".to_string(),
                ));
            }
            if upload.status == "taken_down" {
                return Err(AppError::BadRequest(
                    "taken-down uploads cannot be archived".to_string(),
                ));
            }
            Ok(())
        }
        "make_public" | "make_unlisted" => {
            if upload.status == "processing" || upload.status == "taken_down" {
                return Err(AppError::BadRequest(
                    "processing or taken-down uploads cannot change public visibility".to_string(),
                ));
            }
            Ok(())
        }
        "delete" => {
            if !matches!(upload.status.as_str(), "draft" | "archived" | "taken_down") {
                return Err(AppError::BadRequest(
                    "only draft, archived, or taken-down uploads can be deleted".to_string(),
                ));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(super) fn filter_creator_uploads(
    mut uploads: Vec<Upload>,
    query: &CreatorContentQuery,
) -> AppResult<Vec<Upload>> {
    if let Some(kind) = query.kind.as_deref() {
        if kind != "all" {
            uploads.retain(|upload| upload.kind == kind);
        }
    }
    if let Some(status) = query.status.as_deref() {
        if status != "all" {
            uploads.retain(|upload| upload.status == status);
        }
    }
    if let Some(q) = query.q.as_deref() {
        let normalized = q.trim().to_lowercase();
        if !normalized.is_empty() {
            uploads.retain(|upload| upload.title.to_lowercase().contains(&normalized));
        }
    }

    match query.sort.as_deref().unwrap_or("uploaded") {
        "uploaded" => {
            uploads.sort_by(|left, right| right.uploaded_at.cmp(&left.uploaded_at));
        }
        "views" => uploads.sort_by(|left, right| right.views.cmp(&left.views)),
        "hours" => uploads.sort_by(|left, right| right.watch_hours.cmp(&left.watch_hours)),
        "title" => uploads.sort_by(|left, right| left.title.cmp(&right.title)),
        other => {
            return Err(AppError::BadRequest(format!(
                "unsupported creator content sort: {other}"
            )));
        }
    }

    Ok(uploads)
}

pub(super) fn summarize_creator_content(
    uploads: &[Upload],
    filtered_count: i64,
) -> CreatorContentSummary {
    CreatorContentSummary {
        total_uploads: uploads.len() as i64,
        published_uploads: uploads
            .iter()
            .filter(|upload| upload.status == "published")
            .count() as i64,
        scheduled_uploads: uploads
            .iter()
            .filter(|upload| upload.status == "scheduled")
            .count() as i64,
        processing_uploads: uploads
            .iter()
            .filter(|upload| upload.status == "processing")
            .count() as i64,
        draft_uploads: uploads
            .iter()
            .filter(|upload| upload.status == "draft")
            .count() as i64,
        archived_uploads: uploads
            .iter()
            .filter(|upload| upload.status == "archived")
            .count() as i64,
        total_views: uploads.iter().map(|upload| upload.views).sum(),
        total_watch_hours: uploads.iter().map(|upload| upload.watch_hours).sum(),
        total_storage_bytes: uploads.iter().map(|upload| upload.size_bytes).sum(),
        filtered_count,
    }
}

pub(super) async fn fetch_broadcasts(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<Broadcast>> {
    let rows = sqlx::query(
        "SELECT id, title, category, tags_json, status, started_at, ended_at, duration_sec, peak_viewers, average_viewers, chat_messages, new_followers, new_subscribers, revenue, thumbnail, is_mature FROM broadcasts WHERE creator_id = ? ORDER BY started_at DESC",
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Broadcast {
            id: row.get("id"),
            title: row.get("title"),
            category: row.get("category"),
            tags: from_json(row.get::<String, _>("tags_json")).unwrap_or_default(),
            status: row.get("status"),
            started_at: row.get("started_at"),
            ended_at: row.get("ended_at"),
            duration_sec: row.get("duration_sec"),
            peak_viewers: row.get("peak_viewers"),
            average_viewers: row.get("average_viewers"),
            chat_messages: row.get("chat_messages"),
            new_followers: row.get("new_followers"),
            new_subscribers: row.get("new_subscribers"),
            revenue: row.get("revenue"),
            thumbnail: row.get("thumbnail"),
            is_mature: row.get::<i64, _>("is_mature") == 1,
        })
        .collect())
}

pub(super) async fn fetch_broadcast_by_id(
    pool: &SqlitePool,
    creator_id: &str,
    id: &str,
) -> AppResult<Broadcast> {
    let row = sqlx::query(
        "SELECT id, title, category, tags_json, status, started_at, ended_at, duration_sec, peak_viewers, average_viewers, chat_messages, new_followers, new_subscribers, revenue, thumbnail, is_mature FROM broadcasts WHERE creator_id = ? AND id = ?",
    )
    .bind(creator_id)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Broadcast {
        id: row.get("id"),
        title: row.get("title"),
        category: row.get("category"),
        tags: from_json(row.get::<String, _>("tags_json")).unwrap_or_default(),
        status: row.get("status"),
        started_at: row.get("started_at"),
        ended_at: row.get("ended_at"),
        duration_sec: row.get("duration_sec"),
        peak_viewers: row.get("peak_viewers"),
        average_viewers: row.get("average_viewers"),
        chat_messages: row.get("chat_messages"),
        new_followers: row.get("new_followers"),
        new_subscribers: row.get("new_subscribers"),
        revenue: row.get("revenue"),
        thumbnail: row.get("thumbnail"),
        is_mature: row.get::<i64, _>("is_mature") == 1,
    })
}

pub(super) async fn fetch_uploads(pool: &SqlitePool, creator_id: &str) -> AppResult<Vec<Upload>> {
    publish_due_scheduled_upload_releases(pool, Some(creator_id), None).await?;
    let rows = sqlx::query(
        "SELECT id, slug, title, description, kind, duration_sec, uploaded_at, published_at, release_at, status, visibility, access_policy, access_tier_id, price_cents, currency, rental_window_hours, views, likes, comments, watch_hours, thumbnail, series_title, season_number, episode_number, size_bytes, resolution, transcode_progress FROM uploads WHERE creator_id = ? ORDER BY uploaded_at DESC",
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Upload {
            id: row.get("id"),
            slug: row.get("slug"),
            title: row.get("title"),
            description: row.get("description"),
            kind: row.get("kind"),
            duration_sec: row.get("duration_sec"),
            uploaded_at: row.get("uploaded_at"),
            published_at: row.get("published_at"),
            release_at: row.get("release_at"),
            status: row.get("status"),
            visibility: row.get("visibility"),
            access_policy: row.get("access_policy"),
            access_tier_id: row.get("access_tier_id"),
            price_cents: row.get("price_cents"),
            currency: row.get("currency"),
            rental_window_hours: row.get("rental_window_hours"),
            views: row.get("views"),
            likes: row.get("likes"),
            comments: row.get("comments"),
            watch_hours: row.get("watch_hours"),
            thumbnail: row.get("thumbnail"),
            series_title: row.get("series_title"),
            season_number: row.get("season_number"),
            episode_number: row.get("episode_number"),
            size_bytes: row.get("size_bytes"),
            resolution: row.get("resolution"),
            transcode_progress: row.get("transcode_progress"),
        })
        .collect())
}

pub(super) async fn fetch_upload_by_id(
    pool: &SqlitePool,
    creator_id: &str,
    id: &str,
) -> AppResult<Upload> {
    publish_due_scheduled_upload_releases(pool, Some(creator_id), Some(id)).await?;
    let row = sqlx::query(
        "SELECT id, slug, title, description, kind, duration_sec, uploaded_at, published_at, release_at, status, visibility, access_policy, access_tier_id, price_cents, currency, rental_window_hours, views, likes, comments, watch_hours, thumbnail, series_title, season_number, episode_number, size_bytes, resolution, transcode_progress FROM uploads WHERE creator_id = ? AND id = ?",
    )
    .bind(creator_id)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Upload {
        id: row.get("id"),
        slug: row.get("slug"),
        title: row.get("title"),
        description: row.get("description"),
        kind: row.get("kind"),
        duration_sec: row.get("duration_sec"),
        uploaded_at: row.get("uploaded_at"),
        published_at: row.get("published_at"),
        release_at: row.get("release_at"),
        status: row.get("status"),
        visibility: row.get("visibility"),
        access_policy: row.get("access_policy"),
        access_tier_id: row.get("access_tier_id"),
        price_cents: row.get("price_cents"),
        currency: row.get("currency"),
        rental_window_hours: row.get("rental_window_hours"),
        views: row.get("views"),
        likes: row.get("likes"),
        comments: row.get("comments"),
        watch_hours: row.get("watch_hours"),
        thumbnail: row.get("thumbnail"),
        series_title: row.get("series_title"),
        season_number: row.get("season_number"),
        episode_number: row.get("episode_number"),
        size_bytes: row.get("size_bytes"),
        resolution: row.get("resolution"),
        transcode_progress: row.get("transcode_progress"),
    })
}

pub(super) async fn fetch_creator_upload_operations_response(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorUploadOperationsResponse> {
    let jobs = fetch_upload_jobs(pool, creator_id).await?;
    let ingest_sessions = fetch_upload_ingest_sessions(pool, creator_id).await?;
    let media_assets = fetch_media_assets(pool, creator_id).await?;
    let uploads = fetch_uploads(pool, creator_id).await?;

    let session_by_job: HashMap<String, UploadIngestSession> = ingest_sessions
        .iter()
        .cloned()
        .map(|session| (session.job_id.clone(), session))
        .collect();
    let asset_by_job: HashMap<String, MediaAsset> = media_assets
        .iter()
        .cloned()
        .map(|asset| (asset.upload_job_id.clone(), asset))
        .collect();
    let upload_by_id: HashMap<String, Upload> = uploads
        .into_iter()
        .map(|upload| (upload.id.clone(), upload))
        .collect();

    let records = jobs
        .iter()
        .cloned()
        .map(|job| {
            let media_asset = asset_by_job.get(&job.id).cloned();
            let published_upload_id = job
                .upload_id
                .clone()
                .or_else(|| {
                    media_asset
                        .as_ref()
                        .and_then(|asset| asset.upload_id.clone())
                })
                .or_else(|| job.published_content_id.clone())
                .or_else(|| {
                    media_asset
                        .as_ref()
                        .and_then(|asset| asset.published_content_id.clone())
                });
            CreatorUploadOperationRecord {
                ingest_session: session_by_job.get(&job.id).cloned(),
                media_asset,
                published_upload: published_upload_id
                    .as_ref()
                    .and_then(|upload_id| upload_by_id.get(upload_id).cloned()),
                upload_job: job,
            }
        })
        .collect::<Vec<_>>();

    let summary = summarize_creator_upload_operations(&records);
    Ok(CreatorUploadOperationsResponse { summary, records })
}

pub(super) fn summarize_creator_upload_operations(
    records: &[CreatorUploadOperationRecord],
) -> CreatorUploadOperationsSummary {
    let mut summary = CreatorUploadOperationsSummary {
        total_jobs: records.len() as i64,
        created_jobs: 0,
        uploaded_jobs: 0,
        processing_jobs: 0,
        ready_jobs: 0,
        failed_jobs: 0,
        published_jobs: 0,
        active_ingest_sessions: 0,
        completed_ingest_sessions: 0,
        ready_assets: 0,
        processing_assets: 0,
        failed_assets: 0,
        published_assets: 0,
        total_bytes_expected: 0,
        total_bytes_received: 0,
        total_asset_bytes: 0,
    };

    for record in records {
        summary.total_bytes_expected += record.upload_job.bytes_expected;
        summary.total_bytes_received += record.upload_job.bytes_received;

        match record.upload_job.status.as_str() {
            "created" => summary.created_jobs += 1,
            "uploaded" => summary.uploaded_jobs += 1,
            "processing" => summary.processing_jobs += 1,
            "ready" => summary.ready_jobs += 1,
            "failed" => summary.failed_jobs += 1,
            "published" => summary.published_jobs += 1,
            _ => {}
        }

        if let Some(session) = record.ingest_session.as_ref() {
            if session.status == "completed" {
                summary.completed_ingest_sessions += 1;
            } else {
                summary.active_ingest_sessions += 1;
            }
        }

        if let Some(asset) = record.media_asset.as_ref() {
            summary.total_asset_bytes += asset.file_size_bytes;
            match asset.status.as_str() {
                "ready" => summary.ready_assets += 1,
                "processing" => summary.processing_assets += 1,
                "failed" => summary.failed_assets += 1,
                "published" => summary.published_assets += 1,
                _ => {}
            }
        }
    }

    summary
}

pub(super) async fn fetch_analytics(
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
    let rows =
        sqlx::query("SELECT source, sessions, share FROM traffic_sources WHERE creator_id = ? ORDER BY sessions DESC")
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

pub(super) async fn fetch_revenue_entries(
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

pub(super) fn summarize_creator_analytics(analytics: &[AnalyticsPoint]) -> CreatorAnalyticsSummary {
    CreatorAnalyticsSummary {
        window_days: analytics.len() as i64,
        total_viewers: analytics.iter().map(|point| point.viewers).sum(),
        total_watch_minutes: analytics.iter().map(|point| point.watch_minutes).sum(),
        total_revenue: analytics.iter().map(|point| point.revenue).sum(),
        total_new_followers: analytics.iter().map(|point| point.new_followers).sum(),
    }
}

pub(super) fn summarize_creator_revenue(
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

pub(super) async fn fetch_creator_series_title(
    pool: &SqlitePool,
    creator_id: &str,
    series_id: &str,
) -> AppResult<Option<String>> {
    Ok(
        sqlx::query("SELECT title FROM creator_series_projects WHERE creator_id = ? AND id = ?")
            .bind(creator_id)
            .bind(series_id)
            .fetch_optional(pool)
            .await?
            .map(|row| row.get("title")),
    )
}

pub(super) async fn ensure_creator_series_season(
    pool: &SqlitePool,
    creator_id: &str,
    series_id: &str,
    season_number: i64,
    title: String,
    synopsis: String,
) -> AppResult<()> {
    let exists =
        sqlx::query("SELECT 1 FROM creator_series_projects WHERE creator_id = ? AND id = ?")
            .bind(creator_id)
            .bind(series_id)
            .fetch_optional(pool)
            .await?
            .is_some();
    if !exists {
        return Err(AppError::BadRequest(
            "seriesId does not belong to creator".to_string(),
        ));
    }

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO creator_series_seasons (
            id, series_id, season_number, title, synopsis, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(series_id, season_number) DO UPDATE SET
            title = excluded.title,
            synopsis = excluded.synopsis,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(format!("season-{series_id}-{season_number}"))
    .bind(series_id)
    .bind(season_number)
    .bind(title)
    .bind(synopsis)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(())
}
