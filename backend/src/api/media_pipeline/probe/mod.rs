use super::*;

mod inspect;
mod integrity;
mod validate;

pub(crate) use inspect::probe_media;
pub(crate) use integrity::verify_media_integrity;
pub(crate) use validate::{classify_media_processing_error, validate_probed_media};

#[derive(Clone, Debug)]
pub(crate) struct ProbedMedia {
    pub(crate) container_format: Option<String>,
    pub(crate) duration_sec: f64,
    pub(crate) width: Option<i64>,
    pub(crate) height: Option<i64>,
    pub(crate) frame_rate: Option<f64>,
    pub(crate) video_codec: Option<String>,
    pub(crate) audio_codec: Option<String>,
    pub(crate) audio_sample_rate_hz: Option<i64>,
    pub(crate) audio_channels: Option<i64>,
    pub(crate) has_video: bool,
    pub(crate) has_audio: bool,
    pub(crate) bitrate_bps: Option<i64>,
    pub(crate) audio_streams: Vec<ProbedAudioStream>,
    pub(crate) subtitle_streams: Vec<ProbedSubtitleStream>,
}

#[derive(Clone, Debug)]
pub(crate) struct ProbedAudioStream {
    pub(crate) stream_index: i64,
    pub(crate) codec: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) sample_rate_hz: Option<i64>,
    pub(crate) channels: Option<i64>,
}

#[derive(Clone, Debug)]
pub(crate) struct ProbedSubtitleStream {
    pub(crate) stream_index: i64,
    pub(crate) codec: Option<String>,
    pub(crate) language: Option<String>,
}
