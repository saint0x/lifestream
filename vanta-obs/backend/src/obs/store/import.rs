use std::collections::HashMap;

use serde_json::{Value, json};

use crate::obs::import::ObsImportPlan;

use super::{
    ObsStore, ObsStoreError,
    row::{id, now},
};

impl ObsStore {
    pub async fn apply_obs_import(
        &self,
        plan: ObsImportPlan,
    ) -> Result<serde_json::Value, ObsStoreError> {
        let now = now();
        let collection_id = id();
        let active_scene_id = id();
        let mut scene_ids = HashMap::new();

        sqlx::query(
            "INSERT INTO obs_scene_collections
            (id, creator_id, name, description, canvas_width, canvas_height, frame_rate, default_transition, active_scene_id, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, 'Imported from OBS scene collection.', ?, ?, ?, 'fade', ?, ?, ?)",
        )
        .bind(&collection_id)
        .bind(&plan.collection_name)
        .bind(plan.canvas_width)
        .bind(plan.canvas_height)
        .bind(plan.frame_rate)
        .bind(&active_scene_id)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        for (index, scene) in plan.scenes.iter().enumerate() {
            let scene_id = if index == 0 {
                active_scene_id.clone()
            } else {
                id()
            };
            scene_ids.insert(scene.obs_name.clone(), scene_id.clone());
            sqlx::query(
                "INSERT INTO obs_scenes
                (id, collection_id, creator_id, name, order_index, transition_kind, transition_duration_ms, hotkey, locked, validation_state, created_at, updated_at)
                VALUES (?, ?, 'creator_vanta_originals', ?, ?, ?, ?, NULL, ?, 'ready', ?, ?)",
            )
            .bind(&scene_id)
            .bind(&collection_id)
            .bind(&scene.vanta_name)
            .bind(scene.order_index)
            .bind(&scene.transition_kind)
            .bind(scene.transition_duration_ms)
            .bind(if scene.locked { 1 } else { 0 })
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }

        let mut source_ids = HashMap::new();
        for source in &plan.sources {
            let source_id = id();
            source_ids.insert(source.obs_name.clone(), source_id.clone());
            let source_settings = if source.vanta_kind == "scene_group" {
                scene_group_import_settings(&source.settings, &scene_ids)
            } else {
                source.settings.clone()
            };
            let default_settings = imported_source_settings(
                &source.obs_kind,
                source_settings.clone(),
                &source.original_metadata,
            );
            sqlx::query(
                "INSERT INTO obs_sources
                (id, creator_id, source_kind, display_name, device_id, media_asset_id, browser_url, default_settings_json, permission_state, health_state, created_at, updated_at)
                VALUES (?, 'creator_vanta_originals', ?, ?, NULL, NULL, ?, ?, 'pending', 'unknown', ?, ?)",
            )
            .bind(&source_id)
            .bind(&source.vanta_kind)
            .bind(&source.display_name)
            .bind(source_settings.get("url").and_then(serde_json::Value::as_str))
            .bind(default_settings.to_string())
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }

        for instance in &plan.instances {
            let Some(scene_id) = scene_ids.get(&instance.scene_name) else {
                continue;
            };
            let Some(source_id) = source_ids.get(&instance.source_name) else {
                continue;
            };
            sqlx::query(
                "INSERT INTO obs_source_instances
                (id, scene_id, source_id, order_index, visible, locked, x, y, width, height, crop_json, transform_json, opacity, settings_json, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id())
            .bind(scene_id)
            .bind(source_id)
            .bind(instance.order_index)
            .bind(if instance.visible { 1 } else { 0 })
            .bind(if instance.locked { 1 } else { 0 })
            .bind(instance.x)
            .bind(instance.y)
            .bind(instance.width)
            .bind(instance.height)
            .bind(instance.crop.to_string())
            .bind(instance.transform.to_string())
            .bind(instance.opacity)
            .bind(json!({"original_metadata": instance.original_metadata}).to_string())
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }

        let report_id = id();
        sqlx::query(
            "INSERT INTO obs_import_reports
            (id, creator_id, label, collection_id, status, report_json, original_metadata_json, created_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, ?, ?, ?, ?)",
        )
        .bind(&report_id)
        .bind(&plan.label)
        .bind(&collection_id)
        .bind(&plan.report.status)
        .bind(serde_json::to_string(&plan.report)?)
        .bind(plan.original_metadata.to_string())
        .bind(&now)
        .execute(&self.pool)
        .await?;

        self.import_report(&report_id).await
    }

    pub async fn import_reports(&self) -> Result<Vec<serde_json::Value>, ObsStoreError> {
        self.list(
            "SELECT * FROM obs_import_reports ORDER BY created_at DESC",
            &[],
        )
        .await
    }

    pub async fn import_report(&self, report_id: &str) -> Result<serde_json::Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_import_reports WHERE id = ?",
            &[report_id],
        )
        .await
    }
}

fn scene_group_import_settings(settings: &Value, scene_ids: &HashMap<String, String>) -> Value {
    let mut settings = settings.clone();
    if let Some(object) = settings.as_object_mut()
        && let Some(obs_name) = object
            .get("obs_nested_scene_name")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        && let Some(scene_id) = scene_ids.get(&obs_name)
    {
        object.insert("scene_id".to_string(), json!(scene_id));
    }
    settings
}

fn imported_source_settings(obs_kind: &str, settings: Value, original_metadata: &Value) -> Value {
    let mut default_settings = settings;
    if let Some(object) = default_settings.as_object_mut() {
        object.insert("obs_kind".to_string(), json!(obs_kind));
        object.insert("original_metadata".to_string(), original_metadata.clone());
    }
    default_settings
}
