use super::*;

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/health", get(health))
        .route("/health/live", get(health_live))
        .route("/health/ready", get(health_ready))
        .route("/metrics", get(metrics))
        .route("/api/v1/home", get(home))
        .route("/api/v1/bootstrap", get(bootstrap))
        .route("/api/v1/catalog/series", get(list_series))
        .route("/api/v1/catalog/series/:slug", get(get_series))
        .route("/api/v1/catalog/films", get(list_films))
        .route("/api/v1/catalog/films/:slug", get(get_film))
        .route("/api/v1/catalog/content/:id", get(get_content))
        .route(
            "/api/v1/catalog/creator/series",
            get(list_creator_catalog_series),
        )
        .route(
            "/api/v1/catalog/creator/series/:slug",
            get(get_creator_catalog_series),
        )
        .route(
            "/api/v1/catalog/creator/films",
            get(list_creator_catalog_films),
        )
        .route(
            "/api/v1/catalog/creator/films/:slug",
            get(get_creator_catalog_film),
        )
        .route("/api/v1/live/streams", get(list_live_streams))
        .route("/api/v1/live/streams/:slug", get(get_live_stream))
        .route("/api/v1/live/discovery", get(get_live_discovery))
        .route(
            "/api/v1/live/streams/:stream_id/notify",
            post(enable_live_notify),
        )
        .route(
            "/api/v1/live/streams/:stream_id/clip",
            post(create_clip_request),
        )
        .route(
            "/api/v1/live/streams/:stream_id/report",
            post(report_live_stream),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/moderators",
            get(list_live_stream_moderators).post(add_live_stream_moderator),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/moderators/:user_id",
            delete(remove_live_stream_moderator),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/actions",
            get(list_live_moderation_actions).post(create_live_moderation_action),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/actions/:action_id",
            get(get_live_moderation_action),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/actions/:action_id/reconcile",
            post(reconcile_live_moderation_action),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/actions/:action_id/revoke",
            post(revoke_live_moderation_action),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/reports",
            get(list_live_stream_reports),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/reports/:report_id",
            patch(resolve_live_stream_report),
        )
        .route(
            "/api/v1/live/streams/:stream_id/moderation/audit",
            get(list_live_moderation_audit_log),
        )
        .route(
            "/api/v1/live/streams/:stream_id/viewers",
            get(get_live_viewer_preview),
        )
        .route(
            "/api/v1/live/streams/:stream_id/chat",
            get(list_chat_messages),
        )
        .route(
            "/api/v1/live/streams/:stream_id/chat/messages",
            post(post_chat_message),
        )
        .route("/api/v1/categories", get(list_categories))
        .route("/api/v1/categories/:slug", get(get_category))
        .route("/api/v1/categories/:slug/browse", get(get_category_browse))
        .route("/api/v1/streamers", get(list_streamers))
        .route("/api/v1/streamers/:id", get(get_streamer))
        .route("/api/v1/search", get(search))
}

pub(super) async fn health(State(state): State<SharedState>) -> AppResult<Json<HealthResponse>> {
    let runtime = check_runtime_dependencies(state.as_ref()).await;
    Ok(Json(HealthResponse {
        status: if runtime.ready {
            "ok".to_string()
        } else {
            "degraded".to_string()
        },
        ready: runtime.ready,
        database: runtime.database,
        dependencies: runtime.dependencies,
        uptime_sec: state.uptime_seconds(),
        timestamp: Utc::now().to_rfc3339(),
    }))
}

pub(super) async fn health_live() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

pub(super) async fn health_ready(State(state): State<SharedState>) -> impl IntoResponse {
    let runtime = check_runtime_dependencies(state.as_ref()).await;
    if runtime.ready {
        StatusCode::NO_CONTENT.into_response()
    } else {
        StatusCode::SERVICE_UNAVAILABLE.into_response()
    }
}

#[derive(Clone, Debug)]
pub(super) struct RuntimeHealthStatus {
    pub(super) ready: bool,
    pub(super) database: bool,
    pub(super) dependencies: HealthDependencies,
}

pub(super) async fn check_runtime_dependencies(state: &AppState) -> RuntimeHealthStatus {
    check_runtime_dependencies_with_binaries(state, "ffmpeg", "ffprobe").await
}

