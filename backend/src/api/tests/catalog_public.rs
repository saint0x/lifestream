use super::*;

async fn response_json<T: serde::de::DeserializeOwned>(response: Response) -> AppResult<T> {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(serde_json::from_slice(&body)?)
}

#[tokio::test]
async fn public_search_uses_catalog_repository_response_shape() -> AppResult<()> {
    let (state, _) = setup_test_state().await?;
    let payload: serde_json::Value = response_json(
        search(
            State(state),
            Query(SearchQuery {
                q: "Northlight".to_string(),
                limit: None,
                offset: None,
            }),
        )
        .await?
        .into_response(),
    )
    .await?;

    assert!(
        payload["series"]
            .as_array()
            .is_some_and(|items| { items.iter().any(|item| item["title"] == "Northlight") })
    );
    assert!(payload["items"].as_array().is_some_and(|items| {
        items.iter().any(|item| {
            item["kind"] == "series"
                && item["title"] == "Northlight"
                && item["href"] == "/series/northlight"
        })
    }));
    assert!(payload["films"].as_array().is_some());
    assert!(payload["liveStreams"].as_array().is_some());
    Ok(())
}

#[tokio::test]
async fn public_search_uses_catalog_metadata_and_people_profiles() -> AppResult<()> {
    let (state, _) = setup_test_state().await?;
    let payload: serde_json::Value = response_json(
        search(
            State(state),
            Query(SearchQuery {
                q: "Mara Vale".to_string(),
                limit: Some(8),
                offset: Some(0),
            }),
        )
        .await?
        .into_response(),
    )
    .await?;

    let items = payload["items"].as_array().expect("items");
    assert!(
        items
            .iter()
            .any(|item| { item["kind"] == "profile" && item["title"] == "Mara Vale" })
    );
    assert!(
        items
            .iter()
            .any(|item| { item["kind"] == "series" && item["title"] == "Northlight" })
    );
    assert!(payload["total"].as_i64().is_some_and(|total| total >= 2));
    Ok(())
}

#[tokio::test]
async fn public_catalog_series_page_is_bounded_and_filterable() -> AppResult<()> {
    let (state, _) = setup_test_state().await?;
    let payload: serde_json::Value = response_json(
        list_series_page(
            State(state),
            Query(CatalogPageQuery {
                genre: Some("Sci-Fi".to_string()),
                originals_only: Some(true),
                sort: Some("score".to_string()),
                limit: Some(2),
                offset: Some(0),
            }),
        )
        .await?
        .into_response(),
    )
    .await?;

    assert_eq!(payload["limit"], 2);
    assert_eq!(payload["offset"], 0);
    assert!(payload["total"].as_i64().is_some_and(|total| total >= 2));
    let items = payload["items"].as_array().expect("items");
    assert!(items.len() <= 2);
    assert!(items.iter().all(|item| item["isOriginal"] == true));
    assert!(items.iter().all(|item| {
        item["genres"]
            .as_array()
            .is_some_and(|genres| genres.iter().any(|genre| genre == "Sci-Fi"))
    }));
    Ok(())
}

#[tokio::test]
async fn public_catalog_films_page_reports_has_more() -> AppResult<()> {
    let (state, _) = setup_test_state().await?;
    let payload: serde_json::Value = response_json(
        list_films_page(
            State(state),
            Query(CatalogPageQuery {
                genre: None,
                originals_only: Some(false),
                sort: Some("trending".to_string()),
                limit: Some(1),
                offset: Some(0),
            }),
        )
        .await?
        .into_response(),
    )
    .await?;

    assert_eq!(payload["limit"], 1);
    assert_eq!(payload["offset"], 0);
    assert_eq!(payload["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        payload["hasMore"],
        payload["total"].as_i64().is_some_and(|total| total > 1)
    );
    Ok(())
}

#[tokio::test]
async fn public_catalog_episode_context_returns_parent_series() -> AppResult<()> {
    let (state, _) = setup_test_state().await?;
    let payload: serde_json::Value = response_json(
        get_series_for_episode(
            State(state),
            HeaderMap::new(),
            Path("ser-northlight-s1e1".to_string()),
        )
        .await?
        .into_response(),
    )
    .await?;

    assert_eq!(payload["id"], "ser-northlight");
    assert_eq!(payload["title"], "Northlight");
    assert!(
        payload["seasons"]
            .as_array()
            .is_some_and(|seasons| seasons.iter().any(|season| {
                season["episodes"].as_array().is_some_and(|episodes| {
                    episodes
                        .iter()
                        .any(|episode| episode["id"] == "ser-northlight-s1e1")
                })
            }))
    );
    Ok(())
}

#[tokio::test]
async fn authenticated_bootstrap_includes_provider_backed_session_listing() -> AppResult<()> {
    let (state, creator) = setup_test_state().await?;
    let token = insert_creator_auth_session(state.db.sqlite_adapter(), &creator).await?;
    let payload: serde_json::Value =
        response_json(bootstrap(State(state), auth_headers(&token)).await?).await?;

    assert_eq!(payload["me"]["id"], creator.user_id);
    assert!(
        payload["viewer"]["sessions"]
            .as_array()
            .is_some_and(|sessions| {
                sessions.iter().any(|session| {
                    session["label"] == "test-creator-session" && session["isCurrent"] == true
                })
            })
    );
    assert_eq!(payload["creator"]["profile"]["id"], creator.id);
    Ok(())
}
