use std::time::Duration;

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};

use super::{
    ObsBridgeAudioInput, ObsBridgeClient, ObsBridgeCommand, ObsBridgeCommandResult, ObsBridgeError,
    ObsBridgeProfile, ObsBridgeScene, ObsBridgeSceneItem, ObsBridgeSnapshot, ObsBridgeSource,
    ObsBridgeTransition, ObsBridgeWarning, bridge_warning,
};
use crate::obs::adapter::{is_audio_obs_kind, obs_kind_to_vanta_kind};

#[derive(Clone)]
pub struct LocalObsWebSocketClient {
    timeout: Duration,
}

impl Default for LocalObsWebSocketClient {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(8),
        }
    }
}

#[async_trait]
impl ObsBridgeClient for LocalObsWebSocketClient {
    async fn snapshot(
        &self,
        profile: &ObsBridgeProfile,
    ) -> Result<ObsBridgeSnapshot, ObsBridgeError> {
        let mut session = ObsSession::connect(profile, self.timeout).await?;
        let version = session.call("GetVersion", json!({})).await?;
        let scenes = session.call("GetSceneList", json!({})).await?;
        let inputs = session.call("GetInputList", json!({})).await?;
        let transitions = session.call("GetSceneTransitionList", json!({})).await?;
        let stream = session.call("GetStreamStatus", json!({})).await?;
        let record = session.call("GetRecordStatus", json!({})).await?;
        let replay = session
            .call("GetReplayBufferStatus", json!({}))
            .await
            .unwrap_or_else(|_| json!({"outputActive": false}));

        let mut bridge_scenes = Vec::new();
        let mut warnings = Vec::new();
        for (index, scene) in scenes["scenes"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let name = text(scene, "sceneName");
            let items = session
                .call("GetSceneItemList", json!({"sceneName": name}))
                .await
                .map(|value| scene_items(&value, &mut warnings))
                .unwrap_or_default();
            bridge_scenes.push(ObsBridgeScene {
                name,
                index: index as i64,
                items,
            });
        }

        let sources = inputs["inputs"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|input| {
                let kind = text(input, "inputKind");
                let name = text(input, "inputName");
                let vanta_kind = obs_kind_to_vanta_kind(&kind).map(str::to_string);
                if vanta_kind.is_none() {
                    warnings.push(bridge_warning(
                        "unsupported_source_kind",
                        &name,
                        &format!("{kind} is not mapped into Vanta OBS"),
                    ));
                }
                ObsBridgeSource {
                    name,
                    kind,
                    vanta_kind,
                    configurable: true,
                }
            })
            .collect::<Vec<_>>();

        let audio_inputs = inputs["inputs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|input| {
                let kind = text(input, "inputKind");
                if !is_audio_obs_kind(&kind) {
                    return None;
                }
                Some(ObsBridgeAudioInput {
                    name: text(input, "inputName"),
                    kind,
                    muted: false,
                    volume_db: 0.0,
                })
            })
            .collect::<Vec<_>>();

        let transition_rows = transitions["transitions"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|transition| ObsBridgeTransition {
                name: text(transition, "transitionName"),
                kind: text(transition, "transitionKind"),
            })
            .collect::<Vec<_>>();

        Ok(ObsBridgeSnapshot {
            obs_version: text(&version, "obsVersion"),
            websocket_version: text(&version, "obsWebSocketVersion"),
            current_program_scene: scenes["currentProgramSceneName"]
                .as_str()
                .map(str::to_string),
            current_preview_scene: scenes["currentPreviewSceneName"]
                .as_str()
                .map(str::to_string),
            stream_state: output_state(&stream),
            recording_state: if record["outputActive"].as_bool().unwrap_or(false) {
                "recording".to_string()
            } else {
                "idle".to_string()
            },
            replay_buffer_state: if replay["outputActive"].as_bool().unwrap_or(false) {
                "active".to_string()
            } else {
                "stopped".to_string()
            },
            scenes: bridge_scenes,
            sources,
            transitions: transition_rows,
            audio_inputs,
            unsupported: warnings,
        })
    }

    async fn execute(
        &self,
        profile: &ObsBridgeProfile,
        command: ObsBridgeCommand,
    ) -> Result<ObsBridgeCommandResult, ObsBridgeError> {
        let mut session = ObsSession::connect(profile, self.timeout).await?;
        let (request_type, request_data, label) = match command {
            ObsBridgeCommand::SetProgramScene { scene_name } => (
                "SetCurrentProgramScene",
                json!({ "sceneName": scene_name }),
                "set_program_scene",
            ),
            ObsBridgeCommand::StartStream => ("StartStream", json!({}), "start_stream"),
            ObsBridgeCommand::StopStream => ("StopStream", json!({}), "stop_stream"),
            ObsBridgeCommand::StartRecording => ("StartRecord", json!({}), "start_recording"),
            ObsBridgeCommand::StopRecording => ("StopRecord", json!({}), "stop_recording"),
            ObsBridgeCommand::SaveReplayBuffer => {
                ("SaveReplayBuffer", json!({}), "save_replay_buffer")
            }
        };
        session.call(request_type, request_data).await?;
        Ok(ObsBridgeCommandResult {
            command: label.to_string(),
            accepted: true,
            detail: format!("{request_type} accepted by OBS"),
        })
    }
}

