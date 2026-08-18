use super::manifest::{
    RoutedVariantSpec, render_routed_master_manifest, render_routed_variant_playlist,
};
use super::*;
use crate::api::control::artifacts::spec::{
    build_collaboration_runtime_bundle, collaboration_bundle_relative_path,
    collaboration_media_relative_path, collaboration_return_relative_path,
};
use crate::api::media::build_collaboration_media_runtime;

pub(super) async fn emit_collaboration_route_artifacts(
    state: &SharedState,
    session: &LiveIngestSession,
    output: &LiveRuntimeOutput,
) -> AppResult<()> {
    let Some(collaboration_session) =
        fetch_active_collaboration_session_for_broadcast(&state.pool, &session.broadcast_id)
            .await?
    else {
        return Ok(());
    };
    let runtime =
        build_collaboration_runtime_response_for_host(&state.pool, collaboration_session).await?;
    let variants = build_live_runtime_variant_specs(session, output)?;
    let runtime_bundle = build_collaboration_runtime_bundle(session, &runtime.topology)?;
    let media_runtime = build_collaboration_media_runtime(&runtime_bundle)?;
    for target in &media_runtime.output_targets {
        emit_collaboration_target_artifact(state, output, target, &variants).await?;
    }
    for program in &runtime.topology.programs {
        emit_collaboration_program_artifact(state, session, program).await?;
    }
    for route in &runtime.topology.audio {
        emit_collaboration_audio_artifact(state, session, route).await?;
    }
    for route in &media_runtime.return_targets {
        emit_collaboration_return_artifact(state, session, route).await?;
    }
    emit_collaboration_engine_artifact(state, session, &runtime.topology.engine).await?;
    emit_collaboration_runtime_bundle_artifact(state, session, &runtime_bundle).await?;
    emit_collaboration_media_runtime_artifact(state, session, &media_runtime).await?;
    Ok(())
}

async fn emit_collaboration_target_artifact(
    state: &SharedState,
    output: &LiveRuntimeOutput,
    target: &crate::models::CollaborationMediaTarget,
    variants: &[LiveRuntimeVariantSpec],
) -> AppResult<()> {
    let Some(relative_path) = target.relative_path.as_deref() else {
        return Ok(());
    };
    match target.output_kind.as_str() {
        "mirror_channel" if target.playback_enabled => {
            emit_collaboration_mirror_manifest(state, &relative_path, variants, output).await?;
        }
        "archive" if target.recording_enabled => {
            emit_collaboration_archive_alias(state, output, relative_path).await?;
        }
        _ => {}
    }
    Ok(())
}

async fn emit_collaboration_mirror_manifest(
    state: &SharedState,
    manifest_relative_path: &str,
    variants: &[LiveRuntimeVariantSpec],
    output: &LiveRuntimeOutput,
) -> AppResult<()> {
    let manifest_path = media_path_for_relative(state, manifest_relative_path);
    ensure_parent_dir(&manifest_path).await?;

    let manifest_dir = FsPath::new(manifest_relative_path)
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| {
            AppError::Internal("collaboration mirror manifest missing parent".to_string())
        })?;

    let route_variants = variants
        .iter()
        .map(|variant| RoutedVariantSpec::new(&manifest_dir, variant))
        .collect::<Vec<_>>();

    for variant in &route_variants {
        emit_routed_variant_playlist(state, variant, output).await?;
    }

    tokio::fs::write(
        &manifest_path,
        render_routed_master_manifest(&route_variants, output),
    )
    .await
    .map_err(AppError::Io)?;
    Ok(())
}

async fn emit_routed_variant_playlist(
    state: &SharedState,
    variant: &RoutedVariantSpec,
    output: &LiveRuntimeOutput,
) -> AppResult<()> {
    let playlist_path = media_path_for_relative(state, &variant.relative_playlist_path);
    ensure_parent_dir(&playlist_path).await?;
    tokio::fs::write(
        &playlist_path,
        render_routed_variant_playlist(variant, output),
    )
    .await
    .map_err(AppError::Io)?;
    Ok(())
}

