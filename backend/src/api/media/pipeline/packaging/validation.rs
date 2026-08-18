use super::*;

pub(crate) async fn validate_generated_hls_package(
    master_path: &FsPath,
    package: &GeneratedHlsPackage,
) -> AppResult<()> {
    if package.variants.is_empty() {
        return Err(AppError::MediaPipeline(
            "generated HLS package did not produce any playback variants".to_string(),
        ));
    }

    let master_body = tokio::fs::read_to_string(master_path).await?;
    let stream_inf_count = master_body
        .lines()
        .filter(|line| line.starts_with("#EXT-X-STREAM-INF"))
        .count();
    if stream_inf_count != package.variants.len() {
        return Err(AppError::MediaPipeline(format!(
            "generated HLS master manifest expected {} stream entries but found {}",
            package.variants.len(),
            stream_inf_count
        )));
    }

    let master_dir = master_path.parent().ok_or_else(|| {
        AppError::MediaPipeline("generated HLS master manifest has no parent directory".to_string())
    })?;
    let subtitle_media_lines = master_body
        .lines()
        .filter(|line| line.starts_with("#EXT-X-MEDIA:TYPE=SUBTITLES"))
        .collect::<Vec<_>>();
    let audio_media_lines = master_body
        .lines()
        .filter(|line| line.starts_with("#EXT-X-MEDIA:TYPE=AUDIO"))
        .collect::<Vec<_>>();
    if audio_media_lines.len() != package.audio_tracks.len() {
        return Err(AppError::MediaPipeline(format!(
            "generated HLS master manifest expected {} audio track entries but found {}",
            package.audio_tracks.len(),
            audio_media_lines.len()
        )));
    }
    if subtitle_media_lines.len() != package.subtitle_tracks.len() {
        return Err(AppError::MediaPipeline(format!(
            "generated HLS master manifest expected {} subtitle track entries but found {}",
            package.subtitle_tracks.len(),
            subtitle_media_lines.len()
        )));
    }
    let listed_variant_paths = master_body
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if listed_variant_paths.len() != package.variants.len() {
        return Err(AppError::MediaPipeline(format!(
            "generated HLS master manifest expected {} variant playlist paths but found {}",
            package.variants.len(),
            listed_variant_paths.len()
        )));
    }

    for variant in &package.variants {
        if !listed_variant_paths
            .iter()
            .any(|path| path == &variant.relative_playlist_path)
        {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS master manifest is missing variant playlist {}",
                variant.relative_playlist_path
            )));
        }

        let playlist_path = master_dir.join(&variant.relative_playlist_path);
        let playlist_body = tokio::fs::read_to_string(&playlist_path).await?;
        if !playlist_body.contains("#EXTM3U") {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS variant playlist {} is missing EXTM3U header",
                variant.relative_playlist_path
            )));
        }
        if !playlist_body.contains("#EXTINF") {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS variant playlist {} does not contain media segments",
                variant.relative_playlist_path
            )));
        }

        let playlist_dir = playlist_path.parent().ok_or_else(|| {
            AppError::MediaPipeline(format!(
                "generated HLS variant playlist {} has no parent directory",
                variant.relative_playlist_path
            ))
        })?;
        let segment_paths = playlist_body
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect::<Vec<_>>();
        if segment_paths.is_empty() {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS variant playlist {} does not list any media segments",
                variant.relative_playlist_path
            )));
        }
        for segment in segment_paths {
            let segment_path = playlist_dir.join(segment);
            let metadata = tokio::fs::metadata(&segment_path).await?;
            if !metadata.is_file() || metadata.len() == 0 {
                return Err(AppError::MediaPipeline(format!(
                    "generated HLS segment {} is missing or empty",
                    segment_path.display()
                )));
            }
        }
    }

    for track in &package.audio_tracks {
        if !master_body.contains(&format!("URI=\"{}\"", track.relative_playlist_path)) {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS master manifest is missing audio track {}",
                track.relative_playlist_path
            )));
        }
        let playlist_path = master_dir.join(&track.relative_playlist_path);
        let playlist_body = tokio::fs::read_to_string(&playlist_path).await?;
        if !playlist_body.contains("#EXTM3U") || !playlist_body.contains("#EXTINF") {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS audio playlist {} is incomplete",
                track.relative_playlist_path
            )));
        }
    }

    for track in &package.subtitle_tracks {
        if !master_body.contains(&format!("URI=\"{}\"", track.relative_path)) {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS master manifest is missing subtitle track {}",
                track.relative_path
            )));
        }
        let subtitle_path = master_dir.join(&track.relative_path);
        let subtitle_body = tokio::fs::read_to_string(&subtitle_path).await?;
        if !subtitle_body.starts_with("WEBVTT") {
            return Err(AppError::MediaPipeline(format!(
                "generated HLS subtitle track {} is missing WEBVTT header",
                track.relative_path
            )));
        }
    }

    Ok(())
}
