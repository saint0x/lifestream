use std::{collections::HashMap, path::Path, process::Stdio, time::Duration};

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::Mutex,
};
use url::Url;

use super::{
    package::package_health,
    protocol::{
        NATIVE_PROTOCOL_VERSION, NativeHelperCommandInput, NativeHelperHeartbeat,
        NativeHelperLaunch, NativeHelperStartInput, default_capabilities, healthy_payload,
    },
};

#[derive(Debug, Error)]
pub enum NativeSupervisorError {
    #[error("native helper launch failed: {0}")]
    Launch(String),
    #[error("native helper command failed: {0}")]
    Command(String),
}

#[async_trait]
pub trait NativeHelperSupervisor: Send + Sync {
    async fn launch(
        &self,
        input: &NativeHelperStartInput,
    ) -> Result<NativeHelperLaunch, NativeSupervisorError>;

    async fn command(
        &self,
        session: &NativeHelperSessionRef,
        input: &NativeHelperCommandInput,
    ) -> Result<NativeHelperHeartbeat, NativeSupervisorError>;
}

#[derive(Debug, Clone)]
pub struct NativeHelperSessionRef {
    pub session_id: String,
    pub helper_kind: String,
    pub launch_mode: String,
    pub process_id: i64,
    pub binary_path: Option<String>,
    pub endpoint: String,
}

#[derive(Debug, Default)]
pub struct LocalNativeHelperSupervisor {
    stdio_helpers: Mutex<HashMap<i64, StdioHelperProcess>>,
}

#[derive(Debug)]
struct StdioHelperProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

#[async_trait]
impl NativeHelperSupervisor for LocalNativeHelperSupervisor {
    async fn launch(
        &self,
        input: &NativeHelperStartInput,
    ) -> Result<NativeHelperLaunch, NativeSupervisorError> {
        if let Some(binary_path) = input.binary_path.as_deref() {
            return self.launch_external_helper(input, binary_path).await;
        }

        let launch_mode = input
            .launch_mode
            .clone()
            .unwrap_or_else(|| "managed".to_string());
        let endpoint = match launch_mode.as_str() {
            "stdio" => "stdio://vanta-native-helper".to_string(),
            "localhost" => input
                .endpoint
                .clone()
                .unwrap_or_else(|| "http://127.0.0.1:47371/command".to_string()),
            _ => "managed://vanta-native-helper".to_string(),
        };
        Ok(NativeHelperLaunch {
            helper_kind: input.helper_kind.clone(),
            protocol_version: NATIVE_PROTOCOL_VERSION.to_string(),
            binary_path: input.binary_path.clone(),
            launch_mode,
            process_id: i64::from(std::process::id()),
            endpoint,
            capabilities_json: default_capabilities(&input.helper_kind),
            health_json: healthy_payload(&input.helper_kind),
        })
    }

    async fn command(
        &self,
        session: &NativeHelperSessionRef,
        input: &NativeHelperCommandInput,
    ) -> Result<NativeHelperHeartbeat, NativeSupervisorError> {
        if session.binary_path.is_some() && session.launch_mode == "stdio" {
            return command_stdio_helper(session, input, &self.stdio_helpers).await;
        }
        if session.launch_mode == "localhost" {
            return command_localhost_helper(session, input).await;
        }
        let status = match input.command_kind.as_str() {
            "shutdown" => "stopped",
            "report_crash" => "crashed",
            "report_degraded" => "degraded",
            _ => "ready",
        };
        Ok(NativeHelperHeartbeat {
            status: status.to_string(),
            health_json: json!({
                "state": status,
                "session_id": session.session_id,
                "command": input.command_kind,
                "transport": "managed",
                "trace_event": format!("native.helper.{}.{}", session.session_id, input.command_kind),
                "package": package_health(&session.helper_kind),
                "detail": input.payload_json.clone().unwrap_or_else(|| json!({}))
            }),
        })
    }
}

#[derive(Debug, Deserialize)]
struct HelperHandshake {
    helper_kind: String,
    protocol_version: String,
    process_id: i64,
    endpoint: String,
    health_json: Value,
}

