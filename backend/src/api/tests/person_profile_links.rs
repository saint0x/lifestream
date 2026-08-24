use super::*;

async fn response_json<T: serde::de::DeserializeOwned>(response: Response) -> AppResult<T> {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(serde_json::from_slice(&body)?)
}

#[tokio::test]
async fn person_profile_public_links_round_trip_to_public_profile() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let headers = auth_headers(&token);

    let current: serde_json::Value = response_json(
        get_my_person_profile(State(state.clone()), headers.clone())
            .await?
            .into_response(),
    )
    .await?;
    let slug = current["slug"].as_str().expect("profile slug").to_string();

    let update = serde_json::json!({
        "instagramUrl": "https://instagram.com/vantatest",
        "xUrl": "https://x.com/vantatest",
        "imdbUrl": "https://www.imdb.com/name/nm0000001/",
        "linkedinUrl": "https://www.linkedin.com/in/vantatest",
        "facebookUrl": null,
        "publicLinks": [
            {
                "platform": "custom",
                "label": "Portfolio",
                "url": "https://creator.example/work"
            },
            {
                "platform": "newsletter",
                "label": "Newsletter",
                "url": "https://creator.example/news"
            }
        ]
    });
    let updated: serde_json::Value = response_json(
        update_my_person_profile(
            State(state.clone()),
            headers.clone(),
            Json(serde_json::from_value(update)?),
        )
        .await?
        .into_response(),
    )
    .await?;

    assert_eq!(updated["instagramUrl"], "https://instagram.com/vantatest");
    assert_eq!(updated["xUrl"], "https://x.com/vantatest");
    assert_eq!(updated["imdbUrl"], "https://www.imdb.com/name/nm0000001/");
    assert_eq!(
        updated["linkedinUrl"],
        "https://www.linkedin.com/in/vantatest"
    );
    assert!(updated["facebookUrl"].is_null());
    assert_eq!(updated["publicLinks"][0]["label"], "Portfolio");
    assert_eq!(updated["publicLinks"][1]["platform"], "newsletter");

    let public: serde_json::Value = response_json(
        get_person_profile(State(state.clone()), Path(slug))
            .await?
            .into_response(),
    )
    .await?;
    assert_eq!(public["instagramUrl"], "https://instagram.com/vantatest");
    assert_eq!(
        public["publicLinks"][0]["url"],
        "https://creator.example/work"
    );

    let cleared: serde_json::Value = response_json(
        update_my_person_profile(
            State(state),
            headers,
            Json(serde_json::from_value(serde_json::json!({
                "instagramUrl": null,
                "publicLinks": []
            }))?),
        )
        .await?
        .into_response(),
    )
    .await?;
    assert!(cleared["instagramUrl"].is_null());
    assert_eq!(cleared["publicLinks"].as_array().map(Vec::len), Some(0));

    Ok(())
}