pub(super) async fn check_runtime_dependencies_with_binaries(
    state: &AppState,
    ffmpeg_binary: &str,
    ffprobe_binary: &str,
) -> RuntimeHealthStatus {
    let database = check_database(&state.pool).await.unwrap_or(false);
    let media_root = check_media_root_writable(&state.media_root).await;
    let ffmpeg = check_binary_available(ffmpeg_binary).await;
    let ffprobe = check_binary_available(ffprobe_binary).await;
    let background_worker = check_background_worker_ready(state).await;
    let dependencies = HealthDependencies {
        media_root,
        ffmpeg,
        ffprobe,
        background_worker,
    };
    let ready = database
        && dependencies.media_root.ready
        && dependencies.ffmpeg.ready
        && dependencies.ffprobe.ready
        && dependencies.background_worker.ready;
    RuntimeHealthStatus {
        ready,
        database,
        dependencies,
    }
}

async fn check_background_worker_ready(state: &AppState) -> HealthDependencyStatus {
    let snapshot = state.background_worker.snapshot().await;
    let Some(last_success_age_seconds) = snapshot.last_success_age_seconds else {
        return HealthDependencyStatus {
            ready: false,
            detail: "background worker has not completed a control-plane pass yet".to_string(),
        };
    };
    if last_success_age_seconds > BACKGROUND_WORKER_STALE_AFTER_SECONDS {
        return HealthDependencyStatus {
            ready: false,
            detail: match snapshot.last_error {
                Some(error) => format!(
                    "background worker last succeeded {}s ago after {} consecutive failures: {}",
                    last_success_age_seconds, snapshot.consecutive_failures, error
                ),
                None => format!(
                    "background worker last succeeded {}s ago and is stale",
                    last_success_age_seconds
                ),
            },
        };
    }
    HealthDependencyStatus {
        ready: true,
        detail: format!(
            "background worker last succeeded {}s ago with {} consecutive failures",
            last_success_age_seconds, snapshot.consecutive_failures
        ),
    }
}

pub(super) async fn check_media_root_writable(media_root: &FsPath) -> HealthDependencyStatus {
    match tokio::fs::create_dir_all(media_root).await {
        Ok(()) => {}
        Err(error) => {
            return HealthDependencyStatus {
                ready: false,
                detail: format!(
                    "media root {} is not creatable: {}",
                    media_root.display(),
                    error
                ),
            };
        }
    }

    let probe_path = media_root.join(format!(".healthcheck-{}", Uuid::new_v4().simple()));
    match tokio::fs::write(&probe_path, b"ok").await {
        Ok(()) => {
            let cleanup = tokio::fs::remove_file(&probe_path).await;
            let detail = match cleanup {
                Ok(()) => format!("media root {} is writable", media_root.display()),
                Err(error) => format!(
                    "media root {} is writable but cleanup failed: {}",
                    media_root.display(),
                    error
                ),
            };
            HealthDependencyStatus {
                ready: true,
                detail,
            }
        }
        Err(error) => HealthDependencyStatus {
            ready: false,
            detail: format!(
                "media root {} is not writable: {}",
                media_root.display(),
                error
            ),
        },
    }
}

pub(super) async fn check_binary_available(binary: &str) -> HealthDependencyStatus {
    match Command::new(binary).arg("-version").output().await {
        Ok(output) if output.status.success() => HealthDependencyStatus {
            ready: true,
            detail: format!("{binary} is executable"),
        },
        Ok(output) => HealthDependencyStatus {
            ready: false,
            detail: format!(
                "{binary} exited with status {} during readiness probe",
                output.status
            ),
        },
        Err(error) => HealthDependencyStatus {
            ready: false,
            detail: format!("{binary} is unavailable: {error}"),
        },
    }
}

