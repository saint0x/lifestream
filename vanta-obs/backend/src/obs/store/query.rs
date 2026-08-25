use serde_json::Value;

use crate::obs::{
    audio::{AudioChannelState, channel_graph, mix_graph},
    scene::scene_validation,
    source::source_summary,
};

use super::{
    ObsStore, ObsStoreError,
    row::{int, object_row, text},
};

impl ObsStore {
    pub(super) async fn active_collection(&self) -> Result<Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_scene_collections ORDER BY updated_at DESC LIMIT 1",
            &[],
        )
        .await
    }

    pub(super) async fn active_broadcast(&self) -> Result<Value, ObsStoreError> {
        self.row(
            "SELECT * FROM obs_broadcast_profiles ORDER BY updated_at DESC LIMIT 1",
            &[],
        )
        .await
    }

    pub(super) async fn scenes(&self, collection_id: &str) -> Result<Vec<Value>, ObsStoreError> {
        let collection = self
            .row(
                "SELECT * FROM obs_scene_collections WHERE id = ?",
                &[collection_id],
            )
            .await?;
        let runtime = self
            .row_optional(
                "SELECT * FROM obs_runtime_bindings WHERE scene_collection_id = ? LIMIT 1",
                &[collection_id],
            )
            .await?;
        let sources = self.sources().await?;
        let mut scenes = self
            .list(
                "SELECT * FROM obs_scenes WHERE collection_id = ? ORDER BY order_index ASC",
                &[collection_id],
            )
            .await?;
        for scene in &mut scenes {
            let scene_id = text(scene, "id");
            let instances = self.scene_instances(&scene_id).await?;
            let validation = scene_validation(
                scene,
                &instances,
                &sources,
                int(&collection, "canvas_width") as f64,
                int(&collection, "canvas_height") as f64,
                &scene_role(&scene_id, &collection, runtime.as_ref()),
            );
            if let Some(object) = scene.as_object_mut() {
                object.insert("scene_validation_json".to_string(), validation);
            }
        }
        Ok(scenes)
    }

    pub async fn scene_templates(&self) -> Result<Vec<Value>, ObsStoreError> {
        self.list(
            "SELECT * FROM obs_scene_templates ORDER BY template_kind ASC",
            &[],
        )
        .await
    }

    pub(super) async fn sources(&self) -> Result<Vec<Value>, ObsStoreError> {
        let sources = self
            .list("SELECT * FROM obs_sources ORDER BY display_name ASC", &[])
            .await?;
        let mut enriched = Vec::new();
        for source in sources {
            let source_id = text(&source, "id");
            let mut source = enrich_source_row(source);
            let filters = self.source_filters(&source_id).await?;
            if let Some(object) = source.as_object_mut() {
                object.insert("filters_chain_json".to_string(), serde_json::json!(filters));
            }
            enriched.push(source);
        }
        Ok(enriched)
    }

    pub(super) async fn scene_instances(
        &self,
        scene_id: &str,
    ) -> Result<Vec<Value>, ObsStoreError> {
        self.list(
            "SELECT * FROM obs_source_instances WHERE scene_id = ? ORDER BY order_index ASC",
            &[scene_id],
        )
        .await
    }

    pub(super) async fn instances(&self, collection_id: &str) -> Result<Vec<Value>, ObsStoreError> {
        self.list("SELECT i.* FROM obs_source_instances i JOIN obs_scenes s ON s.id = i.scene_id WHERE s.collection_id = ? ORDER BY i.order_index ASC", &[collection_id]).await
    }

    pub(super) async fn audio_channels(
        &self,
        broadcast_id: &str,
    ) -> Result<Vec<Value>, ObsStoreError> {
        let channels = self
            .list(
                "SELECT * FROM obs_audio_channels WHERE broadcast_id = ? ORDER BY created_at ASC",
                &[broadcast_id],
            )
            .await?;
        let mut enriched = channels
            .into_iter()
            .map(enrich_audio_row)
            .collect::<Vec<_>>();
        let mix = mix_graph(&enriched);
        for channel in &mut enriched {
            if let Some(object) = channel.as_object_mut() {
                object.insert("audio_mix_json".to_string(), mix.clone());
            }
        }
        Ok(enriched)
    }

    pub(super) async fn guest_room(&self, broadcast_id: &str) -> Result<Value, ObsStoreError> {
        let mut room = self
            .row(
                "SELECT * FROM obs_guest_rooms WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 1",
                &[broadcast_id],
            )
            .await?;
        let mut participants = self
            .list(
                "SELECT * FROM obs_guest_participants WHERE broadcast_id = ? ORDER BY created_at ASC",
                &[broadcast_id],
            )
            .await?;
        for participant in &mut participants {
            let participant_id = text(participant, "id");
            if let Some(session) = self
                .row_optional(
                    "SELECT * FROM obs_guest_webrtc_sessions WHERE participant_id = ? ORDER BY created_at DESC LIMIT 1",
                    &[&participant_id],
                )
                .await?
            {
                if let Some(object) = participant.as_object_mut() {
                    object.insert("webrtc_session_json".to_string(), session);
                }
            }
            if let Some(relay) = self
                .row_optional(
                    "SELECT * FROM obs_guest_media_relays WHERE participant_id = ? ORDER BY created_at DESC LIMIT 1",
                    &[&participant_id],
                )
                .await?
            {
                if let Some(object) = participant.as_object_mut() {
                    object.insert("media_relay_json".to_string(), relay);
                }
            }
        }
        if let Some(object) = room.as_object_mut() {
            object.insert(
                "participants_json".to_string(),
                serde_json::json!(participants),
            );
            object.insert(
                "modes_supported_json".to_string(),
                serde_json::json!(["solo", "dual", "group", "shared_game"]),
            );
        }
        Ok(room)
    }

    pub(super) async fn cues(&self, broadcast_id: &str) -> Result<Vec<Value>, ObsStoreError> {
        self.list("SELECT * FROM obs_live_cues WHERE broadcast_id = ? ORDER BY COALESCE(scheduled_at_seconds, 999999) ASC", &[broadcast_id]).await
    }

    pub(super) async fn replays(&self, broadcast_id: &str) -> Result<Vec<Value>, ObsStoreError> {
        let replays = self
            .list(
                "SELECT * FROM obs_replay_markers WHERE broadcast_id = ? ORDER BY created_at DESC",
                &[broadcast_id],
            )
            .await?;
        let mut enriched = Vec::new();
        for mut replay in replays {
            let marker_id = text(&replay, "id");
            if let Some(clip) = self
                .row_optional(
                    "SELECT * FROM obs_replay_clip_drafts WHERE replay_marker_id = ? ORDER BY created_at DESC LIMIT 1",
                    &[&marker_id],
                )
                .await?
            {
                let clip = self.enrich_replay_clip(clip).await?;
                if let Some(object) = replay.as_object_mut() {
                    object.insert("clip_draft_json".to_string(), clip);
                }
            }
            enriched.push(replay);
        }
        Ok(enriched)
    }

    pub(super) async fn replay_marker(&self, marker_id: &str) -> Result<Value, ObsStoreError> {
        let replay = self
            .row(
                "SELECT * FROM obs_replay_markers WHERE id = ?",
                &[marker_id],
            )
            .await?;
        self.enrich_replay_marker(replay).await
    }

    pub(super) async fn runtime_target(
        &self,
        broadcast_id: &str,
    ) -> Result<Option<Value>, ObsStoreError> {
        self.row_optional(
            "SELECT * FROM vanta_live_runtime_targets WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 1",
            &[broadcast_id],
        )
        .await
    }

    pub(super) async fn runtime_output(
        &self,
        broadcast_id: &str,
    ) -> Result<Option<Value>, ObsStoreError> {
        self.row_optional(
            "SELECT * FROM vanta_live_runtime_outputs WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 1",
            &[broadcast_id],
        )
        .await
    }

    pub(super) async fn playback_readiness(
        &self,
        broadcast_id: &str,
    ) -> Result<Option<Value>, ObsStoreError> {
        self.row_optional(
            "SELECT * FROM vanta_live_playback_readiness WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 1",
            &[broadcast_id],
        )
        .await
    }

    pub(super) async fn latest_runtime_telemetry(
        &self,
        broadcast_id: &str,
    ) -> Result<Option<Value>, ObsStoreError> {
        self.row_optional(
            "SELECT * FROM vanta_live_runtime_telemetry WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 1",
            &[broadcast_id],
        )
        .await
    }

    pub(super) async fn latest_transition(
        &self,
        broadcast_id: &str,
    ) -> Result<Option<Value>, ObsStoreError> {
        self.row_optional(
            "SELECT * FROM obs_scene_transition_runs WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 1",
            &[broadcast_id],
        )
        .await
    }

    pub(super) async fn events(&self, broadcast_id: &str) -> Result<Vec<Value>, ObsStoreError> {
        self.list("SELECT * FROM obs_runtime_events WHERE broadcast_id IS NULL OR broadcast_id = ? ORDER BY created_at DESC LIMIT 30", &[broadcast_id]).await
    }

    pub(super) async fn incidents(&self, broadcast_id: &str) -> Result<Vec<Value>, ObsStoreError> {
        self.list(
            "SELECT * FROM obs_runtime_incidents WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 20",
            &[broadcast_id],
        )
        .await
    }

    pub(super) async fn support_bundles(
        &self,
        broadcast_id: &str,
    ) -> Result<Vec<Value>, ObsStoreError> {
        self.list(
            "SELECT * FROM obs_support_bundles WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 5",
            &[broadcast_id],
        )
        .await
    }

    pub(super) async fn latest_preflight(
        &self,
        broadcast_id: &str,
    ) -> Result<Value, ObsStoreError> {
        self.row_optional(
            "SELECT * FROM obs_preflight_checks WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 1",
            &[broadcast_id],
        )
        .await?
        .ok_or(ObsStoreError::NotFound)
    }

    pub(super) async fn list(
        &self,
        sql: &str,
        binds: &[&str],
    ) -> Result<Vec<Value>, ObsStoreError> {
        let mut query = sqlx::query(sql);
        for value in binds {
            query = query.bind(*value);
        }
        let rows = query.fetch_all(&self.pool).await?;
        rows.iter().map(object_row).collect()
    }

    pub(super) async fn row(&self, sql: &str, binds: &[&str]) -> Result<Value, ObsStoreError> {
        self.row_optional(sql, binds)
            .await?
            .ok_or(ObsStoreError::NotFound)
    }

    pub(super) async fn row_optional(
        &self,
        sql: &str,
        binds: &[&str],
    ) -> Result<Option<Value>, ObsStoreError> {
        let mut query = sqlx::query(sql);
        for value in binds {
            query = query.bind(*value);
        }
        query
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(object_row)
            .transpose()
    }
}