impl LocalNativeHelperSupervisor {
    async fn launch_external_helper(
        &self,
        input: &NativeHelperStartInput,
        binary_path: &str,
    ) -> Result<NativeHelperLaunch, NativeSupervisorError> {
        let launch_mode = input
            .launch_mode
            .clone()
            .unwrap_or_else(|| "stdio".to_string());
        if launch_mode == "stdio" {
            return self.launch_stdio_helper(input, binary_path).await;
        }
        launch_external_helper_handshake(input, binary_path).await
    }

    async fn launch_stdio_helper(
        &self,
        input: &NativeHelperStartInput,
        binary_path: &str,
    ) -> Result<NativeHelperLaunch, NativeSupervisorError> {
        let launch_mode = input
            .launch_mode
            .clone()
            .unwrap_or_else(|| "stdio".to_string());
        if !Path::new(binary_path).is_file() {
            return Err(NativeSupervisorError::Launch(format!(
                "helper binary does not exist at {binary_path}"
            )));
        }

        let mut child = Command::new(binary_path)
            .arg("--serve-stdio")
            .arg(&input.helper_kind)
            .env(
                "VANTA_NATIVE_HELPER_ENDPOINT",
                input
                    .endpoint
                    .clone()
                    .unwrap_or_else(|| "stdio://vanta-native-helper".to_string()),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                NativeSupervisorError::Launch(format!("could not start stdio helper: {error}"))
            })?;
        let process_id = child.id().map(i64::from).ok_or_else(|| {
            NativeSupervisorError::Launch("stdio helper process id was unavailable".to_string())
        })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            NativeSupervisorError::Launch("stdio helper stdin was unavailable".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            NativeSupervisorError::Launch("stdio helper stdout was unavailable".to_string())
        })?;
        let mut helper = StdioHelperProcess {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        };
        let mut handshake_line = String::new();
        tokio::time::timeout(
            Duration::from_secs(3),
            helper.stdout.read_line(&mut handshake_line),
        )
        .await
        .map_err(|_| NativeSupervisorError::Launch("stdio helper handshake timed out".to_string()))?
        .map_err(|error| {
            NativeSupervisorError::Launch(format!("stdio helper handshake read failed: {error}"))
        })?;
        let handshake: HelperHandshake =
            serde_json::from_str(handshake_line.trim()).map_err(|error| {
                NativeSupervisorError::Launch(format!(
                    "stdio helper handshake returned invalid json: {error}"
                ))
            })?;
        validate_handshake(&handshake, &input.helper_kind)?;
        self.stdio_helpers.lock().await.insert(process_id, helper);
        Ok(NativeHelperLaunch {
            helper_kind: input.helper_kind.clone(),
            protocol_version: handshake.protocol_version,
            binary_path: Some(binary_path.to_string()),
            launch_mode,
            process_id,
            endpoint: handshake.endpoint,
            capabilities_json: default_capabilities(&input.helper_kind),
            health_json: lifecycle_health(handshake.health_json, process_id, "stdio", "launched"),
        })
    }
}