pub(super) async fn metrics(State(state): State<SharedState>) -> AppResult<Response> {
    let mut body = String::new();
    let status_counts = state.metrics.status_counts().await;
    let active_streams = state.realtime.active_streams().await;
    let active_collaboration_sessions = state.realtime.active_collaboration_sessions().await;
    reconcile_stale_creator_live_socket_sessions_for_read(&state.pool, None, None).await?;
    let active_viewer_presence = count_all_active_live_viewer_sessions(&state.pool).await?;
    let active_collaboration_presence =
        count_all_active_collaboration_socket_sessions(&state.pool).await?;
    let active_creator_live_presence =
        count_all_active_creator_live_socket_sessions(&state.pool).await?;
    let worker = state.background_worker.snapshot().await;
    let _ = writeln!(body, "# TYPE lifestream_http_requests_total counter");
    let _ = writeln!(
        body,
        "lifestream_http_requests_total {}",
        state.metrics.total_requests()
    );
    let _ = writeln!(body, "# TYPE lifestream_http_requests_in_flight gauge");
    let _ = writeln!(
        body,
        "lifestream_http_requests_in_flight {}",
        state.metrics.in_flight_requests()
    );
    let _ = writeln!(body, "# TYPE lifestream_http_rate_limited_total counter");
    let _ = writeln!(
        body,
        "lifestream_http_rate_limited_total {}",
        state.metrics.total_rate_limits()
    );
    let _ = writeln!(body, "# TYPE lifestream_http_responses_total counter");
    for (status, count) in status_counts {
        let _ = writeln!(
            body,
            "lifestream_http_responses_total{{status=\"{status}\"}} {count}"
        );
    }
    let _ = writeln!(body, "# TYPE lifestream_uptime_seconds gauge");
    let _ = writeln!(body, "lifestream_uptime_seconds {}", state.uptime_seconds());
    let _ = writeln!(body, "# TYPE lifestream_background_worker_ready gauge");
    let _ = writeln!(
        body,
        "lifestream_background_worker_ready {}",
        if worker
            .last_success_age_seconds
            .is_some_and(|age| age <= BACKGROUND_WORKER_STALE_AFTER_SECONDS)
        {
            1
        } else {
            0
        }
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_background_worker_last_tick_age_seconds gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_background_worker_last_tick_age_seconds {}",
        worker.last_tick_age_seconds.unwrap_or(u64::MAX)
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_background_worker_last_success_age_seconds gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_background_worker_last_success_age_seconds {}",
        worker.last_success_age_seconds.unwrap_or(u64::MAX)
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_background_worker_consecutive_failures gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_background_worker_consecutive_failures {}",
        worker.consecutive_failures
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_realtime_collaboration_sessions gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_realtime_collaboration_sessions {}",
        active_collaboration_sessions
    );
    let _ = writeln!(body, "# TYPE lifestream_presence_live_viewers gauge");
    let _ = writeln!(
        body,
        "lifestream_presence_live_viewers {}",
        active_viewer_presence
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_presence_collaboration_participants gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_presence_collaboration_participants {}",
        active_collaboration_presence
    );
    let _ = writeln!(
        body,
        "# TYPE lifestream_presence_creator_live_sockets gauge"
    );
    let _ = writeln!(
        body,
        "lifestream_presence_creator_live_sockets {}",
        active_creator_live_presence
    );
    let _ = writeln!(body, "# TYPE lifestream_db_pool_connections gauge");
    let _ = writeln!(body, "lifestream_db_pool_connections {}", state.pool.size());
    let _ = writeln!(body, "# TYPE lifestream_db_pool_idle_connections gauge");
    let _ = writeln!(
        body,
        "lifestream_db_pool_idle_connections {}",
        state.pool.num_idle()
    );
    let _ = writeln!(body, "# TYPE lifestream_ws_connections gauge");
    let _ = writeln!(
        body,
        "lifestream_ws_connections {}",
        state.realtime.total_connections()
    );
    let _ = writeln!(body, "# TYPE lifestream_ws_active_streams gauge");
    let _ = writeln!(body, "lifestream_ws_active_streams {active_streams}");

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )
        .body(Body::from(body))
        .map_err(|_| AppError::BadRequest("failed to build metrics response".to_string()))
}

