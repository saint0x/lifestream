use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Column, Row, SqlitePool};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{PublishValidation, RenderRequest, TimelinePatch, render_plan};

#[derive(Debug, Error)]
pub enum StoreError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Serialize)]
pub struct EditorProject {
    pub id: String,
    pub creator_id: String,
    pub owner_user_id: String,
    pub title: String,
    pub description: String,
    pub source_kind: String,
    pub campaign_id: Option<String>,
    pub offer_id: Option<String>,
    pub status: String,
    pub active_timeline_id: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectInput {
    pub title: String,
    pub description: Option<String>,
    pub source_kind: Option<String>,
    pub campaign_id: Option<String>,
    pub offer_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CommentInput {
    pub body: String,
    pub visibility: String,
    pub timeline_seconds: f64,
}

#[derive(Debug, Deserialize)]
pub struct ReviewRequestInput {
    pub review_kind: String,
    pub due_at: Option<String>,
}

#[derive(Clone)]
pub struct EditorStore {
    pool: SqlitePool,
}

impl EditorStore {
    pub async fn connect(pool: SqlitePool) -> Result<Self, StoreError> {
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    async fn migrate(&self) -> Result<(), StoreError> {
        for statement in SCHEMA.split(";").map(str::trim).filter(|s| !s.is_empty()) {
            sqlx::query(statement).execute(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn seed(&self) -> Result<(), StoreError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM editor_projects")
            .fetch_one(&self.pool)
            .await?;
        if count > 0 {
            return Ok(());
        }

        let project_id = id();
        let timeline_id = id();
        let video_track = id();
        let audio_track = id();
        let ad_track = id();
        let caption_track = id();
        let now = now();

        sqlx::query(
            "INSERT INTO editor_projects
            (id, creator_id, owner_user_id, title, description, source_kind, campaign_id, offer_id, status, active_timeline_id, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&project_id)
        .bind("creator_vanta_originals")
        .bind("user_creator_owner")
        .bind("Ghost Standard: sponsor cut")
        .bind("Vanta-ready edit with locked sponsor inventory and review routing.")
        .bind("campaign_work")
        .bind("campaign_nova_run")
        .bind("offer_midroll_hostread")
        .bind("editing")
        .bind(&timeline_id)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO editor_timelines
            (id, project_id, name, duration_seconds, frame_rate, resolution_width, resolution_height, sample_rate, status, ui_state_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&timeline_id)
        .bind(&project_id)
        .bind("Main cut")
        .bind(742.0)
        .bind(23.976)
        .bind(3840)
        .bind(2160)
        .bind(48000)
        .bind("active")
        .bind(json!({"playhead_seconds": 186.0, "selected_id": "slot_midroll", "zoom": 1.0, "safe_areas": true, "waveform": true}).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        for (track_id, kind, name, order_index) in [
            (&video_track, "video", "V1 picture", 1),
            (&audio_track, "audio", "A1 mix", 2),
            (&ad_track, "ad", "Sold inventory", 3),
            (&caption_track, "caption", "Captions", 4),
        ] {
            sqlx::query(
                "INSERT INTO editor_tracks (id, timeline_id, kind, name, order_index, locked, muted, visible, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, 0, 0, 1, ?, ?)",
            )
            .bind(track_id)
            .bind(&timeline_id)
            .bind(kind)
            .bind(name)
            .bind(order_index)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }

        for (asset_id, role, name, status, rights, duration, meta) in [
            (
                "media_asset_raw_ep_ghost",
                "raw_video",
                "Ghost Standard raw camera A",
                "ready",
                "cleared",
                742.0,
                json!({"resolution":"4K","owner":"creator","source":"upload"}),
            ),
            (
                "media_asset_nova_logo",
                "sponsor_creative",
                "Nova logo sting",
                "ready",
                "campaign_limited",
                15.0,
                json!({"resolution":"1080p","owner":"advertiser","source":"campaign"}),
            ),
            (
                "media_asset_caption_en",
                "captions",
                "English captions",
                "ready",
                "cleared",
                742.0,
                json!({"format":"vtt","owner":"vanta","source":"transcript"}),
            ),
        ] {
            sqlx::query(
                "INSERT INTO editor_media_assets
                (id, project_id, media_asset_id, role, display_name, processing_status, rights_status, duration_seconds, metadata_json, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id())
            .bind(&project_id)
            .bind(asset_id)
            .bind(role)
            .bind(name)
            .bind(status)
            .bind(rights)
            .bind(duration)
            .bind(meta.to_string())
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }

        sqlx::query(
            "INSERT INTO editor_clips
            (id, timeline_id, track_id, media_asset_id, label, source_in_seconds, source_out_seconds, timeline_in_seconds, timeline_out_seconds, speed, volume, opacity, metadata_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, 0, 742, 0, 742, 1, 1, 1, ?, ?, ?)",
        )
        .bind(id())
        .bind(&timeline_id)
        .bind(&video_track)
        .bind("media_asset_raw_ep_ghost")
        .bind("Episode assembly")
        .bind(json!({"color":"neutral"}).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO editor_ad_slots
            (id, project_id, timeline_id, track_id, label, campaign_id, offer_id, package_id, advertiser_id, placement_type, insertion_mode, timeline_in_seconds, timeline_out_seconds, required_duration_seconds, selected_media_asset_id, status, review_status, measurement_key, requirements_json, validation_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("slot_midroll")
        .bind(&project_id)
        .bind(&timeline_id)
        .bind(&ad_track)
        .bind("Nova mid-roll host read")
        .bind("campaign_nova_run")
        .bind("offer_midroll_hostread")
        .bind("package_creator_prime")
        .bind("advertiser_nova")
        .bind("host-read")
        .bind("host_read")
        .bind(312.0)
        .bind(342.0)
        .bind(30.0)
        .bind("media_asset_nova_logo")
        .bind("approved")
        .bind("approved")
        .bind("nova-ghost-001")
        .bind(json!({"talking_points":["mention creator code VANTA20","show logo for at least 3 seconds"],"prohibited_claims":["guaranteed results"]}).to_string())
        .bind(json!({"valid": true, "warnings": [], "blockers": []}).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        for (start, end, speaker, text) in [
            (
                168.0,
                176.0,
                "Mara",
                "This is where the episode turns from observation into proof.",
            ),
            (
                305.0,
                314.0,
                "Mara",
                "Before the next sequence, we need to talk about the tools that made this possible.",
            ),
            (
                314.0,
                333.0,
                "Mara",
                "Nova helped us keep the field kit light without compromising the image.",
            ),
            (
                512.0,
                525.0,
                "Ike",
                "The final pass should preserve that silence before the reveal.",
            ),
        ] {
            sqlx::query(
                "INSERT INTO editor_transcript_segments
                (id, project_id, timeline_id, start_seconds, end_seconds, speaker, text, flags_json, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id())
            .bind(&project_id)
            .bind(&timeline_id)
            .bind(start)
            .bind(end)
            .bind(speaker)
            .bind(text)
            .bind(json!({"ad_candidate": start > 300.0 && start < 340.0}).to_string())
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }

        sqlx::query(
            "INSERT INTO editor_campaign_requirements
            (id, project_id, campaign_id, title, requirement_kind, status, due_at, body_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id())
        .bind(&project_id)
        .bind("campaign_nova_run")
        .bind("Nova creator prime package")
        .bind("sponsor_deliverable")
        .bind("in_progress")
        .bind("2026-09-04T17:00:00Z")
        .bind(json!({
            "advertiser": "Nova",
            "objective": "Premium creator association for launch week.",
            "placements": ["30s mid-roll host read", "3s logo card", "proof clip"],
            "required_claims": ["Use code VANTA20"],
            "prohibited_claims": ["guaranteed results"],
            "tracking_links": ["https://streamvanta.tv/r/nova"],
            "approval_contacts": ["adops@streamvanta.tv"]
        }).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        self.create_timeline_version(&project_id, "Seeded working cut")
            .await?;
        Ok(())
    }

    pub async fn projects(&self) -> Result<Vec<EditorProject>, StoreError> {
        let rows = sqlx::query(
            "SELECT id, creator_id, owner_user_id, title, description, source_kind, campaign_id, offer_id, status, active_timeline_id, updated_at
            FROM editor_projects ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(project_from_row).collect())
    }

    pub async fn create_project(
        &self,
        input: CreateProjectInput,
    ) -> Result<EditorProject, StoreError> {
        let project_id = id();
        let timeline_id = id();
        let now = now();
        sqlx::query(
            "INSERT INTO editor_projects
            (id, creator_id, owner_user_id, title, description, source_kind, campaign_id, offer_id, status, active_timeline_id, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', 'user_creator_owner', ?, ?, ?, ?, ?, 'draft', ?, ?, ?)",
        )
        .bind(&project_id)
        .bind(input.title)
        .bind(input.description.unwrap_or_default())
        .bind(input.source_kind.unwrap_or_else(|| "imported_raw".to_string()))
        .bind(input.campaign_id)
        .bind(input.offer_id)
        .bind(&timeline_id)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO editor_timelines
            (id, project_id, name, duration_seconds, frame_rate, resolution_width, resolution_height, sample_rate, status, ui_state_json, created_at, updated_at)
            VALUES (?, ?, 'Main cut', 0, 23.976, 1920, 1080, 48000, 'active', ?, ?, ?)",
        )
        .bind(&timeline_id)
        .bind(&project_id)
        .bind(json!({"playhead_seconds":0,"selected_id":null,"zoom":1,"safe_areas":true,"waveform":true}).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.update_project_status(&project_id, "editing").await
    }

    pub async fn update_project_status(
        &self,
        project_id: &str,
        status: &str,
    ) -> Result<EditorProject, StoreError> {
        sqlx::query("UPDATE editor_projects SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status)
            .bind(now())
            .bind(project_id)
            .execute(&self.pool)
            .await?;
        let row = sqlx::query(
            "SELECT id, creator_id, owner_user_id, title, description, source_kind, campaign_id, offer_id, status, active_timeline_id, updated_at
            FROM editor_projects WHERE id = ?",
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(project_from_row(row))
    }

    pub async fn delete_project(&self, project_id: &str) -> Result<Value, StoreError> {
        let current = self
            .project_bundle(project_id)
            .await?
            .unwrap_or_else(|| json!({ "id": project_id }));
        for table in [
            "editor_publish_links",
            "editor_exports",
            "editor_render_jobs",
            "editor_review_requests",
            "editor_comments",
            "editor_transcript_segments",
            "editor_campaign_requirements",
            "editor_ad_slots",
            "editor_media_assets",
        ] {
            let sql = format!("DELETE FROM {table} WHERE project_id = ?");
            sqlx::query(&sql)
                .bind(project_id)
                .execute(&self.pool)
                .await?;
        }
        if let Some(timeline_id) = current["project"]["active_timeline_id"].as_str() {
            sqlx::query("DELETE FROM editor_timeline_versions WHERE timeline_id = ?")
                .bind(timeline_id)
                .execute(&self.pool)
                .await?;
            sqlx::query("DELETE FROM editor_clips WHERE timeline_id = ?")
                .bind(timeline_id)
                .execute(&self.pool)
                .await?;
            sqlx::query("DELETE FROM editor_tracks WHERE timeline_id = ?")
                .bind(timeline_id)
                .execute(&self.pool)
                .await?;
            sqlx::query("DELETE FROM editor_timelines WHERE id = ?")
                .bind(timeline_id)
                .execute(&self.pool)
                .await?;
        }
        sqlx::query("DELETE FROM editor_projects WHERE id = ?")
            .bind(project_id)
            .execute(&self.pool)
            .await?;
        Ok(json!({"deleted": true, "project": current}))
    }

    pub async fn project_bundle(&self, project_id: &str) -> Result<Option<Value>, StoreError> {
        let project = match sqlx::query(
            "SELECT id, creator_id, owner_user_id, title, description, source_kind, campaign_id, offer_id, status, active_timeline_id, updated_at
            FROM editor_projects WHERE id = ?",
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await? {
            Some(row) => project_from_row(row),
            None => return Ok(None),
        };
        let timeline = self
            .timeline_bundle(project_id)
            .await?
            .unwrap_or_else(|| json!({}));
        Ok(Some(json!({
            "project": project,
            "assets": self.assets(project_id).await?,
            "requirements": self.requirements(project_id).await?,
            "comments": self.comments(project_id).await?,
            "timeline": timeline
        })))
    }

    pub async fn timeline_bundle(&self, project_id: &str) -> Result<Option<Value>, StoreError> {
        let Some(row) = sqlx::query(
            "SELECT * FROM editor_timelines WHERE project_id = ? ORDER BY updated_at DESC LIMIT 1",
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        let timeline_id: String = row.get("id");
        Ok(Some(json!({
            "timeline": object_row(&row, &["ui_state_json"])?,
            "tracks": self.list_by_timeline("editor_tracks", &timeline_id).await?,
            "clips": self.list_by_timeline("editor_clips", &timeline_id).await?,
            "ad_slots": self.list_by_project("editor_ad_slots", project_id).await?,
            "transcript": self.list_by_project("editor_transcript_segments", project_id).await?,
            "versions": self.list_by_timeline("editor_timeline_versions", &timeline_id).await?
        })))
    }

    pub async fn apply_timeline_patch(
        &self,
        project_id: &str,
        input: TimelinePatch,
    ) -> Result<Value, StoreError> {
        let Some(bundle) = self.timeline_bundle(project_id).await? else {
            return Ok(json!({}));
        };
        let timeline = &bundle["timeline"];
        let timeline_id = timeline["id"].as_str().unwrap_or_default();
        let mut ui_state = timeline["ui_state_json"].clone();
        if let Some(playhead) = input.playhead_seconds {
            ui_state["playhead_seconds"] = json!(playhead);
        }
        if let Some(selected) = input.selected_id {
            ui_state["selected_id"] = json!(selected);
        }
        if let Some(zoom) = input.zoom {
            ui_state["zoom"] = json!(zoom);
        }
        if let Some(safe_areas) = input.safe_areas {
            ui_state["safe_areas"] = json!(safe_areas);
        }
        if let Some(waveform) = input.waveform {
            ui_state["waveform"] = json!(waveform);
        }
        sqlx::query("UPDATE editor_timelines SET ui_state_json = ?, updated_at = ? WHERE id = ?")
            .bind(ui_state.to_string())
            .bind(now())
            .bind(timeline_id)
            .execute(&self.pool)
            .await?;
        if input.edl_json.is_some() || input.change_summary.is_some() {
            self.create_timeline_version(
                project_id,
                input.change_summary.as_deref().unwrap_or("Timeline update"),
            )
            .await?;
        }
        Ok(self
            .timeline_bundle(project_id)
            .await?
            .unwrap_or_else(|| json!({})))
    }

    pub async fn create_timeline_version(
        &self,
        project_id: &str,
        summary: &str,
    ) -> Result<Value, StoreError> {
        let bundle = self
            .timeline_bundle(project_id)
            .await?
            .unwrap_or_else(|| json!({}));
        let timeline_id = bundle["timeline"]["id"].as_str().unwrap_or_default();
        let version_number: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version_number), 0) + 1 FROM editor_timeline_versions WHERE timeline_id = ?",
        )
        .bind(timeline_id)
        .fetch_one(&self.pool)
        .await?;
        let version_id = id();
        sqlx::query(
            "INSERT INTO editor_timeline_versions
            (id, timeline_id, version_number, parent_version_id, change_summary, edl_json, created_by_user_id, created_at)
            VALUES (?, ?, ?, NULL, ?, ?, 'user_creator_owner', ?)",
        )
        .bind(&version_id)
        .bind(timeline_id)
        .bind(version_number)
        .bind(summary)
        .bind(bundle.to_string())
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(json!({"id": version_id, "version_number": version_number, "change_summary": summary}))
    }

    pub async fn assets(&self, project_id: &str) -> Result<Vec<Value>, StoreError> {
        self.list_by_project("editor_media_assets", project_id)
            .await
    }

    pub async fn update_asset(&self, asset_id: &str, input: Value) -> Result<Value, StoreError> {
        let current = self.row_by_id("editor_media_assets", asset_id).await?;
        sqlx::query(
            "UPDATE editor_media_assets
            SET role = ?, display_name = ?, processing_status = ?, rights_status = ?,
                duration_seconds = ?, metadata_json = ?, updated_at = ?
            WHERE id = ?",
        )
        .bind(
            input["role"]
                .as_str()
                .unwrap_or_else(|| current["role"].as_str().unwrap_or("raw_video")),
        )
        .bind(
            input["display_name"]
                .as_str()
                .unwrap_or_else(|| current["display_name"].as_str().unwrap_or("Media asset")),
        )
        .bind(
            input["processing_status"]
                .as_str()
                .unwrap_or_else(|| current["processing_status"].as_str().unwrap_or("ready")),
        )
        .bind(input["rights_status"].as_str().unwrap_or_else(|| {
            current["rights_status"]
                .as_str()
                .unwrap_or("pending_review")
        }))
        .bind(
            input["duration_seconds"]
                .as_f64()
                .unwrap_or_else(|| current["duration_seconds"].as_f64().unwrap_or(0.0)),
        )
        .bind(input["metadata_json"].as_object().map_or_else(
            || current["metadata_json"].to_string(),
            |value| json!(value).to_string(),
        ))
        .bind(now())
        .bind(asset_id)
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_media_assets", asset_id).await
    }

    pub async fn delete_asset(&self, asset_id: &str) -> Result<Value, StoreError> {
        self.delete_row("editor_media_assets", asset_id, "asset")
            .await
    }

    pub async fn import_asset(
        &self,
        project_id: &str,
        media_asset_id: &str,
        role: &str,
    ) -> Result<Value, StoreError> {
        let row_id = id();
        let now = now();
        sqlx::query(
            "INSERT INTO editor_media_assets
            (id, project_id, media_asset_id, role, display_name, processing_status, rights_status, duration_seconds, metadata_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, 'ready', 'pending_review', 0, ?, ?, ?)",
        )
        .bind(&row_id)
        .bind(project_id)
        .bind(media_asset_id)
        .bind(role)
        .bind(media_asset_id)
        .bind(json!({"source":"existing_vanta_media_asset"}).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(
            json!({"id": row_id, "media_asset_id": media_asset_id, "role": role, "rights_status": "pending_review"}),
        )
    }

    pub async fn create_asset_record(
        &self,
        project_id: &str,
        media_asset_id: &str,
        role: &str,
        display_name: &str,
        processing_status: &str,
        rights_status: &str,
        duration_seconds: f64,
        metadata: Value,
    ) -> Result<Value, StoreError> {
        let row_id = id();
        let now = now();
        sqlx::query(
            "INSERT INTO editor_media_assets
            (id, project_id, media_asset_id, role, display_name, processing_status, rights_status, duration_seconds, metadata_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&row_id)
        .bind(project_id)
        .bind(media_asset_id)
        .bind(role)
        .bind(display_name)
        .bind(processing_status)
        .bind(rights_status)
        .bind(duration_seconds)
        .bind(metadata.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_media_assets", &row_id).await
    }

    pub async fn tracks(&self, project_id: &str) -> Result<Vec<Value>, StoreError> {
        let timeline_id = self.active_timeline_id(project_id).await?;
        self.list_by_timeline("editor_tracks", &timeline_id).await
    }

    pub async fn create_track(&self, project_id: &str, input: Value) -> Result<Value, StoreError> {
        let timeline_id = self.active_timeline_id(project_id).await?;
        let track_id = id();
        let now = now();
        let order_index: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(order_index), 0) + 1 FROM editor_tracks WHERE timeline_id = ?",
        )
        .bind(&timeline_id)
        .fetch_one(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO editor_tracks
            (id, timeline_id, kind, name, order_index, locked, muted, visible, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&track_id)
        .bind(&timeline_id)
        .bind(input["kind"].as_str().unwrap_or("video"))
        .bind(input["name"].as_str().unwrap_or("Track"))
        .bind(input["order_index"].as_i64().unwrap_or(order_index))
        .bind(input["locked"].as_bool().unwrap_or(false) as i64)
        .bind(input["muted"].as_bool().unwrap_or(false) as i64)
        .bind(input["visible"].as_bool().unwrap_or(true) as i64)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_tracks", &track_id).await
    }

    pub async fn update_track(&self, track_id: &str, input: Value) -> Result<Value, StoreError> {
        let current = self.row_by_id("editor_tracks", track_id).await?;
        sqlx::query(
            "UPDATE editor_tracks
            SET kind = ?, name = ?, order_index = ?, locked = ?, muted = ?, visible = ?, updated_at = ?
            WHERE id = ?",
        )
        .bind(input["kind"].as_str().unwrap_or_else(|| current["kind"].as_str().unwrap_or("video")))
        .bind(input["name"].as_str().unwrap_or_else(|| current["name"].as_str().unwrap_or("Track")))
        .bind(input["order_index"].as_i64().unwrap_or_else(|| current["order_index"].as_i64().unwrap_or(0)))
        .bind(input["locked"].as_bool().map(i64::from).unwrap_or_else(|| current["locked"].as_i64().unwrap_or(0)))
        .bind(input["muted"].as_bool().map(i64::from).unwrap_or_else(|| current["muted"].as_i64().unwrap_or(0)))
        .bind(input["visible"].as_bool().map(i64::from).unwrap_or_else(|| current["visible"].as_i64().unwrap_or(1)))
        .bind(now())
        .bind(track_id)
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_tracks", track_id).await
    }

    pub async fn delete_track(&self, track_id: &str) -> Result<Value, StoreError> {
        self.delete_row("editor_tracks", track_id, "track").await
    }

    pub async fn ad_slots(&self, project_id: &str) -> Result<Vec<Value>, StoreError> {
        self.list_by_project("editor_ad_slots", project_id).await
    }

    pub async fn clips(&self, project_id: &str) -> Result<Vec<Value>, StoreError> {
        let timeline_id = self.active_timeline_id(project_id).await?;
        self.list_by_timeline("editor_clips", &timeline_id).await
    }

    pub async fn create_ad_slot(
        &self,
        project_id: &str,
        mut input: Value,
    ) -> Result<Value, StoreError> {
        let timeline_id = self.active_timeline_id(project_id).await?;
        let ad_track = self.ensure_ad_track(&timeline_id).await?;
        let slot_id = id();
        let now = now();
        let label = input["label"]
            .as_str()
            .unwrap_or("Ad placement")
            .to_string();
        let placement_type = input["placement_type"]
            .as_str()
            .unwrap_or("mid-roll")
            .to_string();
        let start = input["timeline_in_seconds"].as_f64().unwrap_or(0.0);
        let end = input["timeline_out_seconds"]
            .as_f64()
            .unwrap_or(start + 30.0);
        input["id"] = json!(slot_id);
        sqlx::query(
            "INSERT INTO editor_ad_slots
            (id, project_id, timeline_id, track_id, label, campaign_id, offer_id, package_id, advertiser_id, placement_type, insertion_mode, timeline_in_seconds, timeline_out_seconds, required_duration_seconds, selected_media_asset_id, status, review_status, measurement_key, requirements_json, validation_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, NULL, NULL, NULL, NULL, ?, 'dynamic', ?, ?, ?, NULL, 'draft', 'not_required', ?, ?, ?, ?, ?)",
        )
        .bind(&slot_id)
        .bind(project_id)
        .bind(&timeline_id)
        .bind(&ad_track)
        .bind(label)
        .bind(placement_type)
        .bind(start)
        .bind(end)
        .bind(input["required_duration_seconds"].as_f64().unwrap_or(end - start))
        .bind(format!("draft-{slot_id}"))
        .bind(json!({}).to_string())
        .bind(json!({"valid": false, "blockers": ["draft slot needs review"]}).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(input)
    }

    pub async fn create_clip(&self, project_id: &str, input: Value) -> Result<Value, StoreError> {
        let timeline_id = self.active_timeline_id(project_id).await?;
        let track_id = input["track_id"]
            .as_str()
            .map(ToString::to_string)
            .unwrap_or(
                self.ensure_track(&timeline_id, "video", "V1 picture")
                    .await?,
            );
        let clip_id = id();
        let now = now();
        let source_in = input["source_in_seconds"].as_f64().unwrap_or(0.0);
        let source_out = input["source_out_seconds"]
            .as_f64()
            .unwrap_or(source_in + 30.0);
        let timeline_in = input["timeline_in_seconds"].as_f64().unwrap_or(0.0);
        let timeline_out = input["timeline_out_seconds"]
            .as_f64()
            .unwrap_or(timeline_in + (source_out - source_in));
        sqlx::query(
            "INSERT INTO editor_clips
            (id, timeline_id, track_id, media_asset_id, label, source_in_seconds, source_out_seconds, timeline_in_seconds, timeline_out_seconds, speed, volume, opacity, metadata_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&clip_id)
        .bind(&timeline_id)
        .bind(&track_id)
        .bind(input["media_asset_id"].as_str().unwrap_or("unlinked_media_asset"))
        .bind(input["label"].as_str().unwrap_or("Timeline clip"))
        .bind(source_in)
        .bind(source_out)
        .bind(timeline_in)
        .bind(timeline_out)
        .bind(input["speed"].as_f64().unwrap_or(1.0))
        .bind(input["volume"].as_f64().unwrap_or(1.0))
        .bind(input["opacity"].as_f64().unwrap_or(1.0))
        .bind(input["metadata_json"].as_object().map_or_else(|| json!({}).to_string(), |value| json!(value).to_string()))
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_clips", &clip_id).await
    }

    pub async fn update_clip(&self, clip_id: &str, input: Value) -> Result<Value, StoreError> {
        let current = self.row_by_id("editor_clips", clip_id).await?;
        sqlx::query(
            "UPDATE editor_clips
            SET label = ?, source_in_seconds = ?, source_out_seconds = ?,
                timeline_in_seconds = ?, timeline_out_seconds = ?, speed = ?,
                volume = ?, opacity = ?, metadata_json = ?, updated_at = ?
            WHERE id = ?",
        )
        .bind(
            input["label"]
                .as_str()
                .unwrap_or_else(|| current["label"].as_str().unwrap_or("Timeline clip")),
        )
        .bind(
            input["source_in_seconds"]
                .as_f64()
                .unwrap_or_else(|| current["source_in_seconds"].as_f64().unwrap_or(0.0)),
        )
        .bind(
            input["source_out_seconds"]
                .as_f64()
                .unwrap_or_else(|| current["source_out_seconds"].as_f64().unwrap_or(0.0)),
        )
        .bind(
            input["timeline_in_seconds"]
                .as_f64()
                .unwrap_or_else(|| current["timeline_in_seconds"].as_f64().unwrap_or(0.0)),
        )
        .bind(
            input["timeline_out_seconds"]
                .as_f64()
                .unwrap_or_else(|| current["timeline_out_seconds"].as_f64().unwrap_or(0.0)),
        )
        .bind(
            input["speed"]
                .as_f64()
                .unwrap_or_else(|| current["speed"].as_f64().unwrap_or(1.0)),
        )
        .bind(
            input["volume"]
                .as_f64()
                .unwrap_or_else(|| current["volume"].as_f64().unwrap_or(1.0)),
        )
        .bind(
            input["opacity"]
                .as_f64()
                .unwrap_or_else(|| current["opacity"].as_f64().unwrap_or(1.0)),
        )
        .bind(input["metadata_json"].as_object().map_or_else(
            || current["metadata_json"].to_string(),
            |value| json!(value).to_string(),
        ))
        .bind(now())
        .bind(clip_id)
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_clips", clip_id).await
    }

    pub async fn delete_clip(&self, clip_id: &str) -> Result<Value, StoreError> {
        let current = self.row_by_id("editor_clips", clip_id).await?;
        sqlx::query("DELETE FROM editor_clips WHERE id = ?")
            .bind(clip_id)
            .execute(&self.pool)
            .await?;
        Ok(json!({"deleted": true, "clip": current}))
    }

    pub async fn delete_ad_slot(&self, ad_slot_id: &str) -> Result<Value, StoreError> {
        self.delete_row("editor_ad_slots", ad_slot_id, "ad_slot")
            .await
    }

    pub async fn update_ad_slot(
        &self,
        ad_slot_id: &str,
        input: Value,
    ) -> Result<Value, StoreError> {
        sqlx::query(
            "UPDATE editor_ad_slots
            SET selected_media_asset_id = COALESCE(?, selected_media_asset_id),
                status = COALESCE(?, status),
                review_status = COALESCE(?, review_status),
                updated_at = ?
            WHERE id = ?",
        )
        .bind(input["selected_media_asset_id"].as_str())
        .bind(input["status"].as_str())
        .bind(input["review_status"].as_str())
        .bind(now())
        .bind(ad_slot_id)
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_ad_slots", ad_slot_id).await
    }

    pub async fn validate_ad_slot(&self, ad_slot_id: &str) -> Result<Value, StoreError> {
        let row = self.row_by_id("editor_ad_slots", ad_slot_id).await?;
        let mut blockers = Vec::new();
        let duration = row["timeline_out_seconds"].as_f64().unwrap_or(0.0)
            - row["timeline_in_seconds"].as_f64().unwrap_or(0.0);
        let required = row["required_duration_seconds"]
            .as_f64()
            .unwrap_or(duration);
        if row["selected_media_asset_id"]
            .as_str()
            .is_none_or(str::is_empty)
        {
            blockers.push("missing selected media asset");
        }
        if duration + 0.25 < required {
            blockers.push("placement duration is shorter than requirement");
        }
        if row["campaign_id"].as_str().is_some_and(str::is_empty) {
            blockers.push("missing campaign association");
        }
        let validation =
            json!({"valid": blockers.is_empty(), "blockers": blockers, "checked_at": now()});
        sqlx::query("UPDATE editor_ad_slots SET validation_json = ?, updated_at = ? WHERE id = ?")
            .bind(validation.to_string())
            .bind(now())
            .bind(ad_slot_id)
            .execute(&self.pool)
            .await?;
        Ok(validation)
    }

    pub async fn lock_ad_slot(&self, ad_slot_id: &str) -> Result<Value, StoreError> {
        sqlx::query("UPDATE editor_ad_slots SET status = 'locked', review_status = 'approved', updated_at = ? WHERE id = ?")
            .bind(now())
            .bind(ad_slot_id)
            .execute(&self.pool)
            .await?;
        self.row_by_id("editor_ad_slots", ad_slot_id).await
    }

    pub async fn requirements(&self, project_id: &str) -> Result<Vec<Value>, StoreError> {
        self.list_by_project("editor_campaign_requirements", project_id)
            .await
    }

    pub async fn create_campaign_requirement(
        &self,
        project_id: &str,
        input: Value,
    ) -> Result<Value, StoreError> {
        let row_id = id();
        let now = now();
        sqlx::query(
            "INSERT INTO editor_campaign_requirements
            (id, project_id, campaign_id, title, requirement_kind, status, due_at, body_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&row_id)
        .bind(project_id)
        .bind(input["campaign_id"].as_str().unwrap_or("campaign_unlinked"))
        .bind(input["title"].as_str().unwrap_or("Campaign requirement"))
        .bind(input["requirement_kind"].as_str().unwrap_or("sponsor_deliverable"))
        .bind(input["status"].as_str().unwrap_or("draft"))
        .bind(input["due_at"].as_str())
        .bind(input["body_json"].as_object().map_or_else(|| json!({}).to_string(), |value| json!(value).to_string()))
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_campaign_requirements", &row_id)
            .await
    }

    pub async fn update_campaign_requirement(
        &self,
        requirement_id: &str,
        input: Value,
    ) -> Result<Value, StoreError> {
        let current = self
            .row_by_id("editor_campaign_requirements", requirement_id)
            .await?;
        sqlx::query(
            "UPDATE editor_campaign_requirements
            SET campaign_id = ?, title = ?, requirement_kind = ?, status = ?, due_at = ?, body_json = ?, updated_at = ?
            WHERE id = ?",
        )
        .bind(input["campaign_id"].as_str().unwrap_or_else(|| current["campaign_id"].as_str().unwrap_or("campaign_unlinked")))
        .bind(input["title"].as_str().unwrap_or_else(|| current["title"].as_str().unwrap_or("Campaign requirement")))
        .bind(input["requirement_kind"].as_str().unwrap_or_else(|| current["requirement_kind"].as_str().unwrap_or("sponsor_deliverable")))
        .bind(input["status"].as_str().unwrap_or_else(|| current["status"].as_str().unwrap_or("draft")))
        .bind(input["due_at"].as_str().or_else(|| current["due_at"].as_str()))
        .bind(input["body_json"].as_object().map_or_else(|| current["body_json"].to_string(), |value| json!(value).to_string()))
        .bind(now())
        .bind(requirement_id)
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_campaign_requirements", requirement_id)
            .await
    }

    pub async fn delete_campaign_requirement(
        &self,
        requirement_id: &str,
    ) -> Result<Value, StoreError> {
        self.delete_row(
            "editor_campaign_requirements",
            requirement_id,
            "campaign_requirement",
        )
        .await
    }

    pub async fn transcript(&self, project_id: &str) -> Result<Vec<Value>, StoreError> {
        self.list_by_project("editor_transcript_segments", project_id)
            .await
    }

    pub async fn create_transcript_segment(
        &self,
        project_id: &str,
        input: Value,
    ) -> Result<Value, StoreError> {
        let timeline_id = self.active_timeline_id(project_id).await?;
        let row_id = id();
        let now = now();
        sqlx::query(
            "INSERT INTO editor_transcript_segments
            (id, project_id, timeline_id, start_seconds, end_seconds, speaker, text, flags_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&row_id)
        .bind(project_id)
        .bind(&timeline_id)
        .bind(input["start_seconds"].as_f64().unwrap_or(0.0))
        .bind(input["end_seconds"].as_f64().unwrap_or(1.0))
        .bind(input["speaker"].as_str().unwrap_or("Speaker"))
        .bind(input["text"].as_str().unwrap_or(""))
        .bind(input["flags_json"].as_object().map_or_else(|| json!({}).to_string(), |value| json!(value).to_string()))
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_transcript_segments", &row_id).await
    }

    pub async fn update_transcript_segment(
        &self,
        segment_id: &str,
        input: Value,
    ) -> Result<Value, StoreError> {
        let current = self
            .row_by_id("editor_transcript_segments", segment_id)
            .await?;
        sqlx::query(
            "UPDATE editor_transcript_segments
            SET start_seconds = ?, end_seconds = ?, speaker = ?, text = ?, flags_json = ?, updated_at = ?
            WHERE id = ?",
        )
        .bind(input["start_seconds"].as_f64().unwrap_or_else(|| current["start_seconds"].as_f64().unwrap_or(0.0)))
        .bind(input["end_seconds"].as_f64().unwrap_or_else(|| current["end_seconds"].as_f64().unwrap_or(1.0)))
        .bind(input["speaker"].as_str().unwrap_or_else(|| current["speaker"].as_str().unwrap_or("Speaker")))
        .bind(input["text"].as_str().unwrap_or_else(|| current["text"].as_str().unwrap_or("")))
        .bind(input["flags_json"].as_object().map_or_else(|| current["flags_json"].to_string(), |value| json!(value).to_string()))
        .bind(now())
        .bind(segment_id)
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_transcript_segments", segment_id)
            .await
    }

    pub async fn delete_transcript_segment(&self, segment_id: &str) -> Result<Value, StoreError> {
        self.delete_row(
            "editor_transcript_segments",
            segment_id,
            "transcript_segment",
        )
        .await
    }

    pub async fn comments(&self, project_id: &str) -> Result<Vec<Value>, StoreError> {
        self.list_by_project("editor_comments", project_id).await
    }

    pub async fn create_comment(
        &self,
        project_id: &str,
        input: CommentInput,
    ) -> Result<Value, StoreError> {
        let row_id = id();
        let now = now();
        sqlx::query(
            "INSERT INTO editor_comments
            (id, project_id, timeline_seconds, body, visibility, author_user_id, resolved, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, 'user_creator_owner', 0, ?, ?)",
        )
        .bind(&row_id)
        .bind(project_id)
        .bind(input.timeline_seconds)
        .bind(input.body)
        .bind(input.visibility)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_comments", &row_id).await
    }

    pub async fn resolve_comment(&self, comment_id: &str) -> Result<Value, StoreError> {
        sqlx::query("UPDATE editor_comments SET resolved = 1, updated_at = ? WHERE id = ?")
            .bind(now())
            .bind(comment_id)
            .execute(&self.pool)
            .await?;
        self.row_by_id("editor_comments", comment_id).await
    }

    pub async fn update_comment(
        &self,
        comment_id: &str,
        input: Value,
    ) -> Result<Value, StoreError> {
        let current = self.row_by_id("editor_comments", comment_id).await?;
        sqlx::query(
            "UPDATE editor_comments
            SET timeline_seconds = ?, body = ?, visibility = ?, resolved = ?, updated_at = ?
            WHERE id = ?",
        )
        .bind(
            input["timeline_seconds"]
                .as_f64()
                .unwrap_or_else(|| current["timeline_seconds"].as_f64().unwrap_or(0.0)),
        )
        .bind(
            input["body"]
                .as_str()
                .unwrap_or_else(|| current["body"].as_str().unwrap_or("")),
        )
        .bind(
            input["visibility"]
                .as_str()
                .unwrap_or_else(|| current["visibility"].as_str().unwrap_or("creator_team")),
        )
        .bind(
            input["resolved"]
                .as_bool()
                .map(i64::from)
                .unwrap_or_else(|| current["resolved"].as_i64().unwrap_or(0)),
        )
        .bind(now())
        .bind(comment_id)
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_comments", comment_id).await
    }

    pub async fn delete_comment(&self, comment_id: &str) -> Result<Value, StoreError> {
        self.delete_row("editor_comments", comment_id, "comment")
            .await
    }

    pub async fn create_review_request(
        &self,
        project_id: &str,
        input: ReviewRequestInput,
    ) -> Result<Value, StoreError> {
        let timeline_id = self.active_timeline_id(project_id).await?;
        let version_id: String = sqlx::query_scalar(
            "SELECT id FROM editor_timeline_versions WHERE timeline_id = ? ORDER BY version_number DESC LIMIT 1",
        )
        .bind(&timeline_id)
        .fetch_one(&self.pool)
        .await?;
        let row_id = id();
        let now = now();
        sqlx::query(
            "INSERT INTO editor_review_requests
            (id, project_id, timeline_version_id, export_id, review_kind, campaign_id, offer_id, status, due_at, submitted_by_user_id, submitted_at, resolved_at)
            VALUES (?, ?, ?, NULL, ?, NULL, NULL, 'submitted', ?, 'user_creator_owner', ?, NULL)",
        )
        .bind(&row_id)
        .bind(project_id)
        .bind(version_id)
        .bind(input.review_kind)
        .bind(input.due_at)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_review_requests", &row_id).await
    }

    pub async fn review_requests(&self, project_id: &str) -> Result<Vec<Value>, StoreError> {
        let rows = sqlx::query(
            "SELECT * FROM editor_review_requests WHERE project_id = ? ORDER BY submitted_at ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|row| object_row(row, &JSON_COLUMNS))
            .collect()
    }

    pub async fn update_review_request(
        &self,
        review_request_id: &str,
        input: Value,
    ) -> Result<Value, StoreError> {
        let current = self
            .row_by_id("editor_review_requests", review_request_id)
            .await?;
        let status = input["status"]
            .as_str()
            .unwrap_or_else(|| current["status"].as_str().unwrap_or("submitted"));
        let resolved_at = if matches!(status, "approved" | "rejected" | "resolved") {
            input["resolved_at"]
                .as_str()
                .map(ToString::to_string)
                .unwrap_or_else(now)
        } else {
            current["resolved_at"]
                .as_str()
                .map(ToString::to_string)
                .unwrap_or_default()
        };
        sqlx::query(
            "UPDATE editor_review_requests
            SET status = ?, due_at = ?, resolved_at = NULLIF(?, '')
            WHERE id = ?",
        )
        .bind(status)
        .bind(
            input["due_at"]
                .as_str()
                .or_else(|| current["due_at"].as_str()),
        )
        .bind(resolved_at)
        .bind(review_request_id)
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_review_requests", review_request_id)
            .await
    }

    pub async fn delete_review_request(
        &self,
        review_request_id: &str,
    ) -> Result<Value, StoreError> {
        self.delete_row(
            "editor_review_requests",
            review_request_id,
            "review_request",
        )
        .await
    }

    pub async fn create_render_job(
        &self,
        project_id: &str,
        request: RenderRequest,
        validation: PublishValidation,
    ) -> Result<Value, StoreError> {
        let bundle = self
            .timeline_bundle(project_id)
            .await?
            .unwrap_or_else(|| json!({}));
        let assets = self.assets(project_id).await?;
        let has_uploaded_source = assets
            .iter()
            .any(|asset| asset["metadata_json"]["source_path"].as_str().is_some());
        let render_bundle = json!({
            "timeline": bundle["timeline"],
            "assets": assets,
            "ad_slots": bundle["ad_slots"],
            "versions": bundle["versions"]
        });
        let plan = render_plan(&render_bundle, &request, validation.clone());
        let job_id = id();
        let export_id = id();
        let status = if !validation.valid {
            "waiting_for_approval"
        } else if has_uploaded_source {
            "running"
        } else {
            "waiting_for_asset"
        };
        let progress = if has_uploaded_source && validation.valid {
            0.65
        } else if validation.valid {
            0.2
        } else {
            0.35
        };
        let now = now();
        sqlx::query(
            "INSERT INTO editor_render_jobs
            (id, project_id, timeline_id, timeline_version_id, export_kind, status, progress, render_plan_json, error_message, output_media_asset_id, created_by_user_id, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, 'user_creator_owner', ?, ?)",
        )
        .bind(&job_id)
        .bind(project_id)
        .bind(render_bundle["timeline"]["id"].as_str().unwrap_or_default())
        .bind(plan["timeline_revision_id"].as_str())
        .bind(&request.export_kind)
        .bind(status)
        .bind(progress)
        .bind(plan.to_string())
        .bind(format!("rendered_{export_id}"))
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO editor_exports
            (id, project_id, timeline_version_id, render_job_id, export_kind, media_asset_id, duration_seconds, checksum, status, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&export_id)
        .bind(project_id)
        .bind(plan["timeline_revision_id"].as_str())
        .bind(&job_id)
        .bind(&request.export_kind)
        .bind(format!("rendered_{export_id}"))
        .bind(
            render_bundle["timeline"]["duration_seconds"]
                .as_f64()
                .unwrap_or(0.0),
        )
        .bind(format!("sha256:{job_id}"))
        .bind(if has_uploaded_source && validation.valid {
            "rendering"
        } else {
            "blocked"
        })
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.render_job(&job_id).await.map(Option::unwrap)
    }

    pub async fn render_job(&self, render_job_id: &str) -> Result<Option<Value>, StoreError> {
        match sqlx::query("SELECT * FROM editor_render_jobs WHERE id = ?")
            .bind(render_job_id)
            .fetch_optional(&self.pool)
            .await?
        {
            Some(row) => Ok(Some(object_row(&row, &["render_plan_json"])?)),
            None => Ok(None),
        }
    }

    pub async fn render_jobs(&self, project_id: &str) -> Result<Vec<Value>, StoreError> {
        self.list_by_project("editor_render_jobs", project_id).await
    }

    pub async fn complete_render_job(
        &self,
        render_job_id: &str,
        package: Value,
    ) -> Result<Value, StoreError> {
        let current = self
            .render_job(render_job_id)
            .await?
            .unwrap_or_else(|| json!({}));
        let mut plan = current["render_plan_json"].clone();
        plan["package"] = package;
        sqlx::query(
            "UPDATE editor_render_jobs
            SET status = 'completed', progress = 1, render_plan_json = ?, output_media_asset_id = COALESCE(output_media_asset_id, ?), updated_at = ?
            WHERE id = ?",
        )
        .bind(plan.to_string())
        .bind(format!("rendered_{render_job_id}"))
        .bind(now())
        .bind(render_job_id)
        .execute(&self.pool)
        .await?;
        sqlx::query("UPDATE editor_exports SET status = 'ready' WHERE render_job_id = ?")
            .bind(render_job_id)
            .execute(&self.pool)
            .await?;
        self.row_by_id("editor_render_jobs", render_job_id).await
    }

    pub async fn cancel_render_job(&self, render_job_id: &str) -> Result<Value, StoreError> {
        sqlx::query("UPDATE editor_render_jobs SET status = 'cancelled', updated_at = ? WHERE id = ? AND status NOT IN ('completed', 'failed')")
            .bind(now())
            .bind(render_job_id)
            .execute(&self.pool)
            .await?;
        self.row_by_id("editor_render_jobs", render_job_id).await
    }

    pub async fn delete_render_job(&self, render_job_id: &str) -> Result<Value, StoreError> {
        let current = self.row_by_id("editor_render_jobs", render_job_id).await?;
        let exports = sqlx::query("SELECT id FROM editor_exports WHERE render_job_id = ?")
            .bind(render_job_id)
            .fetch_all(&self.pool)
            .await?;
        for row in exports {
            let export_id: String = row.get("id");
            self.delete_export(&export_id).await?;
        }
        sqlx::query("DELETE FROM editor_render_jobs WHERE id = ?")
            .bind(render_job_id)
            .execute(&self.pool)
            .await?;
        Ok(json!({"deleted": true, "render_job": current}))
    }

    pub async fn publish_export(&self, export_id: &str) -> Result<Value, StoreError> {
        let export = self.row_by_id("editor_exports", export_id).await?;
        sqlx::query(
            "UPDATE editor_exports SET status = 'published' WHERE id = ? AND status = 'ready'",
        )
        .bind(export_id)
        .execute(&self.pool)
        .await?;
        sqlx::query("UPDATE editor_projects SET status = 'published', updated_at = ? WHERE id = ?")
            .bind(now())
            .bind(export["project_id"].as_str().unwrap_or_default())
            .execute(&self.pool)
            .await?;
        self.row_by_id("editor_exports", export_id).await
    }

    pub async fn exports(&self, project_id: &str) -> Result<Vec<Value>, StoreError> {
        self.list_by_project("editor_exports", project_id).await
    }

    pub async fn export(&self, export_id: &str) -> Result<Option<Value>, StoreError> {
        match sqlx::query("SELECT * FROM editor_exports WHERE id = ?")
            .bind(export_id)
            .fetch_optional(&self.pool)
            .await?
        {
            Some(row) => Ok(Some(object_row(&row, &JSON_COLUMNS)?)),
            None => Ok(None),
        }
    }

    pub async fn update_export(&self, export_id: &str, input: Value) -> Result<Value, StoreError> {
        let current = self.row_by_id("editor_exports", export_id).await?;
        sqlx::query(
            "UPDATE editor_exports
            SET export_kind = ?, media_asset_id = ?, duration_seconds = ?, checksum = ?, status = ?
            WHERE id = ?",
        )
        .bind(
            input["export_kind"]
                .as_str()
                .unwrap_or_else(|| current["export_kind"].as_str().unwrap_or("preview_proxy")),
        )
        .bind(
            input["media_asset_id"]
                .as_str()
                .unwrap_or_else(|| current["media_asset_id"].as_str().unwrap_or("unlinked")),
        )
        .bind(
            input["duration_seconds"]
                .as_f64()
                .unwrap_or_else(|| current["duration_seconds"].as_f64().unwrap_or(0.0)),
        )
        .bind(
            input["checksum"]
                .as_str()
                .unwrap_or_else(|| current["checksum"].as_str().unwrap_or("")),
        )
        .bind(
            input["status"]
                .as_str()
                .unwrap_or_else(|| current["status"].as_str().unwrap_or("blocked")),
        )
        .bind(export_id)
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_exports", export_id).await
    }

    pub async fn delete_export(&self, export_id: &str) -> Result<Value, StoreError> {
        let current = self.row_by_id("editor_exports", export_id).await?;
        sqlx::query("DELETE FROM editor_publish_links WHERE export_id = ?")
            .bind(export_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM editor_review_requests WHERE export_id = ?")
            .bind(export_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM editor_exports WHERE id = ?")
            .bind(export_id)
            .execute(&self.pool)
            .await?;
        Ok(json!({"deleted": true, "export": current}))
    }

    pub async fn create_proof_link(&self, export_id: &str) -> Result<Value, StoreError> {
        let export = self.row_by_id("editor_exports", export_id).await?;
        let row_id = id();
        let token = short_id();
        let now = now();
        sqlx::query(
            "INSERT INTO editor_publish_links
            (id, export_id, project_id, token, audience, status, url, created_at, expires_at)
            VALUES (?, ?, ?, ?, 'advertiser', 'active', ?, ?, ?)",
        )
        .bind(&row_id)
        .bind(export_id)
        .bind(export["project_id"].as_str().unwrap_or_default())
        .bind(&token)
        .bind(format!("https://streamvanta.tv/ad-hub/proofs/{token}"))
        .bind(&now)
        .bind("2026-12-31T23:59:59Z")
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_publish_links", &row_id).await
    }

    pub async fn submit_advertiser_review(&self, export_id: &str) -> Result<Value, StoreError> {
        let export = self.row_by_id("editor_exports", export_id).await?;
        let project_id = export["project_id"].as_str().unwrap_or_default();
        let link = self.create_proof_link(export_id).await?;
        let row_id = id();
        let now = now();
        sqlx::query(
            "INSERT INTO editor_review_requests
            (id, project_id, timeline_version_id, export_id, review_kind, campaign_id, offer_id, status, due_at, submitted_by_user_id, submitted_at, resolved_at)
            VALUES (?, ?, ?, ?, 'advertiser', NULL, NULL, 'submitted_to_ad_hub', NULL, 'user_creator_owner', ?, NULL)",
        )
        .bind(&row_id)
        .bind(project_id)
        .bind(export["timeline_version_id"].as_str())
        .bind(export_id)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "UPDATE editor_projects SET status = 'advertiser_review', updated_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(json!({
            "review_request": self.row_by_id("editor_review_requests", &row_id).await?,
            "proof_link": link,
            "external_room": {
                "system": "vanta-ad-hub",
                "url": link["url"],
                "audience": "advertiser"
            }
        }))
    }

    pub async fn proof_links(&self, project_id: &str) -> Result<Vec<Value>, StoreError> {
        self.list_by_project("editor_publish_links", project_id)
            .await
    }

    pub async fn update_proof_link(
        &self,
        proof_link_id: &str,
        input: Value,
    ) -> Result<Value, StoreError> {
        let current = self
            .row_by_id("editor_publish_links", proof_link_id)
            .await?;
        sqlx::query(
            "UPDATE editor_publish_links
            SET audience = ?, status = ?, expires_at = ?
            WHERE id = ?",
        )
        .bind(
            input["audience"]
                .as_str()
                .unwrap_or_else(|| current["audience"].as_str().unwrap_or("advertiser")),
        )
        .bind(
            input["status"]
                .as_str()
                .unwrap_or_else(|| current["status"].as_str().unwrap_or("active")),
        )
        .bind(input["expires_at"].as_str().unwrap_or_else(|| {
            current["expires_at"]
                .as_str()
                .unwrap_or("2026-12-31T23:59:59Z")
        }))
        .bind(proof_link_id)
        .execute(&self.pool)
        .await?;
        self.row_by_id("editor_publish_links", proof_link_id).await
    }

    pub async fn delete_proof_link(&self, proof_link_id: &str) -> Result<Value, StoreError> {
        self.delete_row("editor_publish_links", proof_link_id, "proof_link")
            .await
    }

    async fn active_timeline_id(&self, project_id: &str) -> Result<String, StoreError> {
        Ok(
            sqlx::query_scalar("SELECT active_timeline_id FROM editor_projects WHERE id = ?")
                .bind(project_id)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    async fn ensure_ad_track(&self, timeline_id: &str) -> Result<String, StoreError> {
        self.ensure_track(timeline_id, "ad", "Sold inventory").await
    }

    async fn ensure_track(
        &self,
        timeline_id: &str,
        kind: &str,
        name: &str,
    ) -> Result<String, StoreError> {
        if let Some(track_id) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM editor_tracks WHERE timeline_id = ? AND kind = ? LIMIT 1",
        )
        .bind(timeline_id)
        .bind(kind)
        .fetch_optional(&self.pool)
        .await?
        {
            return Ok(track_id);
        }
        let track_id = id();
        let now = now();
        let order_index: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(order_index), 0) + 1 FROM editor_tracks WHERE timeline_id = ?",
        )
        .bind(timeline_id)
        .fetch_one(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO editor_tracks (id, timeline_id, kind, name, order_index, locked, muted, visible, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, 0, 0, 1, ?, ?)",
        )
        .bind(&track_id)
        .bind(timeline_id)
        .bind(kind)
        .bind(name)
        .bind(order_index)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(track_id)
    }

    async fn list_by_project(
        &self,
        table: &str,
        project_id: &str,
    ) -> Result<Vec<Value>, StoreError> {
        let sql = format!("SELECT * FROM {table} WHERE project_id = ? ORDER BY created_at ASC");
        let rows = sqlx::query(&sql)
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?;
        rows.iter()
            .map(|row| object_row(row, &JSON_COLUMNS))
            .collect()
    }

    async fn list_by_timeline(
        &self,
        table: &str,
        timeline_id: &str,
    ) -> Result<Vec<Value>, StoreError> {
        let sql = format!("SELECT * FROM {table} WHERE timeline_id = ? ORDER BY created_at ASC");
        let rows = sqlx::query(&sql)
            .bind(timeline_id)
            .fetch_all(&self.pool)
            .await?;
        rows.iter()
            .map(|row| object_row(row, &JSON_COLUMNS))
            .collect()
    }

    async fn row_by_id(&self, table: &str, row_id: &str) -> Result<Value, StoreError> {
        let sql = format!("SELECT * FROM {table} WHERE id = ?");
        let row = sqlx::query(&sql).bind(row_id).fetch_one(&self.pool).await?;
        object_row(&row, &JSON_COLUMNS)
    }

    async fn delete_row(
        &self,
        table: &str,
        row_id: &str,
        label: &str,
    ) -> Result<Value, StoreError> {
        let current = self.row_by_id(table, row_id).await?;
        let sql = format!("DELETE FROM {table} WHERE id = ?");
        sqlx::query(&sql).bind(row_id).execute(&self.pool).await?;
        Ok(json!({"deleted": true, label: current}))
    }
}

fn project_from_row(row: sqlx::sqlite::SqliteRow) -> EditorProject {
    EditorProject {
        id: row.get("id"),
        creator_id: row.get("creator_id"),
        owner_user_id: row.get("owner_user_id"),
        title: row.get("title"),
        description: row.get("description"),
        source_kind: row.get("source_kind"),
        campaign_id: row.get("campaign_id"),
        offer_id: row.get("offer_id"),
        status: row.get("status"),
        active_timeline_id: row.get("active_timeline_id"),
        updated_at: row.get("updated_at"),
    }
}

fn object_row(row: &sqlx::sqlite::SqliteRow, json_columns: &[&str]) -> Result<Value, StoreError> {
    let mut object = serde_json::Map::new();
    for column in row.columns() {
        let name = column.name();
        if json_columns.contains(&name) {
            let raw: String = row.try_get(name)?;
            object.insert(
                name.to_string(),
                serde_json::from_str(&raw).unwrap_or_else(|_| json!({})),
            );
            continue;
        }
        if let Ok(value) = row.try_get::<String, _>(name) {
            object.insert(name.to_string(), json!(value));
        } else if let Ok(value) = row.try_get::<f64, _>(name) {
            object.insert(name.to_string(), json!(value));
        } else if let Ok(value) = row.try_get::<i64, _>(name) {
            object.insert(name.to_string(), json!(value));
        }
    }
    Ok(Value::Object(object))
}

fn id() -> String {
    Uuid::new_v4().to_string()
}

fn short_id() -> String {
    Uuid::new_v4().simple().to_string()[..12].to_string()
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

const JSON_COLUMNS: [&str; 9] = [
    "body_json",
    "edl_json",
    "flags_json",
    "metadata_json",
    "render_plan_json",
    "requirements_json",
    "transform_json",
    "ui_state_json",
    "validation_json",
];

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS editor_projects (
  id TEXT PRIMARY KEY,
  creator_id TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT NOT NULL,
  source_kind TEXT NOT NULL,
  campaign_id TEXT,
  offer_id TEXT,
  status TEXT NOT NULL,
  active_timeline_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS editor_media_assets (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES editor_projects(id),
  media_asset_id TEXT NOT NULL,
  role TEXT NOT NULL,
  display_name TEXT NOT NULL,
  processing_status TEXT NOT NULL,
  rights_status TEXT NOT NULL,
  duration_seconds REAL NOT NULL,
  metadata_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS editor_timelines (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES editor_projects(id),
  name TEXT NOT NULL,
  duration_seconds REAL NOT NULL,
  frame_rate REAL NOT NULL,
  resolution_width INTEGER NOT NULL,
  resolution_height INTEGER NOT NULL,
  sample_rate INTEGER NOT NULL,
  status TEXT NOT NULL,
  ui_state_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS editor_timeline_versions (
  id TEXT PRIMARY KEY,
  timeline_id TEXT NOT NULL REFERENCES editor_timelines(id),
  version_number INTEGER NOT NULL,
  parent_version_id TEXT,
  change_summary TEXT NOT NULL,
  edl_json TEXT NOT NULL,
  created_by_user_id TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS editor_tracks (
  id TEXT PRIMARY KEY,
  timeline_id TEXT NOT NULL REFERENCES editor_timelines(id),
  kind TEXT NOT NULL,
  name TEXT NOT NULL,
  order_index INTEGER NOT NULL,
  locked INTEGER NOT NULL,
  muted INTEGER NOT NULL,
  visible INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS editor_clips (
  id TEXT PRIMARY KEY,
  timeline_id TEXT NOT NULL REFERENCES editor_timelines(id),
  track_id TEXT NOT NULL REFERENCES editor_tracks(id),
  media_asset_id TEXT NOT NULL,
  label TEXT NOT NULL,
  source_in_seconds REAL NOT NULL,
  source_out_seconds REAL NOT NULL,
  timeline_in_seconds REAL NOT NULL,
  timeline_out_seconds REAL NOT NULL,
  speed REAL NOT NULL,
  volume REAL NOT NULL,
  opacity REAL NOT NULL,
  metadata_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS editor_ad_slots (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES editor_projects(id),
  timeline_id TEXT NOT NULL REFERENCES editor_timelines(id),
  track_id TEXT NOT NULL REFERENCES editor_tracks(id),
  label TEXT NOT NULL,
  campaign_id TEXT,
  offer_id TEXT,
  package_id TEXT,
  advertiser_id TEXT,
  placement_type TEXT NOT NULL,
  insertion_mode TEXT NOT NULL,
  timeline_in_seconds REAL NOT NULL,
  timeline_out_seconds REAL NOT NULL,
  required_duration_seconds REAL,
  selected_media_asset_id TEXT,
  status TEXT NOT NULL,
  review_status TEXT NOT NULL,
  measurement_key TEXT NOT NULL,
  requirements_json TEXT NOT NULL,
  validation_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS editor_campaign_requirements (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES editor_projects(id),
  campaign_id TEXT NOT NULL,
  title TEXT NOT NULL,
  requirement_kind TEXT NOT NULL,
  status TEXT NOT NULL,
  due_at TEXT,
  body_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS editor_transcript_segments (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES editor_projects(id),
  timeline_id TEXT NOT NULL REFERENCES editor_timelines(id),
  start_seconds REAL NOT NULL,
  end_seconds REAL NOT NULL,
  speaker TEXT NOT NULL,
  text TEXT NOT NULL,
  flags_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS editor_comments (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES editor_projects(id),
  timeline_seconds REAL NOT NULL,
  body TEXT NOT NULL,
  visibility TEXT NOT NULL,
  author_user_id TEXT NOT NULL,
  resolved INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS editor_review_requests (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES editor_projects(id),
  timeline_version_id TEXT NOT NULL REFERENCES editor_timeline_versions(id),
  export_id TEXT,
  review_kind TEXT NOT NULL,
  campaign_id TEXT,
  offer_id TEXT,
  status TEXT NOT NULL,
  due_at TEXT,
  submitted_by_user_id TEXT NOT NULL,
  submitted_at TEXT NOT NULL,
  resolved_at TEXT
);
CREATE TABLE IF NOT EXISTS editor_render_jobs (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES editor_projects(id),
  timeline_id TEXT NOT NULL REFERENCES editor_timelines(id),
  timeline_version_id TEXT,
  export_kind TEXT NOT NULL,
  status TEXT NOT NULL,
  progress REAL NOT NULL,
  render_plan_json TEXT NOT NULL,
  error_message TEXT,
  output_media_asset_id TEXT,
  created_by_user_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS editor_exports (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES editor_projects(id),
  timeline_version_id TEXT,
  render_job_id TEXT NOT NULL REFERENCES editor_render_jobs(id),
  export_kind TEXT NOT NULL,
  media_asset_id TEXT NOT NULL,
  duration_seconds REAL NOT NULL,
  checksum TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS editor_publish_links (
  id TEXT PRIMARY KEY,
  export_id TEXT NOT NULL REFERENCES editor_exports(id),
  project_id TEXT NOT NULL REFERENCES editor_projects(id),
  token TEXT NOT NULL,
  audience TEXT NOT NULL,
  status TEXT NOT NULL,
  url TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL
);
"#;
