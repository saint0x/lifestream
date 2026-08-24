use super::*;

async fn stream_viewers(pool: &SqlitePool, stream_id: &str) -> AppResult<i64> {
    let row = sqlx::query("SELECT viewers FROM live_streams WHERE id = ?")
        .bind(stream_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(row.get("viewers"))
}

pub(crate) async fn effective_live_viewer_count(
    pool: &SqlitePool,
    stream_id: &str,
) -> AppResult<i64> {
    let reported = stream_viewers(pool, stream_id).await?;
    let connected = count_active_live_viewer_sessions(pool, stream_id).await?;
    Ok(reported.max(connected))
}

pub(crate) async fn count_active_live_viewer_sessions(
    pool: &SqlitePool,
    stream_id: &str,
) -> AppResult<i64> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS count
        FROM (
            SELECT COALESCE('u:' || user_id, 'v:' || visitor_id, 's:' || session_token_hash) AS viewer_key
            FROM live_viewer_sessions
            WHERE stream_id = ?
              AND disconnected_at IS NULL
              AND last_seen_at >= ?
            GROUP BY viewer_key
        ) active_viewers
        "#,
    )
    .bind(stream_id)
    .bind(active_presence_cutoff())
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(crate) async fn count_all_active_live_viewer_sessions(pool: &SqlitePool) -> AppResult<i64> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS count
        FROM (
            SELECT stream_id, COALESCE('u:' || user_id, 'v:' || visitor_id, 's:' || session_token_hash) AS viewer_key
            FROM live_viewer_sessions
            WHERE disconnected_at IS NULL
              AND last_seen_at >= ?
            GROUP BY stream_id, viewer_key
        ) active_viewers
        "#,
    )
    .bind(active_presence_cutoff())
    .fetch_one(pool)
    .await?;
    Ok(row.get("count"))
}

pub(crate) async fn fetch_live_viewer_sample_users(
    pool: &SqlitePool,
    stream_id: &str,
    limit: i64,
) -> AppResult<Vec<String>> {
    let rows = sqlx::query(
        r#"
        SELECT u.handle
        FROM live_viewer_sessions lvs
        JOIN users u ON u.id = lvs.user_id
        WHERE lvs.stream_id = ?
          AND lvs.user_id IS NOT NULL
          AND lvs.disconnected_at IS NULL
          AND lvs.last_seen_at >= ?
        GROUP BY u.id, u.handle
        ORDER BY MAX(lvs.last_seen_at) DESC
        LIMIT ?
        "#,
    )
    .bind(stream_id)
    .bind(active_presence_cutoff())
    .bind(limit.max(1))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.get("handle")).collect())
}

#[derive(Clone, Debug, Default)]
pub(crate) struct LiveViewerAttribution {
    pub(crate) visitor_id: Option<String>,
    pub(crate) landing_url: Option<String>,
    pub(crate) initial_referrer_url: Option<String>,
    pub(crate) current_url: Option<String>,
    pub(crate) current_referrer_url: Option<String>,
    pub(crate) utm_source: Option<String>,
    pub(crate) utm_medium: Option<String>,
    pub(crate) utm_campaign: Option<String>,
    pub(crate) utm_term: Option<String>,
    pub(crate) utm_content: Option<String>,
}

