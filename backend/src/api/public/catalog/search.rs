use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct SearchQuery {
    q: String,
}

pub(crate) async fn search(
    State(state): State<SharedState>,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let Some(fts_query) = build_fts_query(&query.q) else {
        return Ok(Json(serde_json::json!({
            "series": [],
            "films": [],
            "liveStreams": []
        })));
    };

    let rows = sqlx::query(
        r#"
        SELECT entity_id, kind
        FROM search_documents
        WHERE search_documents MATCH ?
        ORDER BY bm25(search_documents, 1.0, 0.3)
        LIMIT 24
        "#,
    )
    .bind(&fts_query)
    .fetch_all(&state.pool)
    .await?;

    let mut series = Vec::new();
    let mut films = Vec::new();
    let mut live_streams = Vec::new();
    for row in rows {
        let entity_id: String = row.get("entity_id");
        let kind: String = row.get("kind");
        match kind.as_str() {
            "series" => {
                if let Ok(item) = fetch_series_by_id(&state.pool, &entity_id, None).await {
                    series.push(item);
                }
            }
            "film" => {
                if let Ok(item) = fetch_film_by_id(&state.pool, &entity_id, None).await {
                    films.push(item);
                }
            }
            "live" => {
                if let Ok(item) = fetch_live_stream_by_id(&state.pool, &entity_id).await {
                    live_streams.push(item);
                }
            }
            _ => {}
        }
    }

    Ok(Json(serde_json::json!({
        "series": series,
        "films": films,
        "liveStreams": live_streams
    })))
}