impl ObsStore {
    async fn enrich_replay_marker(&self, mut replay: Value) -> Result<Value, ObsStoreError> {
        let marker_id = text(&replay, "id");
        if let Some(clip) = self
            .row_optional(
                "SELECT * FROM obs_replay_clip_drafts WHERE replay_marker_id = ? ORDER BY created_at DESC LIMIT 1",
                &[&marker_id],
            )
            .await?
            && let Some(object) = replay.as_object_mut()
        {
            let clip = self.enrich_replay_clip(clip).await?;
            object.insert("clip_draft_json".to_string(), clip);
        }
        Ok(replay)
    }

    async fn enrich_replay_clip(&self, mut clip: Value) -> Result<Value, ObsStoreError> {
        let asset_id = text(&clip, "clip_media_asset_id");
        if let Some(asset) = self
            .row_optional(
                "SELECT * FROM vanta_media_assets WHERE id = ?",
                &[&asset_id],
            )
            .await?
            && let Some(object) = clip.as_object_mut()
        {
            object.insert("vanta_asset_json".to_string(), asset);
        }
        Ok(clip)
    }
}

fn enrich_audio_row(mut channel: Value) -> Value {
    let graph = channel_graph(&AudioChannelState {
        id: text(&channel, "id"),
        channel_kind: text(&channel, "channel_kind"),
        muted: int_bool(&channel, "muted"),
        solo: int_bool(&channel, "solo"),
        gain_db: channel
            .get("gain_db")
            .and_then(Value::as_f64)
            .unwrap_or_default(),
        monitor_enabled: int_bool(&channel, "monitor_enabled"),
        program_enabled: int_bool(&channel, "program_enabled"),
        delay_ms: channel
            .get("delay_ms")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        filters: channel.get("filters_json").cloned().unwrap_or(Value::Null),
        route: channel.get("route_json").cloned().unwrap_or(Value::Null),
    });
    if let Some(object) = channel.as_object_mut() {
        object.insert("audio_graph_json".to_string(), graph);
    }
    channel
}

