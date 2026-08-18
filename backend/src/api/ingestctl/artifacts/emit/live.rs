use super::manifest::{
    build_live_archive_payload, build_minimal_mp4_bytes, build_minimal_mp4_fragment_bytes,
    build_minimal_ts_segment_bytes, render_master_manifest, render_variant_playlist,
};
use super::*;

pub(super) async fn emit_live_packaging_artifacts(
    state: &SharedState,
    session: &LiveIngestSession,
    output: &LiveRuntimeOutput,
) -> AppResult<()> {
    let Some(manifest_relative_path) = output.manifest_relative_path.as_deref() else {
        return Ok(());
    };

    let variants = build_live_runtime_variant_specs(session, output)?;
    for variant in &variants {
        emit_variant_playlist(state, variant, output).await?;
        emit_variant_media_placeholders(state, variant, output).await?;
    }

    let manifest_path = media_path_for_relative(state, manifest_relative_path);
    ensure_parent_dir(&manifest_path).await?;
    tokio::fs::write(
        &manifest_path,
        render_master_manifest(&variants, output, session),
    )
    .await
    .map_err(AppError::Io)?;

    Ok(())
}

pub(super) async fn emit_live_archive_artifacts(
    state: &SharedState,
    session: &LiveIngestSession,
    output: &LiveRuntimeOutput,
) -> AppResult<()> {
    let Some(archive_relative_path) = output.archive_relative_path.as_deref() else {
        return Ok(());
    };

    let staging_relative_path = canonical_live_runtime_archive_staging_relative_path(session);
    let archive_path = media_path_for_relative(state, archive_relative_path);
    let staging_path = media_path_for_relative(state, &staging_relative_path);
    let payload = build_live_archive_payload(session, output);

    ensure_parent_dir(&archive_path).await?;
    ensure_parent_dir(&staging_path).await?;
    tokio::fs::write(&staging_path, payload.clone())
        .await
        .map_err(AppError::Io)?;
    tokio::fs::write(&archive_path, payload)
        .await
        .map_err(AppError::Io)?;

    Ok(())
}

async fn emit_variant_playlist(
    state: &SharedState,
    variant: &LiveRuntimeVariantSpec,
    output: &LiveRuntimeOutput,
) -> AppResult<()> {
    let playlist_path = media_path_for_relative(state, &variant.relative_playlist_path);
    ensure_parent_dir(&playlist_path).await?;
    tokio::fs::write(&playlist_path, render_variant_playlist(variant, output))
        .await
        .map_err(AppError::Io)?;
    Ok(())
}

async fn emit_variant_media_placeholders(
    state: &SharedState,
    variant: &LiveRuntimeVariantSpec,
    output: &LiveRuntimeOutput,
) -> AppResult<()> {
    if output.segment_format == "fmp4" {
        let init_relative_path = format!("{}/init.mp4", variant.output_relative_dir);
        let init_path = media_path_for_relative(state, &init_relative_path);
        ensure_parent_dir(&init_path).await?;
        tokio::fs::write(&init_path, build_minimal_mp4_bytes("runtime-init"))
            .await
            .map_err(AppError::Io)?;

        let segment_relative_path = format!("{}/segment_000.m4s", variant.output_relative_dir);
        let segment_path = media_path_for_relative(state, &segment_relative_path);
        tokio::fs::write(&segment_path, build_minimal_mp4_fragment_bytes("runtime-segment"))
            .await
            .map_err(AppError::Io)?;

        if output.partial_segments_enabled {
            let part_relative_path = format!("{}/part_000_000.m4s", variant.output_relative_dir);
            let part_path = media_path_for_relative(state, &part_relative_path);
            tokio::fs::write(&part_path, build_minimal_mp4_fragment_bytes("runtime-part"))
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
