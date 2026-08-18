use super::moderation::creator_enforcement_action_from_row;
use super::*;

pub(super) async fn fetch_creator_profile(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorProfile> {
    let row = sqlx::query(
        r#"
        SELECT id, user_id, handle, display_name, avatar, banner, tagline, bio, partner_status,
               joined_at, stream_key, rtmp_url, default_category, default_tags_json,
               followers, subscribers, monthly_viewers, total_watch_hours, live_status, current_broadcast_id
        FROM creator_profiles
        WHERE id = ?
        "#,
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let subscriber_tiers = fetch_creator_subscriber_tiers(pool, creator_id).await?;
    let subscribers = subscriber_tiers
        .iter()
        .map(|tier| tier.subscriber_count)
        .sum::<i64>();
    let analytics = fetch_analytics(pool, creator_id).await?;
    let analytics_summary = summarize_creator_analytics(&analytics);
    let vod_watch_hours = sqlx::query(
        "SELECT COALESCE(SUM(watch_hours), 0) AS total FROM uploads WHERE creator_id = ?",
    )
    .bind(creator_id)
    .fetch_one(pool)
    .await?
    .get::<i64, _>("total");
    let total_watch_hours = row
        .get::<i64, _>("total_watch_hours")
        .max(vod_watch_hours)
        .max(analytics_summary.total_watch_minutes / 60);

    Ok(CreatorProfile {
        id: row.get("id"),
        user_id: row.get("user_id"),
        handle: row.get("handle"),
        display_name: row.get("display_name"),
        avatar: row.get("avatar"),
        banner: row.get("banner"),
        tagline: row.get("tagline"),
        bio: row.get("bio"),
        partner_status: row.get("partner_status"),
        joined_at: row.get("joined_at"),
        stream_key: row.get("stream_key"),
        rtmp_url: row.get("rtmp_url"),
        default_category: row.get("default_category"),
        default_tags: from_json(row.get::<String, _>("default_tags_json"))?,
        followers: row.get("followers"),
        subscribers,
        monthly_viewers: analytics_summary
            .total_viewers
            .max(row.get("monthly_viewers")),
        total_watch_hours,
        live_status: row.get("live_status"),
        current_broadcast_id: row.get("current_broadcast_id"),
    })
}

pub(super) async fn fetch_creator_profile_by_stream_key(
    pool: &SqlitePool,
    stream_key: &str,
) -> AppResult<CreatorProfile> {
    let row = sqlx::query(
        r#"
        SELECT id
        FROM creator_profiles
        WHERE stream_key = ?
        "#,
    )
    .bind(stream_key)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;
    let creator_id: String = row.get("id");
    fetch_creator_profile(pool, &creator_id).await
}

pub(super) async fn fetch_creator_operational_state(
    pool: &SqlitePool,
    profile: &CreatorProfile,
) -> AppResult<CreatorOperationalState> {
    reconcile_expired_creator_enforcement_actions_for_read(pool, Some(&profile.id), None).await?;
    let row = sqlx::query(
        r#"
        SELECT legal_name, support_email, business_type, payout_country, payout_provider,
               onboarding_status, identity_status, tax_status, payout_status, hold_reasons_json,
               created_at, updated_at, last_reviewed_at
        FROM creator_operational_state
        WHERE creator_id = ?
        "#,
    )
    .bind(&profile.id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let legal_name: String = row.get("legal_name");
    let support_email: String = row.get("support_email");
    let business_type: String = row.get("business_type");
    let payout_country: String = row.get("payout_country");
    let payout_provider: String = row.get("payout_provider");
    let onboarding_status: String = row.get("onboarding_status");
    let identity_status: String = row.get("identity_status");
    let tax_status: String = row.get("tax_status");
    let payout_status: String = row.get("payout_status");
    let hold_reasons: Vec<String> = from_json(row.get::<String, _>("hold_reasons_json"))?;
    let active_enforcement_actions =
        fetch_active_creator_enforcement_actions(pool, &profile.id).await?;
    let live_streaming_enabled = !active_enforcement_actions
        .iter()
        .any(|action| action.scope == "live_streaming");
    let upload_ingest_enabled = !active_enforcement_actions
        .iter()
        .any(|action| action.scope == "uploads");
    let collaboration_enabled = !active_enforcement_actions
        .iter()
        .any(|action| action.scope == "collaboration");
    let monetization_enabled = !active_enforcement_actions
        .iter()
        .any(|action| action.scope == "monetization");
    let payouts_enabled = !active_enforcement_actions
        .iter()
        .any(|action| action.scope == "payouts");

    let profile_complete = !legal_name.trim().is_empty()
        && !support_email.trim().is_empty()
        && support_email.contains('@')
        && !business_type.trim().is_empty()
        && !payout_country.trim().is_empty()
        && !payout_provider.trim().is_empty();
    let onboarding_complete = onboarding_status == "approved";
    let identity_verified = identity_status == "verified";
    let tax_verified = tax_status == "verified";
    let payout_ready = payout_status == "active";
    let holds_clear = hold_reasons.is_empty();
    let can_monetize =
        onboarding_complete && identity_verified && tax_verified && monetization_enabled;
    let can_receive_payouts = can_monetize && payout_ready && holds_clear && payouts_enabled;

    let checklist = vec![
        CreatorOperationalChecklistItem {
            key: "profileComplete".to_string(),
            label: "Profile complete".to_string(),
            complete: profile_complete,
            detail: if profile_complete {
                "Legal and support contact details are present.".to_string()
            } else {
                "Complete legal name, support email, payout country, provider, and business type."
                    .to_string()
            },
        },
        CreatorOperationalChecklistItem {
            key: "onboardingApproved".to_string(),
            label: "Onboarding approved".to_string(),
            complete: onboarding_complete,
            detail: format!("Current onboarding status: {onboarding_status}."),
        },
        CreatorOperationalChecklistItem {
            key: "identityVerified".to_string(),
            label: "Identity verified".to_string(),
            complete: identity_verified,
            detail: format!("Current identity status: {identity_status}."),
        },
        CreatorOperationalChecklistItem {
            key: "taxProfileReady".to_string(),
            label: "Tax profile ready".to_string(),
            complete: tax_verified,
            detail: format!("Current tax status: {tax_status}."),
        },
        CreatorOperationalChecklistItem {
            key: "payoutMethodReady".to_string(),
            label: "Payout method active".to_string(),
            complete: payout_ready,
            detail: format!("Current payout status: {payout_status}."),
        },
        CreatorOperationalChecklistItem {
            key: "holdsClear".to_string(),
            label: "No active payout holds".to_string(),
            complete: holds_clear,
            detail: if holds_clear {
                "No manual or compliance holds are blocking monetization.".to_string()
            } else {
                format!("Active holds: {}.", hold_reasons.join(", "))
            },
        },
        CreatorOperationalChecklistItem {
            key: "enforcementClear".to_string(),
            label: "No active creator enforcement".to_string(),
            complete: active_enforcement_actions.is_empty(),
            detail: if active_enforcement_actions.is_empty() {
                "No operator-enforced restrictions are active.".to_string()
            } else {
                format!(
                    "Active enforcement scopes: {}.",
                    active_enforcement_actions
                        .iter()
                        .map(|action| action.scope.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
        },
    ];

    Ok(CreatorOperationalState {
        creator_id: profile.id.clone(),
        legal_name,
        support_email,
        business_type,
        payout_country,
        payout_provider,
        onboarding_status,
        identity_status,
        tax_status,
        payout_status,
        hold_reasons,
        active_enforcement_actions,
        live_streaming_enabled,
        upload_ingest_enabled,
        collaboration_enabled,
        monetization_enabled,
        payouts_enabled,
        can_receive_payouts,
        can_monetize,
        can_publish_paid_content: can_monetize,
        requires_action: !can_receive_payouts || !live_streaming_enabled || !upload_ingest_enabled,
        checklist,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        last_reviewed_at: row.get("last_reviewed_at"),
    })
}

pub(super) async fn fetch_creator_enforcement_state(
    pool: &SqlitePool,
    profile: &CreatorProfile,
) -> AppResult<CreatorEnforcementState> {
    reconcile_expired_creator_enforcement_actions_for_read(pool, Some(&profile.id), None).await?;
    let history = fetch_creator_enforcement_actions(pool, &profile.id).await?;
    let active_actions = fetch_active_creator_enforcement_actions(pool, &profile.id).await?;

    Ok(CreatorEnforcementState {
        creator_id: profile.id.clone(),
        live_streaming_enabled: !active_actions
            .iter()
            .any(|action| action.scope == "live_streaming"),
        upload_ingest_enabled: !active_actions
            .iter()
            .any(|action| action.scope == "uploads"),
        collaboration_enabled: !active_actions
            .iter()
            .any(|action| action.scope == "collaboration"),
        monetization_enabled: !active_actions
            .iter()
            .any(|action| action.scope == "monetization"),
        payouts_enabled: !active_actions
            .iter()
            .any(|action| action.scope == "payouts"),
        active_actions,
        history,
    })
}

pub(super) async fn fetch_creator_enforcement_actions(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<CreatorEnforcementAction>> {
    let rows = sqlx::query(
        r#"
        SELECT id, creator_id, scope, state, reason, resolution_note, created_by_user_id,
               released_by_user_id, created_at, released_at, expires_at
        FROM creator_enforcement_actions
        WHERE creator_id = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(creator_enforcement_action_from_row)
        .collect())
}

pub(super) async fn fetch_active_creator_enforcement_actions(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<CreatorEnforcementAction>> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"
        SELECT id, creator_id, scope, state, reason, resolution_note, created_by_user_id,
               released_by_user_id, created_at, released_at, expires_at
        FROM creator_enforcement_actions
        WHERE creator_id = ?
          AND state = 'active'
          AND (expires_at IS NULL OR expires_at > ?)
        ORDER BY created_at DESC
        "#,
    )
    .bind(creator_id)
    .bind(&now)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(creator_enforcement_action_from_row)
        .collect())
}

pub(super) async fn fetch_creator_enforcement_action_by_id(
    pool: &SqlitePool,
    action_id: &str,
) -> AppResult<CreatorEnforcementAction> {
    reconcile_expired_creator_enforcement_actions_for_read(pool, None, Some(action_id)).await?;
    fetch_creator_enforcement_action_by_id_raw(pool, action_id).await
}

pub(super) async fn fetch_creator_enforcement_action_by_id_raw(
    pool: &SqlitePool,
    action_id: &str,
) -> AppResult<CreatorEnforcementAction> {
    let row = sqlx::query(
        r#"
        SELECT id, creator_id, scope, state, reason, resolution_note, created_by_user_id,
               released_by_user_id, created_at, released_at, expires_at
        FROM creator_enforcement_actions
        WHERE id = ?
        "#,
    )
    .bind(action_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(creator_enforcement_action_from_row(row))
}

pub(super) async fn ensure_creator_live_settings_row(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO creator_live_settings (
            creator_id, subscriber_only, slow_mode_seconds, auto_mod_level, notify_followers_default,
            active_scene_id, scenes_json, bitrate_kbps, cpu_percent, dropped_frames, free_disk_gb
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(creator_id)
    .bind(1_i64)
    .bind(3_i64)
    .bind("standard")
    .bind(1_i64)
    .bind("cam-main")
    .bind(json!([
        {"id":"cam-main","label":"Main cam","active":true},
        {"id":"screen","label":"Screen + cam","active":false},
        {"id":"slide","label":"Slideshow","active":false},
        {"id":"brb","label":"BRB loop","active":false}
    ])
    .to_string())
    .bind(0_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind(0.0_f64)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn fetch_creator_live_settings(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorLiveSettings> {
    ensure_creator_live_settings_row(pool, creator_id).await?;
    let row = sqlx::query(
        r#"
        SELECT subscriber_only, slow_mode_seconds, auto_mod_level, notify_followers_default,
               active_scene_id, scenes_json
        FROM creator_live_settings
        WHERE creator_id = ?
        "#,
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(CreatorLiveSettings {
        subscriber_only: row.get::<i64, _>("subscriber_only") == 1,
        slow_mode_seconds: row.get("slow_mode_seconds"),
        auto_mod_level: row.get("auto_mod_level"),
        notify_followers_default: row.get::<i64, _>("notify_followers_default") == 1,
        active_scene_id: row.get("active_scene_id"),
        scenes: from_json(row.get::<String, _>("scenes_json"))?,
    })
}

pub(super) async fn fetch_creator_live_health(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<CreatorLiveHealth> {
    ensure_creator_live_settings_row(pool, creator_id).await?;
    let settings_row = sqlx::query(
        "SELECT bitrate_kbps, cpu_percent, dropped_frames, free_disk_gb FROM creator_live_settings WHERE creator_id = ?",
    )
    .bind(creator_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let sample_rows = sqlx::query(
        r#"
        SELECT collected_at, bitrate_kbps, viewers, cpu_percent, dropped_frames, free_disk_gb
        FROM creator_stream_health_samples
        WHERE creator_id = ?
        ORDER BY collected_at ASC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    Ok(CreatorLiveHealth {
        current_bitrate_kbps: settings_row.get("bitrate_kbps"),
        current_cpu_percent: settings_row.get("cpu_percent"),
        current_dropped_frames: settings_row.get("dropped_frames"),
        current_free_disk_gb: settings_row.get("free_disk_gb"),
        samples: sample_rows
            .into_iter()
            .map(|row| CreatorHealthSample {
                collected_at: row.get("collected_at"),
                bitrate_kbps: row.get("bitrate_kbps"),
                viewers: row.get("viewers"),
                cpu_percent: row.get("cpu_percent"),
                dropped_frames: row.get("dropped_frames"),
                free_disk_gb: row.get("free_disk_gb"),
            })
            .collect(),
    })
}

pub(super) async fn fetch_creator_subscriber_tiers(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<Vec<CreatorSubscriberTier>> {
    let rows = sqlx::query(
        r#"
        SELECT id, tier_name, rank, monthly_price, subscriber_count, accent_color, status, retired_at
        FROM creator_subscriber_tiers
        WHERE creator_id = ?
        ORDER BY CASE WHEN status = 'active' THEN 0 ELSE 1 END ASC, rank ASC, monthly_price ASC
        "#,
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| CreatorSubscriberTier {
            id: row.get("id"),
            tier_name: row.get("tier_name"),
            rank: row.get("rank"),
            monthly_price: row.get("monthly_price"),
            subscriber_count: row.get("subscriber_count"),
            accent_color: row.get("accent_color"),
            status: row.get("status"),
            retired_at: row.get("retired_at"),
        })
        .collect())
}

pub(super) async fn fetch_creator_subscriber_tier_by_id(
    pool: &SqlitePool,
    creator_id: &str,
    tier_id: &str,
) -> AppResult<CreatorSubscriberTier> {
    let row = sqlx::query(
        r#"
        SELECT id, tier_name, rank, monthly_price, subscriber_count, accent_color, status, retired_at
        FROM creator_subscriber_tiers
        WHERE creator_id = ? AND id = ?
        "#,
    )
    .bind(creator_id)
    .bind(tier_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(CreatorSubscriberTier {
        id: row.get("id"),
        tier_name: row.get("tier_name"),
        rank: row.get("rank"),
        monthly_price: row.get("monthly_price"),
        subscriber_count: row.get("subscriber_count"),
        accent_color: row.get("accent_color"),
        status: row.get("status"),
        retired_at: row.get("retired_at"),
    })
}

pub(super) async fn next_creator_subscriber_tier_rank(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<i64> {
    let row = sqlx::query(
        "SELECT COALESCE(MAX(rank), 0) AS max_rank FROM creator_subscriber_tiers WHERE creator_id = ?",
    )
    .bind(creator_id)
    .fetch_one(pool)
    .await?;
    let max_rank: i64 = row.get("max_rank");
    Ok(max_rank + 1)
}

pub(super) async fn normalize_creator_subscriber_tier_ranks(
    pool: &SqlitePool,
    creator_id: &str,
) -> AppResult<()> {
    let rows = sqlx::query(
        "SELECT id FROM creator_subscriber_tiers WHERE creator_id = ? ORDER BY rank ASC, monthly_price ASC, rowid ASC",
    )
    .bind(creator_id)
    .fetch_all(pool)
    .await?;
    for (index, row) in rows.into_iter().enumerate() {
        let tier_id: String = row.get("id");
        sqlx::query("UPDATE creator_subscriber_tiers SET rank = ? WHERE id = ? AND creator_id = ?")
            .bind((index + 1) as i64)
            .bind(&tier_id)
            .bind(creator_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub(super) fn validate_creator_subscriber_tier_input(
    tier_name: &str,
    rank: Option<i64>,
    monthly_price: f64,
    accent_color: &str,
) -> AppResult<()> {
    if tier_name.trim().is_empty() {
        return Err(AppError::BadRequest("tierName is required".to_string()));
    }
    if tier_name.len() > 64 {
        return Err(AppError::BadRequest(
            "tierName must be 64 characters or fewer".to_string(),
        ));
    }
    if rank.is_some_and(|value| value <= 0) {
        return Err(AppError::BadRequest(
            "rank must be greater than zero".to_string(),
        ));
    }
    if monthly_price <= 0.0 {
        return Err(AppError::BadRequest(
            "monthlyPrice must be greater than zero".to_string(),
        ));
    }
    if !accent_color.starts_with('#') || accent_color.len() != 7 {
        return Err(AppError::BadRequest(
            "accentColor must be a 7-character hex color".to_string(),
        ));
    }
    Ok(())
}
