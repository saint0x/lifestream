use super::*;

pub(crate) fn build_image_derivative_plans(
    media: &ProbedMedia,
) -> AppResult<Vec<ImageDerivativePlan>> {
    let width = media
        .width
        .ok_or_else(|| AppError::BadRequest("video width could not be determined".to_string()))?;
    let height = media
        .height
        .ok_or_else(|| AppError::BadRequest("video height could not be determined".to_string()))?;
    let mut plans = Vec::new();

    for candidate in [
        ImageDerivativePlan {
            label: "card_thumbnail",
            max_width: 640,
            max_height: 360,
        },
        ImageDerivativePlan {
            label: "player_thumbnail",
            max_width: 1280,
            max_height: 720,
        },
    ] {
        let candidate_dimensions =
            scaled_dimensions_for_rung(width, height, candidate.max_width, candidate.max_height);
        if candidate_dimensions.0 < 144 || candidate_dimensions.1 < 144 {
            continue;
        }
        if plans.iter().any(|plan: &ImageDerivativePlan| {
            scaled_dimensions_for_rung(width, height, plan.max_width, plan.max_height)
                == candidate_dimensions
        }) {
            continue;
        }
        plans.push(candidate);
    }

    Ok(plans)
}
