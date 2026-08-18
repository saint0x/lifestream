use super::*;

pub(super) struct UploadPlaybackTarget {
    pub(super) creator_id: String,
    pub(super) upload: Upload,
    pub(super) asset: MediaAsset,
}

pub(super) struct LivePlaybackTarget {
    pub(super) creator_id: String,
    pub(super) asset_id: String,
    pub(super) title: String,
    pub(super) poster_relative_path: Option<String>,
    pub(super) playback_relative_path: String,
    pub(super) asset: MediaAsset,
}

pub(super) struct PlaybackSessionRecord {
    pub(super) id: String,
    pub(super) auth_session_id: Option<String>,
    pub(super) user_id: Option<String>,
    pub(super) creator_id: Option<String>,
    pub(super) asset_id: String,
    pub(super) content_id: String,
    pub(super) content_kind: String,
    pub(super) access_scope: String,
    pub(super) created_at: String,
    pub(super) expires_at: String,
    pub(super) last_used_at: String,
}

pub(super) struct UploadAccessTerms {
    pub(super) access_policy: String,
    pub(super) access_tier_id: Option<String>,
    pub(super) price_cents: Option<i64>,
    pub(super) currency: Option<String>,
    pub(super) rental_window_hours: Option<i64>,
}

pub(super) struct PlaybackAccessDecision {
    pub(super) access_scope: String,
}

