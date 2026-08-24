use super::*;

mod attempt;
mod derive;
mod finalize;
mod persist;
mod verify;

use attempt::begin_media_processing_attempt;
use derive::generate_derivatives_and_package;
pub(crate) use finalize::finalize_media_processing;
pub(crate) use persist::persist_media_variants;
use verify::{run_integrity_stage, run_probe_stage};

pub(crate) struct MediaProcessingAttempt {
    pub(crate) job: UploadJob,
    pub(crate) session: UploadIngestSession,
    pub(crate) asset: MediaAsset,
    pub(crate) source_path: PathBuf,
    pub(crate) lease_updated_at: String,
}

pub(crate) struct GeneratedDerivativeBundle {
    pub(crate) poster_relative_path: Option<String>,
    pub(crate) image_derivatives_relative_paths: Vec<(String, String, i64, i64)>,
    pub(crate) timeline_preview_track: Option<NewMediaPreviewTrack>,
    pub(crate) subtitle_variants: Vec<(String, String, String, i64, bool)>,
    pub(crate) generated_package: GeneratedHlsPackage,
    pub(crate) hls_relative_path: String,
}

pub(crate) async fn process_media_job(
    state: SharedState,
    creator_id: &str,
    job_id: &str,
) -> Result<(), (AppError, String)> {
    tracing::info!(creator_id, job_id, "media processing job started");
    let Some(attempt) = begin_media_processing_attempt(&state, creator_id, job_id)
        .await
        .map_err(|error| (error, String::new()))?
    else {
        tracing::info!(
            creator_id,
            job_id,
            "media processing job skipped because no attempt was acquired"
        );
        return Ok(());
    };

    let probed = run_probe_stage(&state, creator_id, job_id, &attempt).await?;
    run_integrity_stage(&state, creator_id, job_id, &attempt, &probed).await?;
    let generated =
        generate_derivatives_and_package(&state, creator_id, job_id, &attempt, &probed).await?;

    if !jobs::media_processing_lease_is_active(
        state.db.sqlite_adapter(),
        creator_id,
        job_id,
        &attempt.lease_updated_at,
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?
    {
        tracing::warn!(
            creator_id,
            job_id,
            "media processing job lost its active lease before persistence"
        );
        return Ok(());
    }

    persist_media_variants(&state, &attempt, &probed, &generated).await?;
    finalize_media_processing(&state, creator_id, job_id, &attempt, &probed, &generated).await?;
    tracing::info!(
        creator_id,
        job_id,
        asset_id = %attempt.asset.id,
        "media processing job finalized"
    );
    Ok(())
}
