use super::manifest::{render_master_manifest, render_variant_playlist};
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
    ensure_parent_dir(&archive_path).await?;
    ensure_parent_dir(&staging_path).await?;
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
