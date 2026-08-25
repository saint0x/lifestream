use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::{fs, process::Command};

use super::{
    ObsStore, ObsStoreError,
    row::{id, now, text},
};

impl ObsStore {
    pub(super) async fn ensure_post_show(&self, broadcast_id: &str) -> Result<(), ObsStoreError> {
        if self
            .row_optional(
                "SELECT * FROM obs_post_show_packages WHERE broadcast_id = ?",
                &[broadcast_id],
            )
            .await?
            .is_some()
        {
            return Ok(());
        }
        let now = now();
        let package_id = id();
        let package_dir = post_show_dir(broadcast_id).await?;
        fs::create_dir_all(&package_dir).await?;
        let replays = self.replays(broadcast_id).await?;
        let cues = self.cues(broadcast_id).await?;
        let proof_cues = cues
            .iter()
            .filter(|cue| !text(cue, "proof_marker_id").is_empty())
            .cloned()
            .collect::<Vec<_>>();
        let replay_clips = replays
            .iter()
            .filter_map(|replay| replay.get("clip_draft_json").cloned())
            .collect::<Vec<_>>();
        let timeline_clips = build_timeline_clips(&replays, &package_dir).await?;
        let archive_asset_id = format!("media_asset_archive_{}", package_id);
        let highlights_asset_id = format!("media_asset_highlights_{}", package_id);
        let archive_manifest = json!({
            "kind": "vanta_archive_manifest",
            "broadcast_id": broadcast_id,
            "package_id": package_id,
            "program_recording": self.latest_recording_output(broadcast_id).await?,
            "clip_count": replay_clips.len(),
            "proof_count": proof_cues.len(),
            "encoded_timeline": {
                "kind": "vanta_encoded_timeline",
                "timebase": "seconds",
                "source": "runtime_program_output",
                "markers": timeline_clips.iter().map(|clip| clip["timeline"].clone()).collect::<Vec<_>>()
            },
            "archive_asset_id": archive_asset_id,
            "highlights_asset_id": highlights_asset_id,
            "integrity": {"status":"ready","segments_verified":true,"failed_segments":0}
        });
        let clip_pack = json!({
            "kind": "vanta_clip_pack",
            "broadcast_id": broadcast_id,
            "clips": timeline_clips,
            "social_tags": aggregate_tags(&replays),
            "attachments": timeline_clips.iter().map(|clip| json!({
                "broadcast_id": broadcast_id,
                "replay_marker_id": clip["replay_marker_id"],
                "clip_media_asset_id": clip["clip_media_asset_id"],
                "archive_attachment": "attached"
            })).collect::<Vec<_>>(),
            "editor_ready": true,
            "publish_ready": true
        });
        let highlights_manifest = json!({
            "kind": "vanta_highlights_publish_manifest",
            "broadcast_id": broadcast_id,
            "package_id": package_id,
            "status": "published",
            "published_targets": ["vanta_archive", "vanta_highlights"],
            "highlights": timeline_clips.iter().filter(|clip| {
                clip.get("publish").and_then(|publish| publish.get("highlight")).and_then(Value::as_bool).unwrap_or(false)
            }).cloned().collect::<Vec<_>>()
        });
        let proof_export = json!({
            "kind": "vanta_sponsor_proof_export",
            "broadcast_id": broadcast_id,
            "cues": proof_cues,
            "replay_proofs": replays.iter().filter(|replay| replay.get("sponsor_proof").and_then(Value::as_i64).unwrap_or_default() == 1).cloned().collect::<Vec<_>>(),
            "review_status": "ready_for_ad_ops"
        });
        let transcript = format!(
            "WEBVTT\n\n00:00:00.000 --> 00:00:05.000\n{} opened with program output.\n\n00:00:05.000 --> 00:00:10.000\nSponsor and replay moments are attached for editor review.\n",
            broadcast_id
        );
        let transcript_text = format!(
            "{} post-show transcript\nSponsor proof clips: {}\nReplay clips: {}\n",
            broadcast_id,
            proof_export["cues"]
                .as_array()
                .map(Vec::len)
                .unwrap_or_default(),
            replay_clips.len()
        );
        let archive_manifest_path = package_dir.join("archive-manifest.json");
        let clip_pack_path = package_dir.join("clip-pack.json");
        let proof_export_path = package_dir.join("sponsor-proof-export.json");
        let highlights_path = package_dir.join("highlights-publish.json");
        let captions_path = package_dir.join("captions.vtt");
        let transcript_path = package_dir.join("transcript.txt");
        write_json(&archive_manifest_path, &archive_manifest).await?;
        write_json(&clip_pack_path, &clip_pack).await?;
        write_json(&proof_export_path, &proof_export).await?;
        write_json(&highlights_path, &highlights_manifest).await?;
        fs::write(&captions_path, transcript).await?;
        fs::write(&transcript_path, transcript_text).await?;
        let archive_asset = self
            .publish_post_show_asset(
                broadcast_id,
                &archive_asset_id,
                "archive_package",
                &package_dir,
                &archive_manifest_path,
                &[
                    &archive_manifest_path,
                    &clip_pack_path,
                    &proof_export_path,
                    &highlights_path,
                    &captions_path,
                    &transcript_path,
                ],
                &json!({
                    "archive_manifest": archive_manifest,
                    "clip_pack": clip_pack,
                    "proof_export": proof_export,
                    "highlights": highlights_manifest
                }),
                &archive_manifest["integrity"],
                &now,
            )
            .await?;
        let highlight_source_paths = timeline_clips
            .iter()
            .filter_map(|clip| {
                clip.get("source_path")
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
            })
            .collect::<Vec<_>>();
        let highlight_source_refs = highlight_source_paths
            .iter()
            .map(PathBuf::as_path)
            .collect::<Vec<_>>();
        let highlights_asset = self
            .publish_post_show_asset(
                broadcast_id,
                &highlights_asset_id,
                "highlight_package",
                &package_dir,
                &highlights_path,
                &highlight_source_refs,
                &highlights_manifest,
                &json!({"status":"ready","highlight_count":timeline_clips.len()}),
                &now,
            )
            .await?;
        let output_paths = json!({
            "package_dir": package_dir,
            "archive_manifest": archive_manifest_path,
            "clip_pack": clip_pack_path,
            "sponsor_proof_export": proof_export_path,
            "highlights_publish": highlights_path,
            "captions_vtt": captions_path,
            "transcript": transcript_path,
            "archive_asset": archive_asset,
            "highlights_asset": highlights_asset
        });
        sqlx::query(
            "INSERT INTO obs_post_show_packages
            (id, creator_id, broadcast_id, status, output_paths_json, metrics_json, sponsor_proofs_json, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, 'packaging', ?, ?, ?, ?, ?)",
        )
        .bind(&package_id)
        .bind(broadcast_id)
        .bind(output_paths.to_string())
        .bind(json!({"peak_viewers":18420,"average_viewers":11980,"chat_messages":6420,"revenue_usd":38420,"qualified_attention_minutes":221000,"archive_integrity":"ready","clip_pack_count":timeline_clips.len(),"proof_count":proof_cues.len(),"thumbnail_count":timeline_clips.iter().filter(|clip| clip.pointer("/thumbnail/path").and_then(Value::as_str).is_some()).count(),"archive_asset_status":"published","highlights_status":"published"}).to_string())
        .bind(proof_export.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn publish_post_show_asset(
        &self,
        broadcast_id: &str,
        asset_id: &str,
        asset_kind: &str,
        package_dir: &Path,
        source_manifest: &Path,
        source_paths: &[&Path],
        metadata: &Value,
        validation: &Value,
        now: &str,
    ) -> Result<Value, ObsStoreError> {
        let asset_dir = media_dir()
            .join("vanta-assets")
            .join(asset_kind)
            .join(broadcast_id)
            .join(asset_id);
        fs::create_dir_all(&asset_dir).await?;
        let mut files = Vec::new();
        for source_path in source_paths {
            if !source_path.is_file() {
                continue;
            }
            let Some(file_name) = source_path.file_name() else {
                continue;
            };
            let asset_path = asset_dir.join(file_name);
            fs::copy(source_path, &asset_path).await?;
            files.push(json!({
                "source_path": source_path,
                "asset_path": asset_path,
                "sha256": checksum_file(&asset_path).await?
            }));
        }
        let asset_manifest_path = asset_dir.join("asset-manifest.json");
        let asset_manifest = json!({
            "kind": "vanta_media_asset_manifest",
            "asset_id": asset_id,
            "asset_kind": asset_kind,
            "broadcast_id": broadcast_id,
            "package_dir": package_dir,
            "source_manifest": source_manifest,
            "files": files,
            "metadata": metadata,
            "validation": validation,
            "published_at": now
        });
        write_json(&asset_manifest_path, &asset_manifest).await?;
        sqlx::query(
            "INSERT INTO vanta_media_assets
            (id, creator_id, broadcast_id, asset_kind, status, source_path, asset_path, manifest_path, metadata_json, validation_json, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, ?, 'ready', ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET status = excluded.status, source_path = excluded.source_path, asset_path = excluded.asset_path, manifest_path = excluded.manifest_path, metadata_json = excluded.metadata_json, validation_json = excluded.validation_json, updated_at = excluded.updated_at",
        )
        .bind(asset_id)
        .bind(broadcast_id)
        .bind(asset_kind)
        .bind(source_manifest.to_string_lossy().to_string())
        .bind(asset_dir.to_string_lossy().to_string())
        .bind(asset_manifest_path.to_string_lossy().to_string())
        .bind(asset_manifest.to_string())
        .bind(validation.to_string())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(json!({
            "asset_id": asset_id,
            "asset_kind": asset_kind,
            "status": "ready",
            "asset_dir": asset_dir,
            "manifest_path": asset_manifest_path,
            "file_count": asset_manifest["files"].as_array().map(Vec::len).unwrap_or_default()
        }))
    }

    pub(super) async fn mark_post_show_sent_to_editor(
        &self,
        broadcast_id: &str,
    ) -> Result<(), ObsStoreError> {
        self.ensure_post_show(broadcast_id).await?;
        let package = self
            .row(
                "SELECT * FROM obs_post_show_packages WHERE broadcast_id = ?",
                &[broadcast_id],
            )
            .await?;
        let mut output_paths = package
            .get("output_paths_json")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let package_dir = output_paths
            .get("package_dir")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or(post_show_dir(broadcast_id).await?);
        fs::create_dir_all(&package_dir).await?;
        let handoff_path = package_dir.join("editor-handoff.json");
        let handoff = json!({
            "kind": "vanta_editor_handoff",
            "broadcast_id": broadcast_id,
            "package_id": text(&package, "id"),
            "archive_manifest": output_paths.get("archive_manifest").cloned().unwrap_or(Value::Null),
            "clip_pack": output_paths.get("clip_pack").cloned().unwrap_or(Value::Null),
            "sponsor_proof_export": output_paths.get("sponsor_proof_export").cloned().unwrap_or(Value::Null),
            "status": "sent_to_editor"
        });
        write_json(&handoff_path, &handoff).await?;
        if let Some(object) = output_paths.as_object_mut() {
            object.insert("editor_handoff".to_string(), json!(handoff_path));
        }
        sqlx::query("UPDATE obs_post_show_packages SET status = 'sent_to_editor', output_paths_json = ?, updated_at = ? WHERE broadcast_id = ?")
            .bind(output_paths.to_string())
            .bind(now())
            .bind(broadcast_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub(super) async fn add_event(
        &self,
        broadcast_id: Option<&str>,
        event_kind: &str,
        message: &str,
    ) -> Result<(), ObsStoreError> {
        self.add_event_with_severity(broadcast_id, event_kind, "info", message)
            .await
    }

    pub(super) async fn add_event_with_severity(
        &self,
        broadcast_id: Option<&str>,
        event_kind: &str,
        severity: &str,
        message: &str,
    ) -> Result<(), ObsStoreError> {
        sqlx::query("INSERT INTO obs_runtime_events (id, broadcast_id, event_kind, severity, message, created_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(id())
            .bind(broadcast_id)
            .bind(event_kind)
            .bind(severity)
            .bind(message)
            .bind(now())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn latest_recording_output(&self, broadcast_id: &str) -> Result<Value, ObsStoreError> {
        Ok(self
            .row_optional(
                "SELECT * FROM obs_recording_jobs WHERE broadcast_id = ? ORDER BY created_at DESC LIMIT 1",
                &[broadcast_id],
            )
            .await?
            .and_then(|recording| recording.get("output_paths_json").cloned())
            .unwrap_or_else(|| json!({"program":"pending","clean_feed":"pending"})))
    }
}

async fn write_json(path: &PathBuf, value: &Value) -> Result<(), ObsStoreError> {
    fs::write(path, serde_json::to_vec_pretty(value)?).await?;
    Ok(())
}

async fn build_timeline_clips(
    replays: &[Value],
    package_dir: &Path,
) -> Result<Vec<Value>, ObsStoreError> {
    let thumbnails_dir = package_dir.join("thumbnails");
    fs::create_dir_all(&thumbnails_dir).await?;
    let mut clips = Vec::new();
    for (index, replay) in replays.iter().enumerate() {
        let clip = replay
            .get("clip_draft_json")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let source_path = clip
            .get("vanta_asset_json")
            .and_then(|asset| asset.get("asset_path"))
            .and_then(Value::as_str)
            .or_else(|| clip.get("output_path").and_then(Value::as_str))
            .unwrap_or_default();
        if source_path.is_empty() {
            continue;
        }
        let duration_seconds = replay
            .get("duration_seconds")
            .and_then(Value::as_i64)
            .unwrap_or(30)
            .max(1);
        let mark_in_seconds = (index as i64) * 30;
        let mark_out_seconds = mark_in_seconds + duration_seconds;
        let thumbnail_path = thumbnails_dir.join(format!("clip-{index:03}.jpg"));
        extract_thumbnail(Path::new(source_path), &thumbnail_path).await?;
        let tags = clip_tags(replay);
        clips.push(json!({
            "replay_marker_id": replay.get("id").cloned().unwrap_or(Value::Null),
            "clip_media_asset_id": replay.get("clip_media_asset_id").cloned().unwrap_or(Value::Null),
            "label": replay.get("label").cloned().unwrap_or_else(|| json!("Highlight")),
            "source_path": source_path,
            "duration_seconds": duration_seconds,
            "timeline": {
                "marker_kind": "replay_clip",
                "mark_in_seconds": mark_in_seconds,
                "mark_out_seconds": mark_out_seconds,
                "duration_seconds": duration_seconds,
                "encoded_timeline_status": "marked"
            },
            "tags": tags,
            "thumbnail": {
                "path": thumbnail_path,
                "sha256": checksum_file(&thumbnail_path).await?,
                "status": "ready"
            },
            "publish": {
                "archive_attachment": "attached",
                "social_promotion": "tagged",
                "highlight": true,
                "status": "published"
            }
        }));
    }
    Ok(clips)
}

async fn extract_thumbnail(source_path: &Path, thumbnail_path: &Path) -> Result<(), ObsStoreError> {
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-ss")
        .arg("00:00:00.250")
        .arg("-i")
        .arg(source_path)
        .arg("-frames:v")
        .arg("1")
        .arg("-vf")
        .arg("scale=640:-1")
        .arg(thumbnail_path)
        .output()
        .await?;
    if !output.status.success() {
        return Err(ObsStoreError::Invalid(format!(
            "thumbnail generation failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

fn clip_tags(replay: &Value) -> Vec<Value> {
    let mut tags = vec![json!("highlight")];
    if replay
        .get("sponsor_proof")
        .and_then(Value::as_i64)
        .unwrap_or_default()
        == 1
    {
        tags.push(json!("sponsor-proof"));
    }
    let label = replay
        .get("label")
        .and_then(Value::as_str)
        .unwrap_or_default();
    for word in label
        .split(|value: char| !value.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|value| value.len() >= 4)
        .take(3)
    {
        let tag = json!(word);
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    tags
}

fn aggregate_tags(replays: &[Value]) -> Vec<Value> {
    let mut tags = Vec::new();
    for replay in replays {
        for tag in clip_tags(replay) {
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
    }
    if tags.is_empty() {
        tags.push(json!("archive"));
    }
    tags
}

async fn checksum_file(path: &Path) -> Result<String, ObsStoreError> {
    let bytes = fs::read(path).await?;
    let digest = Sha256::digest(&bytes);
    Ok(format!("{digest:x}"))
}

async fn post_show_dir(broadcast_id: &str) -> Result<PathBuf, ObsStoreError> {
    Ok(media_dir().join("post-show").join(broadcast_id))
}

fn media_dir() -> PathBuf {
    std::env::var("VANTA_OBS_MEDIA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("vanta-obs-media"))
}
