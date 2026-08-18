use super::*;

mod assets;
mod jobs;
mod packaging;
mod probe;
mod processing;

pub(crate) use assets::{
    NewMediaPreviewTrack, NewMediaVariant, StoredMediaPreviewTrack, ensure_media_asset_shell,
    fetch_media_asset_by_id_any_creator, fetch_media_asset_by_upload_id,
    fetch_media_asset_by_upload_job, fetch_media_asset_variants, fetch_media_assets,
    fetch_media_preview_track_rows, fetch_media_processing_runs, finish_media_processing_run,
    replace_media_preview_tracks, replace_media_variants, start_media_processing_run,
};
#[cfg(test)]
pub(crate) use jobs::{MAX_MEDIA_PROCESSING_ATTEMPTS, fail_media_job_for_lease};
pub(crate) use jobs::{
    fetch_admin_media_job_record, fetch_admin_media_jobs, fetch_pending_media_jobs,
    fetch_upload_ingest_session, fetch_upload_ingest_sessions, fetch_upload_job_by_id,
    fetch_upload_job_by_id_global, fetch_upload_job_creator_id, fetch_upload_jobs,
    publish_due_scheduled_upload_releases, reconcile_single_media_job,
    reconcile_stale_media_processing_jobs, reconcile_stale_media_processing_jobs_for_read,
    requeue_media_job_for_processing, schedule_media_processing,
};
pub(crate) use packaging::{
    GeneratedHlsPackage, GeneratedHlsSubtitleTrack, HlsVariantPlan, build_image_derivative_plans,
    extract_subtitle_stream_to_webvtt, generate_hls, generate_poster, generate_thumbnail,
    generate_timeline_preview_track, plan_hls_variants, scaled_dimensions_for_rung,
    subtitle_codec_supported_for_normalization,
};
#[cfg(test)]
pub(crate) use packaging::{
    GeneratedHlsVariant, validate_generated_hls_package, write_hls_master_manifest,
};
pub(crate) use probe::{
    ProbedAudioStream, ProbedMedia, classify_media_processing_error, probe_media,
    validate_probed_media, verify_media_integrity,
};
pub(crate) use processing::process_media_job;
