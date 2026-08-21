use super::manifest::{render_master_manifest, render_variant_playlist};
use super::*;

const SYNTHETIC_TS_SEGMENT_BYTES: &[u8] = b"G@LIFESTREAM_SYNTHETIC_TS_SEGMENT_000";
const SYNTHETIC_FMP4_INIT_BYTES: &[u8] = b"\x00\x00\x00\x18ftypiso6\x00\x00\x00\x01iso6mp41";
const SYNTHETIC_FMP4_PART_BYTES: &[u8] = b"\x00\x00\x00\x10stypmsdh\x00\x00\x00\x00";
const SYNTHETIC_FMP4_SEGMENT_BYTES: &[u8] = b"\x00\x00\x00\x18moof\x00\x00\x00\x00mdatLIFESTREAM";

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
    sync_live_playback_persistence(state, session, output).await?;

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
    emit_variant_media_artifacts(state, variant, output).await?;
    Ok(())
}

async fn emit_variant_media_artifacts(
    state: &SharedState,
    variant: &LiveRuntimeVariantSpec,
    output: &LiveRuntimeOutput,
) -> AppResult<()> {
    let variant_dir = media_path_for_relative(state, &variant.output_relative_dir);
    ensure_parent_dir(&variant_dir.join("placeholder")).await?;

    if output.segment_format == "fmp4" {
        tokio::fs::write(variant_dir.join("init.mp4"), SYNTHETIC_FMP4_INIT_BYTES)
            .await
            .map_err(AppError::Io)?;
        tokio::fs::write(variant_dir.join("part_000_000.m4s"), SYNTHETIC_FMP4_PART_BYTES)
            .await
            .map_err(AppError::Io)?;
        tokio::fs::write(variant_dir.join("segment_000.m4s"), SYNTHETIC_FMP4_SEGMENT_BYTES)
            .await
            .map_err(AppError::Io)?;
    } else {
        tokio::fs::write(variant_dir.join("segment_000.ts"), SYNTHETIC_TS_SEGMENT_BYTES)
            .await
            .map_err(AppError::Io)?;
    }

    Ok(())
}

async fn sync_live_playback_persistence(
    state: &SharedState,
    session: &LiveIngestSession,
    output: &LiveRuntimeOutput,
) -> AppResult<()> {
    let Some(manifest_relative_path) = output.manifest_relative_path.as_deref() else {
        return Ok(());
    };

    let creator = fetch_creator_profile(&state.pool, &session.creator_id).await?;
    let broadcast =
        fetch_broadcast_by_id(&state.pool, &session.creator_id, &session.broadcast_id).await?;
    let live_stream_id = format!("lv-{}-live", creator.handle);
    let upload_job_id = format!("upjob-live-{}", session.id);
    let asset_id = format!("ast-live-{}", session.id);
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO upload_jobs (
            id, creator_id, upload_id, series_id, kind, source_type, status, title,
            intended_visibility, bytes_expected, bytes_received, storage_key, created_at,
            updated_at, published_content_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            status = excluded.status,
            title = excluded.title,
            intended_visibility = excluded.intended_visibility,
            bytes_received = excluded.bytes_received,
            storage_key = excluded.storage_key,
            updated_at = excluded.updated_at,
            published_content_id = excluded.published_content_id
        "#,
    )
    .bind(&upload_job_id)
    .bind(&session.creator_id)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind("live")
    .bind("live_runtime")
    .bind("ready")
    .bind(&broadcast.title)
    .bind("public")
    .bind(0_i64)
    .bind(0_i64)
    .bind(manifest_relative_path)
    .bind(&now)
    .bind(&now)
    .bind(Some(live_stream_id.clone()))
    .execute(&state.pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO media_assets (
            id, creator_id, upload_job_id, upload_id, series_id, kind, title, status, visibility,
            source_relative_path, poster_relative_path, playback_relative_path, mime_type,
            checksum_sha256, container_format, file_size_bytes, duration_sec, width, height,
            frame_rate, video_codec, audio_codec, has_video, has_audio, created_at, updated_at,
            processed_at, published_content_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            status = excluded.status,
            visibility = excluded.visibility,
            source_relative_path = excluded.source_relative_path,
            poster_relative_path = excluded.poster_relative_path,
            playback_relative_path = excluded.playback_relative_path,
            mime_type = excluded.mime_type,
            container_format = excluded.container_format,
            updated_at = excluded.updated_at,
            processed_at = excluded.processed_at,
            published_content_id = excluded.published_content_id
        "#,
    )
    .bind(&asset_id)
    .bind(&session.creator_id)
    .bind(&upload_job_id)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind("live")
    .bind(&broadcast.title)
    .bind("ready")
    .bind("public")
    .bind(manifest_relative_path)
    .bind(broadcast.thumbnail.clone())
    .bind(Some(manifest_relative_path.to_string()))
    .bind("application/vnd.apple.mpegurl")
    .bind(Option::<String>::None)
    .bind(Some("hls".to_string()))
    .bind(0_i64)
    .bind(0.0_f64)
    .bind(Option::<i64>::None)
    .bind(Option::<i64>::None)
    .bind(Option::<f64>::None)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(1_i64)
    .bind(1_i64)
    .bind(&now)
    .bind(&now)
    .bind(Some(&now))
    .bind(Some(live_stream_id.clone()))
    .execute(&state.pool)
    .await?;

    sqlx::query(
        r#"
        UPDATE live_streams
        SET playback_asset_id = ?,
            poster_relative_path = ?,
            playback_relative_path = ?
        WHERE id = ?
        "#,
    )
    .bind(&asset_id)
    .bind(broadcast.thumbnail)
    .bind(manifest_relative_path)
    .bind(live_stream_id)
    .execute(&state.pool)
    .await?;

    Ok(())
}
