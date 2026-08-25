use serde_json::Value;

use crate::obs::{
    domain::{SourceInput, SourcePatch},
    source::{source_kinds, validate_source},
};

use super::{
    HEALTH_STATES, ObsService, ObsServiceError, ObsServiceResult, PERMISSION_STATES,
    require_one_of, require_text,
};

impl ObsService {
    pub async fn create_source(&self, input: SourceInput) -> ObsServiceResult<Value> {
        if !source_kinds().contains(&input.source_kind.as_str()) {
            return Err(ObsServiceError::Invalid {
                field: "source_kind",
                message: "is not supported by Vanta OBS",
            });
        }
        require_text(&input.display_name, "display_name")?;
        require_valid_source(
            &input.source_kind,
            input.device_id.as_deref(),
            input.browser_url.as_deref(),
            input.media_asset_id.as_deref(),
            input.settings_json.as_ref().unwrap_or(&Value::Null),
        )?;
        Ok(self.store.create_source(input).await?)
    }

    pub async fn patch_source(
        &self,
        source_id: &str,
        input: SourcePatch,
    ) -> ObsServiceResult<Value> {
        require_text(source_id, "source_id")?;
        if let Some(display_name) = input.display_name.as_deref() {
            require_text(display_name, "display_name")?;
        }
        if let Some(permission_state) = input.permission_state.as_deref() {
            require_one_of(permission_state, "permission_state", PERMISSION_STATES)?;
        }
        if let Some(health_state) = input.health_state.as_deref() {
            require_one_of(health_state, "health_state", HEALTH_STATES)?;
        }
        let current = self.store.source(source_id).await?;
        require_valid_source(
            &value_text(&current, "source_kind"),
            optional_text(&current, "device_id").as_deref(),
            optional_text(&current, "browser_url").as_deref(),
            optional_text(&current, "media_asset_id").as_deref(),
            input
                .settings_json
                .as_ref()
                .unwrap_or(&current["default_settings_json"]),
        )?;
        Ok(self.store.patch_source(source_id, input).await?)
    }
}

fn require_valid_source(
    kind: &str,
    device_id: Option<&str>,
    browser_url: Option<&str>,
    media_asset_id: Option<&str>,
    settings: &Value,
) -> ObsServiceResult<()> {
    let validation = validate_source(kind, device_id, browser_url, media_asset_id, settings);
    if !validation.errors.is_empty() {
        return Err(ObsServiceError::Invalid {
            field: "settings_json",
            message: "does not satisfy the Vanta source schema",
        });
    }
    Ok(())
}

fn value_text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn optional_text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}
