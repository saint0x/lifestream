use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use axum::extract::Multipart;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{fs, io::AsyncWriteExt, process::Command};
use uuid::Uuid;

use crate::store::{EditorStore, StoreError};

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("{0}")]
    BadRequest(String),
    #[error("ffmpeg command failed: {0}")]
    Ffmpeg(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Probe {
    pub duration_seconds: f64,
    pub width: i64,
    pub height: i64,
    pub frame_rate: f64,
    pub sample_rate: i64,
    pub codec: String,
}

#[derive(Clone)]
pub struct MediaProcessor {
    root: PathBuf,
}

impl MediaProcessor {
    pub fn new(root: PathBuf) -> Self {
        let root = if root.is_absolute() {
            root
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(root)
        };
        Self { root }
    }

    pub async fn prepare(&self) -> Result<(), MediaError> {
        fs::create_dir_all(self.root.join("uploads")).await?;
        fs::create_dir_all(self.root.join("proxy")).await?;
        fs::create_dir_all(self.root.join("waveform")).await?;
        fs::create_dir_all(self.root.join("thumbs")).await?;
        fs::create_dir_all(self.root.join("renders")).await?;
        fs::create_dir_all(self.root.join("proofs")).await?;
        Ok(())
    }

    pub async fn upload_asset(
        &self,
        store: &EditorStore,
        project_id: &str,
        mut multipart: Multipart,
    ) -> Result<Value, MediaError> {
        let mut role = "raw_video".to_string();
        let mut display_name = "Uploaded media".to_string();
        let mut saved_path: Option<PathBuf> = None;
        let mut checksum: Option<String> = None;
        let upload_id = Uuid::new_v4().to_string();

        while let Some(mut field) = multipart
            .next_field()
            .await
            .map_err(|error| MediaError::BadRequest(error.to_string()))?
        {
            let name = field.name().unwrap_or_default().to_string();
            if name == "role" {
                role = field
                    .text()
                    .await
                    .map_err(|error| MediaError::BadRequest(error.to_string()))?;
                continue;
            }
            if name == "display_name" {
                display_name = field
                    .text()
                    .await
                    .map_err(|error| MediaError::BadRequest(error.to_string()))?;
                continue;
            }
            if name != "file" {
                continue;
            }

            let file_name = sanitize_file_name(field.file_name().unwrap_or("upload.mp4"));
            let path = self
                .root
                .join("uploads")
                .join(format!("{upload_id}-{file_name}"));
            let mut file = fs::File::create(&path).await?;
            let mut hasher = Sha256::new();
            while let Some(chunk) = field
                .chunk()
                .await
                .map_err(|error| MediaError::BadRequest(error.to_string()))?
            {
                hasher.update(&chunk);
                file.write_all(&chunk).await?;
            }
            checksum = Some(format!("{:x}", hasher.finalize()));
            saved_path = Some(path);
        }

        let source_path = saved_path
            .ok_or_else(|| MediaError::BadRequest("file field is required".to_string()))?;
        let probe = self.probe(&source_path).await?;
        let proxy_path = self.create_proxy(&source_path, &upload_id).await?;
        let waveform_path = self.create_waveform(&source_path, &upload_id).await?;
        let thumbnail_path = self.create_thumbnail(&source_path, &upload_id).await?;
        let media_asset_id = format!("editor_media_{upload_id}");
        let metadata = json!({
            "source_path": source_path,
            "proxy_path": proxy_path,
            "waveform_path": waveform_path,
            "thumbnail_path": thumbnail_path,
            "checksum": checksum,
            "probe": probe
        });

        Ok(store
            .create_asset_record(
                project_id,
                &media_asset_id,
                &role,
                &display_name,
                "ready",
                "pending_review",
                probe.duration_seconds,
                metadata,
            )
            .await?)
    }

    pub async fn package_hls(
        &self,
        store: &EditorStore,
        project_id: &str,
        render_job_id: &str,
    ) -> Result<Value, MediaError> {
        let source = store
            .assets(project_id)
            .await?
            .iter()
            .find_map(|asset| {
                asset["metadata_json"]["source_path"]
                    .as_str()
                    .map(ToString::to_string)
            })
            .ok_or_else(|| {
                MediaError::BadRequest("no uploaded source asset has a source_path".to_string())
            })?;
        let output_dir = self.root.join("renders").join(render_job_id);
        fs::create_dir_all(&output_dir).await?;
        let manifest = output_dir.join("master.m3u8");

        let status = Command::new("ffmpeg")
            .arg("-y")
            .arg("-i")
            .arg(&source)
            .args(["-c:v", "libx264", "-preset", "veryfast", "-c:a", "aac"])
            .args(["-f", "hls", "-hls_time", "4", "-hls_playlist_type", "vod"])
            .arg(&manifest)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;
        if !status.success() {
            return Err(MediaError::Ffmpeg(format!(
                "hls packaging exited with {status}"
            )));
        }

        let package = json!({
            "manifest_path": manifest,
            "output_dir": output_dir,
            "source_path": source,
            "packaged_at": chrono::Utc::now().to_rfc3339()
        });
        Ok(store.complete_render_job(render_job_id, package).await?)
    }

    async fn probe(&self, path: &Path) -> Result<Probe, MediaError> {
        let output = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-print_format",
                "json",
                "-show_streams",
                "-show_format",
            ])
            .arg(path)
            .output()
            .await?;
        if !output.status.success() {
            return Err(MediaError::Ffmpeg("ffprobe failed".to_string()));
        }
        let value: Value = serde_json::from_slice(&output.stdout)?;
        let duration = value["format"]["duration"]
            .as_str()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);
        let video = value["streams"]
            .as_array()
            .and_then(|streams| streams.iter().find(|s| s["codec_type"] == "video"));
        let audio = value["streams"]
            .as_array()
            .and_then(|streams| streams.iter().find(|s| s["codec_type"] == "audio"));
        let frame_rate = video
            .and_then(|v| v["avg_frame_rate"].as_str())
            .and_then(parse_ratio)
            .unwrap_or(24.0);
        Ok(Probe {
            duration_seconds: duration,
            width: video.and_then(|v| v["width"].as_i64()).unwrap_or(0),
            height: video.and_then(|v| v["height"].as_i64()).unwrap_or(0),
            frame_rate,
            sample_rate: audio
                .and_then(|v| v["sample_rate"].as_str())
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(48_000),
            codec: video
                .and_then(|v| v["codec_name"].as_str())
                .unwrap_or("unknown")
                .to_string(),
        })
    }

    async fn create_proxy(&self, source: &Path, upload_id: &str) -> Result<PathBuf, MediaError> {
        let output = self.root.join("proxy").join(format!("{upload_id}.mp4"));
        let status = Command::new("ffmpeg")
            .arg("-y")
            .arg("-i")
            .arg(source)
            .args(["-vf", "scale='min(1280,iw)':-2"])
            .args(["-c:v", "libx264", "-preset", "veryfast", "-crf", "24"])
            .args(["-c:a", "aac", "-movflags", "+faststart"])
            .arg(&output)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;
        if !status.success() {
            return Err(MediaError::Ffmpeg(format!(
                "proxy generation exited with {status}"
            )));
        }
        Ok(output)
    }

    async fn create_thumbnail(
        &self,
        source: &Path,
        upload_id: &str,
    ) -> Result<PathBuf, MediaError> {
        let output = self.root.join("thumbs").join(format!("{upload_id}.jpg"));
        let status = Command::new("ffmpeg")
            .arg("-y")
            .arg("-ss")
            .arg("0")
            .arg("-i")
            .arg(source)
            .args(["-frames:v", "1", "-q:v", "3"])
            .arg(&output)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;
        if !status.success() {
            return Err(MediaError::Ffmpeg(format!(
                "thumbnail generation exited with {status}"
            )));
        }
        Ok(output)
    }

    async fn create_waveform(&self, source: &Path, upload_id: &str) -> Result<PathBuf, MediaError> {
        let output = Command::new("ffmpeg")
            .arg("-i")
            .arg(source)
            .args(["-vn", "-ac", "1", "-ar", "8000", "-f", "s16le", "pipe:1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .await?;
        if !output.status.success() {
            return Err(MediaError::Ffmpeg(
                "waveform audio extraction failed".to_string(),
            ));
        }

        let mut peaks = Vec::new();
        let samples: Vec<i16> = output
            .stdout
            .chunks_exact(2)
            .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
            .collect();
        let window = usize::max(samples.len() / 160, 1);
        for chunk in samples.chunks(window) {
            let peak = chunk
                .iter()
                .map(|sample| f64::from(sample.abs()) / f64::from(i16::MAX))
                .fold(0.0, f64::max);
            peaks.push((peak * 1000.0).round() / 1000.0);
        }

        let path = self.root.join("waveform").join(format!("{upload_id}.json"));
        fs::write(
            &path,
            serde_json::to_vec_pretty(&json!({ "peaks": peaks }))?,
        )
        .await?;
        Ok(path)
    }
}

fn parse_ratio(value: &str) -> Option<f64> {
    let (left, right) = value.split_once("/")?;
    let numerator = left.parse::<f64>().ok()?;
    let denominator = right.parse::<f64>().ok()?;
    if denominator == 0.0 {
        None
    } else {
        Some(numerator / denominator)
    }
}

fn sanitize_file_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn processor_generates_real_derivatives_from_video()
    -> Result<(), Box<dyn std::error::Error>> {
        if Command::new("ffmpeg")
            .arg("-version")
            .output()
            .await
            .is_err()
        {
            return Ok(());
        }

        let root = std::env::temp_dir().join(format!("vanta-editor-media-{}", Uuid::new_v4()));
        let processor = MediaProcessor::new(root.clone());
        processor.prepare().await?;
        let source = root.join("sample.mp4");
        let status = Command::new("ffmpeg")
            .arg("-y")
            .args(["-f", "lavfi", "-i", "testsrc=size=320x180:rate=24"])
            .args(["-f", "lavfi", "-i", "sine=frequency=1000:sample_rate=48000"])
            .args([
                "-t", "1", "-pix_fmt", "yuv420p", "-c:v", "libx264", "-c:a", "aac",
            ])
            .arg(&source)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;
        assert!(status.success());

        let probe = processor.probe(&source).await?;
        assert!(probe.duration_seconds > 0.9);
        assert_eq!(probe.width, 320);
        assert_eq!(probe.height, 180);

        let proxy = processor.create_proxy(&source, "sample").await?;
        let thumbnail = processor.create_thumbnail(&source, "sample").await?;
        let waveform = processor.create_waveform(&source, "sample").await?;
        assert!(proxy.exists());
        assert!(thumbnail.exists());
        let waveform_json: Value = serde_json::from_slice(&fs::read(waveform).await?)?;
        assert!(
            waveform_json["peaks"]
                .as_array()
                .is_some_and(|peaks| !peaks.is_empty())
        );

        fs::remove_dir_all(root).await?;
        Ok(())
    }
}
