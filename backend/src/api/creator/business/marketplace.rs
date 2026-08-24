use super::*;

type PackageRow = (
    String,
    String,
    String,
    String,
    String,
    Option<i64>,
    String,
    i64,
    String,
    String,
);

type SubmissionRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
);

#[derive(sqlx::FromRow)]
struct OfferRow {
    offer_id: String,
    offer_title: String,
    brief: String,
    requirements_json: String,
    offer_amount_cents: i64,
    creator_payout_cents: i64,
    platform_fee_cents: i64,
    offer_currency: String,
    offer_status: String,
    advertiser_review_status: String,
    due_at: Option<String>,
    offer_created_at: String,
    offer_updated_at: String,
    accepted_at: Option<String>,
    declined_at: Option<String>,
    package_id: String,
    package_code: String,
    package_title: String,
    package_description: String,
    placement_kind: String,
    spot_length_seconds: Option<i64>,
    deliverables_json: String,
    base_price_cents: i64,
    package_currency: String,
    package_status: String,
    advertiser_id: String,
    advertiser_name: String,
    advertiser_industry: String,
    advertiser_website_url: Option<String>,
    campaign_id: String,
    campaign_name: String,
    campaign_objective: String,
    campaign_starts_at: Option<String>,
    campaign_ends_at: Option<String>,
    campaign_budget_cents: i64,
    campaign_currency: String,
    campaign_status: String,
}

pub(crate) async fn get_ad_hub(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<CreatorAdHubResponse>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    Ok(Json(fetch_ad_hub(&state, creator_id).await?))
}

pub(crate) async fn accept_ad_offer(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(offer_id): Path<String>,
) -> AppResult<Json<AdMarketplaceOffer>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let now = Utc::now().to_rfc3339();

    if let Ok(pool) = state.db.try_sqlite_adapter() {
        let result = sqlx::query(
            r#"
            UPDATE ad_marketplace_offers
            SET status = 'accepted', accepted_at = COALESCE(accepted_at, ?), updated_at = ?
            WHERE id = ? AND creator_id = ? AND status = 'pending'
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(&offer_id)
        .bind(creator_id)
        .execute(pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::Conflict(
                "offer cannot be accepted from its current state".to_string(),
            ));
        }
    } else {
        let pool = state.db.try_postgres_adapter()?;
        let result = sqlx::query(
            r#"
            UPDATE ad_marketplace_offers
            SET status = 'accepted', accepted_at = COALESCE(accepted_at, $1), updated_at = $2
            WHERE id = $3 AND creator_id = $4 AND status = 'pending'
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(&offer_id)
        .bind(creator_id)
        .execute(pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::Conflict(
                "offer cannot be accepted from its current state".to_string(),
            ));
        }
    }

    fetch_ad_offer(&state, creator_id, &offer_id)
        .await
        .map(Json)
}

pub(crate) async fn decline_ad_offer(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(offer_id): Path<String>,
) -> AppResult<Json<AdMarketplaceOffer>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let now = Utc::now().to_rfc3339();

    if let Ok(pool) = state.db.try_sqlite_adapter() {
        let result = sqlx::query(
            r#"
            UPDATE ad_marketplace_offers
            SET status = 'declined', declined_at = COALESCE(declined_at, ?), updated_at = ?
            WHERE id = ? AND creator_id = ? AND status IN ('pending', 'accepted')
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(&offer_id)
        .bind(creator_id)
        .execute(pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::Conflict(
                "offer cannot be declined from its current state".to_string(),
            ));
        }
    } else {
        let pool = state.db.try_postgres_adapter()?;
        let result = sqlx::query(
            r#"
            UPDATE ad_marketplace_offers
            SET status = 'declined', declined_at = COALESCE(declined_at, $1), updated_at = $2
            WHERE id = $3 AND creator_id = $4 AND status IN ('pending', 'accepted')
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(&offer_id)
        .bind(creator_id)
        .execute(pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::Conflict(
                "offer cannot be declined from its current state".to_string(),
            ));
        }
    }

    fetch_ad_offer(&state, creator_id, &offer_id)
        .await
        .map(Json)
}

