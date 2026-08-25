use chrono::Utc;
use serde_json::{Value, json};
use sqlx::{Column, Row, SqlitePool};
use uuid::Uuid;

use super::protocol::{NativeHelperHeartbeat, NativeHelperLaunch};

#[derive(Debug, thiserror::Error)]
pub enum NativeStoreError {
    #[error("not found")]
    NotFound,
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct NativeStore {
    pool: SqlitePool,
}

impl NativeStore {
    pub async fn connect(pool: SqlitePool) -> Result<Self, NativeStoreError> {
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    async fn migrate(&self) -> Result<(), NativeStoreError> {
        for statement in SCHEMA.split(";").map(str::trim).filter(|s| !s.is_empty()) {
            sqlx::query(statement).execute(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn create_session(
        &self,
        launch: NativeHelperLaunch,
    ) -> Result<Value, NativeStoreError> {
        let session_id = id();
        let now = now();
        sqlx::query(
            "INSERT INTO native_helper_sessions
            (id, creator_id, helper_kind, protocol_version, binary_path, launch_mode, process_id, endpoint, status, health_json, capabilities_json, last_heartbeat_at, crash_count, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, ?, ?, ?, ?, 'ready', ?, ?, ?, 0, ?, ?)",
        )
        .bind(&session_id)
        .bind(&launch.helper_kind)
        .bind(&launch.protocol_version)
        .bind(&launch.binary_path)
        .bind(&launch.launch_mode)
        .bind(launch.process_id)
        .bind(&launch.endpoint)
        .bind(launch.health_json.to_string())
        .bind(launch.capabilities_json.to_string())
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.add_event(
            &session_id,
            "launched",
            json!({"endpoint": launch.endpoint, "trace_event": format!("native.helper.{session_id}.launched")}),
        )
        .await?;
        self.add_log(
            &session_id,
            "info",
            "Native helper launched",
            json!({
                "endpoint": launch.endpoint,
                "helper_kind": launch.helper_kind,
                "trace_event": format!("native.helper.{session_id}.launched")
            }),
        )
        .await?;
        self.session(&session_id).await
    }

    pub async fn apply_heartbeat(
        &self,
        session_id: &str,
        heartbeat: NativeHelperHeartbeat,
    ) -> Result<Value, NativeStoreError> {
        let now = now();
        if heartbeat.status == "crashed" {
            sqlx::query(
                "UPDATE native_helper_sessions SET status = ?, health_json = ?, last_heartbeat_at = ?, crash_count = crash_count + 1, updated_at = ? WHERE id = ?",
            )
            .bind(&heartbeat.status)
            .bind(heartbeat.health_json.to_string())
            .bind(&now)
            .bind(&now)
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                "UPDATE native_helper_sessions SET status = ?, health_json = ?, last_heartbeat_at = ?, updated_at = ? WHERE id = ?",
            )
            .bind(&heartbeat.status)
            .bind(heartbeat.health_json.to_string())
            .bind(&now)
            .bind(&now)
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        }
        self.add_event(
            session_id,
            match heartbeat.status.as_str() {
                "stopped" => "stopped",
                "crashed" => "crashed",
                "degraded" => "degraded",
                _ => "heartbeat",
            },
            heartbeat.health_json.clone(),
        )
        .await?;
        self.add_log(
            session_id,
            if heartbeat.status == "crashed" {
                "error"
            } else if heartbeat.status == "degraded" {
                "warn"
            } else {
                "debug"
            },
            if heartbeat.status == "crashed" {
                "Native helper crash reported"
            } else if heartbeat.status == "degraded" {
                "Native helper degraded"
            } else {
                "Native helper heartbeat applied"
            },
            heartbeat.health_json,
        )
        .await?;
        self.session(session_id).await
    }

    pub async fn recover_session(
        &self,
        session_id: &str,
        launch: NativeHelperLaunch,
        reason: &str,
    ) -> Result<Value, NativeStoreError> {
        let now = now();
        let health = json!({
            "state": "ready",
            "helper_kind": launch.helper_kind,
            "protocol_version": launch.protocol_version,
            "recovered": true,
            "recovery_reason": reason,
            "trace_event": format!("native.helper.{session_id}.recovered"),
            "degraded": false
        });
        sqlx::query(
            "UPDATE native_helper_sessions
            SET helper_kind = ?, protocol_version = ?, binary_path = ?, launch_mode = ?,
                process_id = ?, endpoint = ?, status = 'ready', health_json = ?,
                capabilities_json = ?, last_heartbeat_at = ?, updated_at = ?
            WHERE id = ?",
        )
        .bind(&launch.helper_kind)
        .bind(&launch.protocol_version)
        .bind(&launch.binary_path)
        .bind(&launch.launch_mode)
        .bind(launch.process_id)
        .bind(&launch.endpoint)
        .bind(health.to_string())
        .bind(launch.capabilities_json.to_string())
        .bind(&now)
        .bind(&now)
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        self.add_event(
            session_id,
            "recovered",
            json!({
                "endpoint": launch.endpoint,
                "reason": reason,
                "trace_event": format!("native.helper.{session_id}.recovered")
            }),
        )
        .await?;
        self.add_log(
            session_id,
            "warn",
            "Native helper recovered",
            json!({
                "endpoint": launch.endpoint,
                "reason": reason,
                "trace_event": format!("native.helper.{session_id}.recovered")
            }),
        )
        .await?;
        self.session(session_id).await
    }

    pub async fn sessions(&self) -> Result<Vec<Value>, NativeStoreError> {
        self.list(
            "SELECT * FROM native_helper_sessions ORDER BY updated_at DESC",
            &[],
        )
        .await
    }

    pub async fn session(&self, session_id: &str) -> Result<Value, NativeStoreError> {
        self.row(
            "SELECT * FROM native_helper_sessions WHERE id = ?",
            &[session_id],
        )
        .await
    }

    pub async fn events(&self, session_id: &str) -> Result<Vec<Value>, NativeStoreError> {
        self.list(
            "SELECT * FROM native_helper_events WHERE session_id = ? ORDER BY created_at DESC",
            &[session_id],
        )
        .await
    }

    pub async fn logs(&self, session_id: &str) -> Result<Vec<Value>, NativeStoreError> {
        self.list(
            "SELECT * FROM native_helper_logs WHERE session_id = ? ORDER BY created_at DESC",
            &[session_id],
        )
        .await
    }

    async fn add_event(
        &self,
        session_id: &str,
        event_kind: &str,
        payload_json: Value,
    ) -> Result<(), NativeStoreError> {
        sqlx::query(
            "INSERT INTO native_helper_events (id, session_id, event_kind, payload_json, created_at)
            VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id())
        .bind(session_id)
        .bind(event_kind)
        .bind(payload_json.to_string())
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn add_log(
        &self,
        session_id: &str,
        severity: &str,
        message: &str,
        payload_json: Value,
    ) -> Result<(), NativeStoreError> {
        let trace_event_id = payload_json
            .get("trace_event")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                payload_json
                    .get("trace_event_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| format!("native.helper.{session_id}.log"));
        sqlx::query(
            "INSERT INTO native_helper_logs (id, session_id, severity, message, trace_event_id, payload_json, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id())
        .bind(session_id)
        .bind(severity)
        .bind(message)
        .bind(trace_event_id)
        .bind(payload_json.to_string())
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list(&self, sql: &str, binds: &[&str]) -> Result<Vec<Value>, NativeStoreError> {
        let mut query = sqlx::query(sql);
        for value in binds {
            query = query.bind(*value);
        }
        let rows = query.fetch_all(&self.pool).await?;
        rows.iter().map(object_row).collect()
    }

    async fn row(&self, sql: &str, binds: &[&str]) -> Result<Value, NativeStoreError> {
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
            .ok_or(NativeStoreError::NotFound)
    }
}

fn object_row(row: &sqlx::sqlite::SqliteRow) -> Result<Value, NativeStoreError> {
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

const JSON_COLUMNS: [&str; 3] = ["health_json", "capabilities_json", "payload_json"];

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS native_helper_sessions (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL, helper_kind TEXT NOT NULL,
  protocol_version TEXT NOT NULL, binary_path TEXT, launch_mode TEXT NOT NULL,
  process_id INTEGER NOT NULL, endpoint TEXT NOT NULL, status TEXT NOT NULL,
  health_json TEXT NOT NULL, capabilities_json TEXT NOT NULL, last_heartbeat_at TEXT NOT NULL,
  crash_count INTEGER NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS native_helper_events (
  id TEXT PRIMARY KEY, session_id TEXT NOT NULL, event_kind TEXT NOT NULL,
  payload_json TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS native_helper_logs (
  id TEXT PRIMARY KEY, session_id TEXT NOT NULL, severity TEXT NOT NULL,
  message TEXT NOT NULL, trace_event_id TEXT NOT NULL, payload_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
"#;
