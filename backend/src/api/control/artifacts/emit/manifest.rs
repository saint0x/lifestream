use super::*;

pub(super) fn render_master_manifest(
    variants: &[LiveRuntimeVariantSpec],
    output: &LiveRuntimeOutput,
    session: &LiveIngestSession,
) -> String {
    let mut body = format!(
        "#EXTM3U\n#EXT-X-VERSION:{}\n",
        if output.segment_format == "fmp4" {
            9
        } else {
            3
        }
    );
    if variants.is_empty() {
        body.push_str("# backend-owned live runtime manifest awaiting probe-derived variants\n");
        return body;
    }

    let audio_codec = session
        .source_probe
        .as_ref()
        .and_then(|probe| probe.audio_codec.as_deref())
        .unwrap_or("aac");
    let video_codec = session
        .source_probe
        .as_ref()
        .and_then(|probe| probe.video_codec.as_deref())
        .unwrap_or("h264");
    let codec_string = live_manifest_codec_string(video_codec, audio_codec);

    for variant in variants {
        body.push_str(&format!(
            "#EXT-X-STREAM-INF:BANDWIDTH={},AVERAGE-BANDWIDTH={},RESOLUTION={}x{},CODECS=\"{}\"\n{}\n",
            variant.bandwidth_bps,
            variant.bandwidth_bps,
            variant.width,
            variant.height,
            codec_string,
            relative_reference_from_master(&variant.relative_playlist_path),
        ));
    }
    body
}

pub(super) fn render_routed_master_manifest(
    variants: &[RoutedVariantSpec],
    output: &LiveRuntimeOutput,
) -> String {
    let mut body = format!(
        "#EXTM3U\n#EXT-X-VERSION:{}\n",
        if output.segment_format == "fmp4" {
            9
        } else {
            3
        }
    );
    if variants.is_empty() {
        body.push_str("# collaboration route manifest awaiting source ladder\n");
        return body;
    }

    for variant in variants {
        body.push_str(&format!(
            "#EXT-X-STREAM-INF:BANDWIDTH={},AVERAGE-BANDWIDTH={},RESOLUTION={}x{},CODECS=\"avc1.64001f,mp4a.40.2\"\n{}\n",
            variant.bandwidth_bps,
            variant.bandwidth_bps,
            variant.width,
            variant.height,
            relative_reference_from_master(&variant.relative_playlist_path),
        ));
    }
    body
}

pub(super) fn render_variant_playlist(
    variant: &LiveRuntimeVariantSpec,
    output: &LiveRuntimeOutput,
) -> String {
    let mut body = format!(
        "#EXTM3U\n#EXT-X-VERSION:{}\n#EXT-X-TARGETDURATION:{}\n#EXT-X-PLAYLIST-TYPE:EVENT\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-DISCONTINUITY-SEQUENCE:{}\n",
        if output.segment_format == "fmp4" {
            9
        } else {
            3
        },
        output.target_segment_duration_sec,
        output.discontinuity_sequence
    );
    if output.segment_format == "fmp4" {
        body.push_str("#EXT-X-INDEPENDENT-SEGMENTS\n");
        body.push_str(&format!(
            "#EXT-X-MAP:URI=\"{}\"\n",
            relative_reference_from_variant(&format!("{}/init.mp4", variant.output_relative_dir))
        ));
    }
    if output.partial_segments_enabled {
        let part_target = (output.target_segment_duration_sec as f64 / 2.0).max(0.5);
        body.push_str(&format!(
            "#EXT-X-SERVER-CONTROL:CAN-BLOCK-RELOAD=YES,PART-HOLD-BACK={:.3}\n#EXT-X-PART-INF:PART-TARGET={:.3}\n",
            part_target * 2.0,
            part_target
        ));
        body.push_str(&format!(
            "#EXT-X-PART:DURATION={:.3},URI=\"{}\",INDEPENDENT=YES\n",
            part_target,
            relative_reference_from_variant(&format!(
                "{}/part_000_000.m4s",
                variant.output_relative_dir
            ))
        ));
    }
    if output.discontinuity_sequence > 0 {
        body.push_str("#EXT-X-DISCONTINUITY\n");
    }
    body.push_str(&format!(
        "#EXTINF:{:.3},\n{}\n",
        output.target_segment_duration_sec as f64,
        relative_reference_from_variant(&segment_relative_path(
            &variant.output_relative_dir,
            output,
        ))
    ));
    if playlist_is_terminal(output) {
        body.push_str("#EXT-X-ENDLIST\n");
    }
    body
}

