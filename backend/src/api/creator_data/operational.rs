use super::*;

pub(crate) async fn fetch_creator_operational_state(
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
