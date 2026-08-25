use serde_json::{Value, json};

use crate::obs::bridge::{
    ObsBridgeEvent, ObsBridgeProfile, ObsBridgeProfileInput, ObsBridgeSnapshot,
};
use crate::obs::domain::bool_int;

use super::{
    ObsStore, ObsStoreError,
    row::{id, now, text},
};

impl ObsStore {
    pub async fn create_bridge_connection(
        &self,
        input: ObsBridgeProfileInput,
    ) -> Result<Value, ObsStoreError> {
        let connection_id = id();
        let now = now();
        sqlx::query(
            "INSERT INTO obs_bridge_connections
            (id, creator_id, label, websocket_url, password_json, auto_sync, sync_status, last_error, last_snapshot_json, created_at, updated_at, last_synced_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, ?, ?, 'created', NULL, ?, ?, ?, NULL)",
        )
        .bind(&connection_id)
        .bind(input.label)
        .bind(input.websocket_url)
        .bind(json!({"password": input.password}).to_string())
        .bind(bool_int(input.auto_sync.unwrap_or(false)))
        .bind(json!({}).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.bridge_connection(&connection_id).await
    }

    pub async fn bridge_connections(&self) -> Result<Vec<Value>, ObsStoreError> {
        self.list(
            "SELECT * FROM obs_bridge_connections ORDER BY updated_at DESC",
            &[],
        )
        .await
    }

    pub async fn bridge_connection(&self, connection_id: &str) -> Result<Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_bridge_connections WHERE id = ?",
            &[connection_id],
        )
        .await
    }

    pub async fn bridge_profile(
        &self,
        connection_id: &str,
    ) -> Result<ObsBridgeProfile, ObsStoreError> {
        let row = self.bridge_connection(connection_id).await?;
        Ok(ObsBridgeProfile {
            id: text(&row, "id"),
            label: text(&row, "label"),
            websocket_url: text(&row, "websocket_url"),
            password: row["password_json"]["password"]
                .as_str()
                .map(str::to_string),
            auto_sync: row["auto_sync"].as_i64().unwrap_or_default() != 0,
        })
    }

    pub async fn mark_bridge_connecting(&self, connection_id: &str) -> Result<(), ObsStoreError> {
        sqlx::query(
            "UPDATE obs_bridge_connections SET sync_status = 'connecting', last_error = NULL, updated_at = ? WHERE id = ?",
        )
        .bind(now())
        .bind(connection_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn save_bridge_snapshot(
        &self,
        connection_id: &str,
        snapshot: &ObsBridgeSnapshot,
    ) -> Result<Value, ObsStoreError> {
        let now = now();
        sqlx::query(
            "UPDATE obs_bridge_connections SET sync_status = 'synced', last_error = NULL, last_snapshot_json = ?, updated_at = ?, last_synced_at = ? WHERE id = ?",
        )
        .bind(serde_json::to_string(snapshot)?)
        .bind(&now)
        .bind(&now)
        .bind(connection_id)
        .execute(&self.pool)
        .await?;
        self.record_bridge_event(
            connection_id,
            ObsBridgeEvent {
                event_kind: "snapshot_synced".to_string(),
                payload: serde_json::to_value(snapshot)?,
            },
        )
        .await?;
        self.bridge_connection(connection_id).await
    }

    pub async fn save_bridge_error(
        &self,
        connection_id: &str,
        error: &str,
    ) -> Result<Value, ObsStoreError> {
        sqlx::query(
            "UPDATE obs_bridge_connections SET sync_status = 'error', last_error = ?, updated_at = ? WHERE id = ?",
        )
        .bind(error)
        .bind(now())
        .bind(connection_id)
        .execute(&self.pool)
        .await?;
        self.bridge_connection(connection_id).await
    }

    pub async fn record_bridge_event(
        &self,
        connection_id: &str,
        event: ObsBridgeEvent,
    ) -> Result<Value, ObsStoreError> {
        sqlx::query(
            "INSERT INTO obs_bridge_events (id, connection_id, event_kind, payload_json, created_at)
            VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id())
        .bind(connection_id)
        .bind(event.event_kind)
        .bind(event.payload.to_string())
        .bind(now())
        .execute(&self.pool)
        .await?;
        self.row(
            "SELECT * FROM obs_bridge_events WHERE connection_id = ? ORDER BY created_at DESC LIMIT 1",
            &[connection_id],
        )
        .await
    }

    pub async fn bridge_events(&self, connection_id: &str) -> Result<Vec<Value>, ObsStoreError> {
        self.list(
            "SELECT * FROM obs_bridge_events WHERE connection_id = ? ORDER BY created_at DESC LIMIT 100",
            &[connection_id],
        )
        .await
    }
}
