use super::*;

fn asset_tracks_published(status: &str) -> bool {
    status == "published"
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

pub(crate) fn build_media_audio_tracks(
    status: &str,
    asset_id: &str,
    variants: &[MediaAssetVariant],
    audio_codec: Option<&str>,
    playback_media_url: Option<&dyn Fn(&str) -> String>,
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
            let playlist_url = playback_media_url
                .map(|media_url| media_url(&variant.relative_path))
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

pub(crate) fn default_audio_track_id(tracks: &[PlaybackAudioTrack]) -> Option<String> {
    tracks
        .iter()
        .find(|track| track.is_default)
        .map(|track| track.id.clone())
}
