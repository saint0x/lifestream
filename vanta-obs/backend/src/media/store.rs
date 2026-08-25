use chrono::Utc;
use serde_json::{Value, json};
use sqlx::{Column, Row, SqlitePool};
use uuid::Uuid;

use super::domain::{CaptureStartInput, EncodeStartInput};

#[derive(Debug, thiserror::Error)]
pub enum MediaStoreError {
    #[error("not found")]
    NotFound,
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct MediaStore {
    pool: SqlitePool,
}

impl MediaStore {
    pub async fn connect(pool: SqlitePool) -> Result<Self, MediaStoreError> {
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    async fn migrate(&self) -> Result<(), MediaStoreError> {
        for statement in SCHEMA.split(";").map(str::trim).filter(|s| !s.is_empty()) {
            sqlx::query(statement).execute(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn create_capture_session(
        &self,
        input: CaptureStartInput,
        helper_session_id: String,
        helper_command_json: Value,
        source_health_json: Value,
    ) -> Result<Value, MediaStoreError> {
        let session_id = id();
        let now = now();
        let duration_seconds = input.duration_seconds.unwrap_or(2);
        let permission_json = source_health_json
            .get("permission")
            .cloned()
            .unwrap_or_else(|| json!({"status":"unknown","required":false,"remediation":""}));
        let settings = json!({
            "width": input.width,
            "height": input.height,
            "frame_rate": input.frame_rate,
            "audio": input.audio.unwrap_or(false),
            "duration_seconds": duration_seconds,
            "long_capture_validation": duration_seconds >= 5,
            "low_latency_preview": true,
            "permission": permission_json,
            "source_health": source_health_json
        });
        sqlx::query(
            "INSERT INTO media_capture_sessions
            (id, creator_id, source_id, helper_session_id, capture_kind, status, width, height, frame_rate, audio_enabled, settings_json, helper_command_json, health_json, started_at, stopped_at, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, ?, 'capturing', ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)",
        )
        .bind(&session_id)
        .bind(input.source_id)
        .bind(helper_session_id)
        .bind(input.capture_kind)
        .bind(input.width)
        .bind(input.height)
        .bind(input.frame_rate)
        .bind(bool_int(input.audio.unwrap_or(false)))
        .bind(settings.to_string())
        .bind(helper_command_json.to_string())
        .bind(json!({
            "state":"capturing",
            "dropped_frames":0,
            "permission": permission_json,
            "duration_seconds": duration_seconds,
            "long_capture_validation": duration_seconds >= 5,
            "source_health": source_health_json,
            "events": [{
                "event_kind": "source_health",
                "payload_json": source_health_json
            }]
        }).to_string())
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.capture_session(&session_id).await
    }

    pub async fn stop_capture_session(&self, session_id: &str) -> Result<Value, MediaStoreError> {
        let now = now();
        let result = sqlx::query(
            "UPDATE media_capture_sessions SET status = 'stopped', stopped_at = ?, updated_at = ?, health_json = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(&now)
        .bind(json!({"state":"stopped","dropped_frames":0}).to_string())
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(MediaStoreError::NotFound);
        }
        self.capture_session(session_id).await
    }

    pub async fn reconcile_capture_session(
        &self,
        session_id: &str,
        status: &str,
        source_health_json: Value,
        reconnect_json: Value,
        helper_command_json: Value,
    ) -> Result<Value, MediaStoreError> {
        let now = now();
        let session = self.capture_session(session_id).await?;
        let mut settings = json_object(session.get("settings_json"));
        let mut health = json_object(session.get("health_json"));
        let permission_json = source_health_json
            .get("permission")
            .cloned()
            .unwrap_or_else(|| json!({"status":"unknown","required":false,"remediation":""}));

        settings.insert("permission".to_string(), permission_json.clone());
        settings.insert("source_health".to_string(), source_health_json.clone());
        settings.insert("native_reconnect".to_string(), reconnect_json.clone());

        health.insert("state".to_string(), json!(status));
        health.insert("permission".to_string(), permission_json);
        health.insert("source_health".to_string(), source_health_json.clone());
        health.insert("native_reconnect".to_string(), reconnect_json.clone());
        let mut events = health
            .remove("events")
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default();
        events.push(json!({
            "event_kind": "native_reconnect",
            "payload_json": reconnect_json
        }));
        events.push(json!({
            "event_kind": "source_health",
            "payload_json": source_health_json
        }));
        health.insert("events".to_string(), Value::Array(events));

        let result = sqlx::query(
            "UPDATE media_capture_sessions SET status = ?, settings_json = ?, helper_command_json = ?, health_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(status)
        .bind(Value::Object(settings).to_string())
        .bind(helper_command_json.to_string())
        .bind(Value::Object(health).to_string())
        .bind(&now)
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(MediaStoreError::NotFound);
        }
        self.capture_session(session_id).await
    }

    pub async fn create_capture_frame(
        &self,
        capture_session_id: &str,
        artifact_path: &str,
        validation_json: Value,
    ) -> Result<Value, MediaStoreError> {
        self.create_capture_frame_with_kind(
            capture_session_id,
            artifact_path,
            "preview_png",
            validation_json,
        )
        .await
    }

    pub async fn create_capture_frame_with_kind(
        &self,
        capture_session_id: &str,
        artifact_path: &str,
        frame_kind: &str,
        validation_json: Value,
    ) -> Result<Value, MediaStoreError> {
        let frame_id = id();
        let now = now();
        sqlx::query(
            "INSERT INTO media_capture_frames
            (id, creator_id, capture_session_id, artifact_path, frame_kind, status, validation_json, captured_at, created_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, ?, 'ready', ?, ?, ?)",
        )
        .bind(&frame_id)
        .bind(capture_session_id)
        .bind(artifact_path)
        .bind(frame_kind)
        .bind(validation_json.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        if frame_kind == "runtime_browser_surface_png" {
            self.update_browser_surface_session_health(capture_session_id, &validation_json, &now)
                .await?;
        }
        self.update_runtime_compositor_health(
            capture_session_id,
            frame_kind,
            "frame",
            &validation_json,
            &now,
        )
        .await?;
        self.capture_frame(&frame_id).await
    }

    pub async fn create_capture_artifact(
        &self,
        capture_session_id: &str,
        artifact_kind: &str,
        artifact_path: &str,
        validation_json: Value,
    ) -> Result<Value, MediaStoreError> {
        let artifact_id = id();
        let now = now();
        sqlx::query(
            "INSERT INTO media_capture_artifacts
            (id, creator_id, capture_session_id, artifact_kind, status, artifact_path, validation_json, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, 'ready', ?, ?, ?, ?)",
        )
        .bind(&artifact_id)
        .bind(capture_session_id)
        .bind(artifact_kind)
        .bind(artifact_path)
        .bind(validation_json.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        let session = self.capture_session(capture_session_id).await?;
        let mut settings = json_object(session.get("settings_json"));
        let mut health = json_object(session.get("health_json"));
        settings.insert(
            "continuous_capture".to_string(),
            json!({
                "status": "ready",
                "artifact_id": artifact_id,
                "artifact_path": artifact_path,
                "artifact_kind": artifact_kind
            }),
        );
        health.insert("state".to_string(), json!("capturing"));
        health.insert(
            "continuous_capture".to_string(),
            json!({
                "status": "ready",
                "artifact_id": artifact_id,
                "artifact_path": artifact_path,
                "artifact_kind": artifact_kind,
                "validation": validation_json
            }),
        );
        merge_runtime_compositor_health(
            &mut settings,
            &mut health,
            artifact_kind,
            "artifact",
            &validation_json,
            &now,
        );
        sqlx::query(
            "UPDATE media_capture_sessions SET settings_json = ?, health_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(Value::Object(settings).to_string())
        .bind(Value::Object(health).to_string())
        .bind(&now)
        .bind(capture_session_id)
        .execute(&self.pool)
        .await?;

        self.capture_artifact(&artifact_id).await
    }

    pub async fn capture_sessions(&self) -> Result<Vec<Value>, MediaStoreError> {
        self.list(
            "SELECT * FROM media_capture_sessions ORDER BY updated_at DESC",
            &[],
        )
        .await
    }

    pub async fn capture_session(&self, session_id: &str) -> Result<Value, MediaStoreError> {
        self.row(
            "SELECT * FROM media_capture_sessions WHERE id = ?",
            &[session_id],
        )
        .await
    }

    pub async fn capture_frame(&self, frame_id: &str) -> Result<Value, MediaStoreError> {
        self.row(
            "SELECT * FROM media_capture_frames WHERE id = ?",
            &[frame_id],
        )
        .await
    }

    pub async fn capture_artifact(&self, artifact_id: &str) -> Result<Value, MediaStoreError> {
        self.row(
            "SELECT * FROM media_capture_artifacts WHERE id = ?",
            &[artifact_id],
        )
        .await
    }

    pub async fn obs_source(&self, source_id: &str) -> Result<Value, MediaStoreError> {
        self.row("SELECT * FROM obs_sources WHERE id = ?", &[source_id])
            .await
    }

    pub async fn capture_frames(&self, session_id: &str) -> Result<Vec<Value>, MediaStoreError> {
        self.list(
            "SELECT * FROM media_capture_frames WHERE capture_session_id = ? ORDER BY captured_at DESC",
            &[session_id],
        )
        .await
    }

    pub async fn capture_artifacts(&self, session_id: &str) -> Result<Vec<Value>, MediaStoreError> {
        self.list(
            "SELECT * FROM media_capture_artifacts WHERE capture_session_id = ? ORDER BY updated_at DESC",
            &[session_id],
        )
        .await
    }

    pub async fn create_source_artifact(
        &self,
        source_id: &str,
        artifact_kind: &str,
        artifact_path: &str,
        validation_json: Value,
    ) -> Result<Value, MediaStoreError> {
        let artifact_id = id();
        let now = now();
        sqlx::query(
            "INSERT INTO media_source_artifacts
            (id, creator_id, source_id, artifact_kind, status, artifact_path, validation_json, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, 'ready', ?, ?, ?, ?)",
        )
        .bind(&artifact_id)
        .bind(source_id)
        .bind(artifact_kind)
        .bind(artifact_path)
        .bind(validation_json.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.source_artifact(&artifact_id).await
    }

    pub async fn source_artifact(&self, artifact_id: &str) -> Result<Value, MediaStoreError> {
        self.row(
            "SELECT * FROM media_source_artifacts WHERE id = ?",
            &[artifact_id],
        )
        .await
    }

    pub async fn source_artifacts(&self, source_id: &str) -> Result<Vec<Value>, MediaStoreError> {
        self.list(
            "SELECT * FROM media_source_artifacts WHERE source_id = ? ORDER BY updated_at DESC",
            &[source_id],
        )
        .await
    }

    pub async fn create_encode_job(
        &self,
        input: EncodeStartInput,
        helper_session_id: String,
        helper_command_json: Value,
    ) -> Result<Value, MediaStoreError> {
        let job_id = id();
        let now = now();
        let output_path = format!("vanta://media/encoded/{job_id}.{}", input.container);
        let profile = json!({
            "codec": input.codec,
            "audio_codec": input.audio_codec,
            "container": input.container,
            "bitrate_kbps": input.bitrate_kbps,
            "keyframe_interval_seconds": input.keyframe_interval_seconds,
            "latency_profile": input.latency_profile,
            "hardware_encoder": "auto",
            "muxer_recovery": true
        });
        sqlx::query(
            "INSERT INTO media_encode_jobs
            (id, creator_id, broadcast_id, capture_session_id, helper_session_id, status, codec, audio_codec, container, bitrate_kbps, keyframe_interval_seconds, latency_profile, output_path, profile_json, helper_command_json, health_json, started_at, stopped_at, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, ?, 'encoding', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)",
        )
        .bind(&job_id)
        .bind(input.broadcast_id)
        .bind(input.capture_session_id)
        .bind(helper_session_id)
        .bind(input.codec)
        .bind(input.audio_codec)
        .bind(input.container)
        .bind(input.bitrate_kbps)
        .bind(input.keyframe_interval_seconds)
        .bind(input.latency_profile)
        .bind(output_path)
        .bind(profile.to_string())
        .bind(helper_command_json.to_string())
        .bind(json!({"state":"encoding","dropped_frames":0,"bitrate_stable":true}).to_string())
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.encode_job(&job_id).await
    }

    pub async fn stop_encode_job(&self, job_id: &str) -> Result<Value, MediaStoreError> {
        let now = now();
        let result = sqlx::query(
            "UPDATE media_encode_jobs SET status = 'finalizing', stopped_at = ?, updated_at = ?, health_json = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(&now)
        .bind(json!({"state":"finalizing","playable_validation":"pending"}).to_string())
        .bind(job_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(MediaStoreError::NotFound);
        }
        self.encode_job(job_id).await
    }

    pub async fn mark_encode_rendered(
        &self,
        job_id: &str,
        output_path: &str,
        validation_json: Value,
    ) -> Result<Value, MediaStoreError> {
        let now = now();
        let health = json!({
            "state": "playable",
            "playable_validation": "passed",
            "validation": validation_json
        });
        let result = sqlx::query(
            "UPDATE media_encode_jobs SET status = 'playable', output_path = ?, health_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(output_path)
        .bind(health.to_string())
        .bind(&now)
        .bind(job_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(MediaStoreError::NotFound);
        }
        self.encode_job(job_id).await
    }

    pub async fn create_package(
        &self,
        encode_job_id: &str,
        package_kind: &str,
        manifest_path: &str,
        package_json: Value,
    ) -> Result<Value, MediaStoreError> {
        let package_id = id();
        let now = now();
        sqlx::query(
            "INSERT INTO media_packages
            (id, creator_id, encode_job_id, package_kind, status, manifest_path, package_json, validation_json, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, 'ready', ?, ?, ?, ?, ?)",
        )
        .bind(&package_id)
        .bind(encode_job_id)
        .bind(package_kind)
        .bind(manifest_path)
        .bind(package_json.to_string())
        .bind(json!({"playback_ready":true,"manifest_exists":true}).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.package(&package_id).await
    }

    pub async fn packages(&self) -> Result<Vec<Value>, MediaStoreError> {
        self.list("SELECT * FROM media_packages ORDER BY updated_at DESC", &[])
            .await
    }

    pub async fn package(&self, package_id: &str) -> Result<Value, MediaStoreError> {
        self.row("SELECT * FROM media_packages WHERE id = ?", &[package_id])
            .await
    }

    pub async fn encode_jobs(&self) -> Result<Vec<Value>, MediaStoreError> {
        self.list(
            "SELECT * FROM media_encode_jobs ORDER BY updated_at DESC",
            &[],
        )
        .await
    }

    pub async fn encode_job(&self, job_id: &str) -> Result<Value, MediaStoreError> {
        self.row("SELECT * FROM media_encode_jobs WHERE id = ?", &[job_id])
            .await
    }

    async fn list(&self, sql: &str, binds: &[&str]) -> Result<Vec<Value>, MediaStoreError> {
        let mut query = sqlx::query(sql);
        for value in binds {
            query = query.bind(*value);
        }
        let rows = query.fetch_all(&self.pool).await?;
        rows.iter().map(object_row).collect()
    }

    async fn row(&self, sql: &str, binds: &[&str]) -> Result<Value, MediaStoreError> {
        let mut query = sqlx::query(sql);
        for value in binds {
            query = query.bind(*value);
        }
        query
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(object_row)
            .transpose()?
            .ok_or(MediaStoreError::NotFound)
    }

    async fn update_browser_surface_session_health(
        &self,
        capture_session_id: &str,
        validation_json: &Value,
        now: &str,
    ) -> Result<(), MediaStoreError> {
        let session = self.capture_session(capture_session_id).await?;
        let mut settings = json_object(session.get("settings_json"));
        let mut health = json_object(session.get("health_json"));
        let mut source_surface = json_object(health.get("browser_surface"));
        let previous_frames = source_surface
            .get("frames_received")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let previous_dropped = source_surface
            .get("cumulative_dropped_frames")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let previous_reconnect = source_surface
            .get("max_reconnect_count")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let previous_latency = source_surface
            .get("max_ingest_latency_ms")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let long_session = validation_json
            .get("long_session")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let dropped = long_session
            .get("dropped_frames")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let reconnect_count = long_session
            .get("reconnect_count")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let ingest_latency = long_session
            .get("ingest_latency_ms")
            .and_then(Value::as_i64)
            .unwrap_or_default();

        source_surface.insert("status".to_string(), json!("receiving"));
        source_surface.insert(
            "source_id".to_string(),
            validation_json
                .get("source_id")
                .cloned()
                .unwrap_or_else(|| json!("")),
        );
        source_surface.insert(
            "source_kind".to_string(),
            validation_json
                .get("source_kind")
                .cloned()
                .unwrap_or_else(|| json!("")),
        );
        source_surface.insert(
            "latest_frame_kind".to_string(),
            validation_json
                .get("frame_kind")
                .cloned()
                .unwrap_or_else(|| json!("runtime_browser_surface_png")),
        );
        source_surface.insert("frames_received".to_string(), json!(previous_frames + 1));
        source_surface.insert(
            "cumulative_dropped_frames".to_string(),
            json!(previous_dropped + dropped),
        );
        source_surface.insert(
            "max_reconnect_count".to_string(),
            json!(previous_reconnect.max(reconnect_count)),
        );
        source_surface.insert(
            "max_ingest_latency_ms".to_string(),
            json!(previous_latency.max(ingest_latency)),
        );
        source_surface.insert("latest_long_session".to_string(), long_session);
        source_surface.insert(
            "latest_artifact_sha256".to_string(),
            validation_json
                .get("sha256")
                .cloned()
                .unwrap_or_else(|| json!("")),
        );
        source_surface.insert(
            "latest_frame_sequence".to_string(),
            validation_json
                .get("frame_sequence")
                .cloned()
                .unwrap_or_else(|| json!(0)),
        );
        source_surface.insert("updated_at".to_string(), json!(now));

        settings.insert(
            "browser_surface".to_string(),
            Value::Object(source_surface.clone()),
        );
        health.insert("state".to_string(), json!("capturing"));
        health.insert("browser_surface".to_string(), Value::Object(source_surface));
        sqlx::query(
            "UPDATE media_capture_sessions SET settings_json = ?, health_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(Value::Object(settings).to_string())
        .bind(Value::Object(health).to_string())
        .bind(now)
        .bind(capture_session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_runtime_compositor_health(
        &self,
        capture_session_id: &str,
        output_kind: &str,
        output_scope: &str,
        validation_json: &Value,
        now: &str,
    ) -> Result<(), MediaStoreError> {
        let session = self.capture_session(capture_session_id).await?;
        let mut settings = json_object(session.get("settings_json"));
        let mut health = json_object(session.get("health_json"));
        merge_runtime_compositor_health(
            &mut settings,
            &mut health,
            output_kind,
            output_scope,
            validation_json,
            now,
        );
        sqlx::query(
            "UPDATE media_capture_sessions SET settings_json = ?, health_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(Value::Object(settings).to_string())
        .bind(Value::Object(health).to_string())
        .bind(now)
        .bind(capture_session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn object_row(row: &sqlx::sqlite::SqliteRow) -> Result<Value, MediaStoreError> {
    let mut object = serde_json::Map::new();
    for column in row.columns() {
        let name = column.name();
        if JSON_COLUMNS.contains(&name) {
            let raw: String = row.try_get(name)?;
            object.insert(
                name.to_string(),
                serde_json::from_str(&raw).unwrap_or_else(|_| json!({})),
            );
        } else if let Ok(value) = row.try_get::<String, _>(name) {
            object.insert(name.to_string(), json!(value));
        } else if let Ok(value) = row.try_get::<i64, _>(name) {
            object.insert(name.to_string(), json!(value));
        }
    }
    Ok(Value::Object(object))
}

fn id() -> String {
    Uuid::new_v4().to_string()
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn bool_int(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

fn json_object(value: Option<&Value>) -> serde_json::Map<String, Value> {
    value
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn merge_runtime_compositor_health(
    settings: &mut serde_json::Map<String, Value>,
    health: &mut serde_json::Map<String, Value>,
    output_kind: &str,
    output_scope: &str,
    validation_json: &Value,
    now: &str,
) {
    let mut compositor = json_object(health.get("runtime_compositor"));
    let previous_outputs = compositor
        .get("outputs_observed")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let previous_dropped = compositor
        .get("cumulative_dropped_frames")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let previous_max_latency = compositor
        .get("max_ingest_latency_ms")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let capture_kind = validation_json
        .get("capture_kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let source_kind = validation_json
        .get("source_kind")
        .and_then(Value::as_str)
        .or_else(|| {
            validation_json
                .get("frame_pacing")
                .and_then(|value| value.get("reported_for_source_kind"))
                .and_then(Value::as_str)
        })
        .unwrap_or(capture_kind);
    let dropped_frames = validation_json
        .get("frame_pacing")
        .and_then(|value| value.get("dropped_frames"))
        .and_then(Value::as_i64)
        .or_else(|| {
            validation_json
                .get("dropped_frames")
                .and_then(Value::as_i64)
        })
        .or_else(|| {
            validation_json
                .get("long_session")
                .and_then(|value| value.get("dropped_frames"))
                .and_then(Value::as_i64)
        })
        .unwrap_or_default();
    let max_ingest_latency_ms = validation_json
        .get("frame_pacing")
        .and_then(|value| value.get("max_ingest_latency_ms"))
        .and_then(Value::as_i64)
        .or_else(|| {
            validation_json
                .get("long_session")
                .and_then(|value| value.get("ingest_latency_ms"))
                .and_then(Value::as_i64)
        })
        .unwrap_or_default();
    let pacing_mode = validation_json
        .get("frame_pacing")
        .and_then(|value| value.get("mode"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            if output_scope == "artifact" {
                "native_runtime_capture"
            } else {
                "runtime_program_clock"
            }
        });
    let cumulative_dropped_frames = previous_dropped + dropped_frames;
    let pressure = if cumulative_dropped_frames > 120 || max_ingest_latency_ms > 2500 {
        "degraded"
    } else if cumulative_dropped_frames > 0 || max_ingest_latency_ms > 1000 {
        "watch"
    } else {
        "nominal"
    };
    let latest = json!({
        "output_kind": output_kind,
        "output_scope": output_scope,
        "capture_kind": capture_kind,
        "source_kind": source_kind,
        "pacing_mode": pacing_mode,
        "dropped_frames": dropped_frames,
        "max_ingest_latency_ms": max_ingest_latency_ms,
        "frame_sequence": validation_json.get("frame_sequence").cloned().unwrap_or_else(|| json!(0)),
        "compositor_backend": validation_json.get("compositor_backend").cloned().unwrap_or_else(|| json!("native_runtime")),
        "updated_at": now
    });
    compositor.insert("status".to_string(), json!(pressure));
    compositor.insert("coverage".to_string(), json!("all_live_capture_outputs"));
    compositor.insert("outputs_observed".to_string(), json!(previous_outputs + 1));
    compositor.insert(
        "cumulative_dropped_frames".to_string(),
        json!(cumulative_dropped_frames),
    );
    compositor.insert(
        "max_ingest_latency_ms".to_string(),
        json!(previous_max_latency.max(max_ingest_latency_ms)),
    );
    compositor.insert("latest_output".to_string(), latest);
    compositor.insert(
        "durable_frame_pacing".to_string(),
        json!({
            "clock": "program_clock",
            "drop_policy": "hold_last_good_frame_then_resync",
            "source_kind": source_kind,
            "capture_kind": capture_kind
        }),
    );
    compositor.insert("updated_at".to_string(), json!(now));
    settings.insert(
        "runtime_compositor".to_string(),
        Value::Object(compositor.clone()),
    );
    health.insert("state".to_string(), json!("capturing"));
    health.insert("runtime_compositor".to_string(), Value::Object(compositor));
}

const JSON_COLUMNS: [&str; 6] = [
    "settings_json",
    "profile_json",
    "helper_command_json",
    "health_json",
    "package_json",
    "validation_json",
];

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS media_capture_sessions (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, source_id TEXT NOT NULL,
  helper_session_id TEXT NOT NULL, capture_kind TEXT NOT NULL, status TEXT NOT NULL,
  width INTEGER NOT NULL, height INTEGER NOT NULL, frame_rate INTEGER NOT NULL,
  audio_enabled INTEGER NOT NULL, settings_json TEXT NOT NULL,
  helper_command_json TEXT NOT NULL, health_json TEXT NOT NULL,
  started_at TEXT NOT NULL, stopped_at TEXT, created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS media_encode_jobs (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, broadcast_id TEXT NOT NULL,
  capture_session_id TEXT NOT NULL, helper_session_id TEXT NOT NULL,
  status TEXT NOT NULL, codec TEXT NOT NULL, audio_codec TEXT NOT NULL,
  container TEXT NOT NULL, bitrate_kbps INTEGER NOT NULL,
  keyframe_interval_seconds INTEGER NOT NULL, latency_profile TEXT NOT NULL,
  output_path TEXT NOT NULL, profile_json TEXT NOT NULL,
  helper_command_json TEXT NOT NULL, health_json TEXT NOT NULL,
  started_at TEXT NOT NULL, stopped_at TEXT, created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS media_capture_frames (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, capture_session_id TEXT NOT NULL,
  artifact_path TEXT NOT NULL, frame_kind TEXT NOT NULL, status TEXT NOT NULL,
  validation_json TEXT NOT NULL, captured_at TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS media_capture_artifacts (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, capture_session_id TEXT NOT NULL,
  artifact_kind TEXT NOT NULL, status TEXT NOT NULL, artifact_path TEXT NOT NULL,
  validation_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS media_source_artifacts (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, source_id TEXT NOT NULL,
  artifact_kind TEXT NOT NULL, status TEXT NOT NULL, artifact_path TEXT NOT NULL,
  validation_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS media_packages (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, encode_job_id TEXT NOT NULL,
  package_kind TEXT NOT NULL, status TEXT NOT NULL, manifest_path TEXT NOT NULL,
  package_json TEXT NOT NULL, validation_json TEXT NOT NULL, created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
"#;
