use super::*;

#[tokio::test]
async fn creator_can_accept_ad_offer_and_submit_review() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);
    let now = Utc::now().to_rfc3339();
    let offer_id = format!("offer-test-{}", Uuid::new_v4().simple());

    sqlx::query(
        r#"
        INSERT INTO ad_marketplace_offers (
            id, campaign_id, package_id, creator_id, title, brief, requirements_json,
            offer_amount_cents, creator_payout_cents, platform_fee_cents, currency, status,
            advertiser_review_status, due_at, created_at, updated_at, accepted_at, declined_at
        ) VALUES (
            ?, 'camp-vanta-seed-devtools-launch', 'pkg-creator-read-30', ?, 'Test creator read',
            'Test campaign brief', '["Submit rough cut", "Include campaign link"]',
            400000, 320000, 80000, 'USD', 'pending', 'not_submitted', ?, ?, ?, NULL, NULL
        )
        "#,
    )
    .bind(&offer_id)
    .bind(&creator.id)
    .bind((Utc::now() + chrono::Duration::days(7)).to_rfc3339())
    .bind(&now)
    .bind(&now)
    .execute(state.db.sqlite_adapter())
    .await?;

    let hub = get_ad_hub(State(state.clone()), headers.clone()).await?.0;
    assert!(hub.offers.iter().any(|offer| offer.id == offer_id));
    assert!(
        hub.packages
            .iter()
            .any(|package| package.code == "creator_read_30")
    );
    assert_eq!(hub.payment_provider.provider_key, "whop");

    let accepted = accept_ad_offer(
        State(state.clone()),
        headers.clone(),
        Path(offer_id.clone()),
    )
    .await?
    .0;
    assert_eq!(accepted.status, "accepted");
    assert!(accepted.accepted_at.is_some());

    let reviewed = submit_ad_offer_review(
        State(state.clone()),
        headers,
        Path(offer_id.clone()),
        Json(SubmitAdOfferReviewRequest {
            submission_url: "https://vanta.example/review/test-cut".to_string(),
            notes: Some("Rough cut is ready for advertiser approval.".to_string()),
        }),
    )
    .await?
    .0;

    assert_eq!(reviewed.status, "in_review");
    assert_eq!(reviewed.advertiser_review_status, "review_pending");
    assert_eq!(reviewed.submissions.len(), 1);
    assert_eq!(
        reviewed.submissions[0].submission_url,
        "https://vanta.example/review/test-cut"
    );
    Ok(())
}