impl LiveViewerAttribution {
    fn normalized(&self) -> NormalizedLiveViewerAttribution {
        let visitor_id = normalize_token_like_value(self.visitor_id.as_deref(), 96);
        let landing_url = normalize_text_value(self.landing_url.as_deref(), 2048);
        let initial_referrer_url = normalize_text_value(self.initial_referrer_url.as_deref(), 2048);
        let current_url = normalize_text_value(self.current_url.as_deref(), 2048);
        let current_referrer_url = normalize_text_value(self.current_referrer_url.as_deref(), 2048);
        let utm_source = normalize_text_value(self.utm_source.as_deref(), 160);
        let utm_medium = normalize_text_value(self.utm_medium.as_deref(), 160);
        let utm_campaign = normalize_text_value(self.utm_campaign.as_deref(), 160);
        let utm_term = normalize_text_value(self.utm_term.as_deref(), 160);
        let utm_content = normalize_text_value(self.utm_content.as_deref(), 160);
        let attribution_source = utm_source
            .clone()
            .or_else(|| referrer_host(current_referrer_url.as_deref()))
            .or_else(|| referrer_host(initial_referrer_url.as_deref()))
            .unwrap_or_else(|| "direct".to_string());
        let attribution_medium = utm_medium.clone().unwrap_or_else(|| {
            if attribution_source == "direct" {
                "direct"
            } else {
                "referral"
            }
            .to_string()
        });
        NormalizedLiveViewerAttribution {
            visitor_id,
            landing_url,
            initial_referrer_url,
            current_url,
            current_referrer_url,
            utm_source,
            utm_medium,
            utm_campaign: utm_campaign.clone(),
            utm_term,
            utm_content,
            attribution_source: Some(attribution_source),
            attribution_medium: Some(attribution_medium),
            attribution_campaign: utm_campaign,
        }
    }
}

struct NormalizedLiveViewerAttribution {
    visitor_id: Option<String>,
    landing_url: Option<String>,
    initial_referrer_url: Option<String>,
    current_url: Option<String>,
    current_referrer_url: Option<String>,
    utm_source: Option<String>,
    utm_medium: Option<String>,
    utm_campaign: Option<String>,
    utm_term: Option<String>,
    utm_content: Option<String>,
    attribution_source: Option<String>,
    attribution_medium: Option<String>,
    attribution_campaign: Option<String>,
}

