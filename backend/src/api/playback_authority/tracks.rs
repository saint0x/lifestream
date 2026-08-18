use super::*;

fn asset_tracks_published(status: &str) -> bool {
    status == "published"
}

pub(crate) fn build_media_audio_tracks(
    status: &str,
    asset_id: &str,
    variants: &[MediaAssetVariant],
    audio_codec: Option<&str>,
    playback_token: Option<&str>,
    preferred_audio_language: Option<&str>,
    prefer_dubbed: bool,
) -> Vec<PlaybackAudioTrack> {
    let mut tracks = variants
        .iter()
        .filter(|variant| variant.variant_type == "audio")
        .map(|variant| {
            let mut parts = variant.label.split(':');
            let label = parts.next().unwrap_or("audio").to_string();
            let language = parts.next().unwrap_or("und").to_string();
            let source = parts.next().unwrap_or("source-provided").to_string();
            let is_dubbed = parts.next().unwrap_or("0") == "1";
            let variant_codec = parts.next().map(str::to_string);
            let playlist_url = playback_token
                .map(|token| {
                    format!(
                        "/api/v1/media/{}?playbackToken={}",
                        variant.relative_path, token
                    )
                })
                .or_else(|| Some(variant.url.clone()));

            PlaybackAudioTrack {
                id: variant.id.clone(),
                label,
                language,
                codec: variant_codec
                    .or_else(|| variant.mime_type.strip_prefix("audio/").map(str::to_string))
                    .or_else(|| audio_codec.map(str::to_string)),
                playlist_path: Some(variant.relative_path.clone()),
                playlist_url,
                source,
                is_dubbed,
                is_default: variant.is_default,
                published: asset_tracks_published(status),
            }
        })
        .collect::<Vec<_>>();

    if tracks.is_empty() && audio_codec.is_some() {
        tracks.push(PlaybackAudioTrack {
            id: format!("{asset_id}:audio:primary"),
            label: audio_codec
                .map(|codec| format!("primary-{codec}"))
                .unwrap_or_else(|| "primary".to_string()),
            language: "und".to_string(),
            codec: audio_codec.map(str::to_string),
            playlist_path: None,
            playlist_url: None,
            source: "source-provided".to_string(),
            is_dubbed: false,
            is_default: true,
            published: asset_tracks_published(status),
        });
        return tracks;
    }

    if let Some(preferred_language) = normalized_track_preference(preferred_audio_language) {
        if let Some(matching_id) = tracks
            .iter()
            .find(|track| {
                track.language.eq_ignore_ascii_case(&preferred_language)
                    && (!prefer_dubbed || track.is_dubbed)
            })
            .or_else(|| {
                tracks
                    .iter()
                    .find(|track| track.language.eq_ignore_ascii_case(&preferred_language))
            })
            .map(|track| track.id.clone())
        {
            for track in &mut tracks {
                track.is_default = track.id == matching_id;
            }
            return tracks;
        }
    }

    if prefer_dubbed {
        if let Some(dubbed_id) = tracks
            .iter()
            .find(|track| track.is_dubbed)
            .map(|track| track.id.clone())
        {
            for track in &mut tracks {
                track.is_default = track.id == dubbed_id;
            }
            return tracks;
        }
    }

    if tracks.iter().all(|track| !track.is_default) {
        if let Some(first_track) = tracks.first_mut() {
            first_track.is_default = true;
        }
    }

    tracks
}

fn caption_role_for_label(label: &str) -> &'static str {
    let normalized = label.to_ascii_lowercase();
    if normalized.contains("forced") {
        "forced"
    } else if normalized.contains("sdh") || normalized.contains("cc") {
        "sdh"
    } else {
        "standard"
    }
}

fn caption_source_for_label(label: &str) -> &'static str {
    let normalized = label.to_ascii_lowercase();
    if normalized.contains("auto") || normalized.contains("generated") {
        "auto-generated"
    } else if normalized.contains("reviewed") {
        "human-reviewed"
    } else {
        "source-provided"
    }
}

fn normalized_track_preference(preference: Option<&str>) -> Option<String> {
    let value = preference?.trim();
    if value.is_empty()
        || value.eq_ignore_ascii_case("off")
        || value.eq_ignore_ascii_case("none")
        || value.eq_ignore_ascii_case("disabled")
        || value.eq_ignore_ascii_case("auto")
    {
        None
    } else {
        Some(value.to_ascii_lowercase())
    }
}

pub(crate) fn build_media_caption_tracks(
    status: &str,
    variants: &[MediaAssetVariant],
    playback_token: Option<&str>,
    preferred_subtitle_language: Option<&str>,
) -> Vec<PlaybackCaptionTrack> {
    let mut tracks = variants
        .iter()
        .filter(|variant| variant.variant_type == "caption")
        .map(|variant| {
            let (label, language) = variant
                .label
                .split_once(':')
                .map(|(label, language)| (label.to_string(), language.to_string()))
                .unwrap_or_else(|| (variant.label.clone(), "und".to_string()));
            let url = if let Some(token) = playback_token {
                format!(
                    "/api/v1/media/{}?playbackToken={}",
                    variant.relative_path, token
                )
            } else {
                variant.url.clone()
            };

            PlaybackCaptionTrack {
                id: variant.id.clone(),
                label,
                language,
                role: caption_role_for_label(&variant.label).to_string(),
                source: caption_source_for_label(&variant.label).to_string(),
                mime_type: variant.mime_type.clone(),
                url,
                is_default: variant.is_default,
                published: asset_tracks_published(status),
            }
        })
        .collect::<Vec<_>>();

    if tracks.is_empty() {
        return tracks;
    }

    if let Some(preferred_language) = normalized_track_preference(preferred_subtitle_language) {
        if let Some(matching_track_id) = tracks
            .iter()
            .find(|track| track.language.eq_ignore_ascii_case(&preferred_language))
            .map(|track| track.id.clone())
        {
            for track in &mut tracks {
                track.is_default = track.id == matching_track_id;
            }
            return tracks;
        }
    }

    if tracks.iter().all(|track| !track.is_default) {
        if let Some(first_track) = tracks.first_mut() {
            first_track.is_default = true;
        }
    }

    tracks
}

pub(crate) fn default_audio_track_id(tracks: &[PlaybackAudioTrack]) -> Option<String> {
    tracks
        .iter()
        .find(|track| track.is_default)
        .map(|track| track.id.clone())
}

pub(crate) fn default_caption_track_id(tracks: &[PlaybackCaptionTrack]) -> Option<String> {
    tracks
        .iter()
        .find(|track| track.is_default)
        .map(|track| track.id.clone())
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

pub(crate) async fn fetch_user_subtitle_preference(
    pool: &SqlitePool,
    user_id: Option<&str>,
) -> AppResult<Option<String>> {
    let Some(user_id) = user_id else {
        return Ok(None);
    };
    let row = sqlx::query("SELECT subtitle_language FROM user_playback_settings WHERE user_id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| row.get("subtitle_language")))
}

pub(crate) async fn fetch_user_audio_preferences(
    pool: &SqlitePool,
    user_id: Option<&str>,
) -> AppResult<(Option<String>, bool)> {
    let Some(user_id) = user_id else {
        return Ok((None, false));
    };
    let row = sqlx::query(
        "SELECT audio_language, prefer_dubbed FROM user_playback_settings WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row
        .map(|row| {
            (
                Some(row.get::<String, _>("audio_language")),
                row.get::<i64, _>("prefer_dubbed") == 1,
            )
        })
        .unwrap_or((None, false)))
}
