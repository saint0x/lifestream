use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

pub const NATIVE_PROTOCOL_VERSION: &str = "vanta-native-helper.v1";

#[derive(Debug, Clone, Deserialize)]
pub struct NativeHelperStartInput {
    pub helper_kind: String,
    pub launch_mode: Option<String>,
    pub binary_path: Option<String>,
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NativeHelperCommandInput {
    pub command_kind: String,
    pub payload_json: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NativeHelperRecoverInput {
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeHelperLaunch {
    pub helper_kind: String,
    pub protocol_version: String,
    pub binary_path: Option<String>,
    pub launch_mode: String,
    pub process_id: i64,
    pub endpoint: String,
    pub capabilities_json: Value,
    pub health_json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeHelperHeartbeat {
    pub status: String,
    pub health_json: Value,
}

#[derive(Debug, Error)]
pub enum NativeProtocolError {
    #[error("invalid {field}: {message}")]
    Invalid {
        field: &'static str,
        message: &'static str,
    },
}

pub fn validate_start_input(input: &NativeHelperStartInput) -> Result<(), NativeProtocolError> {
    require_one_of(
        &input.helper_kind,
        "helper_kind",
        &["capture", "encode", "replay", "audio"],
    )?;
    if let Some(mode) = &input.launch_mode {
        require_one_of(mode, "launch_mode", &["managed", "stdio", "localhost"])?;
    }
    if let Some(path) = &input.binary_path {
        require_text(path, "binary_path")?;
    }
    if let Some(endpoint) = &input.endpoint {
        require_text(endpoint, "endpoint")?;
        if !endpoint.starts_with("http://127.0.0.1:") && !endpoint.starts_with("stdio://") {
            return Err(NativeProtocolError::Invalid {
                field: "endpoint",
                message: "must be a local stdio or localhost endpoint",
            });
        }
    }
    Ok(())
}

pub fn validate_command(input: &NativeHelperCommandInput) -> Result<(), NativeProtocolError> {
    require_one_of(
        &input.command_kind,
        "command_kind",
        &[
            "heartbeat",
            "shutdown",
            "report_crash",
            "report_degraded",
            "capabilities",
            "prepare_capture",
            "reconcile_capture",
            "prepare_encode",
        ],
    )
}

pub fn default_capabilities(helper_kind: &str) -> Value {
    match helper_kind {
        "capture" => json!({
            "camera": true,
            "microphone": true,
            "display": true,
            "window": true,
            "application_audio": false,
            "hotplug_events": true,
            "crash_recovery": true,
            "structured_logs": true
        }),
        "encode" => json!({
            "h264": true,
            "h265_detection": true,
            "av1_detection": true,
            "fragmented_mp4": true,
            "mkv": true
        }),
        "replay" => json!({
            "rolling_buffer": true,
            "durations_seconds": [15, 30, 60],
            "sponsor_proof_tags": true
        }),
        "audio" => json!({
            "program_bus": true,
            "monitor_bus": true,
            "mix_minus": true,
            "isolated_recording": true
        }),
        _ => json!({}),
    }
}

pub fn healthy_payload(helper_kind: &str) -> Value {
    json!({
        "state": "ready",
        "helper_kind": helper_kind,
        "protocol_version": NATIVE_PROTOCOL_VERSION,
        "heartbeat_interval_ms": 2500,
        "restart_policy": "recover_same_kind",
        "log_trace_events": true,
        "package": super::package::package_health(helper_kind),
        "degraded": false
    })
}

fn require_text(value: &str, field: &'static str) -> Result<(), NativeProtocolError> {
    if value.trim().is_empty() {
        return Err(NativeProtocolError::Invalid {
            field,
            message: "must not be empty",
        });
    }
    Ok(())
}

fn require_one_of(
    value: &str,
    field: &'static str,
    accepted: &'static [&'static str],
) -> Result<(), NativeProtocolError> {
    if !accepted.contains(&value) {
        return Err(NativeProtocolError::Invalid {
            field,
            message: "is not supported by Vanta native helper",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_input_validation_accepts_only_local_helper_transports() {
        assert!(
            validate_start_input(&NativeHelperStartInput {
                helper_kind: "capture".to_string(),
                launch_mode: Some("localhost".to_string()),
                binary_path: Some("/tmp/vanta-helper".to_string()),
                endpoint: Some("http://127.0.0.1:49000".to_string()),
            })
            .is_ok()
        );
        assert!(
            validate_start_input(&NativeHelperStartInput {
                helper_kind: "audio".to_string(),
                launch_mode: Some("stdio".to_string()),
                binary_path: Some("/tmp/vanta-audio-helper".to_string()),
                endpoint: Some("stdio://vanta-audio-helper".to_string()),
            })
            .is_ok()
        );

        assert_invalid_field(
            validate_start_input(&NativeHelperStartInput {
                helper_kind: "plugin".to_string(),
                launch_mode: Some("managed".to_string()),
                binary_path: None,
                endpoint: None,
            }),
            "helper_kind",
        );
        assert_invalid_field(
            validate_start_input(&NativeHelperStartInput {
                helper_kind: "capture".to_string(),
                launch_mode: Some("remote".to_string()),
                binary_path: None,
                endpoint: None,
            }),
            "launch_mode",
        );
        assert_invalid_field(
            validate_start_input(&NativeHelperStartInput {
                helper_kind: "capture".to_string(),
                launch_mode: Some("localhost".to_string()),
                binary_path: None,
                endpoint: Some("https://example.com/helper".to_string()),
            }),
            "endpoint",
        );
    }

    #[test]
    fn command_validation_keeps_helper_surface_value_filtered() {
        assert!(
            validate_command(&NativeHelperCommandInput {
                command_kind: "prepare_capture".to_string(),
                payload_json: None,
            })
            .is_ok()
        );
        assert!(
            validate_command(&NativeHelperCommandInput {
                command_kind: "reconcile_capture".to_string(),
                payload_json: None,
            })
            .is_ok()
        );
        assert_invalid_field(
            validate_command(&NativeHelperCommandInput {
                command_kind: "raw_obs_request".to_string(),
                payload_json: None,
            }),
            "command_kind",
        );
    }

    fn assert_invalid_field(result: Result<(), NativeProtocolError>, expected: &'static str) {
        match result {
            Err(NativeProtocolError::Invalid { field, .. }) => assert_eq!(field, expected),
            other => panic!("expected invalid field {expected}, got {other:?}"),
        }
    }
}