struct ObsSession {
    socket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    next_request_id: u64,
}

impl ObsSession {
    async fn connect(
        profile: &ObsBridgeProfile,
        timeout: Duration,
    ) -> Result<Self, ObsBridgeError> {
        let request = profile
            .websocket_url
            .clone()
            .into_client_request()
            .map_err(|error| ObsBridgeError::Connection(error.to_string()))?;
        let (socket, _) = tokio::time::timeout(timeout, connect_async(request))
            .await
            .map_err(|_| ObsBridgeError::Connection("timed out".to_string()))?
            .map_err(|error| ObsBridgeError::Connection(error.to_string()))?;
        let mut session = Self {
            socket,
            next_request_id: 1,
        };

        let hello = session.read_frame().await?;
        let auth = hello["d"]["authentication"].as_object().map(|_| {
            build_auth(
                profile.password.as_deref().unwrap_or_default(),
                text(&hello["d"]["authentication"], "salt"),
                text(&hello["d"]["authentication"], "challenge"),
            )
        });
        let mut identify = json!({
            "op": 1,
            "d": {
                "rpcVersion": 1,
                "eventSubscriptions": 2047
            }
        });
        if let Some(auth) = auth {
            identify["d"]["authentication"] = json!(auth);
        }
        session.write_frame(identify).await?;
        let identified = session.read_frame().await?;
        if identified["op"].as_i64() != Some(2) {
            return Err(ObsBridgeError::Protocol(
                "OBS did not identify the websocket session".to_string(),
            ));
        }
        Ok(session)
    }

    async fn call(
        &mut self,
        request_type: &str,
        request_data: Value,
    ) -> Result<Value, ObsBridgeError> {
        let request_id = self.next_request_id.to_string();
        self.next_request_id += 1;
        self.write_frame(json!({
            "op": 6,
            "d": {
                "requestType": request_type,
                "requestId": request_id,
                "requestData": request_data
            }
        }))
        .await?;
        loop {
            let frame = self.read_frame().await?;
            if frame["op"].as_i64() != Some(7) {
                continue;
            }
            let data = &frame["d"];
            if data["requestId"].as_str() != Some(request_id.as_str()) {
                continue;
            }
            if !data["requestStatus"]["result"].as_bool().unwrap_or(false) {
                return Err(ObsBridgeError::Command(format!(
                    "{} failed: {}",
                    request_type,
                    data["requestStatus"]["comment"]
                        .as_str()
                        .unwrap_or("unknown")
                )));
            }
            return Ok(data["responseData"].clone());
        }
    }

    async fn read_frame(&mut self) -> Result<Value, ObsBridgeError> {
        while let Some(message) = self.socket.next().await {
            let message = message.map_err(|error| ObsBridgeError::Protocol(error.to_string()))?;
            match message {
                Message::Text(text) => {
                    return serde_json::from_str(&text)
                        .map_err(|error| ObsBridgeError::Protocol(error.to_string()));
                }
                Message::Close(frame) => {
                    return Err(ObsBridgeError::Connection(format!(
                        "OBS closed websocket: {frame:?}"
                    )));
                }
                _ => {}
            }
        }
        Err(ObsBridgeError::Connection(
            "OBS websocket ended".to_string(),
        ))
    }

    async fn write_frame(&mut self, value: Value) -> Result<(), ObsBridgeError> {
        self.socket
            .send(Message::Text(value.to_string().into()))
            .await
            .map_err(|error| ObsBridgeError::Protocol(error.to_string()))
    }
}

fn build_auth(password: &str, salt: String, challenge: String) -> String {
    let secret = STANDARD.encode(Sha256::digest(format!("{password}{salt}").as_bytes()));
    STANDARD.encode(Sha256::digest(format!("{secret}{challenge}").as_bytes()))
}

fn scene_items(value: &Value, warnings: &mut Vec<ObsBridgeWarning>) -> Vec<ObsBridgeSceneItem> {
    value["sceneItems"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| {
            let source_kind = text(item, "sourceType");
            let source_name = text(item, "sourceName");
            if obs_kind_to_vanta_kind(&source_kind).is_none() {
                warnings.push(bridge_warning(
                    "unsupported_scene_item_kind",
                    &source_name,
                    &format!("{source_kind} is not mapped into Vanta OBS"),
                ));
            }
            ObsBridgeSceneItem {
                id: item["sceneItemId"].as_i64().unwrap_or_default(),
                source_name,
                source_kind,
                enabled: item["sceneItemEnabled"].as_bool().unwrap_or(true),
                locked: item["sceneItemLocked"].as_bool().unwrap_or(false),
                index: item["sceneItemIndex"].as_i64().unwrap_or_default(),
                transform: item["sceneItemTransform"].clone(),
            }
        })
        .collect()
}

fn output_state(value: &Value) -> String {
    if value["outputActive"].as_bool().unwrap_or(false) {
        "live".to_string()
    } else if value["outputReconnecting"].as_bool().unwrap_or(false) {
        "reconnecting".to_string()
    } else {
        "idle".to_string()
    }
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
