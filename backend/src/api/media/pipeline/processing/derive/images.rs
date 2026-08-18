use super::*;

pub(super) async fn generate_image_derivatives(
    state: &SharedState,
    creator_id: &str,
    job_id: &str,
    attempt: &MediaProcessingAttempt,
    probed: &ProbedMedia,
    processed_root: &str,
) -> Result<Vec<(String, String, i64, i64)>, (AppError, String)> {
    if !probed.has_video {
        return Ok(Vec::new());
    }

    let image_derivative_plans = build_image_derivative_plans(probed)
        .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    let derivatives_run_id = start_media_processing_run(
        &state.pool,
        creator_id,
        job_id,
        &attempt.asset.id,
        "thumbnails",
        json!({
            "targets": image_derivative_plans.iter().map(|plan| {
                json!({
                    "label": plan.label,
                    "maxWidth": plan.max_width,
                    "maxHeight": plan.max_height,
                })
            }).collect::<Vec<_>>(),
        }),
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    let mut derived = Vec::with_capacity(image_derivative_plans.len());
    for plan in &image_derivative_plans {
        let relative_path = format!("{processed_root}/images/{}.jpg", plan.label);
        let full_path = media_path_for_relative(state, &relative_path);
        ensure_parent_dir(&full_path)
            .await
            .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
        let (width, height) = scaled_dimensions_for_rung(
            probed.width.unwrap_or(plan.max_width),
            probed.height.unwrap_or(plan.max_height),
            plan.max_width,
            plan.max_height,
        );
        if let Err(error) = generate_thumbnail(
            &attempt.source_path,
            &full_path,
            probed.duration_sec,
            width,
            height,
        )
        .await
        {
            let _ = finish_media_processing_run(
                &state.pool,
                &derivatives_run_id,
                "failed",
                json!({
                    "target": relative_path,
                    "error": error.to_string(),
                }),
            )
            .await;
            return Err((error, attempt.lease_updated_at.clone()));
        }
        derived.push((plan.label.to_string(), relative_path, width, height));
    }
    finish_media_processing_run(
        &state.pool,
        &derivatives_run_id,
        "completed",
        json!({
            "targets": derived.iter().map(|(label, relative_path, width, height)| {
                json!({
                    "label": label,
                    "target": relative_path,
                    "width": width,
                    "height": height,
                })
            }).collect::<Vec<_>>(),
        }),
    )
    .await
    .map_err(|error| (error, attempt.lease_updated_at.clone()))?;
    Ok(derived)
}
