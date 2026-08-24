use super::*;

#[tokio::test]
async fn scheduled_publish_keeps_media_asset_scheduled_until_release() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let row = sqlx::query(
        r#"
        SELECT upload_jobs.id
        FROM upload_jobs
        INNER JOIN media_assets
            ON media_assets.upload_job_id = upload_jobs.id
           AND media_assets.creator_id = upload_jobs.creator_id
        WHERE upload_jobs.creator_id = ?
          AND upload_jobs.status = 'ready'
          AND media_assets.playback_relative_path IS NOT NULL
        ORDER BY upload_jobs.created_at ASC
        LIMIT 1
        "#,
    )
    .bind(&creator.id)
    .fetch_one(state.db.sqlite_adapter())
    .await?;
    let job_id: String = row.get("id");
    let release_at = (Utc::now() + chrono::Duration::hours(6)).to_rfc3339();

    let upload = publish_upload_job(
        State(state.clone()),
        headers,
        Path(job_id.clone()),
        Json(PublishUploadJobRequest {
            description: Some("Scheduled premiere".to_string()),
            visibility: Some("public".to_string()),
            slug: Some(format!("scheduled-premiere-{}", Uuid::new_v4().simple())),
            release_at: Some(release_at.clone()),
            access_policy: Some("free".to_string()),
            access_tier_id: None,
            price_cents: None,
            currency: None,
            rental_window_hours: None,
            season_number: None,
            episode_number: None,
            season_title: None,
            season_synopsis: None,
        }),
    )
    .await?
    .0;

    let asset =
        fetch_media_asset_by_upload_job(state.db.sqlite_adapter(), &creator.id, &job_id).await?;
    let thumbnail_variant = asset
        .variants
        .iter()
        .find(|variant| variant.variant_type == "thumbnail")
        .expect("processed asset should include at least one thumbnail derivative");

    assert_eq!(upload.status, "scheduled");
    assert_eq!(upload.release_at.as_deref(), Some(release_at.as_str()));
    assert_eq!(asset.status, "scheduled");
    assert_eq!(asset.visibility, "public");
    assert_eq!(upload.thumbnail, thumbnail_variant.url);
    Ok(())
}

