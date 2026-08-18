use super::*;

pub(crate) async fn health(State(state): State<SharedState>) -> AppResult<Json<HealthResponse>> {
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

pub(crate) async fn health_live() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

pub(crate) async fn health_ready(State(state): State<SharedState>) -> impl IntoResponse {
    let runtime = check_runtime_dependencies(state.as_ref()).await;
    if runtime.ready {
        StatusCode::NO_CONTENT.into_response()
    } else {
        StatusCode::SERVICE_UNAVAILABLE.into_response()
    }
}

pub(crate) async fn check_runtime_dependencies(state: &AppState) -> RuntimeHealthStatus {
    check_runtime_dependencies_with_binaries(state, "ffmpeg", "ffprobe").await
}

pub(crate) async fn check_runtime_dependencies_with_binaries(
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

pub(crate) async fn check_media_root_writable(media_root: &FsPath) -> HealthDependencyStatus {
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

pub(crate) async fn check_binary_available(binary: &str) -> HealthDependencyStatus {
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
