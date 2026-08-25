use serde_json::{Value, json};

use crate::obs::{
    domain::{SourceFilterInput, SourceFilterPatch, bool_int},
    source::{source_filter_summary, validate_source_filter},
};

use super::{
    ObsStore, ObsStoreError,
    row::{id, int, now, text},
};

impl ObsStore {
    pub(super) async fn source_filters(
        &self,
        source_id: &str,
    ) -> Result<Vec<Value>, ObsStoreError> {
        let filters = self
            .list(
                "SELECT * FROM obs_source_filters WHERE source_id = ? ORDER BY order_index ASC, created_at ASC",
                &[source_id],
            )
            .await?;
        Ok(filters.into_iter().map(enrich_filter_row).collect())
    }

    pub async fn create_source_filter(
        &self,
        source_id: &str,
        input: SourceFilterInput,
    ) -> Result<Value, ObsStoreError> {
        self.row("SELECT * FROM obs_sources WHERE id = ?", &[source_id])
            .await?;
        let settings = input.settings_json.unwrap_or_else(|| json!({}));
        let validation = validate_source_filter(&input.filter_kind, &settings);
        let Some(mapping) = source_filter_summary(&input.filter_kind) else {
            return Err(ObsStoreError::Invalid(format!(
                "filter_kind {} is not supported by Vanta OBS",
                input.filter_kind
            )));
        };
        let now = now();
        let filter_id = id();
        let order_index = input.order_index.unwrap_or_else(|| 1);
        sqlx::query(
            "INSERT INTO obs_source_filters
            (id, creator_id, source_id, filter_kind, label, enabled, order_index, settings_json, obs_mapping_json, validation_json, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, ?, 1, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&filter_id)
        .bind(source_id)
        .bind(input.filter_kind)
        .bind(input.label)
        .bind(order_index)
        .bind(settings.to_string())
        .bind(mapping.to_string())
        .bind(validation.to_json().to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.add_event(
            Some(source_id),
            "source_filter_create",
            "Source filter created",
        )
        .await?;
        self.source_filter(&filter_id).await
    }

    pub async fn patch_source_filter(
        &self,
        filter_id: &str,
        input: SourceFilterPatch,
    ) -> Result<Value, ObsStoreError> {
        let current = self
            .row(
                "SELECT * FROM obs_source_filters WHERE id = ?",
                &[filter_id],
            )
            .await?;
        let settings = input.settings_json.unwrap_or_else(|| {
            current
                .get("settings_json")
                .cloned()
                .unwrap_or_else(|| json!({}))
        });
        let filter_kind = text(&current, "filter_kind");
        let validation = validate_source_filter(&filter_kind, &settings);
        sqlx::query(
            "UPDATE obs_source_filters SET label = ?, enabled = ?, order_index = ?, settings_json = ?, validation_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(input.label.unwrap_or_else(|| text(&current, "label")))
        .bind(bool_int(input.enabled.unwrap_or_else(|| int(&current, "enabled") != 0)))
        .bind(input.order_index.unwrap_or_else(|| int(&current, "order_index")))
        .bind(settings.to_string())
        .bind(validation.to_json().to_string())
        .bind(now())
        .bind(filter_id)
        .execute(&self.pool)
        .await?;
        self.source_filter(filter_id).await
    }

    pub async fn disable_source_filter(&self, filter_id: &str) -> Result<Value, ObsStoreError> {
        sqlx::query("UPDATE obs_source_filters SET enabled = 0, updated_at = ? WHERE id = ?")
            .bind(now())
            .bind(filter_id)
            .execute(&self.pool)
            .await?;
        self.source_filter(filter_id).await
    }

    async fn source_filter(&self, filter_id: &str) -> Result<Value, ObsStoreError> {
        let filter = self
            .row(
                "SELECT * FROM obs_source_filters WHERE id = ?",
                &[filter_id],
            )
            .await?;
        Ok(enrich_filter_row(filter))
    }
}

fn enrich_filter_row(mut filter: Value) -> Value {
    let kind = text(&filter, "filter_kind");
    let settings = filter
        .get("settings_json")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let validation = validate_source_filter(&kind, &settings);
    if let Some(object) = filter.as_object_mut() {
        object.insert(
            "filter_contract_json".to_string(),
            source_filter_summary(&kind).unwrap_or_else(|| json!({ "kind": kind })),
        );
        object.insert("validation_json".to_string(), validation.to_json());
    }
    filter
}