pub(crate) async fn submit_ad_offer_review(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(offer_id): Path<String>,
    Json(input): Json<SubmitAdOfferReviewRequest>,
) -> AppResult<Json<AdMarketplaceOffer>> {
    let identity = require_identity(&state.db, &headers).await?;
    let creator_id = identity.require_creator_scope()?;
    let submission_url = input.submission_url.trim();
    if submission_url.is_empty() || submission_url.len() > 500 {
        return Err(AppError::BadRequest(
            "submission url is required and must be under 500 characters".to_string(),
        ));
    }
    let notes = input.notes.unwrap_or_default().trim().to_string();
    if notes.len() > 2_000 {
        return Err(AppError::BadRequest(
            "submission notes must be under 2000 characters".to_string(),
        ));
    }

    let now = Utc::now().to_rfc3339();
    let submission_id = format!("adsub-{}", Uuid::new_v4().simple());

    if let Ok(pool) = state.db.try_sqlite_adapter() {
        let result = sqlx::query(
            r#"
            UPDATE ad_marketplace_offers
            SET status = 'in_review', advertiser_review_status = 'review_pending', updated_at = ?
            WHERE id = ? AND creator_id = ? AND status IN ('accepted', 'in_review')
            "#,
        )
        .bind(&now)
        .bind(&offer_id)
        .bind(creator_id)
        .execute(pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::Conflict(
                "offer must be accepted before submitting for review".to_string(),
            ));
        }
        sqlx::query(
            r#"
            INSERT INTO ad_marketplace_submissions (
                id, offer_id, creator_id, submission_url, notes, status, submitted_at,
                reviewed_at, advertiser_feedback, revision_due_at
            ) VALUES (?, ?, ?, ?, ?, 'review_pending', ?, NULL, NULL, NULL)
            "#,
        )
        .bind(&submission_id)
        .bind(&offer_id)
        .bind(creator_id)
        .bind(submission_url)
        .bind(&notes)
        .bind(&now)
        .execute(pool)
        .await?;
    } else {
        let pool = state.db.try_postgres_adapter()?;
        let result = sqlx::query(
            r#"
            UPDATE ad_marketplace_offers
            SET status = 'in_review', advertiser_review_status = 'review_pending', updated_at = $1
            WHERE id = $2 AND creator_id = $3 AND status IN ('accepted', 'in_review')
            "#,
        )
        .bind(&now)
        .bind(&offer_id)
        .bind(creator_id)
        .execute(pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::Conflict(
                "offer must be accepted before submitting for review".to_string(),
            ));
        }
        sqlx::query(
            r#"
            INSERT INTO ad_marketplace_submissions (
                id, offer_id, creator_id, submission_url, notes, status, submitted_at,
                reviewed_at, advertiser_feedback, revision_due_at
            ) VALUES ($1, $2, $3, $4, $5, 'review_pending', $6, NULL, NULL, NULL)
            "#,
        )
        .bind(&submission_id)
        .bind(&offer_id)
        .bind(creator_id)
        .bind(submission_url)
        .bind(&notes)
        .bind(&now)
        .execute(pool)
        .await?;
    }

    fetch_ad_offer(&state, creator_id, &offer_id)
        .await
        .map(Json)
}

async fn fetch_ad_hub(state: &SharedState, creator_id: &str) -> AppResult<CreatorAdHubResponse> {
    let offers = list_ad_offers(state, creator_id).await?;
    let packages = list_ad_packages(state).await?;
    let payment_provider = fetch_payment_provider(state, "whop").await?;
    let summary = summarize_offers(&offers);
    Ok(CreatorAdHubResponse {
        summary,
        offers,
        packages,
        payment_provider,
    })
}

async fn fetch_ad_offer(
    state: &SharedState,
    creator_id: &str,
    offer_id: &str,
) -> AppResult<AdMarketplaceOffer> {
    let offers = list_ad_offers(state, creator_id).await?;
    offers
        .into_iter()
        .find(|offer| offer.id == offer_id)
        .ok_or(AppError::NotFound)
}

async fn list_ad_packages(state: &SharedState) -> AppResult<Vec<AdMarketplacePackage>> {
    if let Ok(pool) = state.db.try_sqlite_adapter() {
        let rows = sqlx::query_as::<_, PackageRow>(
            r#"
            SELECT id, code, title, description, placement_kind, spot_length_seconds,
                   deliverables_json, base_price_cents, currency, status
            FROM ad_marketplace_inventory_packages
            WHERE status = 'active'
            ORDER BY base_price_cents ASC
            "#,
        )
        .fetch_all(pool)
        .await?;
        return rows.into_iter().map(package_from_row).collect();
    }
    let rows = sqlx::query_as::<_, PackageRow>(
        r#"
        SELECT id, code, title, description, placement_kind, spot_length_seconds,
               deliverables_json, base_price_cents, currency, status
        FROM ad_marketplace_inventory_packages
        WHERE status = 'active'
        ORDER BY base_price_cents ASC
        "#,
    )
    .fetch_all(state.db.try_postgres_adapter()?)
    .await?;
    rows.into_iter().map(package_from_row).collect()
}

