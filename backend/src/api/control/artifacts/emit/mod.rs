use super::spec::{
    LiveRuntimeVariantSpec, build_live_runtime_variant_specs, collaboration_audio_relative_path,
    collaboration_engine_relative_path, collaboration_program_relative_path,
};
use super::*;
use crate::api::collab::{
    build_collaboration_runtime_response_for_host, fetch_active_collaboration_session_for_broadcast,
};
use crate::models::{
    CollaborationAudioRoute, CollaborationExecutionPlan, CollaborationProgramRoute,
};

mod collab;
mod live;
mod manifest;

use collab::emit_collaboration_route_artifacts;
use live::{emit_live_archive_artifacts, emit_live_packaging_artifacts};

pub(crate) async fn sync_live_runtime_output_artifacts(
    state: &SharedState,
    session: &LiveIngestSession,
    output: &LiveRuntimeOutput,
) -> AppResult<()> {
    if matches!(output.packaging_status.as_str(), "ready" | "complete") {
        emit_live_packaging_artifacts(state, session, output).await?;
    }
    if matches!(output.archive_status.as_str(), "finalizing" | "complete") {
        emit_live_archive_artifacts(state, session, output).await?;
    }
    emit_collaboration_route_artifacts(state, session, output).await?;
    Ok(())
}