fn scene_role(scene_id: &str, collection: &Value, runtime: Option<&Value>) -> String {
    let mut roles = Vec::new();
    if text(collection, "active_scene_id") == scene_id {
        roles.push("active");
    }
    if runtime
        .map(|runtime| text(runtime, "program_scene_id") == scene_id)
        .unwrap_or(false)
    {
        roles.push("program");
    }
    if runtime
        .map(|runtime| text(runtime, "preview_scene_id") == scene_id)
        .unwrap_or(false)
    {
        roles.push("preview");
    }
    if roles.is_empty() {
        "standby".to_string()
    } else {
        roles.join("+")
    }
}

fn int_bool(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_i64).unwrap_or_default() != 0
}

fn enrich_source_row(mut source: Value) -> Value {
    let settings = normalized_default_source_settings(
        source
            .get("default_settings_json")
            .cloned()
            .unwrap_or(Value::Null),
    );
    if let Some(object) = source.as_object_mut() {
        object.insert("default_settings_json".to_string(), settings.clone());
    }
    let summary = source_summary(
        &text(&source, "source_kind"),
        text(&source, "device_id").as_nonempty(),
        text(&source, "browser_url").as_nonempty(),
        text(&source, "media_asset_id").as_nonempty(),
        &text(&source, "permission_state"),
        &text(&source, "health_state"),
        settings,
    );
    if let Some(object) = source.as_object_mut() {
        object.insert(
            "source_contract_json".to_string(),
            summary["contract"].clone(),
        );
        object.insert(
            "source_validation_json".to_string(),
            summary["validation"].clone(),
        );
        object.insert(
            "source_permission_json".to_string(),
            summary["permission"].clone(),
        );
        object.insert(
            "source_sync_json".to_string(),
            summary["local_sync"].clone(),
        );
    }
    source
}

fn normalized_default_source_settings(settings: Value) -> Value {
    let Some(inner) = settings.get("settings").cloned() else {
        return settings;
    };
    let mut normalized = inner;
    if let (Some(target), Some(source)) = (normalized.as_object_mut(), settings.as_object()) {
        for key in ["obs_kind", "original_metadata"] {
            if let Some(value) = source.get(key) {
                target.insert(key.to_string(), value.clone());
            }
        }
    }
    normalized
}

trait NonEmpty {
    fn as_nonempty(&self) -> Option<&str>;
}

impl NonEmpty for String {
    fn as_nonempty(&self) -> Option<&str> {
        if self.trim().is_empty() {
            None
        } else {
            Some(self.as_str())
        }
    }
}
