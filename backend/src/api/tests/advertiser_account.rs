use super::*;
use crate::models::UpdateAdvertiserCompanyRequest;

#[tokio::test]
async fn advertiser_admin_can_manage_company_profile_and_invites() -> AppResult<()> {
    let (state, _) = setup_test_state().await?;
    let auth_user_id = "user-demo-advertiser-admin";

    let account = state
        .db
        .fetch_advertiser_account_for_auth_user(auth_user_id)
        .await?;
    assert_eq!(account.company.name, "Northstar DevTools");
    assert_eq!(account.current_seat.role, "admin");
    assert!(
        account
            .current_seat
            .permissions
            .iter()
            .any(|permission| permission == "manage_team")
    );

    let now = Utc::now().to_rfc3339();
    let updated = state
        .db
        .update_advertiser_company_for_auth_user(
            auth_user_id,
            &UpdateAdvertiserCompanyRequest {
                name: "Northstar Supply Co.".to_string(),
                industry: "Outdoor retail and gear".to_string(),
                website_url: Some("https://northstarsupply.example".to_string()),
                billing_name: "Northstar Supply Co. Marketing".to_string(),
                billing_email: "ap@northstarsupply.example".to_string(),
            },
            &now,
        )
        .await?;

    assert_eq!(updated.company.name, "Northstar Supply Co.");
    assert_eq!(updated.company.billing_status, "active");

    let invite_id = format!("adv-invite-test-{}", Uuid::new_v4().simple());
    let invited = state
        .db
        .create_advertiser_invite_for_auth_user(
            auth_user_id,
            &invite_id,
            "reviewer@northstarsupply.example",
            "reviewer",
            &hash_token(&invite_id),
            &now,
            &(Utc::now() + chrono::Duration::days(14)).to_rfc3339(),
        )
        .await?;

    let invite = invited
        .invites
        .iter()
        .find(|invite| invite.id == invite_id)
        .expect("new invite");
    assert_eq!(invite.role, "reviewer");
    assert_eq!(invite.status, "pending");
    assert_eq!(invite.permissions, vec!["approve_work".to_string()]);

    Ok(())
}
