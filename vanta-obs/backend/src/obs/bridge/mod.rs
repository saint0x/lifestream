use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub mod websocket;

#[derive(Debug, Clone, Deserialize)]
pub struct ObsBridgeProfileInput {
    pub label: String,
    pub websocket_url: String,
    pub password: Option<String>,
    pub auto_sync: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ObsBridgeProfile {
    pub id: String,
    pub label: String,
    pub websocket_url: String,
    pub password: Option<String>,
    pub auto_sync: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsBridgeSnapshot {
    pub obs_version: String,
    pub websocket_version: String,
    pub current_program_scene: Option<String>,
    pub current_preview_scene: Option<String>,
    pub stream_state: String,
    pub recording_state: String,
    pub replay_buffer_state: String,
    pub scenes: Vec<ObsBridgeScene>,
    pub sources: Vec<ObsBridgeSource>,
    pub transitions: Vec<ObsBridgeTransition>,
    pub audio_inputs: Vec<ObsBridgeAudioInput>,
    pub unsupported: Vec<ObsBridgeWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsBridgeScene {
    pub name: String,
    pub index: i64,
    pub items: Vec<ObsBridgeSceneItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsBridgeSceneItem {
    pub id: i64,
    pub source_name: String,
    pub source_kind: String,
    pub enabled: bool,
    pub locked: bool,
    pub index: i64,
    pub transform: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsBridgeSource {
    pub name: String,
    pub kind: String,
    pub vanta_kind: Option<String>,
    pub configurable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsBridgeTransition {
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsBridgeAudioInput {
    pub name: String,
    pub kind: String,
    pub muted: bool,
    pub volume_db: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsBridgeWarning {
    pub code: String,
    pub subject: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsBridgeEvent {
    pub event_kind: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsBridgeCommandResult {
    pub command: String,
    pub accepted: bool,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub enum ObsBridgeCommand {
    SetProgramScene { scene_name: String },
    StartStream,
    StopStream,
    StartRecording,
    StopRecording,
    SaveReplayBuffer,
}

#[derive(Debug, Error)]
pub enum ObsBridgeError {
    #[error("bridge connection failed: {0}")]
    Connection(String),
    #[error("bridge protocol failed: {0}")]
    Protocol(String),
    #[error("bridge command failed: {0}")]
    Command(String),
}

#[async_trait]
pub trait ObsBridgeClient: Send + Sync {
    async fn snapshot(
        &self,
        profile: &ObsBridgeProfile,
    ) -> Result<ObsBridgeSnapshot, ObsBridgeError>;

    async fn execute(
        &self,
        profile: &ObsBridgeProfile,
        command: ObsBridgeCommand,
    ) -> Result<ObsBridgeCommandResult, ObsBridgeError>;
}

pub fn bridge_warning(code: &str, subject: &str, detail: &str) -> ObsBridgeWarning {
    ObsBridgeWarning {
        code: code.to_string(),
        subject: subject.to_string(),
        detail: detail.to_string(),
    }
}