async fn list_ad_offers(
    state: &SharedState,
    creator_id: &str,
) -> AppResult<Vec<AdMarketplaceOffer>> {
    let mut offers = if let Ok(pool) = state.db.try_sqlite_adapter() {
        let rows = sqlx::query_as::<_, OfferRow>(ad_offer_select_sql("?").as_str())
            .bind(creator_id)
            .fetch_all(pool)
            .await?;
        rows.into_iter()
            .map(offer_from_row)
            .collect::<AppResult<Vec<_>>>()?
    } else {
        let rows = sqlx::query_as::<_, OfferRow>(ad_offer_select_sql("$1").as_str())
            .bind(creator_id)
            .fetch_all(state.db.try_postgres_adapter()?)
            .await?;
        rows.into_iter()
            .map(offer_from_row)
            .collect::<AppResult<Vec<_>>>()?
    };

    for offer in &mut offers {
        offer.submissions = list_ad_submissions(state, creator_id, &offer.id).await?;
    }
    Ok(offers)
}

async fn list_ad_submissions(
    state: &SharedState,
    creator_id: &str,
    offer_id: &str,
) -> AppResult<Vec<AdMarketplaceSubmission>> {
    if let Ok(pool) = state.db.try_sqlite_adapter() {
        let rows = sqlx::query_as::<_, SubmissionRow>(
            r#"
            SELECT id, offer_id, submission_url, notes, status, submitted_at, reviewed_at,
                   advertiser_feedback, revision_due_at
            FROM ad_marketplace_submissions
            WHERE creator_id = ? AND offer_id = ?
            ORDER BY submitted_at DESC
            "#,
        )
        .bind(creator_id)
        .bind(offer_id)
        .fetch_all(pool)
        .await?;
        return Ok(rows.into_iter().map(submission_from_row).collect());
    }

    let rows = sqlx::query_as::<_, SubmissionRow>(
        r#"
        SELECT id, offer_id, submission_url, notes, status, submitted_at, reviewed_at,
               advertiser_feedback, revision_due_at
        FROM ad_marketplace_submissions
        WHERE creator_id = $1 AND offer_id = $2
        ORDER BY submitted_at DESC
        "#,
    )
    .bind(creator_id)
    .bind(offer_id)
    .fetch_all(state.db.try_postgres_adapter()?)
    .await?;
    Ok(rows.into_iter().map(submission_from_row).collect())
}

async fn fetch_payment_provider(
    state: &SharedState,
    provider_key: &str,
) -> AppResult<AdMarketplacePaymentProvider> {
    if let Ok(pool) = state.db.try_sqlite_adapter() {
        let row = sqlx::query(
            r#"
            SELECT provider_key, display_name, enabled, mode, status
            FROM ad_marketplace_payment_providers
            WHERE provider_key = ?
            "#,
        )
        .bind(provider_key)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
        return Ok(AdMarketplacePaymentProvider {
            provider_key: row.get("provider_key"),
            display_name: row.get("display_name"),
            enabled: row.get::<i64, _>("enabled") == 1,
            mode: row.get("mode"),
            status: row.get("status"),
        });
    }

    let row = sqlx::query(
        r#"
        SELECT provider_key, display_name, enabled, mode, status
        FROM ad_marketplace_payment_providers
        WHERE provider_key = $1
        "#,
    )
    .bind(provider_key)
    .fetch_optional(state.db.try_postgres_adapter()?)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(AdMarketplacePaymentProvider {
        provider_key: row.get("provider_key"),
        display_name: row.get("display_name"),
        enabled: row.get("enabled"),
        mode: row.get("mode"),
        status: row.get("status"),
    })
}

fn ad_offer_select_sql(creator_placeholder: &str) -> String {
    format!(
        r#"
        SELECT
            o.id AS offer_id,
            o.title AS offer_title,
            o.brief,
            o.requirements_json,
            o.offer_amount_cents,
            o.creator_payout_cents,
            o.platform_fee_cents,
            o.currency AS offer_currency,
            o.status AS offer_status,
            o.advertiser_review_status,
            o.due_at,
            o.created_at AS offer_created_at,
            o.updated_at AS offer_updated_at,
            o.accepted_at,
            o.declined_at,
            p.id AS package_id,
            p.code AS package_code,
            p.title AS package_title,
            p.description AS package_description,
            p.placement_kind,
            p.spot_length_seconds,
            p.deliverables_json,
            p.base_price_cents,
            p.currency AS package_currency,
            p.status AS package_status,
            a.id AS advertiser_id,
            a.name AS advertiser_name,
            a.industry AS advertiser_industry,
            a.website_url AS advertiser_website_url,
            c.id AS campaign_id,
            c.name AS campaign_name,
            c.objective AS campaign_objective,
            c.starts_at AS campaign_starts_at,
            c.ends_at AS campaign_ends_at,
            c.budget_cents AS campaign_budget_cents,
            c.currency AS campaign_currency,
            c.status AS campaign_status
        FROM ad_marketplace_offers o
        JOIN ad_marketplace_inventory_packages p ON p.id = o.package_id
        JOIN ad_marketplace_campaigns c ON c.id = o.campaign_id
        JOIN ad_marketplace_advertisers a ON a.id = c.advertiser_id
        WHERE o.creator_id = {creator_placeholder}
        ORDER BY
            CASE o.status
                WHEN 'pending' THEN 0
                WHEN 'accepted' THEN 1
                WHEN 'in_review' THEN 2
                WHEN 'approved' THEN 3
                WHEN 'declined' THEN 4
                ELSE 5
            END,
            o.updated_at DESC
        "#,
    )
}

