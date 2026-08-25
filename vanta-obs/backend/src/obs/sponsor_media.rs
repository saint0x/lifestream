use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::{fs, process::Command};

use super::store::ObsStoreError;

pub struct SponsorProofMediaInput<'a> {
    pub broadcast_id: &'a str,
    pub inventory_id: &'a str,
    pub cue_id: &'a str,
    pub proof_id: &'a str,
    pub proof_marker_id: &'a str,
    pub media_time_seconds: f64,
    pub source_media_path: Option<&'a str>,
}

pub struct SponsorProofMediaArtifact {
    pub asset_id: String,
    pub asset_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub artifact_json: Value,
    pub validation_json: Value,
}

pub async fn capture_proof_media(
    input: SponsorProofMediaInput<'_>,
) -> Result<SponsorProofMediaArtifact, ObsStoreError> {
    let asset_id = format!("media_asset_{}", input.proof_id);
    let asset_dir = media_dir()
        .join("vanta-assets")
        .join("sponsor-proof")
        .join(input.broadcast_id)
        .join(&asset_id);
    fs::create_dir_all(&asset_dir).await?;
    let clip_path = asset_dir.join("proof-clip.mp4");
    let thumbnail_path = asset_dir.join("proof-frame.jpg");
    let manifest_path = asset_dir.join("asset-manifest.json");
    remove_if_exists(&clip_path).await?;
    remove_if_exists(&thumbnail_path).await?;
    remove_if_exists(&manifest_path).await?;

    if let Some(source_path) = input
        .source_media_path
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_file())
    {
        render_from_media(&source_path, &clip_path, input.media_time_seconds).await?;
        extract_thumbnail(&clip_path, &thumbnail_path).await?;
    } else {
        render_runtime_proof(&clip_path, &thumbnail_path).await?;
    }

    let validation = probe(&clip_path).await?;
    let clip_sha = checksum_file(&clip_path).await?;
    let thumbnail_sha = checksum_file(&thumbnail_path).await?;
    let clip_metadata = fs::metadata(&clip_path).await?;
    let thumbnail_metadata = fs::metadata(&thumbnail_path).await?;
    let source_kind = input
        .source_media_path
        .filter(|path| Path::new(path).is_file())
        .map(|_| "captured_program_media")
        .unwrap_or("generated_runtime_proof");
    let manifest = json!({
        "kind": "vanta_sponsor_proof_media_manifest",
        "asset_id": asset_id,
        "asset_kind": "sponsor_proof",
        "broadcast_id": input.broadcast_id,
        "inventory_id": input.inventory_id,
        "cue_id": input.cue_id,
        "proof_id": input.proof_id,
        "proof_marker_id": input.proof_marker_id,
        "media_time_seconds": input.media_time_seconds,
        "source_kind": source_kind,
        "source_media_path": input.source_media_path,
        "clip_path": clip_path,
        "thumbnail_path": thumbnail_path,
        "manifest_path": manifest_path,
        "clip_sha256": clip_sha,
        "thumbnail_sha256": thumbnail_sha,
        "clip_byte_length": clip_metadata.len(),
        "thumbnail_byte_length": thumbnail_metadata.len(),
        "validation": validation,
        "captured_at": chrono::Utc::now().to_rfc3339()
    });
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?).await?;
    Ok(SponsorProofMediaArtifact {
        asset_id,
        asset_dir,
        manifest_path,
        artifact_json: manifest,
        validation_json: validation,
    })
}

async fn render_from_media(
    source_path: &Path,
    clip_path: &Path,
    media_time_seconds: f64,
) -> Result<(), ObsStoreError> {
    let seek = format!("{:.3}", media_time_seconds.max(0.0));
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-ss")
        .arg(seek)
        .arg("-i")
        .arg(source_path)
        .arg("-t")
        .arg("2")
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a:0?")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("veryfast")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("aac")
        .arg("-movflags")
        .arg("+faststart")
        .arg(clip_path)
        .output()
        .await?;
    if !output.status.success() {
        return Err(ObsStoreError::Invalid(format!(
            "sponsor proof media extraction failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

async fn render_runtime_proof(
    clip_path: &Path,
    thumbnail_path: &Path,
) -> Result<(), ObsStoreError> {
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-t")
        .arg("2")
        .arg("-i")
        .arg("testsrc2=size=1280x720:rate=30")
        .arg("-f")
        .arg("lavfi")
        .arg("-t")
        .arg("2")
        .arg("-i")
        .arg("sine=frequency=880:sample_rate=48000")
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("1:a:0")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("veryfast")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("aac")
        .arg("-movflags")
        .arg("+faststart")
        .arg(clip_path)
        .output()
        .await?;
    if !output.status.success() {
        return Err(ObsStoreError::Invalid(format!(
            "sponsor proof runtime media render failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    extract_thumbnail(clip_path, thumbnail_path).await
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
            "sponsor proof thumbnail generation failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

async fn probe(path: &Path) -> Result<Value, ObsStoreError> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-print_format")
        .arg("json")
        .arg("-show_format")
        .arg("-show_streams")
        .arg(path)
        .output()
        .await?;
    if !output.status.success() {
        return Err(ObsStoreError::Invalid(format!(
            "sponsor proof probe failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let probed: Value = serde_json::from_slice(&output.stdout)?;
    let streams = probed
        .get("streams")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let has_video = streams
        .iter()
        .any(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"));
    if !has_video {
        return Err(ObsStoreError::Invalid(
            "sponsor proof media must contain video".to_string(),
        ));
    }
    Ok(json!({
        "playable": true,
        "has_video": has_video,
        "has_audio": streams.iter().any(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("audio")),
        "format": probed.get("format").cloned().unwrap_or_else(|| json!({})),
        "streams": streams
    }))
}

async fn checksum_file(path: &Path) -> Result<String, ObsStoreError> {
    let bytes = fs::read(path).await?;
    let digest = Sha256::digest(&bytes);
    Ok(format!("{digest:x}"))
}

async fn remove_if_exists(path: &Path) -> Result<(), ObsStoreError> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn media_dir() -> PathBuf {
    std::env::var("VANTA_OBS_MEDIA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("vanta-obs-media"))
}
