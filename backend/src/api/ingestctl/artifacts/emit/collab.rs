use super::manifest::{
    RoutedVariantSpec, build_minimal_mp4_bytes, build_minimal_mp4_fragment_bytes,
    build_minimal_ts_segment_bytes, render_master_manifest, render_routed_master_manifest,
    render_routed_variant_playlist,
};
use super::*;

pub(super) async fn emit_collaboration_route_artifacts(
    state: &SharedState,
    session: &LiveIngestSession,
    output: &LiveRuntimeOutput,
) -> AppResult<()> {
    let Some(collaboration_session) =
        fetch_active_collaboration_session_for_broadcast(&state.pool, &session.broadcast_id).await?
    else {
        return Ok(());
    };
    let runtime =
        build_collaboration_runtime_response_for_host(&state.pool, collaboration_session).await?;
    let variants = build_live_runtime_variant_specs(session, output)?;
    for route in &runtime.topology.outputs {
        emit_collaboration_route_artifact(state, session, route, &variants, output).await?;
    }
    Ok(())
}

async fn emit_collaboration_route_artifact(
    state: &SharedState,
    session: &LiveIngestSession,
    route: &CollaborationOutputRoute,
    variants: &[LiveRuntimeVariantSpec],
    output: &LiveRuntimeOutput,
) -> AppResult<()> {
    let Some(relative_path) = collaboration_route_relative_path(session, route) else {
        return Ok(());
    };
    match route.output_kind.as_str() {
        "host_channel" => {
            let manifest_path = media_path_for_relative(state, &relative_path);
            ensure_parent_dir(&manifest_path).await?;
            tokio::fs::write(
                &manifest_path,
                render_master_manifest(variants, output, session),
            )
            .await
            .map_err(AppError::Io)?;
        }
        "mirror_channel" if route.playback_enabled => {
            emit_collaboration_mirror_manifest(state, &relative_path, variants, output).await?;
        }
        "archive" if route.recording_enabled => {
            let archive_path = media_path_for_relative(state, &relative_path);
            ensure_parent_dir(&archive_path).await?;
            tokio::fs::write(
                &archive_path,
                build_minimal_mp4_bytes(&format!(
                    "collaboration-archive:{}:{}:{}",
                    session.broadcast_id, route.id, route.route_state
                )),
            )
            .await
            .map_err(AppError::Io)?;
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
        emit_routed_variant_media_placeholders(state, variant, output).await?;
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
    tokio::fs::write(&playlist_path, render_routed_variant_playlist(variant, output))
        .await
        .map_err(AppError::Io)?;
    Ok(())
}

async fn emit_routed_variant_media_placeholders(
    state: &SharedState,
    variant: &RoutedVariantSpec,
    output: &LiveRuntimeOutput,
) -> AppResult<()> {
    if output.segment_format == "fmp4" {
        let init_relative_path = format!("{}/init.mp4", variant.output_relative_dir);
        let init_path = media_path_for_relative(state, &init_relative_path);
        ensure_parent_dir(&init_path).await?;
        tokio::fs::write(&init_path, build_minimal_mp4_bytes("collaboration-init"))
            .await
            .map_err(AppError::Io)?;

        let segment_relative_path = format!("{}/segment_000.m4s", variant.output_relative_dir);
        let segment_path = media_path_for_relative(state, &segment_relative_path);
        tokio::fs::write(
            &segment_path,
            build_minimal_mp4_fragment_bytes("collaboration-segment"),
        )
        .await
        .map_err(AppError::Io)?;

        if output.partial_segments_enabled {
            let part_relative_path = format!("{}/part_000_000.m4s", variant.output_relative_dir);
            let part_path = media_path_for_relative(state, &part_relative_path);
            tokio::fs::write(
                &part_path,
                build_minimal_mp4_fragment_bytes("collaboration-part"),
            )
            .await
            .map_err(AppError::Io)?;
        }
    } else {
        let segment_relative_path = format!("{}/segment_000.ts", variant.output_relative_dir);
        let segment_path = media_path_for_relative(state, &segment_relative_path);
        ensure_parent_dir(&segment_path).await?;
        tokio::fs::write(&segment_path, build_minimal_ts_segment_bytes())
            .await
            .map_err(AppError::Io)?;
    }
    Ok(())
}
