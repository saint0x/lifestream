use std::path::{Path, PathBuf};

use chrono::Utc;
use serde_json::{Value, json};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use thiserror::Error;
use tokio::fs;

#[derive(Debug, Error)]
pub enum IntegrationError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct VantaIntegrations {
    media_pool: Option<SqlitePool>,
    ad_hub_outbox: PathBuf,
}

impl VantaIntegrations {
    pub async fn new(
        media_database: Option<PathBuf>,
        ad_hub_outbox: PathBuf,
    ) -> Result<Self, IntegrationError> {
        fs::create_dir_all(&ad_hub_outbox).await?;
        let media_pool = match media_database {
            Some(path) => {
                if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
                    fs::create_dir_all(parent).await?;
                }
                let pool = SqlitePoolOptions::new()
                    .max_connections(4)
                    .connect_with(
                        SqliteConnectOptions::new()
                            .filename(path)
                            .create_if_missing(true),
                    )
                    .await?;
                Self::prepare_media_pipeline(&pool).await?;
                Some(pool)
            }
            None => None,
        };

        Ok(Self {
            media_pool,
            ad_hub_outbox,
        })
    }

    pub async fn publish_export(
        &self,
        export: &Value,
        project_bundle: &Value,
        render_job: &Value,
    ) -> Result<Value, IntegrationError> {
        let upload_id = format!("upl-editor-{}", export["id"].as_str().unwrap_or("export"));
        let upload_job_id = format!("job-editor-{}", export["id"].as_str().unwrap_or("export"));
        let asset_id = export["media_asset_id"]
            .as_str()
            .unwrap_or("editor-rendered-export");
        let creator_id = project_bundle["project"]["creator_id"]
            .as_str()
            .unwrap_or("creator_vanta_originals");
        let title = project_bundle["project"]["title"]
            .as_str()
            .unwrap_or("Vanta editor export");
        let now = Utc::now().to_rfc3339();
        let package = &render_job["render_plan_json"]["package"];
        let manifest_path = package["manifest_path"].as_str().unwrap_or_default();
        let source_path = package["source_path"].as_str().unwrap_or_default();
        let published_content_id = format!(
            "content-editor-{}",
            export["id"].as_str().unwrap_or("export")
        );

        if let Some(pool) = &self.media_pool {
            self.upsert_media_pipeline_rows(
                pool,
                MediaPipelinePublish {
                    creator_id,
                    upload_id: &upload_id,
                    upload_job_id: &upload_job_id,
                    asset_id,
                    title,
                    source_path,
                    manifest_path,
                    duration_sec: export["duration_seconds"].as_f64().unwrap_or_default(),
                    checksum: export["checksum"].as_str().unwrap_or_default(),
                    published_content_id: &published_content_id,
                    now: &now,
                },
            )
            .await?;
        }

        Ok(json!({
            "system": "vanta-media-pipeline",
            "mode": if self.media_pool.is_some() { "sqlite-upsert" } else { "not-configured" },
            "upload_id": upload_id,
            "upload_job_id": upload_job_id,
            "media_asset_id": asset_id,
            "published_content_id": published_content_id,
            "playback_manifest": manifest_path
        }))
    }

    pub async fn submit_advertiser_review(
        &self,
        submission: &Value,
        export: &Value,
        project_bundle: &Value,
    ) -> Result<Value, IntegrationError> {
        let review = &submission["review_request"];
        let proof = &submission["proof_link"];
        let room_id = format!("room-{}", review["id"].as_str().unwrap_or("review"));
        let project = &project_bundle["project"];
        let now = Utc::now().to_rfc3339();
        let room = json!({
            "id": room_id,
            "system": "vanta-ad-hub",
            "campaignId": project["campaign_id"],
            "offerId": project["offer_id"],
            "projectId": project["id"],
            "projectTitle": project["title"],
            "exportId": export["id"],
            "reviewRequestId": review["id"],
            "status": review["status"],
            "submissionUrl": proof["url"],
            "submittedAt": review["submitted_at"],
            "createdAt": now,
            "audience": proof["audience"],
            "brief": project_bundle["requirements"],
            "proof": proof
        });

        let path = self
            .ad_hub_outbox
            .join(format!("{}.json", sanitize_file_component(&room_id)));
        fs::write(&path, serde_json::to_vec_pretty(&room)?).await?;

        if let Some(pool) = &self.media_pool {
            self.insert_ad_marketplace_submission(pool, project_bundle, proof, review, &now)
                .await?;
        }

        Ok(json!({
            "system": "vanta-ad-hub",
            "mode": if self.media_pool.is_some() { "sqlite-submission-and-outbox" } else { "outbox" },
            "room_id": room_id,
            "outbox_path": path,
            "url": proof["url"]
        }))
    }

    async fn prepare_media_pipeline(pool: &SqlitePool) -> Result<(), IntegrationError> {
        for statement in MEDIA_PIPELINE_SCHEMA
            .split(';')
            .map(str::trim)
            .filter(|statement| !statement.is_empty())
        {
            sqlx::query(statement).execute(pool).await?;
        }
        Ok(())
    }

    async fn upsert_media_pipeline_rows(
        &self,
        pool: &SqlitePool,
        publish: MediaPipelinePublish<'_>,
    ) -> Result<(), IntegrationError> {
        let source_size = file_size(publish.source_path).await.unwrap_or_default();
        sqlx::query(
            "INSERT INTO upload_jobs
            (id, creator_id, upload_id, series_id, kind, source_type, status, title, intended_visibility, bytes_expected, bytes_received, storage_key, created_at, updated_at, published_content_id)
            VALUES (?, ?, ?, NULL, ?, 'vanta_editor_render', 'published', ?, 'unlisted', ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET status = 'published', upload_id = excluded.upload_id, published_content_id = excluded.published_content_id, updated_at = excluded.updated_at",
        )
        .bind(publish.upload_job_id)
        .bind(publish.creator_id)
        .bind(publish.upload_id)
        .bind("video")
        .bind(publish.title)
        .bind(source_size)
        .bind(source_size)
        .bind(publish.source_path)
        .bind(publish.now)
        .bind(publish.now)
        .bind(publish.published_content_id)
        .execute(pool)
        .await?;

        sqlx::query(
            "INSERT INTO media_assets
            (id, creator_id, upload_job_id, upload_id, series_id, kind, title, status, visibility, source_relative_path, poster_relative_path, playback_relative_path, mime_type, checksum_sha256, container_format, file_size_bytes, duration_sec, width, height, frame_rate, video_codec, audio_codec, has_video, has_audio, created_at, updated_at, processed_at, published_content_id)
            VALUES (?, ?, ?, ?, NULL, 'video', ?, 'published', 'unlisted', ?, NULL, ?, 'application/vnd.apple.mpegurl', ?, 'hls', ?, ?, NULL, NULL, NULL, 'h264', 'aac', 1, 1, ?, ?, ?, ?)
            ON CONFLICT(upload_job_id) DO UPDATE SET status = 'published', playback_relative_path = excluded.playback_relative_path, published_content_id = excluded.published_content_id, updated_at = excluded.updated_at",
        )
        .bind(publish.asset_id)
        .bind(publish.creator_id)
        .bind(publish.upload_job_id)
        .bind(publish.upload_id)
        .bind(publish.title)
        .bind(publish.source_path)
        .bind(publish.manifest_path)
        .bind(publish.checksum)
        .bind(source_size)
        .bind(publish.duration_sec)
        .bind(publish.now)
        .bind(publish.now)
        .bind(publish.now)
        .bind(publish.published_content_id)
        .execute(pool)
        .await?;

        if !publish.manifest_path.is_empty() {
            sqlx::query(
                "INSERT OR REPLACE INTO media_asset_variants
                (id, asset_id, variant_type, label, relative_path, mime_type, width, height, bitrate_bps, file_size_bytes, is_default, created_at)
                VALUES (?, ?, 'hls', 'master', ?, 'application/vnd.apple.mpegurl', NULL, NULL, NULL, ?, 1, ?)",
            )
            .bind(format!("var-editor-master-{}", publish.asset_id))
            .bind(publish.asset_id)
            .bind(publish.manifest_path)
            .bind(file_size(publish.manifest_path).await.unwrap_or_default())
            .bind(publish.now)
            .execute(pool)
            .await?;
        }

        Ok(())
    }

    async fn insert_ad_marketplace_submission(
        &self,
        pool: &SqlitePool,
        project_bundle: &Value,
        proof: &Value,
        review: &Value,
        now: &str,
    ) -> Result<(), IntegrationError> {
        let offer_id = project_bundle["project"]["offer_id"]
            .as_str()
            .unwrap_or("offer_editor_unlinked");
        let creator_id = project_bundle["project"]["creator_id"]
            .as_str()
            .unwrap_or("creator_vanta_originals");
        let submission_id = format!("sub-editor-{}", review["id"].as_str().unwrap_or("review"));

        if table_exists(pool, "ad_marketplace_submissions").await? {
            sqlx::query(
                "INSERT OR REPLACE INTO ad_marketplace_submissions
                (id, offer_id, creator_id, submission_url, notes, status, submitted_at, reviewed_at, advertiser_feedback, revision_due_at)
                VALUES (?, ?, ?, ?, ?, 'review_pending', ?, NULL, NULL, NULL)",
            )
            .bind(submission_id)
            .bind(offer_id)
            .bind(creator_id)
            .bind(proof["url"].as_str().unwrap_or_default())
            .bind("Submitted from Vanta Editor advertiser review workflow.")
            .bind(now)
            .execute(pool)
            .await?;
        }
        Ok(())
    }
}

