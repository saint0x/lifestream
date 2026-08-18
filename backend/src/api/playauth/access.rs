use super::*;

pub(crate) async fn resolve_upload_playback_access(
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

pub(crate) fn resolve_upload_access_terms(
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

pub(crate) async fn fetch_active_creator_membership(
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

async fn fetch_valid_content_purchase(
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
