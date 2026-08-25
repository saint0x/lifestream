use std::sync::Arc;

use serde_json::{Value, json};
use thiserror::Error;

use super::{
    package::{NativePackageState, package_states},
    protocol::{
        NativeHelperCommandInput, NativeHelperRecoverInput, NativeHelperStartInput,
        NativeProtocolError, validate_command, validate_start_input,
    },
    sandbox::validate_command_payload,
    store::{NativeStore, NativeStoreError},
    supervisor::NativeHelperSessionRef,
    supervisor::{LocalNativeHelperSupervisor, NativeHelperSupervisor, NativeSupervisorError},
};

#[derive(Clone)]
pub struct NativeService {
    store: Arc<NativeStore>,
    supervisor: Arc<dyn NativeHelperSupervisor>,
}

impl NativeService {
    pub fn new(store: NativeStore) -> Self {
        Self {
            store: Arc::new(store),
            supervisor: Arc::new(LocalNativeHelperSupervisor::default()),
        }
    }

    pub fn with_supervisor(
        store: NativeStore,
        supervisor: Arc<dyn NativeHelperSupervisor>,
    ) -> Self {
        Self {
            store: Arc::new(store),
            supervisor,
        }
    }

    pub async fn start_session(
        &self,
        input: NativeHelperStartInput,
    ) -> Result<Value, NativeServiceError> {
        validate_start_input(&input)?;
        let launch = self.supervisor.launch(&input).await?;
        Ok(self.store.create_session(launch).await?)
    }

    pub async fn command(
        &self,
        session_id: &str,
        input: NativeHelperCommandInput,
    ) -> Result<Value, NativeServiceError> {
        require_text(session_id, "session_id")?;
        validate_command(&input)?;
        let session = self.store.session(session_id).await?;
        let status = value_text(&session, "status");
        let helper_kind = value_text(&session, "helper_kind");
        let sandbox =
            validate_command_payload(input.payload_json.as_ref().unwrap_or(&Value::Null))?;
        if input.command_kind == "heartbeat" {
            if status == "crashed" || status == "degraded" {
                return self
                    .recover_session(
                        session_id,
                        NativeHelperRecoverInput {
                            reason: Some(format!("auto_restart_after_{status}")),
                        },
                    )
                    .await;
            }
        }
        let mut heartbeat = self
            .supervisor
            .command(
                &session_ref(&session),
                &with_helper_kind(input, &helper_kind),
            )
            .await?;
        attach_sandbox_report(&mut heartbeat.health_json, sandbox.as_json());
        Ok(self.store.apply_heartbeat(session_id, heartbeat).await?)
    }

    pub async fn sessions(&self) -> Result<Vec<Value>, NativeServiceError> {
        Ok(self.store.sessions().await?)
    }

    pub fn packages(&self) -> Vec<NativePackageState> {
        package_states()
    }

    pub async fn session(&self, session_id: &str) -> Result<Value, NativeServiceError> {
        require_text(session_id, "session_id")?;
        Ok(self.store.session(session_id).await?)
    }

    pub async fn events(&self, session_id: &str) -> Result<Vec<Value>, NativeServiceError> {
        require_text(session_id, "session_id")?;
        Ok(self.store.events(session_id).await?)
    }

    pub async fn logs(&self, session_id: &str) -> Result<Vec<Value>, NativeServiceError> {
        require_text(session_id, "session_id")?;
        Ok(self.store.logs(session_id).await?)
    }

    pub async fn recover_session(
        &self,
        session_id: &str,
        input: NativeHelperRecoverInput,
    ) -> Result<Value, NativeServiceError> {
        require_text(session_id, "session_id")?;
        let current = self.store.session(session_id).await?;
        let helper_kind = value_text(&current, "helper_kind");
        let launch_mode = value_text(&current, "launch_mode");
        let binary_path = optional_value_text(&current, "binary_path");
        let launch = self
            .supervisor
            .launch(&NativeHelperStartInput {
                helper_kind,
                launch_mode: Some(launch_mode),
                binary_path,
                endpoint: None,
            })
            .await?;
        Ok(self
            .store
            .recover_session(
                session_id,
                launch,
                input.reason.as_deref().unwrap_or("manual_recovery"),
            )
            .await?)
    }
}

fn session_ref(session: &Value) -> NativeHelperSessionRef {
    NativeHelperSessionRef {
        session_id: value_text(session, "id"),
        helper_kind: value_text(session, "helper_kind"),
        launch_mode: value_text(session, "launch_mode"),
        process_id: value_i64(session, "process_id"),
        binary_path: optional_value_text(session, "binary_path"),
        endpoint: value_text(session, "endpoint"),
    }
}

#[derive(Debug, Error)]
pub enum NativeServiceError {
    #[error(transparent)]
    Store(#[from] NativeStoreError),
    #[error(transparent)]
    Supervisor(#[from] NativeSupervisorError),
    #[error(transparent)]
    Protocol(#[from] NativeProtocolError),
}

fn require_text(value: &str, field: &'static str) -> Result<(), NativeServiceError> {
    if value.trim().is_empty() {
        return Err(NativeProtocolError::Invalid {
            field,
            message: "must not be empty",
        }
        .into());
    }
    Ok(())
}

fn value_text(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn optional_value_text(value: &Value, field: &str) -> Option<String> {
    let text = value_text(value, field);
    if text.is_empty() { None } else { Some(text) }
}

fn value_i64(value: &Value, field: &str) -> i64 {
    value.get(field).and_then(Value::as_i64).unwrap_or_default()
}

fn with_helper_kind(
    mut input: NativeHelperCommandInput,
    helper_kind: &str,
) -> NativeHelperCommandInput {
    let payload = input.payload_json.take().unwrap_or_else(|| json!({}));
    let mut object = payload.as_object().cloned().unwrap_or_default();
    object.insert("helper_kind".to_string(), json!(helper_kind));
    input.payload_json = Some(Value::Object(object));
    input
}

fn attach_sandbox_report(health: &mut Value, sandbox: Value) {
    if let Some(object) = health.as_object_mut() {
        object.insert("sandbox".to_string(), sandbox);
    }
}