async fn launch_external_helper_handshake(
    input: &NativeHelperStartInput,
    binary_path: &str,
) -> Result<NativeHelperLaunch, NativeSupervisorError> {
    if !Path::new(binary_path).is_file() {
        return Err(NativeSupervisorError::Launch(format!(
            "helper binary does not exist at {binary_path}"
        )));
    }

    let output = Command::new(binary_path)
        .arg("--handshake")
        .arg(&input.helper_kind)
        .env(
            "VANTA_NATIVE_HELPER_ENDPOINT",
            input.endpoint.clone().unwrap_or_default(),
        )
        .output()
        .await
        .map_err(|error| {
            NativeSupervisorError::Launch(format!("could not start helper handshake: {error}"))
        })?;

    if !output.status.success() {
        return Err(NativeSupervisorError::Launch(format!(
            "helper handshake exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let handshake: HelperHandshake = serde_json::from_slice(&output.stdout).map_err(|error| {
        NativeSupervisorError::Launch(format!("helper handshake returned invalid json: {error}"))
    })?;

    validate_handshake(&handshake, &input.helper_kind)?;

    Ok(NativeHelperLaunch {
        helper_kind: input.helper_kind.clone(),
        protocol_version: handshake.protocol_version,
        binary_path: Some(binary_path.to_string()),
        launch_mode: input
            .launch_mode
            .clone()
            .unwrap_or_else(|| "stdio".to_string()),
        process_id: handshake.process_id,
        endpoint: handshake.endpoint,
        capabilities_json: default_capabilities(&input.helper_kind),
        health_json: handshake.health_json,
    })
}

#[derive(Debug, Deserialize)]
struct HelperCommandResponse {
    status: String,
    health_json: Value,
}

async fn command_stdio_helper(
    session: &NativeHelperSessionRef,
    input: &NativeHelperCommandInput,
    helpers: &Mutex<HashMap<i64, StdioHelperProcess>>,
) -> Result<NativeHelperHeartbeat, NativeSupervisorError> {
    let mut helpers = helpers.lock().await;
    let Some(helper) = helpers.get_mut(&session.process_id) else {
        return Err(NativeSupervisorError::Command(format!(
            "stdio helper process {} is not registered for session {}",
            session.process_id, session.session_id
        )));
    };
    if let Some(status) = helper.child.try_wait().map_err(|error| {
        NativeSupervisorError::Command(format!("stdio helper liveness check failed: {error}"))
    })? {
        helpers.remove(&session.process_id);
        return Err(NativeSupervisorError::Command(format!(
            "stdio helper process {} exited before command {} with status {}",
            session.process_id, input.command_kind, status
        )));
    }
    let request = json!({
        "session_id": session.session_id,
        "helper_kind": session.helper_kind,
        "command_kind": input.command_kind,
        "payload_json": input.payload_json.clone().unwrap_or_else(|| json!({}))
    });
    helper
        .stdin
        .write_all(format!("{request}\n").as_bytes())
        .await
        .map_err(|error| {
            NativeSupervisorError::Command(format!("stdio helper command write failed: {error}"))
        })?;
    helper.stdin.flush().await.map_err(|error| {
        NativeSupervisorError::Command(format!("stdio helper command flush failed: {error}"))
    })?;
    let mut response_line = String::new();
    tokio::time::timeout(
        Duration::from_secs(3),
        helper.stdout.read_line(&mut response_line),
    )
    .await
    .map_err(|_| NativeSupervisorError::Command("stdio helper command timed out".to_string()))?
    .map_err(|error| {
        NativeSupervisorError::Command(format!("stdio helper command read failed: {error}"))
    })?;
    if response_line.trim().is_empty() {
        helpers.remove(&session.process_id);
        return Err(NativeSupervisorError::Command(
            "stdio helper closed stdout during command".to_string(),
        ));
    }
    let response: HelperCommandResponse =
        serde_json::from_str(response_line.trim()).map_err(|error| {
            NativeSupervisorError::Command(format!(
                "stdio helper command returned invalid json: {error}"
            ))
        })?;
    let heartbeat = with_transport_health(session, input, response, "stdio");
    if matches!(heartbeat.status.as_str(), "stopped" | "crashed") {
        if let Some(mut helper) = helpers.remove(&session.process_id) {
            let _ = helper.child.kill().await;
            let _ = helper.child.wait().await;
        }
    }
    Ok(heartbeat)
}

async fn command_localhost_helper(
    session: &NativeHelperSessionRef,
    input: &NativeHelperCommandInput,
) -> Result<NativeHelperHeartbeat, NativeSupervisorError> {
    let url = Url::parse(&session.endpoint).map_err(|error| {
        NativeSupervisorError::Command(format!("invalid helper endpoint: {error}"))
    })?;
    if url.scheme() != "http" || url.host_str() != Some("127.0.0.1") {
        return Err(NativeSupervisorError::Command(
            "localhost helper endpoint must be http://127.0.0.1:<port>".to_string(),
        ));
    }
    let port = url.port().ok_or_else(|| {
        NativeSupervisorError::Command("localhost helper endpoint is missing a port".to_string())
    })?;
    let path = if url.path().is_empty() || url.path() == "/" {
        "/command"
    } else {
        url.path()
    };
    let body = json!({
        "session_id": session.session_id,
        "helper_kind": session.helper_kind,
        "command_kind": input.command_kind,
        "payload_json": input.payload_json.clone().unwrap_or_else(|| json!({}))
    })
    .to_string();
    let mut stream = tokio::time::timeout(
        Duration::from_secs(3),
        TcpStream::connect(("127.0.0.1", port)),
    )
    .await
    .map_err(|_| {
        NativeSupervisorError::Command("localhost helper connection timed out".to_string())
    })?
    .map_err(|error| {
        NativeSupervisorError::Command(format!("localhost helper connection failed: {error}"))
    })?;
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|error| {
            NativeSupervisorError::Command(format!("localhost helper write failed: {error}"))
        })?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.map_err(|error| {
        NativeSupervisorError::Command(format!("localhost helper read failed: {error}"))
    })?;
    let response = String::from_utf8(response).map_err(|error| {
        NativeSupervisorError::Command(format!("localhost helper response was not utf8: {error}"))
    })?;
    let (_, body) = response.split_once("\r\n\r\n").ok_or_else(|| {
        NativeSupervisorError::Command("localhost helper response was malformed".to_string())
    })?;
    if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
        return Err(NativeSupervisorError::Command(format!(
            "localhost helper rejected command: {}",
            response.lines().next().unwrap_or("unknown status")
        )));
    }
    let command_response: HelperCommandResponse = serde_json::from_str(body).map_err(|error| {
        NativeSupervisorError::Command(format!(
            "localhost helper command returned invalid json: {error}"
        ))
    })?;
    Ok(with_transport_health(
        session,
        input,
        command_response,
        "localhost",
    ))
}