pub(crate) async fn register_live_viewer_session(
    pool: &SqlitePool,
    stream_id: &str,
    identity: Option<&RequestIdentity>,
    session_token: Option<&str>,
    attribution: Option<&LiveViewerAttribution>,
) -> AppResult<(String, bool, String)> {
    let now = Utc::now().to_rfc3339();
    let attribution = attribution
        .map(LiveViewerAttribution::normalized)
        .unwrap_or_default();
    if let Some(token) = session_token.filter(|value| !value.trim().is_empty()) {
        let token_hash = hash_token(token);
        let existing = sqlx::query(
            r#"
            SELECT user_id
            FROM live_viewer_sessions
            WHERE stream_id = ? AND session_token_hash = ?
            ORDER BY connected_at DESC
            LIMIT 1
            "#,
        )
        .bind(&stream_id)
        .bind(&token_hash)
        .fetch_optional(pool)
        .await?;
        if let Some(row) = existing {
            let bound_user_id = row.get::<Option<String>, _>("user_id");
            let requested_user_id = identity.map(|item| item.user_id.as_str());
            if bound_user_id
                .as_deref()
                .is_some_and(|bound| Some(bound) != requested_user_id)
            {
                return Err(AppError::Forbidden);
            }

            let result = sqlx::query(
                r#"
                UPDATE live_viewer_sessions
                SET user_id = COALESCE(?, user_id),
                    visitor_id = COALESCE(visitor_id, ?),
                    landing_url = COALESCE(landing_url, ?),
                    initial_referrer_url = COALESCE(initial_referrer_url, ?),
                    current_url = COALESCE(?, current_url),
                    current_referrer_url = COALESCE(?, current_referrer_url),
                    utm_source = COALESCE(?, utm_source),
                    utm_medium = COALESCE(?, utm_medium),
                    utm_campaign = COALESCE(?, utm_campaign),
                    utm_term = COALESCE(?, utm_term),
                    utm_content = COALESCE(?, utm_content),
                    attribution_source = COALESCE(?, attribution_source),
                    attribution_medium = COALESCE(?, attribution_medium),
                    attribution_campaign = COALESCE(?, attribution_campaign),
                    connected_at = ?,
                    last_seen_at = ?,
                    disconnected_at = NULL
                WHERE stream_id = ? AND session_token_hash = ?
                "#,
            )
            .bind(requested_user_id)
            .bind(attribution.visitor_id.as_deref())
            .bind(attribution.landing_url.as_deref())
            .bind(attribution.initial_referrer_url.as_deref())
            .bind(attribution.current_url.as_deref())
            .bind(attribution.current_referrer_url.as_deref())
            .bind(attribution.utm_source.as_deref())
            .bind(attribution.utm_medium.as_deref())
            .bind(attribution.utm_campaign.as_deref())
            .bind(attribution.utm_term.as_deref())
            .bind(attribution.utm_content.as_deref())
            .bind(attribution.attribution_source.as_deref())
            .bind(attribution.attribution_medium.as_deref())
            .bind(attribution.attribution_campaign.as_deref())
            .bind(&now)
            .bind(&now)
            .bind(&stream_id)
            .bind(&token_hash)
            .execute(pool)
            .await?;
            if result.rows_affected() > 0 {
                return Ok((token.to_string(), true, now));
            }
        }
    }

    let raw_token = format!(
        "wss_{}_{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    sqlx::query(
        r#"
        INSERT INTO live_viewer_sessions (
            id, stream_id, user_id, session_token_hash, visitor_id, landing_url,
            initial_referrer_url, current_url, current_referrer_url, utm_source,
            utm_medium, utm_campaign, utm_term, utm_content, attribution_source,
            attribution_medium, attribution_campaign, connected_at, last_seen_at,
            disconnected_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(format!("lvs-{}", Uuid::new_v4().simple()))
    .bind(stream_id)
    .bind(identity.map(|item| item.user_id.as_str()))
    .bind(hash_token(&raw_token))
    .bind(attribution.visitor_id.as_deref())
    .bind(attribution.landing_url.as_deref())
    .bind(attribution.initial_referrer_url.as_deref())
    .bind(attribution.current_url.as_deref())
    .bind(attribution.current_referrer_url.as_deref())
    .bind(attribution.utm_source.as_deref())
    .bind(attribution.utm_medium.as_deref())
    .bind(attribution.utm_campaign.as_deref())
    .bind(attribution.utm_term.as_deref())
    .bind(attribution.utm_content.as_deref())
    .bind(attribution.attribution_source.as_deref())
    .bind(attribution.attribution_medium.as_deref())
    .bind(attribution.attribution_campaign.as_deref())
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok((raw_token, false, now))
}

impl Default for NormalizedLiveViewerAttribution {
    fn default() -> Self {
        Self {
            visitor_id: None,
            landing_url: None,
            initial_referrer_url: None,
            current_url: None,
            current_referrer_url: None,
            utm_source: None,
            utm_medium: None,
            utm_campaign: None,
            utm_term: None,
            utm_content: None,
            attribution_source: Some("direct".to_string()),
            attribution_medium: Some("direct".to_string()),
            attribution_campaign: None,
        }
    }
}

fn normalize_text_value(value: Option<&str>, max_len: usize) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(max_len).collect())
}

fn normalize_token_like_value(value: Option<&str>, max_len: usize) -> Option<String> {
    normalize_text_value(value, max_len)
        .map(|value| {
            value
                .chars()
                .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':'))
                .collect::<String>()
        })
        .filter(|value| !value.is_empty())
}

fn referrer_host(value: Option<&str>) -> Option<String> {
    let value = value?;
    let without_scheme = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
        .unwrap_or(value);
    without_scheme
        .split('/')
        .next()
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .map(|host| host.chars().take(160).collect())
}

pub(crate) async fn touch_live_viewer_session(
    pool: &SqlitePool,
    stream_id: &str,
    session_token: &str,
    lease_connected_at: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE live_viewer_sessions SET last_seen_at = ?, disconnected_at = NULL WHERE stream_id = ? AND session_token_hash = ? AND connected_at = ?",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(stream_id)
    .bind(hash_token(session_token))
    .bind(lease_connected_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn disconnect_live_viewer_session(
    pool: &SqlitePool,
    stream_id: &str,
    session_token: &str,
    lease_connected_at: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE live_viewer_sessions SET last_seen_at = ?, disconnected_at = ? WHERE stream_id = ? AND session_token_hash = ? AND connected_at = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(stream_id)
    .bind(hash_token(session_token))
    .bind(lease_connected_at)
    .execute(pool)
    .await?;
    Ok(())
}