fn summarize_offers(offers: &[AdMarketplaceOffer]) -> AdMarketplaceSummary {
    AdMarketplaceSummary {
        pending_offers: offers
            .iter()
            .filter(|offer| offer.status == "pending")
            .count() as i64,
        active_offers: offers
            .iter()
            .filter(|offer| offer.status == "accepted")
            .count() as i64,
        in_review_offers: offers
            .iter()
            .filter(|offer| offer.status == "in_review")
            .count() as i64,
        approved_offers: offers
            .iter()
            .filter(|offer| offer.status == "approved")
            .count() as i64,
        declined_offers: offers
            .iter()
            .filter(|offer| offer.status == "declined")
            .count() as i64,
        total_offer_amount_cents: offers
            .iter()
            .filter(|offer| offer.status != "declined")
            .map(|offer| offer.offer_amount_cents)
            .sum(),
        total_creator_payout_cents: offers
            .iter()
            .filter(|offer| offer.status != "declined")
            .map(|offer| offer.creator_payout_cents)
            .sum(),
        currency: offers
            .first()
            .map(|offer| offer.currency.clone())
            .unwrap_or_else(|| "USD".to_string()),
    }
}

fn package_from_row(row: PackageRow) -> AppResult<AdMarketplacePackage> {
    let (
        id,
        code,
        title,
        description,
        placement_kind,
        spot_length_seconds,
        deliverables_json,
        base_price_cents,
        currency,
        status,
    ) = row;
    Ok(AdMarketplacePackage {
        id,
        code,
        title,
        description,
        placement_kind,
        spot_length_seconds,
        deliverables: from_json(deliverables_json)?,
        base_price_cents,
        currency,
        status,
    })
}

fn offer_from_row(row: OfferRow) -> AppResult<AdMarketplaceOffer> {
    Ok(AdMarketplaceOffer {
        id: row.offer_id,
        title: row.offer_title,
        brief: row.brief,
        requirements: from_json(row.requirements_json)?,
        offer_amount_cents: row.offer_amount_cents,
        creator_payout_cents: row.creator_payout_cents,
        platform_fee_cents: row.platform_fee_cents,
        currency: row.offer_currency,
        status: row.offer_status,
        advertiser_review_status: row.advertiser_review_status,
        due_at: row.due_at,
        created_at: row.offer_created_at,
        updated_at: row.offer_updated_at,
        accepted_at: row.accepted_at,
        declined_at: row.declined_at,
        package: AdMarketplacePackage {
            id: row.package_id,
            code: row.package_code,
            title: row.package_title,
            description: row.package_description,
            placement_kind: row.placement_kind,
            spot_length_seconds: row.spot_length_seconds,
            deliverables: from_json(row.deliverables_json)?,
            base_price_cents: row.base_price_cents,
            currency: row.package_currency,
            status: row.package_status,
        },
        advertiser: AdMarketplaceAdvertiser {
            id: row.advertiser_id,
            name: row.advertiser_name,
            industry: row.advertiser_industry,
            website_url: row.advertiser_website_url,
        },
        campaign: AdMarketplaceCampaign {
            id: row.campaign_id,
            name: row.campaign_name,
            objective: row.campaign_objective,
            starts_at: row.campaign_starts_at,
            ends_at: row.campaign_ends_at,
            budget_cents: row.campaign_budget_cents,
            currency: row.campaign_currency,
            status: row.campaign_status,
        },
        submissions: Vec::new(),
    })
}

fn submission_from_row(row: SubmissionRow) -> AdMarketplaceSubmission {
    let (
        id,
        offer_id,
        submission_url,
        notes,
        status,
        submitted_at,
        reviewed_at,
        advertiser_feedback,
        revision_due_at,
    ) = row;
    AdMarketplaceSubmission {
        id,
        offer_id,
        submission_url,
        notes,
        status,
        submitted_at,
        reviewed_at,
        advertiser_feedback,
        revision_due_at,
    }
}
