use super::*;

mod hls;
mod images;
mod poster;
mod subtitles;
mod timeline;

use hls::generate_hls_package;
use images::generate_image_derivatives;
use poster::generate_poster_derivative;
use subtitles::generate_subtitle_variants;
use timeline::generate_timeline_preview;

pub(crate) async fn generate_derivatives_and_package(
    state: &SharedState,
    creator_id: &str,
    job_id: &str,
    attempt: &MediaProcessingAttempt,
    probed: &ProbedMedia,
) -> Result<GeneratedDerivativeBundle, (AppError, String)> {
    let processed_root = processed_generation_root(creator_id, attempt);
    let poster_relative_path =
        generate_poster_derivative(state, creator_id, job_id, attempt, probed, &processed_root)
            .await?;
    let image_derivatives_relative_paths =
        generate_image_derivatives(state, creator_id, job_id, attempt, probed, &processed_root)
            .await?;
    let timeline_preview_track =
        generate_timeline_preview(state, creator_id, job_id, attempt, probed, &processed_root)
            .await?;
    let subtitle_variants =
        generate_subtitle_variants(state, creator_id, job_id, attempt, probed, &processed_root)
            .await?;
    let (generated_package, hls_relative_path) = generate_hls_package(
        state,
        creator_id,
        job_id,
        attempt,
        probed,
        &processed_root,
        &subtitle_variants,
    )
    .await?;

    Ok(GeneratedDerivativeBundle {
        poster_relative_path,
        image_derivatives_relative_paths,
        timeline_preview_track,
        subtitle_variants,
        generated_package,
        hls_relative_path,
    })
}

fn processed_generation_root(creator_id: &str, attempt: &MediaProcessingAttempt) -> String {
    format!(
        "processed/{}/{}/gen-{:04}",
        creator_id, attempt.asset.id, attempt.job.processing_attempt_count
    )
}