pub(super) fn render_routed_variant_playlist(
    variant: &RoutedVariantSpec,
    output: &LiveRuntimeOutput,
) -> String {
    let mut body = format!(
        "#EXTM3U\n#EXT-X-VERSION:{}\n#EXT-X-TARGETDURATION:{}\n#EXT-X-PLAYLIST-TYPE:EVENT\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-DISCONTINUITY-SEQUENCE:{}\n",
        if output.segment_format == "fmp4" {
            9
        } else {
            3
        },
        output.target_segment_duration_sec,
        output.discontinuity_sequence
    );
    if output.segment_format == "fmp4" {
        body.push_str("#EXT-X-INDEPENDENT-SEGMENTS\n");
        body.push_str(&format!(
            "#EXT-X-MAP:URI=\"{}\"\n",
            relative_reference_between(
                &variant.output_relative_dir,
                &format!("{}/init.mp4", variant.source_output_relative_dir),
            )
        ));
    }
    if output.partial_segments_enabled {
        let part_target = (output.target_segment_duration_sec as f64 / 2.0).max(0.5);
        body.push_str(&format!(
            "#EXT-X-SERVER-CONTROL:CAN-BLOCK-RELOAD=YES,PART-HOLD-BACK={:.3}\n#EXT-X-PART-INF:PART-TARGET={:.3}\n",
            part_target * 2.0,
            part_target
        ));
        body.push_str(&format!(
            "#EXT-X-PART:DURATION={:.3},URI=\"{}\",INDEPENDENT=YES\n",
            part_target,
            relative_reference_between(
                &variant.output_relative_dir,
                &format!("{}/part_000_000.m4s", variant.source_output_relative_dir),
            )
        ));
    }
    if output.discontinuity_sequence > 0 {
        body.push_str("#EXT-X-DISCONTINUITY\n");
    }
    body.push_str(&format!(
        "#EXTINF:{:.3},\n{}\n",
        output.target_segment_duration_sec as f64,
        relative_reference_between(
            &variant.output_relative_dir,
            &segment_relative_path(&variant.source_output_relative_dir, output),
        )
    ));
    if playlist_is_terminal(output) {
        body.push_str("#EXT-X-ENDLIST\n");
    }
    body
}

#[derive(Clone, Debug)]
pub(super) struct RoutedVariantSpec {
    pub(super) width: i64,
    pub(super) height: i64,
    pub(super) bandwidth_bps: i64,
    pub(super) output_relative_dir: String,
    pub(super) source_output_relative_dir: String,
    pub(super) relative_playlist_path: String,
}

impl RoutedVariantSpec {
    pub(super) fn new(manifest_dir: &PathBuf, variant: &LiveRuntimeVariantSpec) -> Self {
        let label = FsPath::new(&variant.output_relative_dir)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| variant.label.clone());
        let output_relative_dir = manifest_dir.join(&label).to_string_lossy().to_string();
        let relative_playlist_path = format!("{output_relative_dir}/playlist.m3u8");
        Self {
            width: variant.width,
            height: variant.height,
            bandwidth_bps: variant.bandwidth_bps,
            output_relative_dir,
            source_output_relative_dir: variant.output_relative_dir.clone(),
            relative_playlist_path,
        }
    }
}

fn relative_reference_from_master(path: &str) -> String {
    FsPath::new(path)
        .file_name()
        .map(|_| {
            let pieces = path.split('/').collect::<Vec<_>>();
            if pieces.len() >= 2 {
                format!("{}/{}", pieces[pieces.len() - 2], pieces[pieces.len() - 1])
            } else {
                path.to_string()
            }
        })
        .unwrap_or_else(|| path.to_string())
}

fn relative_reference_from_variant(path: &str) -> String {
    FsPath::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn relative_reference_between(from_dir: &str, to_path: &str) -> String {
    let from_components = FsPath::new(from_dir).components().collect::<Vec<_>>();
    let to_components = FsPath::new(to_path).components().collect::<Vec<_>>();

    let mut common_len = 0_usize;
    while common_len < from_components.len()
        && common_len < to_components.len()
        && from_components[common_len] == to_components[common_len]
    {
        common_len += 1;
    }

    let mut relative = PathBuf::new();
    for _ in common_len..from_components.len() {
        relative.push("..");
    }
    for component in &to_components[common_len..] {
        if let Component::Normal(segment) = component {
            relative.push(segment);
        }
    }

    relative.to_string_lossy().to_string()
}

fn segment_relative_path(output_relative_dir: &str, output: &LiveRuntimeOutput) -> String {
    format!(
        "{}/segment_000.{}",
        output_relative_dir,
        if output.segment_format == "fmp4" {
            "m4s"
        } else {
            "ts"
        }
    )
}

fn playlist_is_terminal(output: &LiveRuntimeOutput) -> bool {
    matches!(
        output.runtime_state.as_str(),
        "disconnected" | "stale" | "failed" | "archive_complete"
    ) || output.packaging_status == "complete"
}

fn live_manifest_codec_string(video_codec: &str, audio_codec: &str) -> String {
    let video = match video_codec {
        "h265" | "hevc" => "hvc1.1.6.L93.B0",
        "av1" => "av01.0.08M.08",
        _ => "avc1.64001f",
    };
    let audio = match audio_codec {
        "opus" => "opus",
        _ => "mp4a.40.2",
    };
    format!("{video},{audio}")
}