fn validate_handshake(
    handshake: &HelperHandshake,
    requested_kind: &str,
) -> Result<(), NativeSupervisorError> {
    if handshake.protocol_version != NATIVE_PROTOCOL_VERSION {
        return Err(NativeSupervisorError::Launch(format!(
            "helper protocol {} is not compatible with {}",
            handshake.protocol_version, NATIVE_PROTOCOL_VERSION
        )));
    }

    if handshake.helper_kind != requested_kind {
        return Err(NativeSupervisorError::Launch(format!(
            "helper kind {} did not match requested {}",
            handshake.helper_kind, requested_kind
        )));
    }
    Ok(())
}

fn lifecycle_health(mut health: Value, process_id: i64, transport: &str, state: &str) -> Value {
    if let Some(object) = health.as_object_mut() {
        object.insert("process_id".to_string(), json!(process_id));
        object.insert("transport".to_string(), json!(transport));
        object.insert("lifecycle".to_string(), json!("long_lived"));
        object.insert("state".to_string(), json!(state));
    }
    health
}

fn with_transport_health(
    session: &NativeHelperSessionRef,
    input: &NativeHelperCommandInput,
    response: HelperCommandResponse,
    transport: &str,
) -> NativeHelperHeartbeat {
    let mut health = response.health_json;
    if let Some(object) = health.as_object_mut() {
        object.insert("session_id".to_string(), json!(session.session_id));
        object.insert("helper_kind".to_string(), json!(session.helper_kind));
        object.insert("command".to_string(), json!(input.command_kind));
        object.insert("transport".to_string(), json!(transport));
        object.insert("process_id".to_string(), json!(session.process_id));
        if transport == "stdio" {
            object
                .entry("lifecycle".to_string())
                .or_insert_with(|| json!("long_lived"));
        }
        object.insert(
            "trace_event".to_string(),
            json!(format!(
                "native.helper.{}.{}.{transport}",
                session.session_id, input.command_kind
            )),
        );
        object.insert("package".to_string(), package_health(&session.helper_kind));
    }
    NativeHelperHeartbeat {
        status: response.status,
        health_json: health,
    }
}
