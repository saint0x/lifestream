use super::*;

fn asset_tracks_published(status: &str) -> bool {
    status == "published"
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
    playback_media_url: Option<&dyn Fn(&str) -> String>,
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
            let url = if let Some(media_url) = playback_media_url {
                media_url(&variant.relative_path)
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

pub(crate) fn default_caption_track_id(tracks: &[PlaybackCaptionTrack]) -> Option<String> {
    tracks
        .iter()
        .find(|track| track.is_default)
        .map(|track| track.id.clone())
}
