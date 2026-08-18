use super::*;
use crate::api::control::queries::canonical_live_runtime_spec_relative_path;
use crate::models::LiveRuntimeTarget;

mod build;
mod collab;
mod doc;
mod health;
mod targets;
mod variant;

use build::build_live_runtime_spec;
pub(crate) use collab::build_collaboration_runtime_bundle;
use collab::{build_live_runtime_collaboration_spec, sync_runtime_target_dependents};
pub(in crate::api::control::artifacts) use collab::{
    collaboration_audio_relative_path, collaboration_bundle_relative_path,
    collaboration_engine_relative_path, collaboration_media_relative_path,
    collaboration_program_relative_path, collaboration_return_relative_path,
    collaboration_route_relative_path,
};
use doc::{LiveRuntimeCollaborationSpec, LiveRuntimeSpecDocument};
use health::build_live_runtime_health_spec;
use targets::build_live_runtime_targets;
pub(in crate::api::control::artifacts) use variant::{
    LiveRuntimeVariantSpec, build_live_runtime_variant_specs,
};

pub(crate) async fn provision_live_runtime_workspace(
    state: &SharedState,
    session: &LiveIngestSession,
) -> AppResult<String> {
    let manifest_path = media_path_for_relative(
        state,
        &canonical_live_runtime_manifest_relative_path(session),
    );
    let archive_path = media_path_for_relative(
        state,
        &canonical_live_runtime_archive_relative_path(session),
    );
    let archive_staging_path = media_path_for_relative(
        state,
        &canonical_live_runtime_archive_staging_relative_path(session),
    );
    let spec_relative_path = canonical_live_runtime_spec_relative_path(session);
    let spec_path = media_path_for_relative(state, &spec_relative_path);

    ensure_parent_dir(&manifest_path).await?;
    ensure_parent_dir(&archive_path).await?;
    ensure_parent_dir(&archive_staging_path).await?;
    ensure_parent_dir(&spec_path).await?;
    let output = fetch_live_runtime_output_for_session(&state.pool, &session.id).await?;
    let variant_output = output.as_ref().ok_or_else(|| {
        AppError::Internal("missing live runtime output while provisioning workspace".to_string())
    })?;
    for variant in build_live_runtime_variant_specs(session, variant_output)? {
        let playlist_path = media_path_for_relative(state, &variant.relative_playlist_path);
        ensure_parent_dir(&playlist_path).await?;
    }

    Ok(spec_relative_path)
}

pub(crate) async fn persist_live_runtime_spec(
    state: &SharedState,
    session: &LiveIngestSession,
) -> AppResult<String> {
    let spec_relative_path = provision_live_runtime_workspace(state, session).await?;
    let output = fetch_live_runtime_output_for_session(&state.pool, &session.id)
        .await?
        .ok_or_else(|| {
            AppError::Internal("missing live runtime output while persisting spec".to_string())
        })?;
    let spec_path = media_path_for_relative(state, &spec_relative_path);

    let spec = build_live_runtime_spec(state, session, &output, &spec_relative_path).await?;
    let target_sync = sync_live_runtime_targets(
        &state.pool,
        session,
        &build_live_runtime_targets(session, &spec, &output),
    )
    .await?;
    if target_sync.created > 0 || target_sync.updated > 0 || target_sync.removed > 0 {
        write_live_ingest_event(
            &state.pool,
            &session.id,
            &session.creator_id,
            &session.broadcast_id,
            "runtime_targets_synced",
            json!({
                "created": target_sync.created,
                "updated": target_sync.updated,
                "removed": target_sync.removed,
                "runtimeState": output.runtime_state,
                "packagingStatus": output.packaging_status,
                "archiveStatus": output.archive_status,
            }),
        )
        .await?;
        sync_runtime_target_dependents(state, session).await?;
    }

    tokio::fs::write(
        &spec_path,
        serde_json::to_vec_pretty(&spec).map_err(|error| AppError::Internal(error.to_string()))?,
    )
    .await
    .map_err(AppError::Io)?;

    Ok(spec_relative_path)
}