pub(super) async fn fetch_upload_playback_target(
    pool: &SqlitePool,
    upload_id: &str,
) -> AppResult<UploadPlaybackTarget> {
    let row = sqlx::query(
        r#"
        SELECT creator_id
        FROM uploads
        WHERE id = ?
        "#,
    )
    .bind(upload_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let creator_id: String = row.get("creator_id");
    let upload = fetch_upload_by_id(pool, &creator_id, upload_id).await?;
    let asset = fetch_media_asset_by_upload_id(pool, &creator_id, upload_id).await?;
    if asset.status != "ready" && asset.status != "published" {
        return Err(AppError::BadRequest(
            "asset is not ready for playback".to_string(),
        ));
    }
    Ok(UploadPlaybackTarget {
        creator_id,
        upload,
        asset,
    })
}

pub(super) async fn fetch_live_stream_playback_target(
    pool: &SqlitePool,
    stream_id: &str,
) -> AppResult<LivePlaybackTarget> {
    let fresh_cutoff = stale_live_ingest_cutoff();
    let row = sqlx::query(
        r#"
        SELECT ls.id, ls.title, ls.playback_asset_id, ls.poster_relative_path, ls.playback_relative_path, cp.id AS creator_id
        FROM live_streams ls
        JOIN streamers s ON s.id = ls.streamer_id
        JOIN creator_profiles cp ON cp.handle = s.handle
        WHERE ls.id = ?
          AND (
            EXISTS (
                SELECT 1
                FROM live_ingest_sessions lis
                WHERE lis.creator_id = cp.id
                  AND lis.status = 'connected'
                  AND lis.last_heartbeat_at >= ?
            )
            OR EXISTS (
                SELECT 1
                FROM collaboration_mirror_pickups cmp
                JOIN live_ingest_sessions lis
                  ON lis.creator_id = cmp.host_creator_id
                 AND lis.broadcast_id = cmp.source_broadcast_id
                WHERE cmp.guest_creator_id = cp.id
                  AND cmp.guest_broadcast_id = cp.current_broadcast_id
                  AND cmp.state = 'active'
                  AND lis.status = 'connected'
                  AND lis.last_heartbeat_at >= ?
            )
          )
        "#,
    )
    .bind(stream_id)
    .bind(&fresh_cutoff)
    .bind(&fresh_cutoff)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let playback_asset_id = row
        .get::<Option<String>, _>("playback_asset_id")
        .ok_or_else(|| AppError::BadRequest("live playback asset unavailable".to_string()))?;
    let playback_relative_path = row
        .get::<Option<String>, _>("playback_relative_path")
        .ok_or_else(|| AppError::BadRequest("live playback manifest unavailable".to_string()))?;

    let asset_exists =
        sqlx::query("SELECT 1 FROM media_assets WHERE id = ? AND status IN ('ready', 'published')")
            .bind(&playback_asset_id)
            .fetch_optional(pool)
            .await?
            .is_some();
    if !asset_exists {
        return Err(AppError::BadRequest(
            "live playback asset is not ready".to_string(),
        ));
    }

    let creator_id: String = row.get("creator_id");
    let asset = fetch_media_asset_by_id_any_creator(pool, &playback_asset_id).await?;

    Ok(LivePlaybackTarget {
        creator_id,
        asset_id: playback_asset_id,
        title: row.get("title"),
        poster_relative_path: row.get("poster_relative_path"),
        playback_relative_path,
        asset,
    })
}

pub(super) async fn resolve_upload_playback_access(
    pool: &SqlitePool,
    identity: Option<&RequestIdentity>,
    target: &UploadPlaybackTarget,
) -> AppResult<PlaybackAccessDecision> {
    let now = Utc::now().to_rfc3339();
    let is_creator_owner = identity
        .and_then(|identity| identity.creator_id.as_deref())
        .map(|creator_id| creator_id == target.creator_id)
        .unwrap_or(false);

    if target.upload.status != "published" && !is_creator_owner {
        return Err(AppError::Forbidden);
    }
    if target
        .upload
        .release_at
        .as_ref()
        .is_some_and(|release_at| release_at > &now)
        && !is_creator_owner
    {
        return Err(AppError::Forbidden);
    }

    match target.upload.visibility.as_str() {
        "public" | "unlisted" => {}
        "private" => {
            if !is_creator_owner {
                return Err(AppError::Forbidden);
            }
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "unsupported visibility for playback: {other}"
            )));
        }
    }

    if is_creator_owner {
        return Ok(PlaybackAccessDecision {
            access_scope: "owner".to_string(),
        });
    }

    match target.upload.access_policy.as_str() {
        "free" => Ok(PlaybackAccessDecision {
            access_scope: "free".to_string(),
        }),
        "subscription" => {
            let identity = identity.ok_or(AppError::Unauthorized)?;
            let membership = fetch_active_creator_membership(
                pool,
                &identity.user_id,
                &target.creator_id,
                target.upload.access_tier_id.as_deref(),
            )
            .await?;
            if membership {
                Ok(PlaybackAccessDecision {
                    access_scope: "subscription".to_string(),
                })
            } else {
                Err(AppError::PaymentRequired(
                    "active creator subscription required".to_string(),
                ))
            }
        }
        "purchase" => {
            let identity = identity.ok_or(AppError::Unauthorized)?;
            let has_purchase =
                fetch_valid_content_purchase(pool, &identity.user_id, &target.upload.id).await?;
            if has_purchase {
                Ok(PlaybackAccessDecision {
                    access_scope: "purchase".to_string(),
                })
            } else {
                Err(AppError::PaymentRequired(
                    "purchase required before playback".to_string(),
                ))
            }
        }
        "subscription_or_purchase" => {
            let identity = identity.ok_or(AppError::Unauthorized)?;
            let membership = fetch_active_creator_membership(
                pool,
                &identity.user_id,
                &target.creator_id,
                target.upload.access_tier_id.as_deref(),
            )
            .await?;
            if membership {
                return Ok(PlaybackAccessDecision {
                    access_scope: "subscription".to_string(),
                });
            }
            let has_purchase =
                fetch_valid_content_purchase(pool, &identity.user_id, &target.upload.id).await?;
            if has_purchase {
                Ok(PlaybackAccessDecision {
                    access_scope: "purchase".to_string(),
                })
            } else {
                Err(AppError::PaymentRequired(
                    "subscription or purchase required before playback".to_string(),
                ))
            }
        }
        other => Err(AppError::BadRequest(format!(
            "unsupported access policy for playback: {other}"
        ))),
    }
}

