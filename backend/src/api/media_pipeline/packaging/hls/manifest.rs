use super::*;

pub(crate) async fn write_hls_master_manifest(
    output_path: &FsPath,
    variants: &[GeneratedHlsVariant],
    audio_tracks: &[GeneratedHlsAudioTrack],
    subtitle_tracks: &[GeneratedHlsSubtitleTrack],
) -> AppResult<()> {
    let mut body = String::from("#EXTM3U\n#EXT-X-VERSION:3\n");
    for track in audio_tracks {
        body.push_str(&format!(
            "#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",NAME=\"{}\",LANGUAGE=\"{}\",AUTOSELECT=YES,DEFAULT={},URI=\"{}\"\n",
            track.label,
            track.language,
            if track.is_default { "YES" } else { "NO" },
            track.relative_playlist_path
        ));
    }
    for track in subtitle_tracks {
        body.push_str(&format!(
            "#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"captions\",NAME=\"{}\",LANGUAGE=\"{}\",AUTOSELECT=YES,DEFAULT={},FORCED=NO,URI=\"{}\"\n",
            track.name,
            track.language,
            if track.is_default { "YES" } else { "NO" },
            track.relative_path
        ));
    }
    for variant in variants {
        let codecs = "avc1.64001f,mp4a.40.2";
        body.push_str(&format!(
            "#EXT-X-STREAM-INF:BANDWIDTH={},AVERAGE-BANDWIDTH={},RESOLUTION={}x{},CODECS=\"{}\"{}{}\n{}\n",
            variant.plan.bandwidth_bps,
            variant.plan.bandwidth_bps,
            variant.plan.width,
            variant.plan.height,
            codecs,
            if audio_tracks.is_empty() {
                String::new()
            } else {
                ",AUDIO=\"audio\"".to_string()
            },
            if subtitle_tracks.is_empty() {
                String::new()
            } else {
                ",SUBTITLES=\"captions\"".to_string()
            },
            variant.relative_playlist_path
        ));
    }
    tokio::fs::write(output_path, body).await?;
    Ok(())
}
