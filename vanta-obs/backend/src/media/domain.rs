use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct CaptureStartInput {
    pub source_id: String,
    pub capture_kind: String,
    pub width: i64,
    pub height: i64,
    pub frame_rate: i64,
    pub audio: Option<bool>,
    pub duration_seconds: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EncodeStartInput {
    pub broadcast_id: String,
    pub capture_session_id: String,
    pub codec: String,
    pub audio_codec: String,
    pub container: String,
    pub bitrate_kbps: i64,
    pub keyframe_interval_seconds: i64,
    pub latency_profile: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SourceAudioIngestInput {
    pub source_id: String,
    pub input_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeProgramFrameInput {
    pub image_data_url: String,
    pub compositor_backend: String,
    pub frame_sequence: i64,
    pub captured_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeSourceFrameInput {
    pub image_data_url: String,
    pub compositor_backend: String,
    pub frame_sequence: i64,
    pub captured_at_ms: Option<i64>,
    pub surface_kind: String,
    pub dropped_frames: Option<i64>,
    pub reconnect_count: Option<i64>,
    pub ingest_latency_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeSourcePlayoutInput {
    pub target_frame_rate: Option<i64>,
    pub frame_count: Option<i64>,
}