pub(super) async fn home(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<HomeResponse>> {
    let trending_series = fetch_series(&state.pool, Some("WHERE trending = 1"), Some(6)).await?;
    let trending_films = fetch_films(&state.pool, Some("WHERE trending = 1"), Some(6)).await?;
    let featured_live = fetch_live_streams(&state.pool, None).await?;
    let categories = fetch_categories(&state.pool).await?;
    let maybe_identity = optional_identity(&state.pool, &headers).await?;
    let continue_watching = match maybe_identity {
        Some(identity) => {
            fetch_user(&state.pool, &identity.user_id)
                .await?
                .continue_watching
        }
        None => Vec::new(),
    };

    Ok(Json(HomeResponse {
        trending_series,
        trending_films,
        featured_live,
        categories,
        continue_watching,
    }))
}

pub(super) async fn bootstrap(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<serde_json::Value>> {
    let home = home(State(state.clone()), headers.clone()).await?.0;
    let identity = optional_identity(&state.pool, &headers).await?;
    let me = match identity.as_ref() {
        Some(identity) => Some(fetch_user(&state.pool, &identity.user_id).await?),
        None => None,
    };
    let viewer = match identity.as_ref() {
        Some(identity) => Some(
            fetch_viewer_app_state(&state.pool, &identity.user_id, &identity.session_id).await?,
        ),
        None => None,
    };
    let creator = match identity.as_ref() {
        Some(identity) if identity.creator_id.is_some() => {
            Some(creator_dashboard_payload(&state.pool, identity).await?)
        }
        _ => None,
    };
    let creator_state = match identity.as_ref() {
        Some(identity) if identity.creator_id.is_some() => Some(
            fetch_creator_app_state(
                &state.pool,
                identity,
                &CreatorContentQuery {
                    kind: None,
                    status: None,
                    q: None,
                    sort: None,
                },
            )
            .await?,
        ),
        _ => None,
    };

    Ok(Json(serde_json::json!({
        "home": home,
        "me": me,
        "viewer": viewer,
        "creator": creator,
        "creatorState": creator_state
    })))
}

async fn list_series(State(state): State<SharedState>) -> AppResult<Json<Vec<Series>>> {
    Ok(Json(fetch_series(&state.pool, None, None).await?))
}

async fn get_series(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> AppResult<Json<Series>> {
    let maybe_identity = optional_identity(&state.pool, &headers).await?;
    let progress = match maybe_identity {
        Some(identity) => {
            fetch_continue_watching_entry(&state.pool, &identity.user_id, None, &slug).await?
        }
        None => None,
    };
    let series = fetch_series_by_slug(&state.pool, &slug, progress.as_ref()).await?;
    Ok(Json(series))
}

async fn list_films(State(state): State<SharedState>) -> AppResult<Json<Vec<Film>>> {
    Ok(Json(fetch_films(&state.pool, None, None).await?))
}

async fn get_film(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> AppResult<Json<Film>> {
    let maybe_identity = optional_identity(&state.pool, &headers).await?;
    let progress = match maybe_identity {
        Some(identity) => {
            fetch_continue_watching_entry(&state.pool, &identity.user_id, None, &slug).await?
        }
        None => None,
    };
    Ok(Json(
        fetch_film_by_slug(&state.pool, &slug, progress.as_ref()).await?,
    ))
}

async fn get_content(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let maybe_identity = optional_identity(&state.pool, &headers).await?;
    let progress = match maybe_identity {
        Some(identity) => {
            fetch_continue_watching_entry(&state.pool, &identity.user_id, Some(&id), &id).await?
        }
        None => None,
    };
    if let Ok(series) = fetch_series_by_id(&state.pool, &id, progress.as_ref()).await {
        return Ok(Json(serde_json::to_value(series)?));
    }
    if let Ok(film) = fetch_film_by_id(&state.pool, &id, progress.as_ref()).await {
        return Ok(Json(serde_json::to_value(film)?));
    }
    if let Ok(series) = fetch_creator_catalog_series_by_id(&state.pool, &id, false).await {
        return Ok(Json(serde_json::to_value(series)?));
    }
    if let Ok(film) = fetch_creator_catalog_film_by_id(&state.pool, &id, false).await {
        return Ok(Json(serde_json::to_value(film)?));
    }
    let live = fetch_live_stream_by_id(&state.pool, &id).await?;
    Ok(Json(serde_json::to_value(live)?))
}

async fn list_creator_catalog_series(
    State(state): State<SharedState>,
) -> AppResult<Json<Vec<CreatorCatalogSeries>>> {
    Ok(Json(fetch_creator_catalog_series(&state.pool, true).await?))
}

async fn get_creator_catalog_series(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<CreatorCatalogSeries>> {
    Ok(Json(
        fetch_creator_catalog_series_by_slug(&state.pool, &slug, false).await?,
    ))
}

async fn list_creator_catalog_films(
    State(state): State<SharedState>,
) -> AppResult<Json<Vec<CreatorCatalogFilm>>> {
    Ok(Json(fetch_creator_catalog_films(&state.pool, true).await?))
}

async fn get_creator_catalog_film(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<CreatorCatalogFilm>> {
    Ok(Json(
        fetch_creator_catalog_film_by_slug(&state.pool, &slug, false).await?,
    ))
}

pub(super) async fn list_live_streams(
    State(state): State<SharedState>,
) -> AppResult<Json<Vec<LiveStream>>> {
    Ok(Json(fetch_live_streams(&state.pool, None).await?))
}

async fn get_live_stream(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<LiveStream>> {
    Ok(Json(fetch_live_stream_by_slug(&state.pool, &slug).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LiveDiscoveryQuery {
    category: Option<String>,
    sort: Option<String>,
    limit: Option<i64>,
}

async fn get_live_discovery(
    State(state): State<SharedState>,
    Query(query): Query<LiveDiscoveryQuery>,
) -> AppResult<Json<LiveDiscoveryResponse>> {
    let categories = fetch_categories(&state.pool).await?;
    let active_category = match query.category.as_deref() {
        Some("all") | None => None,
        Some(category_name) => {
            if categories.iter().any(|item| item.name == category_name) {
                Some(category_name.to_string())
            } else {
                return Err(AppError::BadRequest(
                    "unknown live category filter".to_string(),
                ));
            }
        }
    };
    let active_sort = match query.sort.as_deref().unwrap_or("viewers") {
        "viewers" | "newest" => query.sort.unwrap_or_else(|| "viewers".to_string()),
        _ => {
            return Err(AppError::BadRequest(
                "sort must be either 'viewers' or 'newest'".to_string(),
            ));
        }
    };

    let limit = query.limit.unwrap_or(200).clamp(1, 500) as usize;
    let mut streams = fetch_live_streams(&state.pool, None).await?;
    let total_viewers = streams.iter().map(|stream| stream.viewers).sum();
    let total_channels = streams.len() as i64;
    if let Some(category_name) = active_category.as_deref() {
        streams.retain(|stream| stream.category == category_name);
    }
    sort_live_streams(&mut streams, &active_sort);
    if streams.len() > limit {
        streams.truncate(limit);
    }

    Ok(Json(LiveDiscoveryResponse {
        streams,
        categories,
        total_viewers,
        total_channels,
        active_category,
        active_sort,
    }))
}

#[derive(Deserialize)]
pub(super) struct LimitQuery {
    pub(super) limit: Option<i64>,
    pub(super) after_seq: Option<i64>,
}

pub(super) async fn list_chat_messages(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
    Query(query): Query<LimitQuery>,
) -> AppResult<Json<Vec<ChatMessage>>> {
    let maybe_identity = optional_identity(&state.pool, &headers).await?;
    ensure_stream_exists(&state.pool, &stream_id).await?;
    Ok(Json(
        fetch_chat_messages_for_viewer(
            &state.pool,
            &stream_id,
            maybe_identity
                .as_ref()
                .map(|identity| identity.user_id.as_str()),
            query.limit.unwrap_or(100),
            query.after_seq,
        )
        .await?,
    ))
}

#[derive(Debug)]
pub(super) struct PersistedChatMessage {
    pub(super) message: ChatMessage,
    pub(super) hidden_by_moderation: bool,
}

async fn enable_live_notify(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
) -> AppResult<Json<LiveNotifyPreference>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let stream = fetch_live_stream_by_id(&state.pool, &stream_id).await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO live_stream_notification_preferences (user_id, streamer_id, enabled, created_at)
        VALUES (?, ?, 1, ?)
        ON CONFLICT(user_id, streamer_id) DO UPDATE SET enabled = 1
        "#,
    )
    .bind(&identity.user_id)
    .bind(&stream.streamer.id)
    .bind(&now)
    .execute(&state.pool)
    .await?;

    Ok(Json(LiveNotifyPreference {
        streamer_id: stream.streamer.id,
        enabled: true,
    }))
}

pub(super) async fn create_clip_request(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
) -> AppResult<StatusCode> {
    let identity = require_identity(&state.pool, &headers).await?;
    ensure_stream_exists(&state.pool, &stream_id).await?;
    let now = Utc::now();
    let now_rfc3339 = now.to_rfc3339();
    let clip_dedupe_after = (now - chrono::Duration::seconds(30)).to_rfc3339();
    let existing = sqlx::query(
        r#"
        SELECT id
        FROM live_stream_clip_requests
        WHERE stream_id = ?
          AND user_id = ?
          AND created_at >= ?
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&stream_id)
    .bind(&identity.user_id)
    .bind(&clip_dedupe_after)
    .fetch_optional(&state.pool)
    .await?;
    if existing.is_none() {
        sqlx::query(
            "INSERT INTO live_stream_clip_requests (id, stream_id, user_id, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&stream_id)
        .bind(&identity.user_id)
        .bind(&now_rfc3339)
        .execute(&state.pool)
        .await?;
    }
    Ok(StatusCode::ACCEPTED)
}

async fn report_live_stream(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
    Json(input): Json<LiveReportRequest>,
) -> AppResult<StatusCode> {
    let identity = require_identity(&state.pool, &headers).await?;
    let stream = fetch_live_stream_by_id(&state.pool, &stream_id).await?;
    if input.reason.trim().is_empty() {
        return Err(AppError::BadRequest("reason is required".to_string()));
    }
    let report_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO live_stream_reports (id, stream_id, user_id, reason, details, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&report_id)
    .bind(&stream_id)
    .bind(&identity.user_id)
    .bind(input.reason.trim())
    .bind(input.details)
    .bind(&created_at)
    .execute(&state.pool)
    .await?;
    let reporter = fetch_user(&state.pool, &identity.user_id).await?;
    let creator_id = fetch_live_stream_owner_creator_id(&state.pool, &stream_id).await?;
    enqueue_notification_event(
        &state.pool,
        "live_report_received",
        &format!("{} reported {}.", reporter.display_name, stream.title),
        Some(&identity.user_id),
        Some(&reporter.display_name),
        Some(&creator_id),
        Some(&stream_id),
        None,
        json!({
            "reportId": report_id,
            "reason": input.reason.trim(),
        }),
        &[],
        &[creator_id.clone()],
    )
    .await?;
    Ok(StatusCode::ACCEPTED)
}

async fn list_live_stream_moderators(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
) -> AppResult<Json<Vec<CreatorModerator>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    Ok(Json(
        fetch_creator_moderators(&state.pool, &creator_id).await?,
    ))
}

async fn add_live_stream_moderator(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
    Json(input): Json<CreateCreatorModeratorRequest>,
) -> AppResult<Json<CreatorModerator>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_owner(&state.pool, &stream_id, &identity).await?;
    fetch_user(&state.pool, &input.user_id).await?;
    validate_creator_moderator_role(&input.role)?;
    let created_at = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO creator_moderators (creator_id, user_id, role, created_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(creator_id, user_id) DO UPDATE SET role = excluded.role
        "#,
    )
    .bind(&creator_id)
    .bind(&input.user_id)
    .bind(&input.role)
    .bind(&created_at)
    .execute(&state.pool)
    .await?;
    write_moderation_audit_entry(
        &state.pool,
        &creator_id,
        Some(&stream_id),
        &identity.user_id,
        Some(&input.user_id),
        "moderator_added",
        json!({"role": input.role}),
    )
    .await?;
    Ok(Json(
        fetch_creator_moderator(&state.pool, &creator_id, &input.user_id).await?,
    ))
}

pub(super) async fn remove_live_stream_moderator(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((stream_id, user_id)): Path<(String, String)>,
) -> AppResult<StatusCode> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_owner(&state.pool, &stream_id, &identity).await?;
    let result = sqlx::query("DELETE FROM creator_moderators WHERE creator_id = ? AND user_id = ?")
        .bind(&creator_id)
        .bind(&user_id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    write_moderation_audit_entry(
        &state.pool,
        &creator_id,
        Some(&stream_id),
        &identity.user_id,
        Some(&user_id),
        "moderator_removed",
        json!({}),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn list_live_moderation_actions(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
) -> AppResult<Json<Vec<LiveModerationAction>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    Ok(Json(
        fetch_live_moderation_actions(&state.pool, &stream_id, &creator_id).await?,
    ))
}

pub(super) async fn get_live_moderation_action(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((stream_id, action_id)): Path<(String, String)>,
) -> AppResult<Json<LiveModerationAction>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    let action = fetch_live_moderation_action_by_id_raw(&state.pool, &action_id).await?;
    if action.stream_id != stream_id || action.creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    Ok(Json(action))
}

pub(super) async fn reconcile_live_moderation_action(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((stream_id, action_id)): Path<(String, String)>,
) -> AppResult<Json<LiveModerationReconciliationReport>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    let action = fetch_live_moderation_action_by_id_raw(&state.pool, &action_id).await?;
    if action.stream_id != stream_id || action.creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    Ok(Json(
        reconcile_single_live_moderation_action(state, &action_id).await?,
    ))
}

pub(super) async fn create_live_moderation_action(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
    Json(input): Json<CreateLiveModerationActionRequest>,
) -> AppResult<Json<LiveModerationAction>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    let subject = fetch_user(&state.pool, &input.subject_user_id).await?;
    validate_live_moderation_action_type(&input.action_type)?;
    if input.reason.trim().is_empty() {
        return Err(AppError::BadRequest("reason is required".to_string()));
    }
    validate_live_moderation_subject(&state.pool, &stream_id, &creator_id, &identity, &subject.id)
        .await?;
    let now = Utc::now();
    let action_id = format!("lma-{}", Uuid::new_v4().simple());
    let created_at = now.to_rfc3339();
    let expires_at = input.duration_minutes.map(|minutes| {
        (now + chrono::Duration::minutes(minutes.clamp(1, 60 * 24 * 30))).to_rfc3339()
    });
    sqlx::query(
        r#"
        INSERT INTO live_moderation_actions (
            id, stream_id, creator_id, subject_user_id, actor_user_id, action_type, reason,
            state, expires_at, created_at, revoked_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, NULL)
        "#,
    )
    .bind(&action_id)
    .bind(&stream_id)
    .bind(&creator_id)
    .bind(&input.subject_user_id)
    .bind(&identity.user_id)
    .bind(&input.action_type)
    .bind(input.reason.trim())
    .bind(&expires_at)
    .bind(&created_at)
    .execute(&state.pool)
    .await?;
    let action = fetch_live_moderation_action_by_id(&state.pool, &action_id).await?;
    write_moderation_audit_entry(
        &state.pool,
        &creator_id,
        Some(&stream_id),
        &identity.user_id,
        Some(&input.subject_user_id),
        "moderation_action_created",
        json!({
            "actionId": action_id,
            "actionType": input.action_type,
            "reason": input.reason.trim(),
            "expiresAt": expires_at,
        }),
    )
    .await?;
    let actor = fetch_user(&state.pool, &identity.user_id).await?;
    enqueue_notification_event(
        &state.pool,
        "moderation_action",
        &format!(
            "{} applied a moderation action to your live chat access.",
            actor.display_name
        ),
        Some(&identity.user_id),
        Some(&actor.display_name),
        Some(&creator_id),
        Some(&stream_id),
        None,
        json!({
            "actionId": action_id,
            "actionType": input.action_type,
            "reason": input.reason.trim(),
            "expiresAt": expires_at,
        }),
        &[input.subject_user_id.clone()],
        &[],
    )
    .await?;
    state
        .realtime
        .publish(
            &stream_channel_id(&stream_id),
            WsEvent::ModerationAction {
                action: action.clone(),
            },
        )
        .await;
    Ok(Json(action))
}

pub(super) async fn revoke_live_moderation_action(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((stream_id, action_id)): Path<(String, String)>,
) -> AppResult<Json<LiveModerationAction>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    let action = fetch_live_moderation_action_by_id(&state.pool, &action_id).await?;
    if action.stream_id != stream_id || action.creator_id != creator_id {
        return Err(AppError::NotFound);
    }
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE live_moderation_actions SET state = 'revoked', revoked_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&action_id)
    .execute(&state.pool)
    .await?;
    write_moderation_audit_entry(
        &state.pool,
        &creator_id,
        Some(&stream_id),
        &identity.user_id,
        Some(&action.subject_user_id),
        "moderation_action_revoked",
        json!({
            "actionId": action_id,
            "revokedAt": now,
        }),
    )
    .await?;
    let revoked = fetch_live_moderation_action_by_id(&state.pool, &action_id).await?;
    state
        .realtime
        .publish(
            &stream_channel_id(&stream_id),
            WsEvent::ModerationAction {
                action: revoked.clone(),
            },
        )
        .await;
    Ok(Json(revoked))
}

async fn list_live_stream_reports(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
) -> AppResult<Json<Vec<LiveStreamReportRecord>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    Ok(Json(
        fetch_live_stream_reports(&state.pool, &stream_id).await?,
    ))
}

pub(super) async fn resolve_live_stream_report(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((stream_id, report_id)): Path<(String, String)>,
    Json(input): Json<ResolveLiveStreamReportRequest>,
) -> AppResult<Json<LiveStreamReportRecord>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    validate_live_report_status(&input.status)?;
    let report = fetch_live_stream_report_by_id(&state.pool, &report_id).await?;
    if report.stream_id != stream_id {
        return Err(AppError::NotFound);
    }
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"
        UPDATE live_stream_reports
        SET status = ?, resolved_by_user_id = ?, resolution_note = ?, resolved_at = ?
        WHERE id = ? AND stream_id = ?
        "#,
    )
    .bind(&input.status)
    .bind(&identity.user_id)
    .bind(&input.resolution_note)
    .bind(&now)
    .bind(&report_id)
    .bind(&stream_id)
    .execute(&state.pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    write_moderation_audit_entry(
        &state.pool,
        &creator_id,
        Some(&stream_id),
        &identity.user_id,
        None,
        "report_resolved",
        json!({
            "reportId": report_id,
            "status": input.status,
            "resolutionNote": input.resolution_note,
        }),
    )
    .await?;
    Ok(Json(
        fetch_live_stream_report_by_id(&state.pool, &report_id).await?,
    ))
}

async fn list_live_moderation_audit_log(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
) -> AppResult<Json<Vec<ModerationAuditEntry>>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let creator_id = authorize_live_stream_moderation(&state.pool, &stream_id, &identity).await?;
    Ok(Json(
        fetch_moderation_audit_log(&state.pool, &creator_id, Some(&stream_id)).await?,
    ))
}

