use super::*;

mod bulk;
mod content;
mod lifecycle;
mod listing;

pub(crate) use content::update_upload;
pub(crate) use lifecycle::{takedown_upload, unpublish_upload};

pub(super) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/v1/creator/me/uploads", get(listing::list_uploads))
        .route(
            "/api/v1/creator/me/content",
            get(listing::get_creator_content),
        )
        .route("/api/v1/creator/me/uploads/:id", patch(update_upload))
        .route(
            "/api/v1/creator/me/uploads/:id/lifecycle",
            patch(lifecycle::update_upload_lifecycle),
        )
        .route(
            "/api/v1/creator/me/uploads/:id/unpublish",
            post(unpublish_upload),
        )
        .route(
            "/api/v1/creator/me/uploads/:id/takedown",
            post(takedown_upload),
        )
        .route("/api/v1/creator/me/uploads/bulk", post(bulk::bulk_uploads))
}