#[tokio::test]
async fn overdue_scheduled_upload_materializes_on_creator_read() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let row = sqlx::query(
        r#"
        SELECT uploads.id
        FROM uploads
        INNER JOIN media_assets
            ON media_assets.upload_id = uploads.id
           AND media_assets.creator_id = uploads.creator_id
        WHERE uploads.creator_id = ?
          AND uploads.status = 'published'
        ORDER BY uploads.published_at DESC
        LIMIT 1
        "#,
    )
    .bind(&creator.id)
    .fetch_one(state.db.sqlite_adapter())
    .await?;
    let upload_id: String = row.get("id");
    let release_at = (Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
    sqlx::query(
        "UPDATE uploads SET status = 'scheduled', visibility = 'public', release_at = ?, published_at = NULL WHERE id = ? AND creator_id = ?",
    )
    .bind(&release_at)
    .bind(&upload_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query(
        "UPDATE media_assets SET status = 'scheduled', visibility = 'public', updated_at = ? WHERE upload_id = ? AND creator_id = ?",
    )
    .bind(&release_at)
    .bind(&upload_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;

    let upload = fetch_upload_by_id(state.db.sqlite_adapter(), &creator.id, &upload_id).await?;
    let asset =
        fetch_media_asset_by_upload_id(state.db.sqlite_adapter(), &creator.id, &upload_id).await?;

    assert_eq!(upload.status, "published");
    assert_eq!(upload.visibility, "public");
    assert!(upload.published_at.is_some());
    assert_eq!(asset.status, "published");
    assert_eq!(asset.visibility, "public");
    Ok(())
}

#[tokio::test]
async fn overdue_scheduled_catalog_film_materializes_on_read() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let row = sqlx::query(
        r#"
        SELECT uploads.id
        FROM uploads
        INNER JOIN media_assets
            ON media_assets.upload_id = uploads.id
           AND media_assets.creator_id = uploads.creator_id
        WHERE uploads.creator_id = ?
          AND uploads.kind = 'film'
          AND uploads.status = 'published'
          AND uploads.visibility = 'public'
        ORDER BY uploads.published_at DESC
        LIMIT 1
        "#,
    )
    .bind(&creator.id)
    .fetch_one(state.db.sqlite_adapter())
    .await?;
    let upload_id: String = row.get("id");
    let release_at = (Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
    sqlx::query(
        "UPDATE uploads SET status = 'scheduled', visibility = 'public', release_at = ?, published_at = NULL WHERE id = ? AND creator_id = ?",
    )
    .bind(&release_at)
    .bind(&upload_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query(
        "UPDATE media_assets SET status = 'scheduled', visibility = 'public', updated_at = ? WHERE upload_id = ? AND creator_id = ?",
    )
    .bind(&release_at)
    .bind(&upload_id)
    .bind(&creator.id)
    .execute(state.db.sqlite_adapter())
    .await?;

    let film =
        fetch_creator_catalog_film_by_id(state.db.sqlite_adapter(), &upload_id, false).await?;
    let upload = fetch_upload_by_id(state.db.sqlite_adapter(), &creator.id, &upload_id).await?;

    assert_eq!(film.id, upload_id);
    assert_eq!(upload.status, "published");
    assert!(upload.published_at.is_some());
    Ok(())
}

#[tokio::test]
async fn unpublish_upload_moves_media_asset_back_to_draft() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let row = sqlx::query(
        r#"
        SELECT uploads.id
        FROM uploads
        INNER JOIN media_assets
            ON media_assets.upload_id = uploads.id
           AND media_assets.creator_id = uploads.creator_id
        WHERE uploads.creator_id = ?
          AND uploads.status = 'published'
        ORDER BY uploads.published_at DESC
        LIMIT 1
        "#,
    )
    .bind(&creator.id)
    .fetch_one(state.db.sqlite_adapter())
    .await?;
    let upload_id: String = row.get("id");

    let upload = unpublish_upload(State(state.clone()), headers, Path(upload_id.clone()))
        .await?
        .0;

    let asset =
        fetch_media_asset_by_upload_id(state.db.sqlite_adapter(), &creator.id, &upload_id).await?;

    assert_eq!(upload.status, "draft");
    assert_eq!(upload.visibility, "private");
    assert!(upload.release_at.is_none());
    assert_eq!(asset.status, "draft");
    assert_eq!(asset.visibility, "private");
    Ok(())
}

#[tokio::test]
async fn update_upload_syncs_media_asset_lifecycle_state() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let row = sqlx::query(
        r#"
        SELECT uploads.id
        FROM uploads
        INNER JOIN media_assets
            ON media_assets.upload_id = uploads.id
           AND media_assets.creator_id = uploads.creator_id
        WHERE uploads.creator_id = ?
          AND uploads.status = 'published'
        ORDER BY uploads.published_at DESC
        LIMIT 1
        "#,
    )
    .bind(&creator.id)
    .fetch_one(state.db.sqlite_adapter())
    .await?;
    let upload_id: String = row.get("id");
    let release_at = (Utc::now() + chrono::Duration::hours(3)).to_rfc3339();

    let updated = update_upload(
        State(state.clone()),
        headers,
        Path(upload_id.clone()),
        Json(UpdateUploadRequest {
            title: None,
            slug: None,
            description: None,
            visibility: Some("public".to_string()),
            release_at: Some(release_at.clone()),
            access_policy: None,
            access_tier_id: None,
            price_cents: None,
            currency: None,
            rental_window_hours: None,
        }),
    )
    .await?
    .0;

    let asset =
        fetch_media_asset_by_upload_id(state.db.sqlite_adapter(), &creator.id, &upload_id).await?;

    assert_eq!(updated.status, "scheduled");
    assert_eq!(updated.visibility, "public");
    assert_eq!(updated.release_at.as_deref(), Some(release_at.as_str()));
    assert_eq!(asset.status, "scheduled");
    assert_eq!(asset.visibility, "public");
    Ok(())
}

#[tokio::test]
async fn taken_down_upload_rejects_lifecycle_changes_via_general_update() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let row = sqlx::query(
        r#"
        SELECT uploads.id
        FROM uploads
        INNER JOIN media_assets
            ON media_assets.upload_id = uploads.id
           AND media_assets.creator_id = uploads.creator_id
        WHERE uploads.creator_id = ?
          AND uploads.status = 'published'
        ORDER BY uploads.published_at DESC
        LIMIT 1
        "#,
    )
    .bind(&creator.id)
    .fetch_one(state.db.sqlite_adapter())
    .await?;
    let upload_id: String = row.get("id");

    let _ = takedown_upload(
        State(state.clone()),
        headers.clone(),
        Path(upload_id.clone()),
    )
    .await?;

    let error = update_upload(
        State(state.clone()),
        headers,
        Path(upload_id.clone()),
        Json(UpdateUploadRequest {
            title: None,
            slug: None,
            description: None,
            visibility: Some("public".to_string()),
            release_at: Some((Utc::now() + chrono::Duration::hours(1)).to_rfc3339()),
            access_policy: Some("purchase".to_string()),
            access_tier_id: None,
            price_cents: Some(999),
            currency: Some("USD".to_string()),
            rental_window_hours: Some(24),
        }),
    )
    .await
    .expect_err("taken-down uploads must not accept lifecycle or access changes");

    match error {
        AppError::BadRequest(message) => {
            assert!(
                message.contains("taken-down uploads cannot change lifecycle or access controls")
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let upload = fetch_upload_by_id(state.db.sqlite_adapter(), &creator.id, &upload_id).await?;
    let asset =
        fetch_media_asset_by_upload_id(state.db.sqlite_adapter(), &creator.id, &upload_id).await?;
    assert_eq!(upload.status, "taken_down");
    assert_eq!(upload.visibility, "private");
    assert_eq!(asset.status, "taken_down");
    assert_eq!(asset.visibility, "private");
    Ok(())
}

#[tokio::test]
async fn expired_purchase_reconciliation_expires_invalid_playback_session() -> AppResult<()> {
    let (state, _creator) = setup_test_state().await?;
    let now = Utc::now();
    let purchased_at = (now - chrono::Duration::hours(2)).to_rfc3339();
    seed_content_purchase_for_user(
        state.db.sqlite_adapter(),
        "usr-viewer",
        "crt-deepsaint",
        "upl-57fd50bbb54a44f58fe10605f97eeead",
        "purchase",
        1499,
        "USD",
        &purchased_at,
        Some(&(now + chrono::Duration::hours(1)).to_rfc3339()),
        "active",
    )
    .await?;
    let (session_id, _token, _asset) = insert_playback_session_for_upload(
        state.db.sqlite_adapter(),
        "upl-57fd50bbb54a44f58fe10605f97eeead",
        Some("usr-viewer"),
        None,
        "purchase",
    )
    .await?;
    sqlx::query(
        "UPDATE content_purchases SET expires_at = ?, status = 'active' WHERE user_id = ? AND upload_id = ?",
    )
    .bind((Utc::now() - chrono::Duration::minutes(1)).to_rfc3339())
    .bind("usr-viewer")
    .bind("upl-57fd50bbb54a44f58fe10605f97eeead")
    .execute(state.db.sqlite_adapter())
    .await?;

    reconcile_expired_user_entitlements(state.clone()).await?;

    let session =
        fetch_playback_session_record_by_id(state.db.sqlite_adapter(), &session_id).await?;
    let purchase = fetch_current_content_purchase(
        state.db.sqlite_adapter(),
        "usr-viewer",
        "upl-57fd50bbb54a44f58fe10605f97eeead",
    )
    .await?;
    assert!(session.expires_at <= Utc::now().to_rfc3339());
    assert!(purchase.is_none());
    Ok(())
}

#[tokio::test]
async fn expired_membership_keeps_session_when_purchase_still_grants_access() -> AppResult<()> {
    let (state, _creator) = setup_test_state().await?;
    let upload_id = "upl-6f378951e0ee4526b13333f470db77e3";
    let (session_id, _token, _asset) = insert_playback_session_for_upload(
        state.db.sqlite_adapter(),
        upload_id,
        Some("usr-viewer"),
        None,
        "subscription",
    )
    .await?;
    let now = Utc::now();
    let now_rfc3339 = now.to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO content_purchases (
            id, user_id, creator_id, upload_id, access_policy, amount_cents, currency,
            status, purchased_at, expires_at
        ) VALUES (?, ?, ?, ?, 'subscription_or_purchase', ?, ?, 'active', ?, ?)
        "#,
    )
    .bind(format!("pur-test-{}", Uuid::new_v4().simple()))
    .bind("usr-viewer")
    .bind("crt-deepsaint")
    .bind(upload_id)
    .bind(1599_i64)
    .bind("USD")
    .bind(&now_rfc3339)
    .bind((now + chrono::Duration::hours(24)).to_rfc3339())
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query(
        "UPDATE creator_memberships SET renews_at = ?, ends_at = NULL, status = 'active' WHERE user_id = ? AND creator_id = ?",
    )
    .bind((now - chrono::Duration::minutes(1)).to_rfc3339())
    .bind("usr-viewer")
    .bind("crt-deepsaint")
    .execute(state.db.sqlite_adapter())
    .await?;

    reconcile_expired_user_entitlements(state.clone()).await?;

    let session =
        fetch_playback_session_record_by_id(state.db.sqlite_adapter(), &session_id).await?;
    assert!(session.expires_at > Utc::now().to_rfc3339());
    assert!(
        validate_existing_playback_session_access(state.db.sqlite_adapter(), &session, None)
            .await?
    );
    Ok(())
}

#[tokio::test]
async fn user_entitlements_read_self_heals_expired_membership_and_purchase() -> AppResult<()> {
    let (state, _creator) = setup_test_state().await?;
    let now = Utc::now();
    let expired_at = (now - chrono::Duration::minutes(1)).to_rfc3339();
    seed_content_purchase_for_user(
        state.db.sqlite_adapter(),
        "usr-viewer",
        "crt-deepsaint",
        "upl-57fd50bbb54a44f58fe10605f97eeead",
        "purchase",
        1499,
        "USD",
        &(now - chrono::Duration::hours(2)).to_rfc3339(),
        Some(&(now + chrono::Duration::hours(1)).to_rfc3339()),
        "active",
    )
    .await?;

    sqlx::query(
        "UPDATE creator_memberships SET status = 'active', renews_at = ?, ends_at = NULL WHERE user_id = ? AND creator_id = ?",
    )
    .bind(&expired_at)
    .bind("usr-viewer")
    .bind("crt-deepsaint")
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query(
        "UPDATE content_purchases SET status = 'active', expires_at = ? WHERE user_id = ? AND upload_id = ?",
    )
    .bind(&expired_at)
    .bind("usr-viewer")
    .bind("upl-57fd50bbb54a44f58fe10605f97eeead")
    .execute(state.db.sqlite_adapter())
    .await?;

    let entitlements = fetch_user_entitlements(state.db.sqlite_adapter(), "usr-viewer").await?;

    assert!(
        entitlements
            .memberships
            .iter()
            .any(|membership| membership.creator_id == "crt-deepsaint"
                && membership.status == "expired")
    );
    assert!(
        entitlements
            .purchases
            .iter()
            .any(
                |purchase| purchase.upload_id == "upl-57fd50bbb54a44f58fe10605f97eeead"
                    && purchase.status == "expired"
            )
    );
    Ok(())
}

#[tokio::test]
async fn user_can_inspect_and_reconcile_membership_and_purchase_entitlements_by_id() -> AppResult<()>
{
    let (state, _creator) = setup_test_state().await?;
    let token =
        insert_user_auth_session(state.db.sqlite_adapter(), "usr-viewer", &["user"]).await?;
    let headers = auth_headers(&token);
    let expired_at = (Utc::now() - chrono::Duration::minutes(1)).to_rfc3339();
    let purchase_id = format!("pur-test-{}", Uuid::new_v4().simple());
    let purchased_at = (Utc::now() - chrono::Duration::hours(2)).to_rfc3339();

    sqlx::query(
        "UPDATE creator_memberships SET status = 'canceling', renews_at = ?, ends_at = ?, canceled_at = ? WHERE user_id = ? AND creator_id = ?",
    )
    .bind(&expired_at)
    .bind(&expired_at)
    .bind(&expired_at)
    .bind("usr-viewer")
    .bind("crt-deepsaint")
    .execute(state.db.sqlite_adapter())
    .await?;
    sqlx::query(
        r#"
        INSERT INTO content_purchases (
            id, user_id, creator_id, upload_id, access_policy, amount_cents, currency,
            status, purchased_at, expires_at
        ) VALUES (?, ?, ?, ?, 'purchase', ?, ?, 'active', ?, ?)
        "#,
    )
    .bind(&purchase_id)
    .bind("usr-viewer")
    .bind("crt-deepsaint")
    .bind("upl-57fd50bbb54a44f58fe10605f97eeead")
    .bind(1299_i64)
    .bind("USD")
    .bind(&purchased_at)
    .bind(&expired_at)
    .execute(state.db.sqlite_adapter())
    .await?;

    let membership = get_my_membership_entitlement(
        State(state.clone()),
        headers.clone(),
        Path("crt-deepsaint".to_string()),
    )
    .await?
    .0;
    let purchase = get_my_purchase_entitlement(
        State(state.clone()),
        headers.clone(),
        Path(purchase_id.clone()),
    )
    .await?
    .0;

    assert_eq!(membership.status, "canceling");
    assert_eq!(purchase.status, "active");

    let membership_report = reconcile_my_membership_entitlement(
        State(state.clone()),
        headers.clone(),
        Path("crt-deepsaint".to_string()),
    )
    .await?
    .0;
    let purchase_report =
        reconcile_my_purchase_entitlement(State(state.clone()), headers, Path(purchase_id.clone()))
            .await?
            .0;

    assert_eq!(membership_report.creator_id, "crt-deepsaint");
    assert_eq!(membership_report.membership.status, "expired");
    assert_eq!(membership_report.actions.len(), 1);
    assert_eq!(
        membership_report.actions[0].action_type,
        "membership_expired"
    );
    assert_eq!(
        membership_report.actions[0].previous_state.as_deref(),
        Some("canceling")
    );
    assert_eq!(
        membership_report.actions[0].next_state.as_deref(),
        Some("expired")
    );

    assert_eq!(purchase_report.purchase_id, purchase_id);
    assert_eq!(purchase_report.purchase.status, "expired");
    assert_eq!(purchase_report.actions.len(), 1);
    assert_eq!(purchase_report.actions[0].action_type, "purchase_expired");
    assert_eq!(
        purchase_report.actions[0].previous_state.as_deref(),
        Some("active")
    );
    assert_eq!(
        purchase_report.actions[0].next_state.as_deref(),
        Some("expired")
    );
    Ok(())
}

#[tokio::test]
async fn user_entitlements_read_expires_invalid_playback_session_without_background_loop()
-> AppResult<()> {
    let (state, _creator) = setup_test_state().await?;
    let now = Utc::now();
    let purchased_at = (now - chrono::Duration::hours(2)).to_rfc3339();
    seed_content_purchase_for_user(
        state.db.sqlite_adapter(),
        "usr-viewer",
        "crt-deepsaint",
        "upl-57fd50bbb54a44f58fe10605f97eeead",
        "purchase",
        1499,
        "USD",
        &purchased_at,
        Some(&(now + chrono::Duration::hours(1)).to_rfc3339()),
        "active",
    )
    .await?;
    let (session_id, _token, _asset) = insert_playback_session_for_upload(
        state.db.sqlite_adapter(),
        "upl-57fd50bbb54a44f58fe10605f97eeead",
        Some("usr-viewer"),
        None,
        "purchase",
    )
    .await?;
    sqlx::query(
        "UPDATE content_purchases SET expires_at = ?, status = 'active' WHERE user_id = ? AND upload_id = ?",
    )
    .bind((Utc::now() - chrono::Duration::minutes(1)).to_rfc3339())
    .bind("usr-viewer")
    .bind("upl-57fd50bbb54a44f58fe10605f97eeead")
    .execute(state.db.sqlite_adapter())
    .await?;

    let _ = fetch_user_entitlements(state.db.sqlite_adapter(), "usr-viewer").await?;
    let session =
        fetch_playback_session_record_by_id(state.db.sqlite_adapter(), &session_id).await?;

    assert!(session.expires_at <= Utc::now().to_rfc3339());
    Ok(())
}

#[tokio::test]
async fn admin_playback_listing_filters_after_reconciling_invalid_newer_sessions() -> AppResult<()>
{
    let (state, creator) = setup_test_state().await?;
    let upload_id = "upl-57fd50bbb54a44f58fe10605f97eeead";
    sqlx::query(
        "UPDATE uploads SET status = 'published', visibility = 'public', access_policy = 'free', access_tier_id = NULL, price_cents = NULL, currency = NULL, rental_window_hours = NULL WHERE id = ?",
    )
    .bind(upload_id)
    .execute(state.db.sqlite_adapter())
    .await?;
    let (older_valid_session_id, _token, _asset) = insert_playback_session_for_upload(
        state.db.sqlite_adapter(),
        upload_id,
        None,
        None,
        "free",
    )
    .await?;
    tokio::time::sleep(Duration::from_millis(5)).await;
    let revoked_auth_token =
        insert_user_auth_session(state.db.sqlite_adapter(), "usr-viewer", &["user"]).await?;
    let revoked_auth_session_id = sqlx::query("SELECT id FROM auth_sessions WHERE token_hash = ?")
        .bind(hash_token(&revoked_auth_token))
        .fetch_one(state.db.sqlite_adapter())
        .await?
        .get::<String, _>("id");
    let (newer_invalid_session_id, _token, _asset) = insert_playback_session_for_upload(
        state.db.sqlite_adapter(),
        upload_id,
        Some("usr-viewer"),
        Some(&revoked_auth_session_id),
        "free",
    )
    .await?;

    sqlx::query("UPDATE playback_sessions SET created_at = ?, last_used_at = ? WHERE id = ?")
        .bind((Utc::now() - chrono::Duration::hours(2)).to_rfc3339())
        .bind((Utc::now() - chrono::Duration::hours(2)).to_rfc3339())
        .bind(&older_valid_session_id)
        .execute(state.db.sqlite_adapter())
        .await?;
    sqlx::query("UPDATE playback_sessions SET created_at = ?, last_used_at = ? WHERE id = ?")
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .bind(&newer_invalid_session_id)
        .execute(state.db.sqlite_adapter())
        .await?;

    sqlx::query("UPDATE auth_sessions SET revoked_at = ? WHERE id = ?")
        .bind(Utc::now().to_rfc3339())
        .bind(&revoked_auth_session_id)
        .execute(state.db.sqlite_adapter())
        .await?;

    let sessions = fetch_admin_playback_sessions(
        &state.db,
        Some(&creator.id),
        Some(upload_id),
        Some("active"),
        1,
    )
    .await?;

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session.id, older_valid_session_id);
    assert!(sessions[0].active);
    assert!(sessions[0].valid_access);

    let invalid = fetch_admin_playback_sessions(
        &state.db,
        Some(&creator.id),
        Some(upload_id),
        Some("invalid"),
        10,
    )
    .await?;
    assert!(
        invalid
            .iter()
            .any(|record| record.session.id == newer_invalid_session_id)
    );
    Ok(())
}
