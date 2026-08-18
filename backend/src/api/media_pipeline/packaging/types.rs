#[derive(Clone, Debug)]
pub(crate) struct ImageDerivativePlan {
    pub(crate) label: &'static str,
    pub(crate) max_width: i64,
    pub(crate) max_height: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct HlsVariantPlan {
    pub(crate) label: String,
    pub(crate) width: i64,
    pub(crate) height: i64,
    pub(crate) video_bitrate_bps: i64,
    pub(crate) bandwidth_bps: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct GeneratedHlsVariant {
    pub(crate) plan: HlsVariantPlan,
    pub(crate) relative_playlist_path: String,
    pub(crate) file_size_bytes: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct GeneratedHlsSubtitleTrack {
    pub(crate) relative_path: String,
    pub(crate) language: String,
    pub(crate) name: String,
    pub(crate) is_default: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct GeneratedHlsAudioTrack {
    pub(crate) label: String,
    pub(crate) language: String,
    pub(crate) codec: String,
    pub(crate) bitrate_bps: i64,
    pub(crate) relative_playlist_path: String,
    pub(crate) file_size_bytes: i64,
    pub(crate) is_default: bool,
    pub(crate) is_dubbed: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct GeneratedHlsPackage {
    pub(crate) master_relative_path: String,
    pub(crate) variants: Vec<GeneratedHlsVariant>,
    pub(crate) audio_tracks: Vec<GeneratedHlsAudioTrack>,
    pub(crate) subtitle_tracks: Vec<GeneratedHlsSubtitleTrack>,
}
