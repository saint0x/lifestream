use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

use serde_json::{Value, json};
use thiserror::Error;
use tokio::{fs, process::Command};

#[derive(Debug, Clone)]
pub struct RenderRequest {
    pub job_id: String,
    pub codec: String,
    pub audio_codec: String,
    pub container: String,
    pub bitrate_kbps: i64,
    pub keyframe_interval_seconds: i64,
    pub latency_profile: String,
    pub width: i64,
    pub height: i64,
    pub frame_rate: i64,
    pub duration_seconds: i64,
}

#[derive(Debug, Clone)]
pub struct RenderResult {
    pub output_path: String,
    pub validation_json: Value,
}

#[derive(Debug, Clone)]
pub struct PackageRequest {
    pub job_id: String,
    pub input_path: String,
}

#[derive(Debug, Clone)]
pub struct PackageResult {
    pub manifest_path: String,
    pub package_json: Value,
}

#[derive(Debug, Error)]
pub enum FfmpegError {
    #[error("ffmpeg render failed: {0}")]
    Render(String),
    #[error("ffprobe validation failed: {0}")]
    Probe(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Default, Clone)]
pub struct FfmpegMediaEngine;

impl FfmpegMediaEngine {
    pub async fn capabilities(&self) -> Result<Value, FfmpegError> {
        let output = Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-encoders")
            .output()
            .await
            .map_err(|error| {
                FfmpegError::Probe(format!(
                    "could not inspect ffmpeg encoders from PATH: {error}"
                ))
            })?;
        if !output.status.success() {
            return Err(FfmpegError::Probe(format!(
                "status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let encoders = String::from_utf8_lossy(&output.stdout);
        Ok(json!({
            "h264": has_encoder(&encoders, &["libx264", "h264_videotoolbox", "h264_nvenc", "h264_qsv", "h264_amf"]),
            "h265": has_encoder(&encoders, &["libx265", "hevc_videotoolbox", "hevc_nvenc", "hevc_qsv", "hevc_amf"]),
            "av1": has_encoder(&encoders, &["libaom-av1", "librav1e", "libsvtav1", "av1_nvenc", "av1_qsv", "av1_amf"]),
            "aac": has_encoder(&encoders, &[" aac ", "aac_at"]),
            "opus": has_encoder(&encoders, &["libopus"]),
            "hardware_video": {
                "videotoolbox": has_encoder(&encoders, &["h264_videotoolbox", "hevc_videotoolbox"]),
                "nvenc": has_encoder(&encoders, &["h264_nvenc", "hevc_nvenc", "av1_nvenc"]),
                "qsv": has_encoder(&encoders, &["h264_qsv", "hevc_qsv", "av1_qsv"]),
                "amf": has_encoder(&encoders, &["h264_amf", "hevc_amf", "av1_amf"])
            },
            "containers": {
                "fragmented_mp4": true,
                "mkv": true
            }
        }))
    }

    pub async fn render(&self, input: RenderRequest) -> Result<RenderResult, FfmpegError> {
        let output_path = output_path(&input).await?;
        let partial_path = partial_output_path(&output_path);
        remove_if_exists(&output_path).await?;
        remove_if_exists(&partial_path).await?;
        let capabilities = self.capabilities().await?;
        let candidates = video_encoder_candidates(&input.codec, &capabilities);
        let mut attempted = Vec::new();
        let mut selected: Option<EncoderChoice> = None;
        let mut validation_json: Option<Value> = None;
        let mut last_error = String::new();

        for candidate in candidates {
            attempted.push(candidate.name.to_string());
            remove_if_exists(&partial_path).await?;
            let output = run_render_command(&input, &partial_path, candidate.clone())
                .await
                .map_err(|error| {
                    FfmpegError::Render(format!("could not spawn ffmpeg from PATH: {error}"))
                })?;
            if output.status.success() {
                match probe(&partial_path, input.duration_seconds, input.frame_rate).await {
                    Ok(validation) => {
                        fs::rename(&partial_path, &output_path).await?;
                        validation_json = Some(validation);
                        selected = Some(candidate.clone());
                        break;
                    }
                    Err(error) => {
                        last_error = format!("validation failed after muxer write: {error}");
                        remove_if_exists(&partial_path).await?;
                    }
                }
            } else {
                last_error = format!(
                    "status {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                );
                remove_if_exists(&partial_path).await?;
            }
        }

        let Some(selected) = selected else {
            return Err(FfmpegError::Render(last_error));
        };

        let mut validation_json = validation_json.ok_or_else(|| {
            FfmpegError::Probe("render completed without playable validation".to_string())
        })?;
        if let Some(object) = validation_json.as_object_mut() {
            object.insert("selected_encoder".to_string(), json!(selected.name));
            object.insert(
                "hardware_family".to_string(),
                json!(selected.hardware_family),
            );
            object.insert("latency_profile".to_string(), json!(input.latency_profile));
            object.insert("attempted_video_encoders".to_string(), json!(attempted));
            object.insert(
                "muxer_recovery".to_string(),
                json!({
                    "partial_path": partial_path,
                    "stable_path": output_path,
                    "committed_atomically": true
                }),
            );
        }
        Ok(RenderResult {
            output_path: output_path.to_string_lossy().to_string(),
            validation_json,
        })
    }

    pub async fn package_hls(&self, input: PackageRequest) -> Result<PackageResult, FfmpegError> {
        let base = media_dir().await?.join("hls").join(&input.job_id);
        fs::create_dir_all(&base).await?;
        let manifest = base.join("index.m3u8");
        let segment_pattern = base.join("segment_%03d.m4s");
        let init = base.join("init.mp4");

        let output = Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-y")
            .arg("-i")
            .arg(&input.input_path)
            .arg("-c")
            .arg("copy")
            .arg("-hls_time")
            .arg("1")
            .arg("-hls_playlist_type")
            .arg("vod")
            .arg("-hls_segment_type")
            .arg("fmp4")
            .arg("-hls_fmp4_init_filename")
            .arg("init.mp4")
            .arg("-hls_segment_filename")
            .arg(&segment_pattern)
            .arg(&manifest)
            .output()
            .await
            .map_err(|error| {
                FfmpegError::Render(format!("could not spawn ffmpeg HLS packaging: {error}"))
            })?;
        if !output.status.success() {
            return Err(FfmpegError::Render(format!(
                "HLS packaging status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        let manifest_text = fs::read_to_string(&manifest).await?;
        if !manifest_text.contains("#EXTM3U") || !manifest_text.contains("#EXT-X-MAP") {
            return Err(FfmpegError::Probe(
                "HLS/CMAF manifest missing required playlist or init-map tags".to_string(),
            ));
        }
        let entries = fs::read_dir(&base).await?;
        tokio::pin!(entries);
        let mut segments = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) == Some("m4s") {
                segments.push(path.to_string_lossy().to_string());
            }
        }
        segments.sort();
        if segments.is_empty() || !init.is_file() {
            return Err(FfmpegError::Probe(
                "HLS/CMAF package must include init segment and media segments".to_string(),
            ));
        }

        Ok(PackageResult {
            manifest_path: manifest.to_string_lossy().to_string(),
            package_json: json!({
                "kind": "hls_cmaf",
                "manifest_path": manifest,
                "init_segment_path": init,
                "segment_paths": segments,
                "segment_count": segments.len(),
                "playback_ready": true
            }),
        })
    }
}

#[derive(Debug, Clone)]
struct EncoderChoice {
    name: &'static str,
    hardware_family: &'static str,
}

async fn run_render_command(
    input: &RenderRequest,
    output_path: &Path,
    encoder: EncoderChoice,
) -> Result<std::process::Output, std::io::Error> {
    let mut command = Command::new("ffmpeg");
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg(format!(
            "testsrc2=size={}x{}:rate={}:duration={}",
            input.width, input.height, input.frame_rate, input.duration_seconds
        ))
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg(format!(
            "sine=frequency=880:sample_rate=48000:duration={}",
            input.duration_seconds
        ))
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("1:a:0")
        .arg("-c:v")
        .arg(encoder.name)
        .arg("-b:v")
        .arg(format!("{}k", input.bitrate_kbps))
        .arg("-g")
        .arg((input.keyframe_interval_seconds * input.frame_rate).to_string())
        .arg("-pix_fmt")
        .arg("yuv420p");
    push_latency_args(&mut command, input, &encoder);
    command
        .arg("-c:a")
        .arg(audio_encoder(&input.audio_codec))
        .arg("-b:a")
        .arg("160k");
    if input.container == "fragmented_mp4" {
        command
            .arg("-movflags")
            .arg("+frag_keyframe+empty_moov+default_base_moof")
            .arg("-f")
            .arg("mp4");
    } else {
        command.arg("-f").arg("matroska");
    }
    command.arg(output_path);
    command.output().await
}

fn push_latency_args(command: &mut Command, input: &RenderRequest, encoder: &EncoderChoice) {
    if encoder.name == "libx264" || encoder.name == "libx265" {
        match input.latency_profile.as_str() {
            "ultra_low" => {
                command
                    .arg("-preset")
                    .arg("veryfast")
                    .arg("-tune")
                    .arg("zerolatency");
            }
            "low" => {
                command.arg("-preset").arg("fast");
            }
            _ => {
                command.arg("-preset").arg("medium");
            }
        }
    } else if encoder.hardware_family == "videotoolbox" {
        command.arg("-realtime").arg("1");
    }
}

fn video_encoder_candidates(codec: &str, capabilities: &Value) -> Vec<EncoderChoice> {
    let mut candidates = Vec::new();
    if capabilities
        .pointer("/hardware_video/videotoolbox")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        if codec == "h264" {
            candidates.push(EncoderChoice {
                name: "h264_videotoolbox",
                hardware_family: "videotoolbox",
            });
        } else if codec == "h265" {
            candidates.push(EncoderChoice {
                name: "hevc_videotoolbox",
                hardware_family: "videotoolbox",
            });
        }
    }
    if codec == "h265" {
        candidates.push(EncoderChoice {
            name: "libx265",
            hardware_family: "software",
        });
    } else if codec == "av1" {
        candidates.push(EncoderChoice {
            name: "libaom-av1",
            hardware_family: "software",
        });
    } else {
        candidates.push(EncoderChoice {
            name: "libx264",
            hardware_family: "software",
        });
    }
    candidates
}

fn has_encoder(encoders: &str, names: &[&str]) -> bool {
    names.iter().any(|name| encoders.contains(name))
}

async fn output_path(input: &RenderRequest) -> Result<PathBuf, FfmpegError> {
    let base = media_dir().await?;
    Ok(base.join(format!(
        "{}.{}",
        input.job_id,
        if input.container == "mkv" {
            "mkv"
        } else {
            "mp4"
        }
    )))
}

fn partial_output_path(output_path: &Path) -> PathBuf {
    let extension = output_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("media");
    output_path.with_extension(format!("partial.{extension}"))
}

async fn remove_if_exists(path: &Path) -> Result<(), FfmpegError> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

async fn media_dir() -> Result<PathBuf, FfmpegError> {
    let base = std::env::var("VANTA_OBS_MEDIA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("vanta-obs-media"));
    fs::create_dir_all(&base).await?;
    Ok(base)
}

async fn probe(
    path: &Path,
    expected_duration_seconds: i64,
    expected_frame_rate: i64,
) -> Result<Value, FfmpegError> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-count_frames")
        .arg("-print_format")
        .arg("json")
        .arg("-show_format")
        .arg("-show_streams")
        .arg(path)
        .output()
        .await
        .map_err(|error| {
            FfmpegError::Probe(format!("could not spawn ffprobe from PATH: {error}"))
        })?;
    if !output.status.success() {
        return Err(FfmpegError::Probe(format!(
            "status {}: {}",
            output.status,
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
    let has_audio = streams
        .iter()
        .any(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("audio"));
    if !has_video || !has_audio {
        return Err(FfmpegError::Probe(
            "encoded artifact must contain both video and audio streams".to_string(),
        ));
    }
    let duration_seconds = format_duration_seconds(&probed);
    let min_duration = (expected_duration_seconds as f64 - 0.35).max(0.0);
    if duration_seconds < min_duration {
        return Err(FfmpegError::Probe(format!(
            "encoded artifact duration {duration_seconds:.2}s was shorter than expected {expected_duration_seconds}s"
        )));
    }
    let expected_video_frames = expected_duration_seconds * expected_frame_rate;
    let observed_video_frames = observed_video_frames(&streams);
    let minimum_frames = ((expected_video_frames as f64) * 0.95).floor() as i64;
    if observed_video_frames < minimum_frames {
        return Err(FfmpegError::Probe(format!(
            "encoded artifact only exposed {observed_video_frames} video frames; expected at least {minimum_frames}"
        )));
    }
    Ok(json!({
        "playable": true,
        "has_video": has_video,
        "has_audio": has_audio,
        "requested_duration_seconds": expected_duration_seconds,
        "validated_duration_seconds": duration_seconds,
        "expected_video_frames": expected_video_frames,
        "observed_video_frames": observed_video_frames,
        "frame_coverage": observed_video_frames as f64 / expected_video_frames.max(1) as f64,
        "long_capture_validation": expected_duration_seconds >= 5,
        "format": probed.get("format").cloned().unwrap_or_else(|| json!({})),
        "streams": streams
    }))
}

fn audio_encoder(codec: &str) -> &'static str {
    match codec {
        "opus" => "libopus",
        _ => "aac",
    }
}

fn format_duration_seconds(probed: &Value) -> f64 {
    probed
        .pointer("/format/duration")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0)
}

fn observed_video_frames(streams: &[Value]) -> i64 {
    streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"))
        .and_then(|stream| {
            stream
                .get("nb_read_frames")
                .or_else(|| stream.get("nb_frames"))
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<i64>().ok())
        })
        .unwrap_or(0)
}
