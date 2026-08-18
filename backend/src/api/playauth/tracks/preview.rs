use super::*;

fn asset_tracks_published(status: &str) -> bool {
    status == "published"
}

pub(crate) fn build_media_preview_tracks(
    status: &str,
    tracks: &[StoredMediaPreviewTrack],
    playback_token: Option<&str>,
) -> Vec<PlaybackPreviewTrack> {
    tracks
        .iter()
        .map(|track| PlaybackPreviewTrack {
            id: track.id.clone(),
            label: track.label.clone(),
            image_path: track.image_relative_path.clone(),
            image_url: playback_token
                .map(|token| {
                    format!(
                        "/api/v1/media/{}?playbackToken={}",
                        track.image_relative_path, token
                    )
                })
                .unwrap_or_else(|| media_api_url(&track.image_relative_path)),
            vtt_path: track.vtt_relative_path.clone(),
            vtt_url: playback_token
                .map(|token| {
                    format!(
                        "/api/v1/media/{}?playbackToken={}",
                        track.vtt_relative_path, token
                    )
                })
                .unwrap_or_else(|| media_api_url(&track.vtt_relative_path)),
            tile_width: track.tile_width,
            tile_height: track.tile_height,
            columns_count: track.columns_count,
            rows_count: track.rows_count,
            interval_sec: track.interval_sec,
            frame_count: track.frame_count,
            is_default: track.is_default,
            published: asset_tracks_published(status),
        })
        .collect()
}

pub(crate) fn default_preview_track_id(tracks: &[PlaybackPreviewTrack]) -> Option<String> {
    tracks
        .iter()
        .find(|track| track.is_default)
        .map(|track| track.id.clone())
}