async fn emit_collaboration_archive_alias(
    state: &SharedState,
    output: &LiveRuntimeOutput,
    archive_relative_path: &str,
) -> AppResult<()> {
    let Some(source_relative_path) = output.archive_relative_path.as_deref() else {
        return Ok(());
    };
    let source_path = media_path_for_relative(state, source_relative_path);
    let Ok(source_bytes) = tokio::fs::read(&source_path).await else {
        return Ok(());
    };
    let archive_path = media_path_for_relative(state, archive_relative_path);
    ensure_parent_dir(&archive_path).await?;
    tokio::fs::write(&archive_path, source_bytes)
        .await
        .map_err(AppError::Io)?;
    Ok(())
}

async fn emit_collaboration_program_artifact(
    state: &SharedState,
    session: &LiveIngestSession,
    program: &CollaborationProgramRoute,
) -> AppResult<()> {
    let relative_path = collaboration_program_relative_path(session, program);
    let path = media_path_for_relative(state, &relative_path);
    ensure_parent_dir(&path).await?;
    tokio::fs::write(
        &path,
        serde_json::to_vec_pretty(program)
            .map_err(|error| AppError::Internal(error.to_string()))?,
    )
    .await
    .map_err(AppError::Io)?;
    Ok(())
}

async fn emit_collaboration_audio_artifact(
    state: &SharedState,
    session: &LiveIngestSession,
    route: &CollaborationAudioRoute,
) -> AppResult<()> {
    let relative_path = collaboration_audio_relative_path(session, route);
    let path = media_path_for_relative(state, &relative_path);
    ensure_parent_dir(&path).await?;
    tokio::fs::write(
        &path,
        serde_json::to_vec_pretty(route).map_err(|error| AppError::Internal(error.to_string()))?,
    )
    .await
    .map_err(AppError::Io)?;
    Ok(())
}

async fn emit_collaboration_return_artifact(
    state: &SharedState,
    session: &LiveIngestSession,
    route: &crate::models::CollaborationMediaReturn,
) -> AppResult<()> {
    let relative_path = collaboration_return_relative_path(session, route);
    let path = media_path_for_relative(state, &relative_path);
    ensure_parent_dir(&path).await?;
    tokio::fs::write(
        &path,
        serde_json::to_vec_pretty(route).map_err(|error| AppError::Internal(error.to_string()))?,
    )
    .await
    .map_err(AppError::Io)?;
    Ok(())
}

async fn emit_collaboration_engine_artifact(
    state: &SharedState,
    session: &LiveIngestSession,
    engine: &CollaborationExecutionPlan,
) -> AppResult<()> {
    let relative_path = collaboration_engine_relative_path(session);
    let path = media_path_for_relative(state, &relative_path);
    ensure_parent_dir(&path).await?;
    tokio::fs::write(
        &path,
        serde_json::to_vec_pretty(engine).map_err(|error| AppError::Internal(error.to_string()))?,
    )
    .await
    .map_err(AppError::Io)?;
    Ok(())
}

async fn emit_collaboration_runtime_bundle_artifact(
    state: &SharedState,
    session: &LiveIngestSession,
    bundle: &crate::models::CollaborationRuntimeBundle,
) -> AppResult<()> {
    let relative_path = collaboration_bundle_relative_path(session);
    let path = media_path_for_relative(state, &relative_path);
    ensure_parent_dir(&path).await?;
    tokio::fs::write(
        &path,
        serde_json::to_vec_pretty(bundle).map_err(|error| AppError::Internal(error.to_string()))?,
    )
    .await
    .map_err(AppError::Io)?;
    Ok(())
}

async fn emit_collaboration_media_runtime_artifact(
    state: &SharedState,
    session: &LiveIngestSession,
    runtime: &crate::models::CollaborationMediaRuntime,
) -> AppResult<()> {
    let relative_path = collaboration_media_relative_path(session);
    let path = media_path_for_relative(state, &relative_path);
    ensure_parent_dir(&path).await?;
    tokio::fs::write(
        &path,
        serde_json::to_vec_pretty(runtime)
            .map_err(|error| AppError::Internal(error.to_string()))?,
    )
    .await
    .map_err(AppError::Io)?;
    Ok(())
}