pub(super) async fn get_live_viewer_preview(
    State(state): State<SharedState>,
    Path(stream_id): Path<String>,
) -> AppResult<Json<ViewerPreview>> {
    ensure_stream_exists(&state.pool, &stream_id).await?;
    Ok(Json(ViewerPreview {
        total_viewers: effective_live_viewer_count(&state.pool, &stream_id).await?,
        sample_users: fetch_live_viewer_sample_users(&state.pool, &stream_id, 8).await?,
    }))
}

async fn post_chat_message(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(stream_id): Path<String>,
    Json(input): Json<ChatInput>,
) -> AppResult<Json<ChatMessage>> {
    let identity = require_identity(&state.pool, &headers).await?;
    let persisted = persist_chat_message(&state, &stream_id, &identity, input).await?;
    Ok(Json(persisted.message))
}

async fn list_categories(State(state): State<SharedState>) -> AppResult<Json<Vec<Category>>> {
    Ok(Json(fetch_categories(&state.pool).await?))
}

async fn get_category(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<Category>> {
    Ok(Json(fetch_category_by_slug(&state.pool, &slug).await?))
}

async fn get_category_browse(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> AppResult<Json<CategoryBrowseResponse>> {
    let category = fetch_category_by_slug(&state.pool, &slug).await?;
    let live_streams = fetch_live_streams_by_category(&state.pool, &category.name).await?;
    let series = fetch_series_by_genre(&state.pool, &category.name).await?;
    let films = fetch_films_by_genre(&state.pool, &category.name).await?;
    let total_vod_titles = (series.len() + films.len()) as i64;

    Ok(Json(CategoryBrowseResponse {
        category,
        live_streams,
        series,
        films,
        total_vod_titles,
    }))
}

async fn list_streamers(State(state): State<SharedState>) -> AppResult<Json<Vec<Streamer>>> {
    Ok(Json(fetch_streamers(&state.pool).await?))
}

async fn get_streamer(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> AppResult<Json<Streamer>> {
    Ok(Json(fetch_streamer_by_id(&state.pool, &id).await?))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

async fn search(
    State(state): State<SharedState>,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let Some(fts_query) = build_fts_query(&query.q) else {
        return Ok(Json(serde_json::json!({
            "series": [],
            "films": [],
            "liveStreams": []
        })));
    };

    let rows = sqlx::query(
        r#"
        SELECT entity_id, kind
        FROM search_documents
        WHERE search_documents MATCH ?
        ORDER BY bm25(search_documents, 1.0, 0.3)
        LIMIT 24
        "#,
    )
    .bind(&fts_query)
    .fetch_all(&state.pool)
    .await?;

    let mut series = Vec::new();
    let mut films = Vec::new();
    let mut live_streams = Vec::new();
    for row in rows {
        let entity_id: String = row.get("entity_id");
        let kind: String = row.get("kind");
        match kind.as_str() {
            "series" => {
                if let Ok(item) = fetch_series_by_id(&state.pool, &entity_id, None).await {
                    series.push(item);
                }
            }
            "film" => {
                if let Ok(item) = fetch_film_by_id(&state.pool, &entity_id, None).await {
                    films.push(item);
                }
            }
            "live" => {
                if let Ok(item) = fetch_live_stream_by_id(&state.pool, &entity_id).await {
                    live_streams.push(item);
                }
            }
            _ => {}
        }
    }

    Ok(Json(serde_json::json!({
        "series": series,
        "films": films,
        "liveStreams": live_streams
    })))
}