pub(super) fn resolve_upload_access_terms(
    access_policy: Option<String>,
    access_tier_id: Option<String>,
    price_cents: Option<i64>,
    currency: Option<String>,
    rental_window_hours: Option<i64>,
) -> AppResult<UploadAccessTerms> {
    let access_policy = access_policy.unwrap_or_else(|| "free".to_string());
    let currency = currency
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| !value.is_empty());
    let access_tier_id = access_tier_id.filter(|value| !value.trim().is_empty());

    match access_policy.as_str() {
        "free" => Ok(UploadAccessTerms {
            access_policy,
            access_tier_id: None,
            price_cents: None,
            currency: None,
            rental_window_hours: None,
        }),
        "subscription" => {
            if price_cents.is_some() {
                return Err(AppError::BadRequest(
                    "subscription access cannot define a direct purchase price".to_string(),
                ));
            }
            if rental_window_hours.is_some() {
                return Err(AppError::BadRequest(
                    "subscription access cannot define a rental window".to_string(),
                ));
            }
            Ok(UploadAccessTerms {
                access_policy,
                access_tier_id,
                price_cents: None,
                currency: None,
                rental_window_hours: None,
            })
        }
        "purchase" | "subscription_or_purchase" => {
            let price_cents = price_cents.ok_or_else(|| {
                AppError::BadRequest("paid access requires a price in cents".to_string())
            })?;
            if price_cents <= 0 {
                return Err(AppError::BadRequest(
                    "paid access price must be greater than zero".to_string(),
                ));
            }
            if rental_window_hours.is_some_and(|hours| hours <= 0) {
                return Err(AppError::BadRequest(
                    "rental window hours must be greater than zero".to_string(),
                ));
            }
            let is_purchase_only = access_policy == "purchase";
            Ok(UploadAccessTerms {
                access_policy,
                access_tier_id: if is_purchase_only {
                    None
                } else {
                    access_tier_id
                },
                price_cents: Some(price_cents),
                currency: Some(currency.unwrap_or_else(|| "USD".to_string())),
                rental_window_hours,
            })
        }
        other => Err(AppError::BadRequest(format!(
            "unsupported access policy: {other}"
        ))),
    }
}

pub(super) async fn fetch_active_creator_membership(
    pool: &SqlitePool,
    user_id: &str,
    creator_id: &str,
    required_tier_id: Option<&str>,
) -> AppResult<bool> {
    let now = Utc::now().to_rfc3339();
    let row = sqlx::query(
        r#"
        SELECT cms.tier_id, req.rank AS required_rank, actual.rank AS actual_rank
        FROM creator_memberships cms
        JOIN creator_subscriber_tiers actual ON actual.id = cms.tier_id
        LEFT JOIN creator_subscriber_tiers req ON req.id = ?
        WHERE cms.user_id = ?
          AND cms.creator_id = ?
          AND cms.status IN ('active', 'canceling')
          AND (
                COALESCE(cms.ends_at, cms.renews_at) IS NULL
                OR COALESCE(cms.ends_at, cms.renews_at) > ?
              )
        "#,
    )
    .bind(required_tier_id)
    .bind(user_id)
    .bind(creator_id)
    .bind(&now)
    .fetch_optional(pool)
    .await?;

    Ok(match row {
        Some(row) => {
            let required_rank = row.get::<Option<i64>, _>("required_rank").unwrap_or(1);
            let actual_rank: i64 = row.get("actual_rank");
            actual_rank >= required_rank
        }
        None => false,
    })
}

pub(super) async fn fetch_valid_content_purchase(
    pool: &SqlitePool,
    user_id: &str,
    upload_id: &str,
) -> AppResult<bool> {
    let now = Utc::now().to_rfc3339();
    let count: i64 = sqlx::query(
        r#"
        SELECT COUNT(*)
        FROM content_purchases
        WHERE user_id = ?
          AND upload_id = ?
          AND status = 'active'
          AND (expires_at IS NULL OR expires_at > ?)
        "#,
    )
    .bind(user_id)
    .bind(upload_id)
    .bind(&now)
    .fetch_one(pool)
    .await?
    .get(0);
    Ok(count > 0)
}

