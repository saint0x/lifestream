use serde_json::{Value, json};

use crate::obs::bridge::{ObsBridgeCommand, ObsBridgeEvent, ObsBridgeProfileInput};

use super::{ObsService, ObsServiceResult, require_text};

impl ObsService {
    pub async fn create_bridge_connection(
        &self,
        input: ObsBridgeProfileInput,
    ) -> ObsServiceResult<Value> {
        validate_bridge_profile(&input)?;
        Ok(self.store.create_bridge_connection(input).await?)
    }

    pub async fn bridge_connections(&self) -> ObsServiceResult<Vec<Value>> {
        Ok(self.store.bridge_connections().await?)
    }

    pub async fn bridge_connection(&self, connection_id: &str) -> ObsServiceResult<Value> {
        require_text(connection_id, "connection_id")?;
        Ok(self.store.bridge_connection(connection_id).await?)
    }

    pub async fn sync_bridge_connection(&self, connection_id: &str) -> ObsServiceResult<Value> {
        require_text(connection_id, "connection_id")?;
        let profile = self.store.bridge_profile(connection_id).await?;
        self.store.mark_bridge_connecting(connection_id).await?;
        match self.bridge.snapshot(&profile).await {
            Ok(snapshot) => Ok(self
                .store
                .save_bridge_snapshot(connection_id, &snapshot)
                .await?),
            Err(error) => {
                let row = self
                    .store
                    .save_bridge_error(connection_id, &error.to_string())
                    .await?;
                let _ = error;
                Ok(row)
            }
        }
    }

    pub async fn bridge_events(&self, connection_id: &str) -> ObsServiceResult<Vec<Value>> {
        require_text(connection_id, "connection_id")?;
        Ok(self.store.bridge_events(connection_id).await?)
    }

    pub async fn bridge_set_program_scene(
        &self,
        connection_id: &str,
        scene_name: String,
    ) -> ObsServiceResult<Value> {
        require_text(connection_id, "connection_id")?;
        require_text(&scene_name, "scene_name")?;
        self.execute_bridge_command(
            connection_id,
            ObsBridgeCommand::SetProgramScene { scene_name },
        )
        .await
    }

    pub async fn bridge_start_stream(&self, connection_id: &str) -> ObsServiceResult<Value> {
        self.execute_bridge_command(connection_id, ObsBridgeCommand::StartStream)
            .await
    }

    pub async fn bridge_stop_stream(&self, connection_id: &str) -> ObsServiceResult<Value> {
        self.execute_bridge_command(connection_id, ObsBridgeCommand::StopStream)
            .await
    }

    pub async fn bridge_start_recording(&self, connection_id: &str) -> ObsServiceResult<Value> {
        self.execute_bridge_command(connection_id, ObsBridgeCommand::StartRecording)
            .await
    }

    pub async fn bridge_stop_recording(&self, connection_id: &str) -> ObsServiceResult<Value> {
        self.execute_bridge_command(connection_id, ObsBridgeCommand::StopRecording)
            .await
    }

    pub async fn bridge_save_replay_buffer(&self, connection_id: &str) -> ObsServiceResult<Value> {
        self.execute_bridge_command(connection_id, ObsBridgeCommand::SaveReplayBuffer)
            .await
    }

    async fn execute_bridge_command(
        &self,
        connection_id: &str,
        command: ObsBridgeCommand,
    ) -> ObsServiceResult<Value> {
        require_text(connection_id, "connection_id")?;
        let profile = self.store.bridge_profile(connection_id).await?;
        let result = self.bridge.execute(&profile, command).await?;
        self.store
            .record_bridge_event(
                connection_id,
                ObsBridgeEvent {
                    event_kind: "command_executed".to_string(),
                    payload: serde_json::to_value(&result)?,
                },
            )
            .await?;
        Ok(json!(result))
    }
}

fn validate_bridge_profile(input: &ObsBridgeProfileInput) -> ObsServiceResult<()> {
    require_text(&input.label, "label")?;
    require_text(&input.websocket_url, "websocket_url")?;
    if !(input.websocket_url.starts_with("ws://") || input.websocket_url.starts_with("wss://")) {
        return Err(super::ObsServiceError::Invalid {
            field: "websocket_url",
            message: "must start with ws:// or wss://",
        });
    }
    Ok(())
}