struct MediaPipelinePublish<'a> {
    creator_id: &'a str,
    upload_id: &'a str,
    upload_job_id: &'a str,
    asset_id: &'a str,
    title: &'a str,
    source_path: &'a str,
    manifest_path: &'a str,
    duration_sec: f64,
    checksum: &'a str,
    published_content_id: &'a str,
    now: &'a str,
}

async fn file_size(path: &str) -> Result<i64, std::io::Error> {
    if path.is_empty() {
        return Ok(0);
    }
    Ok(fs::metadata(Path::new(path)).await?.len() as i64)
}

async fn table_exists(pool: &SqlitePool, name: &str) -> Result<bool, sqlx::Error> {
    let row = sqlx::query("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ? LIMIT 1")
        .bind(name)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

fn sanitize_file_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

const MEDIA_PIPELINE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS upload_jobs (
  id TEXT PRIMARY KEY,
  creator_id TEXT NOT NULL,
  upload_id TEXT,
  series_id TEXT,
  kind TEXT NOT NULL,
  source_type TEXT NOT NULL,
  status TEXT NOT NULL,
  title TEXT NOT NULL,
  intended_visibility TEXT NOT NULL,
  bytes_expected INTEGER NOT NULL,
  bytes_received INTEGER NOT NULL,
  storage_key TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  published_content_id TEXT
);
CREATE TABLE IF NOT EXISTS media_assets (
  id TEXT PRIMARY KEY,
  creator_id TEXT NOT NULL,
  upload_job_id TEXT NOT NULL UNIQUE,
  upload_id TEXT,
  series_id TEXT,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  status TEXT NOT NULL,
  visibility TEXT NOT NULL,
  source_relative_path TEXT NOT NULL,
  poster_relative_path TEXT,
  playback_relative_path TEXT,
  mime_type TEXT NOT NULL,
  checksum_sha256 TEXT,
  container_format TEXT,
  file_size_bytes INTEGER NOT NULL,
  duration_sec REAL NOT NULL,
  width INTEGER,
  height INTEGER,
  frame_rate REAL,
  video_codec TEXT,
  audio_codec TEXT,
  has_video INTEGER NOT NULL,
  has_audio INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  processed_at TEXT,
  published_content_id TEXT
);
CREATE TABLE IF NOT EXISTS media_asset_variants (
  id TEXT PRIMARY KEY,
  asset_id TEXT NOT NULL,
  variant_type TEXT NOT NULL,
  label TEXT NOT NULL,
  relative_path TEXT NOT NULL,
  mime_type TEXT NOT NULL,
  width INTEGER,
  height INTEGER,
  bitrate_bps INTEGER,
  file_size_bytes INTEGER NOT NULL,
  is_default INTEGER NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS ad_marketplace_submissions (
  id TEXT PRIMARY KEY,
  offer_id TEXT NOT NULL,
  creator_id TEXT NOT NULL,
  submission_url TEXT NOT NULL,
  notes TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'review_pending',
  submitted_at TEXT NOT NULL,
  reviewed_at TEXT,
  advertiser_feedback TEXT,
  revision_due_at TEXT
);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[tokio::test]
    async fn writes_media_pipeline_rows_and_ad_hub_room() {
        let root = std::env::temp_dir().join(format!(
            "vanta-editor-integrations-{}",
            Uuid::new_v4().simple()
        ));
        let db = root.join("pipeline.db");
        let outbox = root.join("ad-hub");
        fs::create_dir_all(&root).await.unwrap();
        let source = root.join("source.mp4");
        let manifest = root.join("master.m3u8");
        fs::write(&source, b"source").await.unwrap();
        fs::write(&manifest, b"#EXTM3U").await.unwrap();

        let integrations = VantaIntegrations::new(Some(db.clone()), outbox.clone())
            .await
            .unwrap();
        let export = json!({
            "id": "export_1",
            "media_asset_id": "asset_1",
            "duration_seconds": 12.0,
            "checksum": "sha256:test"
        });
        let project = json!({
            "project": {
                "id": "project_1",
                "creator_id": "creator_1",
                "title": "Editor publish",
                "campaign_id": "campaign_1",
                "offer_id": "offer_1"
            },
            "requirements": []
        });
        let render = json!({
            "render_plan_json": {
                "package": {
                    "source_path": source,
                    "manifest_path": manifest
                }
            }
        });

        let pipeline = integrations
            .publish_export(&export, &project, &render)
            .await
            .unwrap();
        assert_eq!(pipeline["mode"], "sqlite-upsert");

        let submission = json!({
            "review_request": {
                "id": "review_1",
                "status": "submitted_to_ad_hub",
                "submitted_at": "2026-08-24T00:00:00Z"
            },
            "proof_link": {
                "url": "https://streamvanta.tv/ad-hub/proofs/test",
                "audience": "advertiser"
            }
        });
        let room = integrations
            .submit_advertiser_review(&submission, &export, &project)
            .await
            .unwrap();
        assert_eq!(room["mode"], "sqlite-submission-and-outbox");
        assert!(outbox.join("room-review_1.json").exists());

        let pool = SqlitePoolOptions::new()
            .connect_with(SqliteConnectOptions::new().filename(db))
            .await
            .unwrap();
        let published: String =
            sqlx::query_scalar("SELECT status FROM media_assets WHERE id = 'asset_1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(published, "published");
        let submission_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM ad_marketplace_submissions")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(submission_count, 1);
    }
}