pub(super) fn playback_session_from_record(session: &PlaybackSessionRecord) -> PlaybackSession {
    PlaybackSession {
        id: session.id.clone(),
        content_id: session.content_id.clone(),
        content_kind: session.content_kind.clone(),
        access_scope: session.access_scope.clone(),
        created_at: session.created_at.clone(),
        expires_at: session.expires_at.clone(),
        last_used_at: session.last_used_at.clone(),
    }
}

pub(super) fn playback_session_record_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> PlaybackSessionRecord {
    PlaybackSessionRecord {
        id: row.get("id"),
        auth_session_id: row.get("auth_session_id"),
        user_id: row.get("user_id"),
        creator_id: row.get("creator_id"),
        asset_id: row.get("asset_id"),
        content_id: row.get("content_id"),
        content_kind: row.get("content_kind"),
        access_scope: row.get("access_scope"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        last_used_at: row.get("last_used_at"),
    }
}

pub(super) async fn fetch_playback_session_record_by_id(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<PlaybackSessionRecord> {
    let row = sqlx::query(
        r#"
        SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
               auth_session_id,
               created_at, expires_at, last_used_at
        FROM playback_sessions
        WHERE id = ?
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(playback_session_record_from_row(row))
}

pub(super) async fn validate_playback_session_record(
    pool: &SqlitePool,
    session_id: &str,
    playback_token: &str,
) -> AppResult<PlaybackSessionRecord> {
    let now = Utc::now().to_rfc3339();
    let row = sqlx::query(
        r#"
        SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
               auth_session_id,
               created_at, expires_at, last_used_at
        FROM playback_sessions
        WHERE id = ? AND token_hash = ? AND expires_at > ?
        "#,
    )
    .bind(session_id)
    .bind(crate::auth::hash_token(playback_token))
    .bind(&now)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    let session = playback_session_record_from_row(row);
    if !validate_existing_playback_session_access(pool, &session, None).await? {
        expire_playback_session_by_id(pool, &session.id).await?;
        return Err(AppError::Unauthorized);
    }

    sqlx::query("UPDATE playback_sessions SET last_used_at = ? WHERE id = ?")
        .bind(&now)
        .bind(session_id)
        .execute(pool)
        .await?;

    Ok(PlaybackSessionRecord {
        last_used_at: now,
        ..session
    })
}

pub(super) async fn validate_playback_session_record_for_path(
    pool: &SqlitePool,
    playback_token: &str,
    relative_path: &str,
) -> AppResult<PlaybackSessionRecord> {
    let now = Utc::now().to_rfc3339();
    let row = sqlx::query(
        r#"
        SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
               auth_session_id,
               created_at, expires_at, last_used_at
        FROM playback_sessions
        WHERE token_hash = ? AND expires_at > ?
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(crate::auth::hash_token(playback_token))
    .bind(&now)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    let session = playback_session_record_from_row(row);
    if relative_path.is_empty() {
        return Err(AppError::Unauthorized);
    }
    if !validate_existing_playback_session_access(pool, &session, Some(relative_path)).await? {
        expire_playback_session_by_id(pool, &session.id).await?;
        return Err(AppError::Unauthorized);
    }

    sqlx::query("UPDATE playback_sessions SET last_used_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&session.id)
        .execute(pool)
        .await?;

    Ok(PlaybackSessionRecord {
        last_used_at: now,
        ..session
    })
}

pub(super) async fn validate_existing_playback_session_access(
    pool: &SqlitePool,
    session: &PlaybackSessionRecord,
    relative_path: Option<&str>,
) -> AppResult<bool> {
    if !playback_session_auth_binding_is_active(pool, session).await? {
        return Ok(false);
    }
    if session.content_kind == "live" {
        let target = match fetch_live_stream_playback_target(pool, &session.content_id).await {
            Ok(target) => target,
            Err(_) => return Ok(false),
        };
        if target.asset_id != session.asset_id {
            return Ok(false);
        }
        if let Some(relative_path) = relative_path {
            if !path_allowed_for_paths(
                relative_path,
                &target.playback_relative_path,
                target.poster_relative_path.as_deref(),
                Some(&target.playback_relative_path),
                &[],
            ) {
                return Err(AppError::Forbidden);
            }
        }
        return Ok(true);
    }

    let target = match fetch_upload_playback_target(pool, &session.content_id).await {
        Ok(target) => target,
        Err(_) => return Ok(false),
    };
    if target.asset.id != session.asset_id {
        return Ok(false);
    }
    if let Some(relative_path) = relative_path {
        if !playback_path_allowed_for_asset(&target.asset, relative_path) {
            return Err(AppError::Forbidden);
        }
    }

    let identity = playback_request_identity_for_session(pool, session).await?;
    Ok(
        resolve_upload_playback_access(pool, identity.as_ref(), &target)
            .await
            .is_ok(),
    )
}

async fn playback_session_auth_binding_is_active(
    pool: &SqlitePool,
    session: &PlaybackSessionRecord,
) -> AppResult<bool> {
    let Some(auth_session_id) = session.auth_session_id.as_deref() else {
        return Ok(true);
    };
    let now = Utc::now().to_rfc3339();
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS count
        FROM auth_sessions
        WHERE id = ?
          AND revoked_at IS NULL
          AND (expires_at IS NULL OR expires_at > ?)
        "#,
    )
    .bind(auth_session_id)
    .bind(&now)
    .fetch_one(pool)
    .await?;
    let count: i64 = row.get("count");
    Ok(count == 1)
}

async fn playback_request_identity_for_session(
    pool: &SqlitePool,
    session: &PlaybackSessionRecord,
) -> AppResult<Option<RequestIdentity>> {
    let Some(user_id) = session.user_id.clone() else {
        return Ok(None);
    };
    let creator_id = fetch_creator_id_for_user(pool, &user_id).await?;
    Ok(Some(RequestIdentity {
        session_id: session.id.clone(),
        user_id,
        creator_id,
        scopes: Vec::new(),
    }))
}

pub(super) async fn expire_playback_session_by_id(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE playback_sessions SET expires_at = ?, last_used_at = ? WHERE id = ? AND expires_at > ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(session_id)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn expire_playback_sessions_for_upload(
    pool: &SqlitePool,
    upload_id: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE playback_sessions SET expires_at = ?, last_used_at = ? WHERE content_id = ? AND expires_at > ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(upload_id)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn expire_playback_sessions_for_auth_session(
    pool: &SqlitePool,
    auth_session_id: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE playback_sessions SET expires_at = ?, last_used_at = ? WHERE auth_session_id = ? AND expires_at > ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(auth_session_id)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn reconcile_playback_sessions_for_user(
    pool: &SqlitePool,
    user_id: &str,
    creator_id: Option<&str>,
    upload_id: Option<&str>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let rows = match (creator_id, upload_id) {
        (Some(creator_id), Some(upload_id)) => {
            sqlx::query(
                r#"
        SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
               auth_session_id, created_at, expires_at, last_used_at
        FROM playback_sessions
        WHERE user_id = ? AND creator_id = ? AND content_id = ? AND expires_at > ?
                ORDER BY expires_at ASC
                "#,
            )
            .bind(user_id)
            .bind(creator_id)
            .bind(upload_id)
            .bind(&now)
            .fetch_all(pool)
            .await?
        }
        (Some(creator_id), None) => {
            sqlx::query(
                r#"
        SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
               auth_session_id, created_at, expires_at, last_used_at
        FROM playback_sessions
        WHERE user_id = ? AND creator_id = ? AND expires_at > ?
                ORDER BY expires_at ASC
                "#,
            )
            .bind(user_id)
            .bind(creator_id)
            .bind(&now)
            .fetch_all(pool)
            .await?
        }
        (None, Some(upload_id)) => {
            sqlx::query(
                r#"
        SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
               auth_session_id, created_at, expires_at, last_used_at
        FROM playback_sessions
        WHERE user_id = ? AND content_id = ? AND expires_at > ?
                ORDER BY expires_at ASC
                "#,
            )
            .bind(user_id)
            .bind(upload_id)
            .bind(&now)
            .fetch_all(pool)
            .await?
        }
        (None, None) => {
            sqlx::query(
                r#"
        SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
               auth_session_id, created_at, expires_at, last_used_at
        FROM playback_sessions
        WHERE user_id = ? AND expires_at > ?
                ORDER BY expires_at ASC
                "#,
            )
            .bind(user_id)
            .bind(&now)
            .fetch_all(pool)
            .await?
        }
    };

    for row in rows {
        let session = playback_session_record_from_row(row);
        if !validate_existing_playback_session_access(pool, &session, None).await? {
            expire_playback_session_by_id(pool, &session.id).await?;
        }
    }

    Ok(())
}

pub(super) async fn reconcile_playback_sessions_for_read(
    pool: &SqlitePool,
    creator_id: Option<&str>,
    content_id: Option<&str>,
    session_id: Option<&str>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
               auth_session_id, created_at, expires_at, last_used_at
        FROM playback_sessions
        WHERE expires_at > ?
          AND (?2 IS NULL OR creator_id = ?2)
          AND (?3 IS NULL OR content_id = ?3)
          AND (?4 IS NULL OR id = ?4)
        ORDER BY created_at DESC
        "#,
    )
    .bind(&now)
    .bind(creator_id)
    .bind(content_id)
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let session = playback_session_record_from_row(row);
        if !validate_existing_playback_session_access(pool, &session, None).await? {
            expire_playback_session_by_id(pool, &session.id).await?;
        }
    }

    Ok(())
}

pub(super) async fn reconcile_single_playback_session(
    state: SharedState,
    session_id: &str,
) -> AppResult<PlaybackReconciliationReport> {
    let now = Utc::now().to_rfc3339();
    let session = fetch_playback_session_record_by_id(&state.pool, session_id).await?;
    let mut actions = Vec::new();

    if session.expires_at > now
        && !validate_existing_playback_session_access(&state.pool, &session, None).await?
    {
        expire_playback_session_by_id(&state.pool, &session.id).await?;
        actions.push(PlaybackReconciliationAction {
            action_type: "session_invalidated".to_string(),
            target_id: session.id.clone(),
            previous_state: Some("active".to_string()),
            next_state: Some("invalid".to_string()),
            reason: "playback session no longer satisfied access requirements".to_string(),
            occurred_at: now.clone(),
        });
    }

    let record = fetch_admin_playback_session_record(&state.pool, session_id).await?;
    Ok(PlaybackReconciliationReport {
        session_id: session_id.to_string(),
        reconciled_at: now,
        actions,
        record,
    })
}

pub(super) async fn reconcile_invalid_playback_sessions(state: SharedState) -> AppResult<()> {
    reconcile_playback_sessions_for_read(&state.pool, None, None, None).await
}

fn asset_tracks_published(status: &str) -> bool {
    status == "published"
}

pub(super) fn build_media_audio_tracks(
    status: &str,
    asset_id: &str,
    variants: &[MediaAssetVariant],
    audio_codec: Option<&str>,
    playback_token: Option<&str>,
    preferred_audio_language: Option<&str>,
    prefer_dubbed: bool,
) -> Vec<PlaybackAudioTrack> {
    let mut tracks = variants
        .iter()
        .filter(|variant| variant.variant_type == "audio")
        .map(|variant| {
            let mut parts = variant.label.split(':');
            let label = parts.next().unwrap_or("audio").to_string();
            let language = parts.next().unwrap_or("und").to_string();
            let source = parts.next().unwrap_or("source-provided").to_string();
            let is_dubbed = parts.next().unwrap_or("0") == "1";
            let variant_codec = parts.next().map(str::to_string);
            let playlist_url = playback_token
                .map(|token| {
                    format!(
                        "/api/v1/media/{}?playbackToken={}",
                        variant.relative_path, token
                    )
                })
                .or_else(|| Some(variant.url.clone()));

            PlaybackAudioTrack {
                id: variant.id.clone(),
                label,
                language,
                codec: variant_codec
                    .or_else(|| variant.mime_type.strip_prefix("audio/").map(str::to_string))
                    .or_else(|| audio_codec.map(str::to_string)),
                playlist_path: Some(variant.relative_path.clone()),
                playlist_url,
                source,
                is_dubbed,
                is_default: variant.is_default,
                published: asset_tracks_published(status),
            }
        })
        .collect::<Vec<_>>();

    if tracks.is_empty() && audio_codec.is_some() {
        tracks.push(PlaybackAudioTrack {
            id: format!("{asset_id}:audio:primary"),
            label: audio_codec
                .map(|codec| format!("primary-{codec}"))
                .unwrap_or_else(|| "primary".to_string()),
            language: "und".to_string(),
            codec: audio_codec.map(str::to_string),
            playlist_path: None,
            playlist_url: None,
            source: "source-provided".to_string(),
            is_dubbed: false,
            is_default: true,
            published: asset_tracks_published(status),
        });
        return tracks;
    }

    if let Some(preferred_language) = normalized_track_preference(preferred_audio_language) {
        if let Some(matching_id) = tracks
            .iter()
            .find(|track| {
                track.language.eq_ignore_ascii_case(&preferred_language)
                    && (!prefer_dubbed || track.is_dubbed)
            })
            .or_else(|| {
                tracks
                    .iter()
                    .find(|track| track.language.eq_ignore_ascii_case(&preferred_language))
            })
            .map(|track| track.id.clone())
        {
            for track in &mut tracks {
                track.is_default = track.id == matching_id;
            }
            return tracks;
        }
    }

    if prefer_dubbed {
        if let Some(dubbed_id) = tracks
            .iter()
            .find(|track| track.is_dubbed)
            .map(|track| track.id.clone())
        {
            for track in &mut tracks {
                track.is_default = track.id == dubbed_id;
            }
            return tracks;
        }
    }

    if tracks.iter().all(|track| !track.is_default) {
        if let Some(first_track) = tracks.first_mut() {
            first_track.is_default = true;
        }
    }

    tracks
}

fn caption_role_for_label(label: &str) -> &'static str {
    let normalized = label.to_ascii_lowercase();
    if normalized.contains("forced") {
        "forced"
    } else if normalized.contains("sdh") || normalized.contains("cc") {
        "sdh"
    } else {
        "standard"
    }
}

fn caption_source_for_label(label: &str) -> &'static str {
    let normalized = label.to_ascii_lowercase();
    if normalized.contains("auto") || normalized.contains("generated") {
        "auto-generated"
    } else if normalized.contains("reviewed") {
        "human-reviewed"
    } else {
        "source-provided"
    }
}

fn normalized_track_preference(preference: Option<&str>) -> Option<String> {
    let value = preference?.trim();
    if value.is_empty()
        || value.eq_ignore_ascii_case("off")
        || value.eq_ignore_ascii_case("none")
        || value.eq_ignore_ascii_case("disabled")
        || value.eq_ignore_ascii_case("auto")
    {
        None
    } else {
        Some(value.to_ascii_lowercase())
    }
}

pub(super) fn build_media_caption_tracks(
    status: &str,
    variants: &[MediaAssetVariant],
    playback_token: Option<&str>,
    preferred_subtitle_language: Option<&str>,
) -> Vec<PlaybackCaptionTrack> {
    let mut tracks = variants
        .iter()
        .filter(|variant| variant.variant_type == "caption")
        .map(|variant| {
            let (label, language) = variant
                .label
                .split_once(':')
                .map(|(label, language)| (label.to_string(), language.to_string()))
                .unwrap_or_else(|| (variant.label.clone(), "und".to_string()));
            let url = if let Some(token) = playback_token {
                format!(
                    "/api/v1/media/{}?playbackToken={}",
                    variant.relative_path, token
                )
            } else {
                variant.url.clone()
            };

            PlaybackCaptionTrack {
                id: variant.id.clone(),
                label,
                language,
                role: caption_role_for_label(&variant.label).to_string(),
                source: caption_source_for_label(&variant.label).to_string(),
                mime_type: variant.mime_type.clone(),
                url,
                is_default: variant.is_default,
                published: asset_tracks_published(status),
            }
        })
        .collect::<Vec<_>>();

    if tracks.is_empty() {
        return tracks;
    }

    if let Some(preferred_language) = normalized_track_preference(preferred_subtitle_language) {
        if let Some(matching_track_id) = tracks
            .iter()
            .find(|track| track.language.eq_ignore_ascii_case(&preferred_language))
            .map(|track| track.id.clone())
        {
            for track in &mut tracks {
                track.is_default = track.id == matching_track_id;
            }
            return tracks;
        }
    }

    if tracks.iter().all(|track| !track.is_default) {
        if let Some(first_track) = tracks.first_mut() {
            first_track.is_default = true;
        }
    }

    tracks
}

pub(super) fn default_audio_track_id(tracks: &[PlaybackAudioTrack]) -> Option<String> {
    tracks
        .iter()
        .find(|track| track.is_default)
        .map(|track| track.id.clone())
}

pub(super) fn default_caption_track_id(tracks: &[PlaybackCaptionTrack]) -> Option<String> {
    tracks
        .iter()
        .find(|track| track.is_default)
        .map(|track| track.id.clone())
}

pub(super) fn build_media_preview_tracks(
    status: &str,
    tracks: &[StoredMediaPreviewTrack],
    playback_token: Option<&str>,
) -> Vec<PlaybackPreviewTrack> {
    tracks
        .iter()
        .map(|track| PlaybackPreviewTrack {
            id: track.id.clone(),
            label: track.label.clone(),
            image_path: track.image_relative_path.clone(),
            image_url: playback_token
                .map(|token| {
                    format!(
                        "/api/v1/media/{}?playbackToken={}",
                        track.image_relative_path, token
                    )
                })
                .unwrap_or_else(|| media_api_url(&track.image_relative_path)),
            vtt_path: track.vtt_relative_path.clone(),
            vtt_url: playback_token
                .map(|token| {
                    format!(
                        "/api/v1/media/{}?playbackToken={}",
                        track.vtt_relative_path, token
                    )
                })
                .unwrap_or_else(|| media_api_url(&track.vtt_relative_path)),
            tile_width: track.tile_width,
            tile_height: track.tile_height,
            columns_count: track.columns_count,
            rows_count: track.rows_count,
            interval_sec: track.interval_sec,
            frame_count: track.frame_count,
            is_default: track.is_default,
            published: asset_tracks_published(status),
        })
        .collect()
}

pub(super) fn default_preview_track_id(tracks: &[PlaybackPreviewTrack]) -> Option<String> {
    tracks
        .iter()
        .find(|track| track.is_default)
        .map(|track| track.id.clone())
}

pub(super) async fn fetch_user_subtitle_preference(
    pool: &SqlitePool,
    user_id: Option<&str>,
) -> AppResult<Option<String>> {
    let Some(user_id) = user_id else {
        return Ok(None);
    };
    let row = sqlx::query("SELECT subtitle_language FROM user_playback_settings WHERE user_id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| row.get("subtitle_language")))
}

pub(super) async fn fetch_user_audio_preferences(
    pool: &SqlitePool,
    user_id: Option<&str>,
) -> AppResult<(Option<String>, bool)> {
    let Some(user_id) = user_id else {
        return Ok((None, false));
    };
    let row = sqlx::query(
        "SELECT audio_language, prefer_dubbed FROM user_playback_settings WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row
        .map(|row| {
            (
                Some(row.get::<String, _>("audio_language")),
                row.get::<i64, _>("prefer_dubbed") == 1,
            )
        })
        .unwrap_or((None, false)))
}
