use sqlx::{PgPool, Row, SqlitePool, postgres::PgRow, sqlite::SqliteRow};

use crate::api::PlaybackSessionRecord;
use crate::auth::{RequestIdentity, hash_token};
use crate::config::DatabaseKind;
use crate::error::{AppError, AppResult};
use crate::models::{
    AuthSession, ContinueWatchingEntry, PlaybackSession, ProgressInput, UpdateProfileRequest,
    UpdateSettingsRequest, User,
};

pub struct NewAuthSession<'a> {
    pub id: &'a str,
    pub user_id: &'a str,
    pub label: &'a str,
    pub token_hash: &'a str,
    pub scopes_json: &'a str,
    pub created_at: &'a str,
    pub expires_at: Option<&'a str>,
}

pub struct ProvisionedUser<'a> {
    pub id: &'a str,
    pub handle: &'a str,
    pub display_name: &'a str,
    pub avatar_url: &'a str,
    pub tier: &'a str,
    pub joined_at: &'a str,
}

pub struct ProvisionedCreator<'a> {
    pub id: &'a str,
    pub user_id: &'a str,
    pub handle: &'a str,
    pub display_name: &'a str,
    pub avatar_url: &'a str,
    pub banner_url: &'a str,
    pub tagline: &'a str,
    pub bio: &'a str,
    pub partner_status: &'a str,
    pub joined_at: &'a str,
    pub stream_key: &'a str,
    pub rtmp_url: &'a str,
    pub default_category: &'a str,
    pub default_tags_json: &'a str,
}

pub struct CatalogSearchHit {
    pub entity_id: String,
    pub kind: String,
}

pub struct EmailCredential {
    pub user_id: String,
    pub password_hash: String,
}

pub struct OAuthAccount {
    pub user_id: String,
}

pub struct ReusableLivePlaybackSessionLookup<'a> {
    pub stream_id: &'a str,
    pub auth_session_id: Option<&'a str>,
    pub device_id: Option<&'a str>,
    pub now: &'a str,
}

pub struct NewPlaybackSession<'a> {
    pub id: &'a str,
    pub auth_session_id: Option<&'a str>,
    pub user_id: Option<&'a str>,
    pub creator_id: Option<&'a str>,
    pub asset_id: &'a str,
    pub content_id: &'a str,
    pub content_kind: &'a str,
    pub playback_token: &'a str,
    pub access_scope: &'a str,
    pub created_at: &'a str,
    pub expires_at: &'a str,
    pub device_id: Option<&'a str>,
    pub device_name: Option<&'a str>,
    pub player_version: Option<&'a str>,
    pub capabilities_json: Option<&'a str>,
}

pub struct PlaybackSessionMetadataUpdate<'a> {
    pub device_name: Option<&'a str>,
    pub player_version: Option<&'a str>,
    pub capabilities_json: Option<&'a str>,
}

struct ProgressTarget {
    kind: String,
    episode_id: Option<String>,
    duration_sec: i64,
}

#[derive(Clone)]
pub struct Database {
    provider: DatabaseProvider,
}

#[derive(Clone)]
enum DatabaseProvider {
    Sqlite(SqlitePool),
    #[allow(dead_code)]
    Postgres(PgPool),
}

impl Database {
    pub fn from_sqlite(pool: SqlitePool) -> Self {
        Self {
            provider: DatabaseProvider::Sqlite(pool),
        }
    }

    #[allow(dead_code)]
    pub fn from_postgres(pool: PgPool) -> Self {
        Self {
            provider: DatabaseProvider::Postgres(pool),
        }
    }

    #[allow(dead_code)]
    pub fn kind(&self) -> DatabaseKind {
        match &self.provider {
            DatabaseProvider::Sqlite(_) => DatabaseKind::Sqlite,
            DatabaseProvider::Postgres(_) => DatabaseKind::Postgres,
        }
    }

    pub async fn check(&self) -> AppResult<bool> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                let db_ok: i64 = sqlx::query("SELECT 1").fetch_one(pool).await?.get(0);
                Ok(db_ok == 1)
            }
            DatabaseProvider::Postgres(pool) => {
                let db_ok: i32 = sqlx::query("SELECT 1").fetch_one(pool).await?.get(0);
                Ok(db_ok == 1)
            }
        }
    }

    pub async fn lookup_identity(&self, token_hash: &str, now: &str) -> AppResult<RequestIdentity> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => lookup_sqlite_identity(pool, token_hash, now).await,
            DatabaseProvider::Postgres(pool) => {
                lookup_postgres_identity(pool, token_hash, now).await
            }
        }
    }

    pub async fn touch_auth_session(&self, session_id: &str, now: &str) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query("UPDATE auth_sessions SET last_used_at = ? WHERE id = ?")
                    .bind(now)
                    .bind(session_id)
                    .execute(pool)
                    .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query("UPDATE auth_sessions SET last_used_at = $1 WHERE id = $2")
                    .bind(now)
                    .bind(session_id)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn list_auth_sessions(
        &self,
        user_id: &str,
        current_session_id: &str,
        limit: Option<usize>,
    ) -> AppResult<Vec<AuthSession>> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                let mut query = String::from(
                    r#"
                    SELECT id, label, scopes_json, created_at, expires_at, revoked_at, last_used_at
                    FROM auth_sessions
                    WHERE user_id = ?
                    ORDER BY
                        CASE WHEN id = ? THEN 0 ELSE 1 END,
                        created_at DESC
                    "#,
                );
                if let Some(limit) = limit {
                    query.push_str(&format!(" LIMIT {}", limit.max(1)));
                }
                let rows = sqlx::query(&query)
                    .bind(user_id)
                    .bind(current_session_id)
                    .fetch_all(pool)
                    .await?;
                rows.into_iter()
                    .map(|row| sqlite_auth_session_from_row(row, current_session_id))
                    .collect()
            }
            DatabaseProvider::Postgres(pool) => {
                let mut query = String::from(
                    r#"
                    SELECT id, label, scopes_json, created_at, expires_at, revoked_at, last_used_at
                    FROM auth_sessions
                    WHERE user_id = $1
                    ORDER BY
                        CASE WHEN id = $2 THEN 0 ELSE 1 END,
                        created_at DESC
                    "#,
                );
                if let Some(limit) = limit {
                    query.push_str(&format!(" LIMIT {}", limit.max(1)));
                }
                let rows = sqlx::query(&query)
                    .bind(user_id)
                    .bind(current_session_id)
                    .fetch_all(pool)
                    .await?;
                rows.into_iter()
                    .map(|row| postgres_auth_session_from_row(row, current_session_id))
                    .collect()
            }
        }
    }

    pub async fn create_auth_session(&self, session: NewAuthSession<'_>) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO auth_sessions (
                        id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, NULL, NULL)
                    "#,
                )
                .bind(session.id)
                .bind(session.user_id)
                .bind(session.label)
                .bind(session.token_hash)
                .bind(session.scopes_json)
                .bind(session.created_at)
                .bind(session.expires_at)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO auth_sessions (
                        id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, NULL)
                    "#,
                )
                .bind(session.id)
                .bind(session.user_id)
                .bind(session.label)
                .bind(session.token_hash)
                .bind(session.scopes_json)
                .bind(session.created_at)
                .bind(session.expires_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn ensure_user_exists(&self, user_id: &str) -> AppResult<()> {
        let exists = match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query("SELECT 1 FROM users WHERE id = ? LIMIT 1")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?
                    .is_some()
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query("SELECT 1 FROM users WHERE id = $1 LIMIT 1")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?
                    .is_some()
            }
        };

        if !exists {
            return Err(AppError::BadRequest(format!(
                "user `{user_id}` does not exist; run `provision-user` first"
            )));
        }

        Ok(())
    }

    pub async fn provision_user(&self, user: ProvisionedUser<'_>) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO users (id, handle, display_name, avatar, tier, joined_at)
                    VALUES (?, ?, ?, ?, ?, ?)
                    ON CONFLICT(id) DO UPDATE SET
                        handle = excluded.handle,
                        display_name = excluded.display_name,
                        avatar = excluded.avatar,
                        tier = excluded.tier
                    "#,
                )
                .bind(user.id)
                .bind(user.handle)
                .bind(user.display_name)
                .bind(user.avatar_url)
                .bind(user.tier)
                .bind(user.joined_at)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO users (id, handle, display_name, avatar, tier, joined_at)
                    VALUES ($1, $2, $3, $4, $5, $6)
                    ON CONFLICT(id) DO UPDATE SET
                        handle = excluded.handle,
                        display_name = excluded.display_name,
                        avatar = excluded.avatar,
                        tier = excluded.tier
                    "#,
                )
                .bind(user.id)
                .bind(user.handle)
                .bind(user.display_name)
                .bind(user.avatar_url)
                .bind(user.tier)
                .bind(user.joined_at)
                .execute(pool)
                .await?;
            }
        }

        Ok(())
    }

    pub async fn provision_creator(&self, creator: ProvisionedCreator<'_>) -> AppResult<()> {
        self.ensure_user_exists(creator.user_id).await?;
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO creator_profiles (
                        id, user_id, handle, display_name, avatar, banner, tagline, bio, partner_status,
                        joined_at, stream_key, rtmp_url, default_category, default_tags_json, followers,
                        subscribers, monthly_viewers, total_watch_hours, live_status, current_broadcast_id
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 0, 0, 0, 'offline', NULL)
                    ON CONFLICT(id) DO UPDATE SET
                        user_id = excluded.user_id,
                        handle = excluded.handle,
                        display_name = excluded.display_name,
                        avatar = excluded.avatar,
                        banner = excluded.banner,
                        tagline = excluded.tagline,
                        bio = excluded.bio,
                        partner_status = excluded.partner_status,
                        stream_key = excluded.stream_key,
                        rtmp_url = excluded.rtmp_url,
                        default_category = excluded.default_category,
                        default_tags_json = excluded.default_tags_json
                    "#,
                )
                .bind(creator.id)
                .bind(creator.user_id)
                .bind(creator.handle)
                .bind(creator.display_name)
                .bind(creator.avatar_url)
                .bind(creator.banner_url)
                .bind(creator.tagline)
                .bind(creator.bio)
                .bind(creator.partner_status)
                .bind(creator.joined_at)
                .bind(creator.stream_key)
                .bind(creator.rtmp_url)
                .bind(creator.default_category)
                .bind(creator.default_tags_json)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO creator_profiles (
                        id, user_id, handle, display_name, avatar, banner, tagline, bio, partner_status,
                        joined_at, stream_key, rtmp_url, default_category, default_tags_json, followers,
                        subscribers, monthly_viewers, total_watch_hours, live_status, current_broadcast_id
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, 0, 0, 0, 0, 'offline', NULL)
                    ON CONFLICT(id) DO UPDATE SET
                        user_id = excluded.user_id,
                        handle = excluded.handle,
                        display_name = excluded.display_name,
                        avatar = excluded.avatar,
                        banner = excluded.banner,
                        tagline = excluded.tagline,
                        bio = excluded.bio,
                        partner_status = excluded.partner_status,
                        stream_key = excluded.stream_key,
                        rtmp_url = excluded.rtmp_url,
                        default_category = excluded.default_category,
                        default_tags_json = excluded.default_tags_json
                    "#,
                )
                .bind(creator.id)
                .bind(creator.user_id)
                .bind(creator.handle)
                .bind(creator.display_name)
                .bind(creator.avatar_url)
                .bind(creator.banner_url)
                .bind(creator.tagline)
                .bind(creator.bio)
                .bind(creator.partner_status)
                .bind(creator.joined_at)
                .bind(creator.stream_key)
                .bind(creator.rtmp_url)
                .bind(creator.default_category)
                .bind(creator.default_tags_json)
                .execute(pool)
                .await?;
            }
        }

        Ok(())
    }

    pub async fn unique_user_handle(&self, base: &str) -> AppResult<String> {
        for suffix in 0..100 {
            let candidate = if suffix == 0 {
                base.to_string()
            } else {
                format!("{base}{suffix}")
            };
            let exists = match &self.provider {
                DatabaseProvider::Sqlite(pool) => {
                    sqlx::query("SELECT 1 FROM users WHERE handle = ? LIMIT 1")
                        .bind(&candidate)
                        .fetch_optional(pool)
                        .await?
                        .is_some()
                }
                DatabaseProvider::Postgres(pool) => {
                    sqlx::query("SELECT 1 FROM users WHERE handle = $1 LIMIT 1")
                        .bind(&candidate)
                        .fetch_optional(pool)
                        .await?
                        .is_some()
                }
            };
            if !exists {
                return Ok(candidate);
            }
        }
        Ok(format!(
            "creator{}",
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        ))
    }

    pub async fn fetch_user(&self, user_id: &str) -> AppResult<User> {
        let (id, handle, display_name, avatar, tier, joined_at) = match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                let row = sqlx::query(
                    "SELECT id, handle, display_name, avatar, tier, joined_at FROM users WHERE id = ?",
                )
                .bind(user_id)
                .fetch_optional(pool)
                .await?
                .ok_or(AppError::NotFound)?;
                (
                    row.get("id"),
                    row.get("handle"),
                    row.get("display_name"),
                    row.get("avatar"),
                    row.get("tier"),
                    row.get("joined_at"),
                )
            }
            DatabaseProvider::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, handle, display_name, avatar, tier, joined_at FROM users WHERE id = $1",
                )
                .bind(user_id)
                .fetch_optional(pool)
                .await?
                .ok_or(AppError::NotFound)?;
                (
                    row.get("id"),
                    row.get("handle"),
                    row.get("display_name"),
                    row.get("avatar"),
                    row.get("tier"),
                    row.get("joined_at"),
                )
            }
        };
        let watchlist = self
            .fetch_user_string_ids(
                "SELECT content_id FROM user_watchlist WHERE user_id = ? ORDER BY content_id ASC",
                "SELECT content_id FROM user_watchlist WHERE user_id = $1 ORDER BY content_id ASC",
                "content_id",
                user_id,
            )
            .await?;
        let following = self
            .fetch_user_string_ids(
                "SELECT streamer_id FROM user_following WHERE user_id = ? ORDER BY streamer_id",
                "SELECT streamer_id FROM user_following WHERE user_id = $1 ORDER BY streamer_id",
                "streamer_id",
                user_id,
            )
            .await?;
        let continue_watching = self.fetch_continue_watching_entries(user_id, None).await?;

        Ok(User {
            id,
            handle,
            display_name,
            avatar,
            tier,
            joined_at,
            watchlist,
            following,
            continue_watching,
        })
    }

    pub async fn fetch_email_credential(&self, email: &str) -> AppResult<Option<EmailCredential>> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                let row = sqlx::query(
                    "SELECT user_id, password_hash FROM auth_email_credentials WHERE email = ?",
                )
                .bind(email)
                .fetch_optional(pool)
                .await?;
                Ok(row.map(|row| EmailCredential {
                    user_id: row.get("user_id"),
                    password_hash: row.get("password_hash"),
                }))
            }
            DatabaseProvider::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT user_id, password_hash FROM auth_email_credentials WHERE email = $1",
                )
                .bind(email)
                .fetch_optional(pool)
                .await?;
                Ok(row.map(|row| EmailCredential {
                    user_id: row.get("user_id"),
                    password_hash: row.get("password_hash"),
                }))
            }
        }
    }

    pub async fn fetch_oauth_account(
        &self,
        provider: &str,
        provider_account_id: &str,
    ) -> AppResult<Option<OAuthAccount>> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                let row = sqlx::query(
                    "SELECT user_id FROM auth_oauth_accounts WHERE provider = ? AND provider_account_id = ?",
                )
                .bind(provider)
                .bind(provider_account_id)
                .fetch_optional(pool)
                .await?;
                Ok(row.map(|row| OAuthAccount {
                    user_id: row.get("user_id"),
                }))
            }
            DatabaseProvider::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT user_id FROM auth_oauth_accounts WHERE provider = $1 AND provider_account_id = $2",
                )
                .bind(provider)
                .bind(provider_account_id)
                .fetch_optional(pool)
                .await?;
                Ok(row.map(|row| OAuthAccount {
                    user_id: row.get("user_id"),
                }))
            }
        }
    }

    pub async fn upsert_oauth_account(
        &self,
        id: &str,
        user_id: &str,
        provider: &str,
        provider_account_id: &str,
        email: Option<&str>,
        display_name: Option<&str>,
        now: &str,
    ) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO auth_oauth_accounts (
                        id, user_id, provider, provider_account_id, email, display_name, created_at, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(provider, provider_account_id) DO UPDATE SET
                        user_id = excluded.user_id,
                        email = excluded.email,
                        display_name = excluded.display_name,
                        updated_at = excluded.updated_at
                    "#,
                )
                .bind(id)
                .bind(user_id)
                .bind(provider)
                .bind(provider_account_id)
                .bind(email)
                .bind(display_name)
                .bind(now)
                .bind(now)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO auth_oauth_accounts (
                        id, user_id, provider, provider_account_id, email, display_name, created_at, updated_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                    ON CONFLICT(provider, provider_account_id) DO UPDATE SET
                        user_id = excluded.user_id,
                        email = excluded.email,
                        display_name = excluded.display_name,
                        updated_at = excluded.updated_at
                    "#,
                )
                .bind(id)
                .bind(user_id)
                .bind(provider)
                .bind(provider_account_id)
                .bind(email)
                .bind(display_name)
                .bind(now)
                .bind(now)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    async fn fetch_user_string_ids(
        &self,
        sqlite_query: &str,
        postgres_query: &str,
        column: &str,
        user_id: &str,
    ) -> AppResult<Vec<String>> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => Ok(sqlx::query(sqlite_query)
                .bind(user_id)
                .fetch_all(pool)
                .await?
                .into_iter()
                .map(|row| row.get(column))
                .collect()),
            DatabaseProvider::Postgres(pool) => Ok(sqlx::query(postgres_query)
                .bind(user_id)
                .fetch_all(pool)
                .await?
                .into_iter()
                .map(|row| row.get(column))
                .collect()),
        }
    }

    async fn fetch_continue_watching_entries(
        &self,
        user_id: &str,
        limit: Option<usize>,
    ) -> AppResult<Vec<ContinueWatchingEntry>> {
        let mut sqlite_query = String::from(
            r#"
            SELECT content_id, kind, episode_id, progress_sec, duration_sec, last_watched_at
            FROM continue_watching
            WHERE user_id = ?
            ORDER BY last_watched_at DESC
            "#,
        );
        let mut postgres_query = String::from(
            r#"
            SELECT content_id, kind, episode_id,
                   progress_sec::BIGINT AS progress_sec,
                   duration_sec::BIGINT AS duration_sec,
                   last_watched_at
            FROM continue_watching
            WHERE user_id = $1
            ORDER BY last_watched_at DESC
            "#,
        );
        if let Some(limit) = limit {
            let limit = limit.max(1);
            sqlite_query.push_str(&format!(" LIMIT {limit}"));
            postgres_query.push_str(&format!(" LIMIT {limit}"));
        }
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => Ok(sqlx::query(&sqlite_query)
                .bind(user_id)
                .fetch_all(pool)
                .await?
                .into_iter()
                .map(|row| ContinueWatchingEntry {
                    content_id: row.get("content_id"),
                    kind: row.get("kind"),
                    episode_id: row.get("episode_id"),
                    progress_sec: row.get("progress_sec"),
                    duration_sec: row.get("duration_sec"),
                    last_watched_at: row.get("last_watched_at"),
                })
                .collect()),
            DatabaseProvider::Postgres(pool) => Ok(sqlx::query(&postgres_query)
                .bind(user_id)
                .fetch_all(pool)
                .await?
                .into_iter()
                .map(|row| ContinueWatchingEntry {
                    content_id: row.get("content_id"),
                    kind: row.get("kind"),
                    episode_id: row.get("episode_id"),
                    progress_sec: row.get("progress_sec"),
                    duration_sec: row.get("duration_sec"),
                    last_watched_at: row.get("last_watched_at"),
                })
                .collect()),
        }
    }

    pub async fn create_email_credential(
        &self,
        user_id: &str,
        email: &str,
        password_hash: &str,
        now: &str,
    ) -> AppResult<()> {
        if self.fetch_email_credential(email).await?.is_some() {
            return Err(AppError::BadRequest(
                "Email is already registered.".to_string(),
            ));
        }
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    "INSERT INTO auth_email_credentials (user_id, email, password_hash, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
                )
                .bind(user_id)
                .bind(email)
                .bind(password_hash)
                .bind(now)
                .bind(now)
                .execute(pool)
                .await?;
                sqlx::query(
                    r#"
                    INSERT INTO user_profiles (
                        user_id, email, email_verified, mature_content_allowed, default_audio,
                        subtitle_preset, autoplay_trailers, live_chat_filter, hours_watched
                    ) VALUES (?, ?, 0, 0, 'English', 'English · Standard', 1, 'Standard', 0)
                    ON CONFLICT(user_id) DO UPDATE SET email = excluded.email, email_verified = 0
                    "#,
                )
                .bind(user_id)
                .bind(email)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO auth_email_credentials (user_id, email, password_hash, created_at, updated_at) VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(user_id)
                .bind(email)
                .bind(password_hash)
                .bind(now)
                .bind(now)
                .execute(pool)
                .await?;
                sqlx::query(
                    r#"
                    INSERT INTO user_profiles (
                        user_id, email, email_verified, mature_content_allowed, default_audio,
                        subtitle_preset, autoplay_trailers, live_chat_filter, hours_watched
                    ) VALUES ($1, $2, 0, 0, 'English', 'English · Standard', 1, 'Standard', 0)
                    ON CONFLICT(user_id) DO UPDATE SET email = excluded.email, email_verified = 0
                    "#,
                )
                .bind(user_id)
                .bind(email)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn provision_creator_defaults(
        &self,
        creator_id: &str,
        display_name: &str,
        support_email: &str,
        now: &str,
        scenes_json: &str,
    ) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO creator_operational_state (
                        creator_id, legal_name, support_email, business_type, payout_country,
                        payout_provider, onboarding_status, identity_status, tax_status,
                        payout_status, hold_reasons_json, created_at, updated_at, last_reviewed_at
                    ) VALUES (?, ?, ?, 'individual', 'US', 'vanta', 'approved',
                              'verified', 'verified', 'active', '[]', ?, ?, ?)
                    ON CONFLICT(creator_id) DO UPDATE SET
                        legal_name = excluded.legal_name,
                        support_email = excluded.support_email,
                        onboarding_status = excluded.onboarding_status,
                        identity_status = excluded.identity_status,
                        tax_status = excluded.tax_status,
                        payout_status = excluded.payout_status,
                        hold_reasons_json = excluded.hold_reasons_json,
                        updated_at = excluded.updated_at,
                        last_reviewed_at = excluded.last_reviewed_at
                    "#,
                )
                .bind(creator_id)
                .bind(display_name)
                .bind(support_email)
                .bind(now)
                .bind(now)
                .bind(now)
                .execute(pool)
                .await?;
                sqlx::query(
                    r#"
                    INSERT INTO creator_live_settings (
                        creator_id, subscriber_only, slow_mode_seconds, auto_mod_level, notify_followers_default,
                        delivery_class, active_scene_id, scenes_json, bitrate_kbps, cpu_percent, dropped_frames, free_disk_gb
                    ) VALUES (?, 1, 3, 'standard', 1, 'standard_hls', 'cam-main', ?, 0, 0, 0, 0)
                    ON CONFLICT(creator_id) DO NOTHING
                    "#,
                )
                .bind(creator_id)
                .bind(scenes_json)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO creator_operational_state (
                        creator_id, legal_name, support_email, business_type, payout_country,
                        payout_provider, onboarding_status, identity_status, tax_status,
                        payout_status, hold_reasons_json, created_at, updated_at, last_reviewed_at
                    ) VALUES ($1, $2, $3, 'individual', 'US', 'vanta', 'approved',
                              'verified', 'verified', 'active', '[]', $4, $5, $6)
                    ON CONFLICT(creator_id) DO UPDATE SET
                        legal_name = excluded.legal_name,
                        support_email = excluded.support_email,
                        onboarding_status = excluded.onboarding_status,
                        identity_status = excluded.identity_status,
                        tax_status = excluded.tax_status,
                        payout_status = excluded.payout_status,
                        hold_reasons_json = excluded.hold_reasons_json,
                        updated_at = excluded.updated_at,
                        last_reviewed_at = excluded.last_reviewed_at
                    "#,
                )
                .bind(creator_id)
                .bind(display_name)
                .bind(support_email)
                .bind(now)
                .bind(now)
                .bind(now)
                .execute(pool)
                .await?;
                sqlx::query(
                    r#"
                    INSERT INTO creator_live_settings (
                        creator_id, subscriber_only, slow_mode_seconds, auto_mod_level, notify_followers_default,
                        delivery_class, active_scene_id, scenes_json, bitrate_kbps, cpu_percent, dropped_frames, free_disk_gb
                    ) VALUES ($1, 1, 3, 'standard', 1, 'standard_hls', 'cam-main', $2, 0, 0, 0, 0)
                    ON CONFLICT(creator_id) DO NOTHING
                    "#,
                )
                .bind(creator_id)
                .bind(scenes_json)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn fetch_collaboration_launch_relative_path(
        &self,
        session_id: &str,
    ) -> AppResult<String> {
        let (creator_id, broadcast_id): (String, String) = match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                let row = sqlx::query(
                    r#"
                    SELECT creator_id, broadcast_id
                    FROM live_ingest_sessions
                    WHERE id = ?
                    "#,
                )
                .bind(session_id)
                .fetch_optional(pool)
                .await?
                .ok_or(AppError::NotFound)?;
                (row.get("creator_id"), row.get("broadcast_id"))
            }
            DatabaseProvider::Postgres(pool) => {
                let row = sqlx::query(
                    r#"
                    SELECT creator_id, broadcast_id
                    FROM live_ingest_sessions
                    WHERE id = $1
                    "#,
                )
                .bind(session_id)
                .fetch_optional(pool)
                .await?
                .ok_or(AppError::NotFound)?;
                (row.get("creator_id"), row.get("broadcast_id"))
            }
        };

        Ok(format!(
            "runtime/{creator_id}/{broadcast_id}/{session_id}/collaboration/launch.json"
        ))
    }

    pub async fn revoke_auth_session(
        &self,
        session_id: &str,
        user_id: &str,
        revoked_at: &str,
    ) -> AppResult<u64> {
        let rows_affected = match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE auth_sessions SET revoked_at = ? WHERE id = ? AND user_id = ? AND revoked_at IS NULL",
                )
                .bind(revoked_at)
                .bind(session_id)
                .bind(user_id)
                .execute(pool)
                .await?
                .rows_affected()
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    "UPDATE auth_sessions SET revoked_at = $1 WHERE id = $2 AND user_id = $3 AND revoked_at IS NULL",
                )
                .bind(revoked_at)
                .bind(session_id)
                .bind(user_id)
                .execute(pool)
                .await?
                .rows_affected()
            }
        };
        Ok(rows_affected)
    }

    pub async fn expire_playback_sessions_for_auth_session(
        &self,
        auth_session_id: &str,
        expired_at: &str,
    ) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    r#"
                    UPDATE playback_sessions
                    SET expires_at = ?, last_used_at = ?
                    WHERE auth_session_id = ? AND expires_at > ?
                    "#,
                )
                .bind(expired_at)
                .bind(expired_at)
                .bind(auth_session_id)
                .bind(expired_at)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    r#"
                    UPDATE playback_sessions
                    SET expires_at = $1, last_used_at = $1
                    WHERE auth_session_id = $2 AND expires_at > $1
                    "#,
                )
                .bind(expired_at)
                .bind(auth_session_id)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn search_catalog_documents(
        &self,
        query: &str,
        limit: i64,
    ) -> AppResult<Vec<CatalogSearchHit>> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                let Some(fts_query) = build_sqlite_fts_query(query) else {
                    return Ok(Vec::new());
                };
                let rows = sqlx::query(
                    r#"
                    SELECT entity_id, kind
                    FROM search_documents
                    WHERE search_documents MATCH ?
                    ORDER BY bm25(search_documents, 1.0, 0.3)
                    LIMIT ?
                    "#,
                )
                .bind(&fts_query)
                .bind(limit.max(1))
                .fetch_all(pool)
                .await?;
                Ok(rows
                    .into_iter()
                    .map(|row| CatalogSearchHit {
                        entity_id: row.get("entity_id"),
                        kind: row.get("kind"),
                    })
                    .collect())
            }
            DatabaseProvider::Postgres(pool) => {
                let tokens = search_tokens(query);
                if tokens.is_empty() {
                    return Ok(Vec::new());
                }
                let pattern = format!("%{}%", tokens.join("%"));
                let rows = sqlx::query(
                    r#"
                    SELECT entity_id, kind
                    FROM search_documents
                    WHERE title ILIKE $1
                       OR body ILIKE $1
                       OR slug ILIKE $1
                    ORDER BY
                        CASE WHEN title ILIKE $1 THEN 0 ELSE 1 END,
                        title ASC
                    LIMIT $2
                    "#,
                )
                .bind(&pattern)
                .bind(limit.max(1))
                .fetch_all(pool)
                .await?;
                Ok(rows
                    .into_iter()
                    .map(|row| CatalogSearchHit {
                        entity_id: row.get("entity_id"),
                        kind: row.get("kind"),
                    })
                    .collect())
            }
        }
    }

    pub async fn update_user_profile(
        &self,
        user_id: &str,
        input: &UpdateProfileRequest,
    ) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE users SET display_name = COALESCE(?, display_name) WHERE id = ?",
                )
                .bind(input.display_name.as_deref())
                .bind(user_id)
                .execute(pool)
                .await?;
                sqlx::query(
                    r#"
                    UPDATE user_profiles
                    SET email = COALESCE(?, email),
                        mature_content_allowed = COALESCE(?, mature_content_allowed),
                        default_audio = COALESCE(?, default_audio),
                        subtitle_preset = COALESCE(?, subtitle_preset),
                        autoplay_trailers = COALESCE(?, autoplay_trailers),
                        live_chat_filter = COALESCE(?, live_chat_filter)
                    WHERE user_id = ?
                    "#,
                )
                .bind(input.email.as_deref())
                .bind(input.mature_content_allowed.map(bool_to_postgres_int))
                .bind(input.default_audio.as_deref())
                .bind(input.subtitle_preset.as_deref())
                .bind(input.autoplay_trailers.map(bool_to_postgres_int))
                .bind(input.live_chat_filter.as_deref())
                .bind(user_id)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    "UPDATE users SET display_name = COALESCE($1, display_name) WHERE id = $2",
                )
                .bind(input.display_name.as_deref())
                .bind(user_id)
                .execute(pool)
                .await?;
                sqlx::query(
                    r#"
                    UPDATE user_profiles
                    SET email = COALESCE($1, email),
                        mature_content_allowed = COALESCE($2, mature_content_allowed),
                        default_audio = COALESCE($3, default_audio),
                        subtitle_preset = COALESCE($4, subtitle_preset),
                        autoplay_trailers = COALESCE($5, autoplay_trailers),
                        live_chat_filter = COALESCE($6, live_chat_filter)
                    WHERE user_id = $7
                    "#,
                )
                .bind(input.email.as_deref())
                .bind(input.mature_content_allowed.map(bool_to_sqlite_int))
                .bind(input.default_audio.as_deref())
                .bind(input.subtitle_preset.as_deref())
                .bind(input.autoplay_trailers.map(bool_to_sqlite_int))
                .bind(input.live_chat_filter.as_deref())
                .bind(user_id)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn update_user_settings(
        &self,
        user_id: &str,
        input: &UpdateSettingsRequest,
    ) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                if let Some(playback) = &input.playback {
                    sqlx::query(
                        r#"
                        UPDATE user_playback_settings
                        SET default_quality = ?, audio_language = ?, subtitle_language = ?,
                            subtitle_style = ?, autoplay_next_episode = ?, autoplay_trailers = ?,
                            reduced_motion = ?, prefer_dubbed = ?, playback_speed = ?
                        WHERE user_id = ?
                        "#,
                    )
                    .bind(&playback.default_quality)
                    .bind(&playback.audio_language)
                    .bind(&playback.subtitle_language)
                    .bind(&playback.subtitle_style)
                    .bind(bool_to_sqlite_int(playback.autoplay_next_episode))
                    .bind(bool_to_sqlite_int(playback.autoplay_trailers))
                    .bind(bool_to_sqlite_int(playback.reduced_motion))
                    .bind(bool_to_sqlite_int(playback.prefer_dubbed))
                    .bind(&playback.playback_speed)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                }

                if let Some(notifications) = &input.notifications {
                    sqlx::query(
                        r#"
                        UPDATE user_notification_settings
                        SET series_push = ?, series_email = ?, live_push = ?, live_email = ?,
                            originals_push = ?, originals_email = ?, watchlist_push = ?,
                            watchlist_email = ?, creator_push = ?, creator_email = ?,
                            security_push = ?, security_email = ?
                        WHERE user_id = ?
                        "#,
                    )
                    .bind(bool_to_sqlite_int(notifications.series_releases.push))
                    .bind(bool_to_sqlite_int(notifications.series_releases.email))
                    .bind(bool_to_sqlite_int(notifications.live_streams.push))
                    .bind(bool_to_sqlite_int(notifications.live_streams.email))
                    .bind(bool_to_sqlite_int(notifications.originals.push))
                    .bind(bool_to_sqlite_int(notifications.originals.email))
                    .bind(bool_to_sqlite_int(notifications.watchlist_updates.push))
                    .bind(bool_to_sqlite_int(notifications.watchlist_updates.email))
                    .bind(bool_to_sqlite_int(notifications.creator_updates.push))
                    .bind(bool_to_sqlite_int(notifications.creator_updates.email))
                    .bind(bool_to_sqlite_int(notifications.security_alerts.push))
                    .bind(bool_to_sqlite_int(notifications.security_alerts.email))
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                }

                if let Some(privacy) = &input.privacy {
                    sqlx::query(
                        r#"
                        UPDATE user_privacy_settings
                        SET show_friend_activity = ?, improve_recommendations = ?,
                            personalized_ads = ?, ab_tests = ?, data_export_size_mb = ?,
                            delete_cooldown_days = ?
                        WHERE user_id = ?
                        "#,
                    )
                    .bind(bool_to_sqlite_int(privacy.show_friend_activity))
                    .bind(bool_to_sqlite_int(privacy.improve_recommendations))
                    .bind(bool_to_sqlite_int(privacy.personalized_ads))
                    .bind(bool_to_sqlite_int(privacy.ab_tests))
                    .bind(privacy.data_export_size_mb)
                    .bind(privacy.delete_cooldown_days)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                }

                if let Some(parental) = &input.parental {
                    sqlx::query(
                        r#"
                        UPDATE user_parental_controls
                        SET max_rating = ?, require_pin_for_mature = ?, hide_live_chat_for_kids = ?,
                            block_mature_live_streams = ?, pin_set = ?
                        WHERE user_id = ?
                        "#,
                    )
                    .bind(&parental.max_rating)
                    .bind(bool_to_sqlite_int(parental.require_pin_for_mature))
                    .bind(bool_to_sqlite_int(parental.hide_live_chat_for_kids))
                    .bind(bool_to_sqlite_int(parental.block_mature_live_streams))
                    .bind(bool_to_sqlite_int(parental.pin_set))
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                }

                if let Some(downloads) = &input.downloads {
                    sqlx::query(
                        r#"
                        UPDATE user_download_settings
                        SET video_quality = ?, wifi_only = ?, smart_downloads = ?,
                            storage_used_gb = ?, storage_limit_gb = ?, device_limit = ?,
                            active_devices = ?
                        WHERE user_id = ?
                        "#,
                    )
                    .bind(&downloads.video_quality)
                    .bind(bool_to_sqlite_int(downloads.wifi_only))
                    .bind(bool_to_sqlite_int(downloads.smart_downloads))
                    .bind(downloads.storage_used_gb)
                    .bind(downloads.storage_limit_gb)
                    .bind(downloads.device_limit)
                    .bind(downloads.active_devices)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                }

                if let Some(language) = &input.language {
                    sqlx::query(
                        r#"
                        UPDATE user_language_settings
                        SET interface_language = ?, subtitle_language = ?, catalog_region = ?,
                            date_format = ?, clock_format = ?
                        WHERE user_id = ?
                        "#,
                    )
                    .bind(&language.interface_language)
                    .bind(&language.subtitle_language)
                    .bind(&language.catalog_region)
                    .bind(&language.date_format)
                    .bind(&language.clock_format)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                }
            }
            DatabaseProvider::Postgres(pool) => {
                if let Some(playback) = &input.playback {
                    sqlx::query(
                        r#"
                        UPDATE user_playback_settings
                        SET default_quality = $1, audio_language = $2, subtitle_language = $3,
                            subtitle_style = $4, autoplay_next_episode = $5, autoplay_trailers = $6,
                            reduced_motion = $7, prefer_dubbed = $8, playback_speed = $9
                        WHERE user_id = $10
                        "#,
                    )
                    .bind(&playback.default_quality)
                    .bind(&playback.audio_language)
                    .bind(&playback.subtitle_language)
                    .bind(&playback.subtitle_style)
                    .bind(bool_to_postgres_int(playback.autoplay_next_episode))
                    .bind(bool_to_postgres_int(playback.autoplay_trailers))
                    .bind(bool_to_postgres_int(playback.reduced_motion))
                    .bind(bool_to_postgres_int(playback.prefer_dubbed))
                    .bind(&playback.playback_speed)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                }

                if let Some(notifications) = &input.notifications {
                    sqlx::query(
                        r#"
                        UPDATE user_notification_settings
                        SET series_push = $1, series_email = $2, live_push = $3, live_email = $4,
                            originals_push = $5, originals_email = $6, watchlist_push = $7,
                            watchlist_email = $8, creator_push = $9, creator_email = $10,
                            security_push = $11, security_email = $12
                        WHERE user_id = $13
                        "#,
                    )
                    .bind(bool_to_postgres_int(notifications.series_releases.push))
                    .bind(bool_to_postgres_int(notifications.series_releases.email))
                    .bind(bool_to_postgres_int(notifications.live_streams.push))
                    .bind(bool_to_postgres_int(notifications.live_streams.email))
                    .bind(bool_to_postgres_int(notifications.originals.push))
                    .bind(bool_to_postgres_int(notifications.originals.email))
                    .bind(bool_to_postgres_int(notifications.watchlist_updates.push))
                    .bind(bool_to_postgres_int(notifications.watchlist_updates.email))
                    .bind(bool_to_postgres_int(notifications.creator_updates.push))
                    .bind(bool_to_postgres_int(notifications.creator_updates.email))
                    .bind(bool_to_postgres_int(notifications.security_alerts.push))
                    .bind(bool_to_postgres_int(notifications.security_alerts.email))
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                }

                if let Some(privacy) = &input.privacy {
                    sqlx::query(
                        r#"
                        UPDATE user_privacy_settings
                        SET show_friend_activity = $1, improve_recommendations = $2,
                            personalized_ads = $3, ab_tests = $4, data_export_size_mb = $5,
                            delete_cooldown_days = $6
                        WHERE user_id = $7
                        "#,
                    )
                    .bind(bool_to_postgres_int(privacy.show_friend_activity))
                    .bind(bool_to_postgres_int(privacy.improve_recommendations))
                    .bind(bool_to_postgres_int(privacy.personalized_ads))
                    .bind(bool_to_postgres_int(privacy.ab_tests))
                    .bind(privacy.data_export_size_mb)
                    .bind(privacy.delete_cooldown_days)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                }

                if let Some(parental) = &input.parental {
                    sqlx::query(
                        r#"
                        UPDATE user_parental_controls
                        SET max_rating = $1, require_pin_for_mature = $2,
                            hide_live_chat_for_kids = $3, block_mature_live_streams = $4,
                            pin_set = $5
                        WHERE user_id = $6
                        "#,
                    )
                    .bind(&parental.max_rating)
                    .bind(bool_to_postgres_int(parental.require_pin_for_mature))
                    .bind(bool_to_postgres_int(parental.hide_live_chat_for_kids))
                    .bind(bool_to_postgres_int(parental.block_mature_live_streams))
                    .bind(bool_to_postgres_int(parental.pin_set))
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                }

                if let Some(downloads) = &input.downloads {
                    sqlx::query(
                        r#"
                        UPDATE user_download_settings
                        SET video_quality = $1, wifi_only = $2, smart_downloads = $3,
                            storage_used_gb = $4, storage_limit_gb = $5, device_limit = $6,
                            active_devices = $7
                        WHERE user_id = $8
                        "#,
                    )
                    .bind(&downloads.video_quality)
                    .bind(bool_to_postgres_int(downloads.wifi_only))
                    .bind(bool_to_postgres_int(downloads.smart_downloads))
                    .bind(downloads.storage_used_gb)
                    .bind(downloads.storage_limit_gb)
                    .bind(downloads.device_limit)
                    .bind(downloads.active_devices)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                }

                if let Some(language) = &input.language {
                    sqlx::query(
                        r#"
                        UPDATE user_language_settings
                        SET interface_language = $1, subtitle_language = $2, catalog_region = $3,
                            date_format = $4, clock_format = $5
                        WHERE user_id = $6
                        "#,
                    )
                    .bind(&language.interface_language)
                    .bind(&language.subtitle_language)
                    .bind(&language.catalog_region)
                    .bind(&language.date_format)
                    .bind(&language.clock_format)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                }
            }
        }
        Ok(())
    }

    pub async fn add_watchlist_item(&self, user_id: &str, content_id: &str) -> AppResult<()> {
        self.validate_watchlist_content(content_id).await?;
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    "INSERT OR IGNORE INTO user_watchlist (user_id, content_id) VALUES (?, ?)",
                )
                .bind(user_id)
                .bind(content_id)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO user_watchlist (user_id, content_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                )
                .bind(user_id)
                .bind(content_id)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn remove_watchlist_item(&self, user_id: &str, content_id: &str) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query("DELETE FROM user_watchlist WHERE user_id = ? AND content_id = ?")
                    .bind(user_id)
                    .bind(content_id)
                    .execute(pool)
                    .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query("DELETE FROM user_watchlist WHERE user_id = $1 AND content_id = $2")
                    .bind(user_id)
                    .bind(content_id)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn add_following(&self, user_id: &str, streamer_id: &str) -> AppResult<()> {
        self.ensure_streamer_exists(streamer_id).await?;
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    "INSERT OR IGNORE INTO user_following (user_id, streamer_id) VALUES (?, ?)",
                )
                .bind(user_id)
                .bind(streamer_id)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO user_following (user_id, streamer_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                )
                .bind(user_id)
                .bind(streamer_id)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn remove_following(&self, user_id: &str, streamer_id: &str) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query("DELETE FROM user_following WHERE user_id = ? AND streamer_id = ?")
                    .bind(user_id)
                    .bind(streamer_id)
                    .execute(pool)
                    .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query("DELETE FROM user_following WHERE user_id = $1 AND streamer_id = $2")
                    .bind(user_id)
                    .bind(streamer_id)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn record_progress(
        &self,
        user_id: &str,
        input: &ProgressInput,
        watched_at: &str,
    ) -> AppResult<()> {
        if input.progress_sec < 0 {
            return Err(AppError::BadRequest("progressSec must be >= 0".to_string()));
        }
        let target = self.resolve_progress_target(input).await?;
        let normalized_progress_sec = input.progress_sec.min(target.duration_sec);
        let completed = normalized_progress_sec >= target.duration_sec;

        if completed {
            self.remove_progress(user_id, &input.content_id).await?;
            self.upsert_watch_history_entry(
                user_id,
                &input.content_id,
                &target.kind,
                target.episode_id.as_deref(),
                target.duration_sec,
                target.duration_sec,
                true,
                watched_at,
            )
            .await?;
            return Ok(());
        }

        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO continue_watching (
                        user_id, content_id, kind, episode_id, progress_sec, duration_sec, last_watched_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(user_id, content_id) DO UPDATE SET
                        kind = excluded.kind,
                        episode_id = excluded.episode_id,
                        progress_sec = excluded.progress_sec,
                        duration_sec = excluded.duration_sec,
                        last_watched_at = excluded.last_watched_at
                    "#,
                )
                .bind(user_id)
                .bind(&input.content_id)
                .bind(&target.kind)
                .bind(&target.episode_id)
                .bind(i64_to_postgres_int(normalized_progress_sec)?)
                .bind(i64_to_postgres_int(target.duration_sec)?)
                .bind(watched_at)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO continue_watching (
                        user_id, content_id, kind, episode_id, progress_sec, duration_sec, last_watched_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                    ON CONFLICT(user_id, content_id) DO UPDATE SET
                        kind = excluded.kind,
                        episode_id = excluded.episode_id,
                        progress_sec = excluded.progress_sec,
                        duration_sec = excluded.duration_sec,
                        last_watched_at = excluded.last_watched_at
                    "#,
                )
                .bind(user_id)
                .bind(&input.content_id)
                .bind(&target.kind)
                .bind(&target.episode_id)
                .bind(normalized_progress_sec)
                .bind(target.duration_sec)
                .bind(watched_at)
                .execute(pool)
                .await?;
            }
        }
        self.upsert_watch_history_entry(
            user_id,
            &input.content_id,
            &target.kind,
            target.episode_id.as_deref(),
            normalized_progress_sec,
            target.duration_sec,
            false,
            watched_at,
        )
        .await
    }

    pub async fn remove_progress(&self, user_id: &str, content_id: &str) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query("DELETE FROM continue_watching WHERE user_id = ? AND content_id = ?")
                    .bind(user_id)
                    .bind(content_id)
                    .execute(pool)
                    .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query("DELETE FROM continue_watching WHERE user_id = $1 AND content_id = $2")
                    .bind(user_id)
                    .bind(content_id)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn remove_history_entry(&self, user_id: &str, content_id: &str) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query("DELETE FROM user_watch_history WHERE user_id = ? AND content_id = ?")
                    .bind(user_id)
                    .bind(content_id)
                    .execute(pool)
                    .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    "DELETE FROM user_watch_history WHERE user_id = $1 AND content_id = $2",
                )
                .bind(user_id)
                .bind(content_id)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn mark_user_notification_read(
        &self,
        user_id: &str,
        notification_id: &str,
        read_at: &str,
    ) -> AppResult<u64> {
        let rows_affected = match &self.provider {
            DatabaseProvider::Sqlite(pool) => sqlx::query(
                r#"
                    UPDATE notification_deliveries
                    SET read_at = COALESCE(read_at, ?)
                    WHERE id = ? AND recipient_user_id = ?
                    "#,
            )
            .bind(read_at)
            .bind(notification_id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected(),
            DatabaseProvider::Postgres(pool) => sqlx::query(
                r#"
                    UPDATE notification_deliveries
                    SET read_at = COALESCE(read_at, $1)
                    WHERE id = $2 AND recipient_user_id = $3
                    "#,
            )
            .bind(read_at)
            .bind(notification_id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected(),
        };
        Ok(rows_affected)
    }

    pub async fn find_reusable_live_playback_session(
        &self,
        lookup: ReusableLivePlaybackSessionLookup<'_>,
    ) -> AppResult<Option<PlaybackSession>> {
        let (Some(auth_session_id), Some(device_id)) = (lookup.auth_session_id, lookup.device_id)
        else {
            return self
                .find_reusable_live_playback_session_without_full_identity(lookup)
                .await;
        };

        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                let row = sqlx::query(
                    r#"
                    SELECT id, content_id, content_kind, access_scope, created_at, expires_at, last_used_at
                    FROM playback_sessions
                    WHERE content_id = ?
                      AND content_kind = 'live'
                      AND access_scope = 'live'
                      AND auth_session_id = ?
                      AND device_id = ?
                      AND expires_at > ?
                    ORDER BY last_used_at DESC
                    LIMIT 1
                    "#,
                )
                .bind(lookup.stream_id)
                .bind(auth_session_id)
                .bind(device_id)
                .bind(lookup.now)
                .fetch_optional(pool)
                .await?;
                Ok(row.map(sqlite_playback_session_from_row))
            }
            DatabaseProvider::Postgres(pool) => {
                let row = sqlx::query(
                    r#"
                    SELECT id, content_id, content_kind, access_scope, created_at, expires_at, last_used_at
                    FROM playback_sessions
                    WHERE content_id = $1
                      AND content_kind = 'live'
                      AND access_scope = 'live'
                      AND auth_session_id = $2
                      AND device_id = $3
                      AND expires_at > $4
                    ORDER BY last_used_at DESC
                    LIMIT 1
                    "#,
                )
                .bind(lookup.stream_id)
                .bind(auth_session_id)
                .bind(device_id)
                .fetch_optional(pool)
                .await?;
                Ok(row.map(postgres_playback_session_from_row))
            }
        }
    }

    async fn find_reusable_live_playback_session_without_full_identity(
        &self,
        lookup: ReusableLivePlaybackSessionLookup<'_>,
    ) -> AppResult<Option<PlaybackSession>> {
        match (lookup.auth_session_id, lookup.device_id) {
            (Some(auth_session_id), None) => match &self.provider {
                DatabaseProvider::Sqlite(pool) => {
                    let row = sqlx::query(
                        r#"
                        SELECT id, content_id, content_kind, access_scope, created_at, expires_at, last_used_at
                        FROM playback_sessions
                        WHERE content_id = ?
                          AND content_kind = 'live'
                          AND access_scope = 'live'
                          AND auth_session_id = ?
                          AND expires_at > ?
                        ORDER BY last_used_at DESC
                        LIMIT 1
                        "#,
                    )
                    .bind(lookup.stream_id)
                    .bind(auth_session_id)
                    .bind(lookup.now)
                    .fetch_optional(pool)
                    .await?;
                    Ok(row.map(sqlite_playback_session_from_row))
                }
                DatabaseProvider::Postgres(pool) => {
                    let row = sqlx::query(
                        r#"
                        SELECT id, content_id, content_kind, access_scope, created_at, expires_at, last_used_at
                        FROM playback_sessions
                        WHERE content_id = $1
                          AND content_kind = 'live'
                          AND access_scope = 'live'
                          AND auth_session_id = $2
                          AND expires_at > $3
                        ORDER BY last_used_at DESC
                        LIMIT 1
                        "#,
                    )
                    .bind(lookup.stream_id)
                    .bind(auth_session_id)
                    .bind(lookup.now)
                    .fetch_optional(pool)
                    .await?;
                    Ok(row.map(postgres_playback_session_from_row))
                }
            },
            (None, Some(device_id)) => match &self.provider {
                DatabaseProvider::Sqlite(pool) => {
                    let row = sqlx::query(
                        r#"
                        SELECT id, content_id, content_kind, access_scope, created_at, expires_at, last_used_at
                        FROM playback_sessions
                        WHERE content_id = ?
                          AND content_kind = 'live'
                          AND access_scope = 'live'
                          AND auth_session_id IS NULL
                          AND user_id IS NULL
                          AND device_id = ?
                          AND expires_at > ?
                        ORDER BY last_used_at DESC
                        LIMIT 1
                        "#,
                    )
                    .bind(lookup.stream_id)
                    .bind(device_id)
                    .bind(lookup.now)
                    .fetch_optional(pool)
                    .await?;
                    Ok(row.map(sqlite_playback_session_from_row))
                }
                DatabaseProvider::Postgres(pool) => {
                    let row = sqlx::query(
                        r#"
                        SELECT id, content_id, content_kind, access_scope, created_at, expires_at, last_used_at
                        FROM playback_sessions
                        WHERE content_id = $1
                          AND content_kind = 'live'
                          AND access_scope = 'live'
                          AND auth_session_id IS NULL
                          AND user_id IS NULL
                          AND device_id = $2
                          AND expires_at > $3
                        ORDER BY last_used_at DESC
                        LIMIT 1
                        "#,
                    )
                    .bind(lookup.stream_id)
                    .bind(device_id)
                    .bind(lookup.now)
                    .fetch_optional(pool)
                    .await?;
                    Ok(row.map(postgres_playback_session_from_row))
                }
            },
            (Some(_), Some(_)) => {
                unreachable!("full reusable live playback lookup is handled before this helper")
            }
            (None, None) => Ok(None),
        }
    }

    pub async fn create_playback_session(&self, session: NewPlaybackSession<'_>) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO playback_sessions (
                        id, auth_session_id, user_id, creator_id, asset_id, content_id, content_kind, token_hash,
                        access_scope, created_at, expires_at, last_used_at,
                        device_id, device_name, player_version, capabilities_json
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind(session.id)
                .bind(session.auth_session_id)
                .bind(session.user_id)
                .bind(session.creator_id)
                .bind(session.asset_id)
                .bind(session.content_id)
                .bind(session.content_kind)
                .bind(hash_token(session.playback_token))
                .bind(session.access_scope)
                .bind(session.created_at)
                .bind(session.expires_at)
                .bind(session.created_at)
                .bind(session.device_id)
                .bind(session.device_name)
                .bind(session.player_version)
                .bind(session.capabilities_json)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO playback_sessions (
                        id, auth_session_id, user_id, creator_id, asset_id, content_id, content_kind, token_hash,
                        access_scope, created_at, expires_at, last_used_at,
                        device_id, device_name, player_version, capabilities_json
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $10, $12, $13, $14, $15)
                    "#,
                )
                .bind(session.id)
                .bind(session.auth_session_id)
                .bind(session.user_id)
                .bind(session.creator_id)
                .bind(session.asset_id)
                .bind(session.content_id)
                .bind(session.content_kind)
                .bind(hash_token(session.playback_token))
                .bind(session.access_scope)
                .bind(session.created_at)
                .bind(session.expires_at)
                .bind(session.device_id)
                .bind(session.device_name)
                .bind(session.player_version)
                .bind(session.capabilities_json)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn rotate_reusable_live_playback_session(
        &self,
        session: PlaybackSession,
        refreshed_token: &str,
        refreshed_at: &str,
        refreshed_expires_at: &str,
        metadata: PlaybackSessionMetadataUpdate<'_>,
    ) -> AppResult<PlaybackSession> {
        let rows_affected = match &self.provider {
            DatabaseProvider::Sqlite(pool) => sqlx::query(
                r#"
                    UPDATE playback_sessions
                    SET token_hash = ?, expires_at = ?, last_used_at = ?,
                        device_name = COALESCE(?, device_name),
                        player_version = COALESCE(?, player_version),
                        capabilities_json = COALESCE(?, capabilities_json)
                    WHERE id = ? AND expires_at > ?
                    "#,
            )
            .bind(hash_token(refreshed_token))
            .bind(refreshed_expires_at)
            .bind(refreshed_at)
            .bind(metadata.device_name)
            .bind(metadata.player_version)
            .bind(metadata.capabilities_json)
            .bind(&session.id)
            .bind(refreshed_at)
            .execute(pool)
            .await?
            .rows_affected(),
            DatabaseProvider::Postgres(pool) => sqlx::query(
                r#"
                    UPDATE playback_sessions
                    SET token_hash = $1, expires_at = $2, last_used_at = $3,
                        device_name = COALESCE($4, device_name),
                        player_version = COALESCE($5, player_version),
                        capabilities_json = COALESCE($6, capabilities_json)
                    WHERE id = $7 AND expires_at > $3
                    "#,
            )
            .bind(hash_token(refreshed_token))
            .bind(refreshed_expires_at)
            .bind(refreshed_at)
            .bind(metadata.device_name)
            .bind(metadata.player_version)
            .bind(metadata.capabilities_json)
            .bind(&session.id)
            .execute(pool)
            .await?
            .rows_affected(),
        };
        if rows_affected != 1 {
            return Err(AppError::Unauthorized);
        }
        Ok(PlaybackSession {
            expires_at: refreshed_expires_at.to_string(),
            last_used_at: refreshed_at.to_string(),
            ..session
        })
    }

    pub async fn rotate_playback_session_token(
        &self,
        session_id: &str,
        current_playback_token: &str,
        refreshed_token: &str,
        refreshed_at: &str,
        refreshed_expires_at: &str,
    ) -> AppResult<()> {
        let rows_affected = match &self.provider {
            DatabaseProvider::Sqlite(pool) => sqlx::query(
                r#"
                    UPDATE playback_sessions
                    SET token_hash = ?, expires_at = ?, last_used_at = ?
                    WHERE id = ? AND token_hash = ? AND expires_at > ?
                    "#,
            )
            .bind(hash_token(refreshed_token))
            .bind(refreshed_expires_at)
            .bind(refreshed_at)
            .bind(session_id)
            .bind(hash_token(current_playback_token))
            .bind(refreshed_at)
            .execute(pool)
            .await?
            .rows_affected(),
            DatabaseProvider::Postgres(pool) => sqlx::query(
                r#"
                    UPDATE playback_sessions
                    SET token_hash = $1, expires_at = $2, last_used_at = $3
                    WHERE id = $4 AND token_hash = $5 AND expires_at > $3
                    "#,
            )
            .bind(hash_token(refreshed_token))
            .bind(refreshed_expires_at)
            .bind(refreshed_at)
            .bind(session_id)
            .bind(hash_token(current_playback_token))
            .execute(pool)
            .await?
            .rows_affected(),
        };
        if rows_affected != 1 {
            return Err(AppError::Unauthorized);
        }
        Ok(())
    }

    pub async fn fetch_active_playback_session_record(
        &self,
        session_id: &str,
        playback_token: &str,
        now: &str,
    ) -> AppResult<PlaybackSessionRecord> {
        let session = match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                let row = sqlx::query(
                    r#"
                    SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
                           auth_session_id, created_at, expires_at, last_used_at
                    FROM playback_sessions
                    WHERE id = ? AND token_hash = ? AND expires_at > ?
                    "#,
                )
                .bind(session_id)
                .bind(hash_token(playback_token))
                .bind(now)
                .fetch_optional(pool)
                .await?
                .ok_or(AppError::Unauthorized)?;

                sqlx::query("UPDATE playback_sessions SET last_used_at = ? WHERE id = ?")
                    .bind(now)
                    .bind(session_id)
                    .execute(pool)
                    .await?;

                sqlite_playback_session_record_from_row(row)
            }
            DatabaseProvider::Postgres(pool) => {
                let row = sqlx::query(
                    r#"
                    SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
                           auth_session_id, created_at, expires_at, last_used_at
                    FROM playback_sessions
                    WHERE id = $1 AND token_hash = $2 AND expires_at > $3
                    "#,
                )
                .bind(session_id)
                .bind(hash_token(playback_token))
                .bind(now)
                .fetch_optional(pool)
                .await?
                .ok_or(AppError::Unauthorized)?;

                sqlx::query("UPDATE playback_sessions SET last_used_at = $1 WHERE id = $2")
                    .bind(now)
                    .bind(session_id)
                    .execute(pool)
                    .await?;

                postgres_playback_session_record_from_row(row)
            }
        };

        Ok(PlaybackSessionRecord {
            last_used_at: now.to_string(),
            ..session
        })
    }

    pub async fn fetch_latest_active_playback_session_record_by_token(
        &self,
        playback_token: &str,
        now: &str,
    ) -> AppResult<PlaybackSessionRecord> {
        let session = match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                let row = sqlx::query(
                    r#"
                    SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
                           auth_session_id, created_at, expires_at, last_used_at
                    FROM playback_sessions
                    WHERE token_hash = ? AND expires_at > ?
                    ORDER BY created_at DESC
                    LIMIT 1
                    "#,
                )
                .bind(hash_token(playback_token))
                .bind(now)
                .fetch_optional(pool)
                .await?
                .ok_or(AppError::Unauthorized)?;
                let session = sqlite_playback_session_record_from_row(row);

                sqlx::query("UPDATE playback_sessions SET last_used_at = ? WHERE id = ?")
                    .bind(now)
                    .bind(&session.id)
                    .execute(pool)
                    .await?;

                session
            }
            DatabaseProvider::Postgres(pool) => {
                let row = sqlx::query(
                    r#"
                    SELECT id, user_id, creator_id, asset_id, content_id, content_kind, access_scope,
                           auth_session_id, created_at, expires_at, last_used_at
                    FROM playback_sessions
                    WHERE token_hash = $1 AND expires_at > $2
                    ORDER BY created_at DESC
                    LIMIT 1
                    "#,
                )
                .bind(hash_token(playback_token))
                .bind(now)
                .fetch_optional(pool)
                .await?
                .ok_or(AppError::Unauthorized)?;
                let session = postgres_playback_session_record_from_row(row);

                sqlx::query("UPDATE playback_sessions SET last_used_at = $1 WHERE id = $2")
                    .bind(now)
                    .bind(&session.id)
                    .execute(pool)
                    .await?;

                session
            }
        };

        Ok(PlaybackSessionRecord {
            last_used_at: now.to_string(),
            ..session
        })
    }

    pub fn sqlite_adapter(&self) -> &SqlitePool {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => pool,
            DatabaseProvider::Postgres(_) => {
                panic!("sqlite repository adapter requested while postgres provider is active")
            }
        }
    }

    #[allow(dead_code)]
    pub fn try_sqlite_adapter(&self) -> AppResult<&SqlitePool> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => Ok(pool),
            DatabaseProvider::Postgres(_) => Err(AppError::BadRequest(
                "this endpoint is not available on the active database provider".to_string(),
            )),
        }
    }

    pub fn try_postgres_adapter(&self) -> AppResult<&PgPool> {
        match &self.provider {
            DatabaseProvider::Postgres(pool) => Ok(pool),
            DatabaseProvider::Sqlite(_) => Err(AppError::Internal(
                "postgres repository adapter requested while sqlite provider is active".to_string(),
            )),
        }
    }
}

impl Database {
    async fn validate_watchlist_content(&self, content_id: &str) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                if sqlite_exists(
                    pool,
                    "SELECT 1 FROM series WHERE id = ? LIMIT 1",
                    content_id,
                )
                .await?
                    || sqlite_exists(pool, "SELECT 1 FROM films WHERE id = ? LIMIT 1", content_id)
                        .await?
                {
                    return Ok(());
                }
                if sqlite_exists(
                    pool,
                    "SELECT 1 FROM live_streams WHERE id = ? LIMIT 1",
                    content_id,
                )
                .await?
                {
                    return Err(AppError::BadRequest(
                        "watchlist only supports series and films".to_string(),
                    ));
                }
            }
            DatabaseProvider::Postgres(pool) => {
                if postgres_exists(
                    pool,
                    "SELECT 1 FROM series WHERE id = $1 LIMIT 1",
                    content_id,
                )
                .await?
                    || postgres_exists(
                        pool,
                        "SELECT 1 FROM films WHERE id = $1 LIMIT 1",
                        content_id,
                    )
                    .await?
                {
                    return Ok(());
                }
                if postgres_exists(
                    pool,
                    "SELECT 1 FROM live_streams WHERE id = $1 LIMIT 1",
                    content_id,
                )
                .await?
                {
                    return Err(AppError::BadRequest(
                        "watchlist only supports series and films".to_string(),
                    ));
                }
            }
        }
        Err(AppError::NotFound)
    }

    async fn ensure_streamer_exists(&self, streamer_id: &str) -> AppResult<()> {
        let exists = match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlite_exists(
                    pool,
                    "SELECT 1 FROM streamers WHERE id = ? LIMIT 1",
                    streamer_id,
                )
                .await?
            }
            DatabaseProvider::Postgres(pool) => {
                postgres_exists(
                    pool,
                    "SELECT 1 FROM streamers WHERE id = $1 LIMIT 1",
                    streamer_id,
                )
                .await?
            }
        };
        if exists {
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }

    async fn resolve_progress_target(&self, input: &ProgressInput) -> AppResult<ProgressTarget> {
        match input.kind.as_str() {
            "film" => {
                if input.episode_id.is_some() {
                    return Err(AppError::BadRequest(
                        "film progress cannot include an episodeId".to_string(),
                    ));
                }
                let duration_sec = match &self.provider {
                    DatabaseProvider::Sqlite(pool) => {
                        sqlx::query("SELECT duration_sec FROM films WHERE id = ?")
                            .bind(&input.content_id)
                            .fetch_optional(pool)
                            .await?
                            .ok_or(AppError::NotFound)?
                            .get("duration_sec")
                    }
                    DatabaseProvider::Postgres(pool) => sqlx::query(
                        "SELECT duration_sec::BIGINT AS duration_sec FROM films WHERE id = $1",
                    )
                    .bind(&input.content_id)
                    .fetch_optional(pool)
                    .await?
                    .ok_or(AppError::NotFound)?
                    .get("duration_sec"),
                };
                Ok(ProgressTarget {
                    kind: "film".to_string(),
                    episode_id: None,
                    duration_sec,
                })
            }
            "series" => {
                let episode_id = input.episode_id.clone().ok_or_else(|| {
                    AppError::BadRequest("series progress requires an episodeId".to_string())
                })?;
                let (series_exists, episode_series_id, duration_sec) = match &self.provider {
                    DatabaseProvider::Sqlite(pool) => {
                        let series_exists = sqlite_exists(
                            pool,
                            "SELECT 1 FROM series WHERE id = ? LIMIT 1",
                            &input.content_id,
                        )
                        .await?;
                        let row = sqlx::query(
                            "SELECT series_id, duration_sec FROM episodes WHERE id = ?",
                        )
                        .bind(&episode_id)
                        .fetch_optional(pool)
                        .await?
                        .ok_or(AppError::NotFound)?;
                        (
                            series_exists,
                            row.get::<String, _>("series_id"),
                            row.get::<i64, _>("duration_sec"),
                        )
                    }
                    DatabaseProvider::Postgres(pool) => {
                        let series_exists = postgres_exists(
                            pool,
                            "SELECT 1 FROM series WHERE id = $1 LIMIT 1",
                            &input.content_id,
                        )
                        .await?;
                        let row = sqlx::query(
                            "SELECT series_id, duration_sec::BIGINT AS duration_sec FROM episodes WHERE id = $1",
                        )
                        .bind(&episode_id)
                        .fetch_optional(pool)
                        .await?
                        .ok_or(AppError::NotFound)?;
                        (
                            series_exists,
                            row.get::<String, _>("series_id"),
                            row.get::<i64, _>("duration_sec"),
                        )
                    }
                };
                if !series_exists {
                    return Err(AppError::NotFound);
                }
                if episode_series_id != input.content_id {
                    return Err(AppError::BadRequest(
                        "episodeId does not belong to the requested series".to_string(),
                    ));
                }
                Ok(ProgressTarget {
                    kind: "series".to_string(),
                    episode_id: Some(episode_id),
                    duration_sec,
                })
            }
            _ => Err(AppError::BadRequest(
                "kind must be either 'film' or 'series'".to_string(),
            )),
        }
    }

    async fn upsert_watch_history_entry(
        &self,
        user_id: &str,
        content_id: &str,
        kind: &str,
        episode_id: Option<&str>,
        progress_sec: i64,
        duration_sec: i64,
        completed: bool,
        watched_at: &str,
    ) -> AppResult<()> {
        let completed_at = completed.then_some(watched_at);
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO user_watch_history (
                        user_id, content_id, kind, episode_id, progress_sec, duration_sec,
                        completed, completed_at, last_watched_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(user_id, content_id) DO UPDATE SET
                        kind = excluded.kind,
                        episode_id = excluded.episode_id,
                        progress_sec = excluded.progress_sec,
                        duration_sec = excluded.duration_sec,
                        completed = excluded.completed,
                        completed_at = excluded.completed_at,
                        last_watched_at = excluded.last_watched_at
                    "#,
                )
                .bind(user_id)
                .bind(content_id)
                .bind(kind)
                .bind(episode_id)
                .bind(progress_sec)
                .bind(duration_sec)
                .bind(bool_to_sqlite_int(completed))
                .bind(completed_at)
                .bind(watched_at)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO user_watch_history (
                        user_id, content_id, kind, episode_id, progress_sec, duration_sec,
                        completed, completed_at, last_watched_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                    ON CONFLICT(user_id, content_id) DO UPDATE SET
                        kind = excluded.kind,
                        episode_id = excluded.episode_id,
                        progress_sec = excluded.progress_sec,
                        duration_sec = excluded.duration_sec,
                        completed = excluded.completed,
                        completed_at = excluded.completed_at,
                        last_watched_at = excluded.last_watched_at
                    "#,
                )
                .bind(user_id)
                .bind(content_id)
                .bind(kind)
                .bind(episode_id)
                .bind(i64_to_postgres_int(progress_sec)?)
                .bind(i64_to_postgres_int(duration_sec)?)
                .bind(bool_to_postgres_int(completed))
                .bind(completed_at)
                .bind(watched_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }
}

fn bool_to_sqlite_int(value: bool) -> i64 {
    value as i64
}

fn bool_to_postgres_int(value: bool) -> i32 {
    value as i32
}

fn i64_to_postgres_int(value: i64) -> AppResult<i32> {
    i32::try_from(value)
        .map_err(|_| AppError::BadRequest("integer value is out of range".to_string()))
}

async fn sqlite_exists(pool: &SqlitePool, query: &str, value: &str) -> AppResult<bool> {
    Ok(sqlx::query(query)
        .bind(value)
        .fetch_optional(pool)
        .await?
        .is_some())
}

async fn postgres_exists(pool: &PgPool, query: &str, value: &str) -> AppResult<bool> {
    Ok(sqlx::query(query)
        .bind(value)
        .fetch_optional(pool)
        .await?
        .is_some())
}

fn search_tokens(input: &str) -> Vec<String> {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .take(6)
        .map(ToOwned::to_owned)
        .collect()
}

fn build_sqlite_fts_query(input: &str) -> Option<String> {
    let tokens = search_tokens(input)
        .into_iter()
        .map(|token| format!("{token}*"))
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" "))
    }
}

async fn lookup_sqlite_identity(
    pool: &SqlitePool,
    token_hash: &str,
    now: &str,
) -> AppResult<RequestIdentity> {
    let row = sqlx::query(
        r#"
        SELECT
            auth_sessions.id,
            auth_sessions.user_id,
            auth_sessions.scopes_json,
            creator_profiles.id AS creator_id
        FROM auth_sessions
        LEFT JOIN creator_profiles ON creator_profiles.user_id = auth_sessions.user_id
        WHERE auth_sessions.token_hash = ?
          AND auth_sessions.revoked_at IS NULL
          AND (auth_sessions.expires_at IS NULL OR auth_sessions.expires_at > ?)
        "#,
    )
    .bind(token_hash)
    .bind(now)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    identity_from_row(
        row.get("id"),
        row.get("user_id"),
        row.get("creator_id"),
        row.get("scopes_json"),
    )
}

async fn lookup_postgres_identity(
    pool: &PgPool,
    token_hash: &str,
    now: &str,
) -> AppResult<RequestIdentity> {
    let row = sqlx::query(
        r#"
        SELECT
            auth_sessions.id,
            auth_sessions.user_id,
            auth_sessions.scopes_json,
            creator_profiles.id AS creator_id
        FROM auth_sessions
        LEFT JOIN creator_profiles ON creator_profiles.user_id = auth_sessions.user_id
        WHERE auth_sessions.token_hash = $1
          AND auth_sessions.revoked_at IS NULL
          AND (auth_sessions.expires_at IS NULL OR auth_sessions.expires_at > $2)
        "#,
    )
    .bind(token_hash)
    .bind(now)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    identity_from_row(
        row.get("id"),
        row.get("user_id"),
        row.get("creator_id"),
        row.get("scopes_json"),
    )
}

fn identity_from_row(
    session_id: String,
    user_id: String,
    creator_id: Option<String>,
    scopes_json: String,
) -> AppResult<RequestIdentity> {
    Ok(RequestIdentity {
        session_id,
        user_id,
        creator_id,
        scopes: serde_json::from_str(&scopes_json)?,
    })
}

fn sqlite_auth_session_from_row(
    row: SqliteRow,
    current_session_id: &str,
) -> AppResult<AuthSession> {
    let id: String = row.get("id");
    Ok(AuthSession {
        is_current: id == current_session_id,
        id: id.clone(),
        label: row.get("label"),
        scopes: serde_json::from_str(&row.get::<String, _>("scopes_json")).unwrap_or_default(),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        revoked_at: row.get("revoked_at"),
        last_used_at: row.get("last_used_at"),
    })
}

fn postgres_auth_session_from_row(row: PgRow, current_session_id: &str) -> AppResult<AuthSession> {
    let id: String = row.get("id");
    Ok(AuthSession {
        is_current: id == current_session_id,
        id: id.clone(),
        label: row.get("label"),
        scopes: serde_json::from_str(&row.get::<String, _>("scopes_json")).unwrap_or_default(),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        revoked_at: row.get("revoked_at"),
        last_used_at: row.get("last_used_at"),
    })
}

fn sqlite_playback_session_from_row(row: SqliteRow) -> PlaybackSession {
    PlaybackSession {
        id: row.get("id"),
        content_id: row.get("content_id"),
        content_kind: row.get("content_kind"),
        access_scope: row.get("access_scope"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        last_used_at: row.get("last_used_at"),
    }
}

fn postgres_playback_session_from_row(row: PgRow) -> PlaybackSession {
    PlaybackSession {
        id: row.get("id"),
        content_id: row.get("content_id"),
        content_kind: row.get("content_kind"),
        access_scope: row.get("access_scope"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        last_used_at: row.get("last_used_at"),
    }
}

fn sqlite_playback_session_record_from_row(row: SqliteRow) -> PlaybackSessionRecord {
    PlaybackSessionRecord {
        id: row.get("id"),
        auth_session_id: row.get("auth_session_id"),
        user_id: row.get("user_id"),
        creator_id: row.get("creator_id"),
        asset_id: row.get("asset_id"),
        content_id: row.get("content_id"),
        content_kind: row.get("content_kind"),
        access_scope: row.get("access_scope"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        last_used_at: row.get("last_used_at"),
    }
}

fn postgres_playback_session_record_from_row(row: PgRow) -> PlaybackSessionRecord {
    PlaybackSessionRecord {
        id: row.get("id"),
        auth_session_id: row.get("auth_session_id"),
        user_id: row.get("user_id"),
        creator_id: row.get("creator_id"),
        asset_id: row.get("asset_id"),
        content_id: row.get("content_id"),
        content_kind: row.get("content_kind"),
        access_scope: row.get("access_scope"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        last_used_at: row.get("last_used_at"),
    }
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::auth::hash_token;
    use crate::models::{LanguageSettings, PlaybackSettings};

    use super::*;

    #[tokio::test]
    async fn sqlite_provider_reports_ready() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("sqlite pool");
        let database = Database::from_sqlite(pool);

        assert_eq!(database.kind(), DatabaseKind::Sqlite);
        assert!(database.check().await.expect("database check"));
    }

    #[tokio::test]
    async fn sqlite_provider_looks_up_and_touches_auth_identity() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("sqlite pool");
        sqlx::raw_sql(
            r#"
            CREATE TABLE users (
                id TEXT PRIMARY KEY
            );
            CREATE TABLE creator_profiles (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL
            );
            CREATE TABLE auth_sessions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                token_hash TEXT NOT NULL UNIQUE,
                scopes_json TEXT NOT NULL,
                expires_at TEXT,
                revoked_at TEXT,
                last_used_at TEXT
            );
            INSERT INTO users (id) VALUES ('usr-test');
            INSERT INTO creator_profiles (id, user_id) VALUES ('crt-test', 'usr-test');
            "#,
        )
        .execute(&pool)
        .await
        .expect("schema");

        let token_hash = hash_token("session-token");
        sqlx::query(
            r#"
            INSERT INTO auth_sessions (
                id, user_id, token_hash, scopes_json, expires_at, revoked_at, last_used_at
            ) VALUES ('ses-test', 'usr-test', ?, '["viewer","creator"]', NULL, NULL, NULL)
            "#,
        )
        .bind(&token_hash)
        .execute(&pool)
        .await
        .expect("session");

        let database = Database::from_sqlite(pool.clone());
        let identity = database
            .lookup_identity(&token_hash, "2026-08-23T00:00:00Z")
            .await
            .expect("identity");

        assert_eq!(identity.session_id, "ses-test");
        assert_eq!(identity.user_id, "usr-test");
        assert_eq!(identity.creator_id.as_deref(), Some("crt-test"));
        assert_eq!(identity.scopes, vec!["viewer", "creator"]);

        database
            .touch_auth_session("ses-test", "2026-08-23T00:01:00Z")
            .await
            .expect("touch");
        let last_used_at: String =
            sqlx::query("SELECT last_used_at FROM auth_sessions WHERE id = 'ses-test'")
                .fetch_one(&pool)
                .await
                .expect("row")
                .get("last_used_at");
        assert_eq!(last_used_at, "2026-08-23T00:01:00Z");
    }

    #[tokio::test]
    async fn sqlite_provider_manages_user_auth_sessions() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("sqlite pool");
        sqlx::raw_sql(
            r#"
            CREATE TABLE auth_sessions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                label TEXT NOT NULL,
                token_hash TEXT NOT NULL UNIQUE,
                scopes_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                expires_at TEXT,
                revoked_at TEXT,
                last_used_at TEXT
            );
            CREATE TABLE playback_sessions (
                id TEXT PRIMARY KEY,
                auth_session_id TEXT,
                expires_at TEXT NOT NULL,
                last_used_at TEXT NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("schema");

        let database = Database::from_sqlite(pool.clone());
        database
            .create_auth_session(NewAuthSession {
                id: "ses-current",
                user_id: "usr-test",
                label: "Current",
                token_hash: "hash-current",
                scopes_json: r#"["viewer"]"#,
                created_at: "2026-08-23T00:00:00Z",
                expires_at: None,
            })
            .await
            .expect("current session");
        database
            .create_auth_session(NewAuthSession {
                id: "ses-next",
                user_id: "usr-test",
                label: "Next",
                token_hash: "hash-next",
                scopes_json: r#"["viewer","creator"]"#,
                created_at: "2026-08-23T00:01:00Z",
                expires_at: Some("2026-09-23T00:01:00Z"),
            })
            .await
            .expect("next session");

        let sessions = database
            .list_auth_sessions("usr-test", "ses-current", None)
            .await
            .expect("sessions");
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].id, "ses-current");
        assert!(sessions[0].is_current);
        assert_eq!(sessions[1].scopes, vec!["viewer", "creator"]);

        let rows = database
            .revoke_auth_session("ses-next", "usr-test", "2026-08-23T00:02:00Z")
            .await
            .expect("revoke");
        assert_eq!(rows, 1);
        sqlx::query(
            r#"
            INSERT INTO playback_sessions (id, auth_session_id, expires_at, last_used_at)
            VALUES ('pbs-next', 'ses-next', '2026-08-24T00:00:00Z', '2026-08-23T00:01:00Z')
            "#,
        )
        .execute(&pool)
        .await
        .expect("playback session");
        database
            .expire_playback_sessions_for_auth_session("ses-next", "2026-08-23T00:02:00Z")
            .await
            .expect("expire playback");
        let playback_row = sqlx::query(
            "SELECT expires_at, last_used_at FROM playback_sessions WHERE id = 'pbs-next'",
        )
        .fetch_one(&pool)
        .await
        .expect("playback row");
        assert_eq!(
            playback_row.get::<String, _>("expires_at"),
            "2026-08-23T00:02:00Z"
        );
        assert_eq!(
            playback_row.get::<String, _>("last_used_at"),
            "2026-08-23T00:02:00Z"
        );
        let rows = database
            .revoke_auth_session("ses-next", "usr-test", "2026-08-23T00:03:00Z")
            .await
            .expect("second revoke");
        assert_eq!(rows, 0);
    }

    #[tokio::test]
    async fn sqlite_provider_manages_playback_session_lifecycle() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("sqlite pool");
        sqlx::raw_sql(
            r#"
            CREATE TABLE playback_sessions (
                id TEXT PRIMARY KEY,
                auth_session_id TEXT,
                user_id TEXT,
                creator_id TEXT,
                asset_id TEXT NOT NULL,
                content_id TEXT NOT NULL,
                content_kind TEXT NOT NULL,
                token_hash TEXT NOT NULL,
                access_scope TEXT NOT NULL,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                last_used_at TEXT NOT NULL,
                device_id TEXT,
                device_name TEXT,
                player_version TEXT,
                capabilities_json TEXT
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("schema");

        let database = Database::from_sqlite(pool.clone());
        database
            .create_playback_session(NewPlaybackSession {
                id: "pbs-live",
                auth_session_id: Some("ses-live"),
                user_id: Some("usr-test"),
                creator_id: Some("crt-test"),
                asset_id: "asset-live",
                content_id: "live-test",
                content_kind: "live",
                playback_token: "playback-token",
                access_scope: "live",
                created_at: "2026-08-23T00:00:00Z",
                expires_at: "2026-08-23T06:00:00Z",
                device_id: Some("device-a"),
                device_name: Some("Safari"),
                player_version: Some("1.0.0"),
                capabilities_json: Some(r#"{"hls":true}"#),
            })
            .await
            .expect("create playback");

        let active = database
            .fetch_active_playback_session_record(
                "pbs-live",
                "playback-token",
                "2026-08-23T00:04:00Z",
            )
            .await
            .expect("fetch active playback");
        assert_eq!(active.content_id, "live-test");
        assert_eq!(active.last_used_at, "2026-08-23T00:04:00Z");

        let latest = database
            .fetch_latest_active_playback_session_record_by_token(
                "playback-token",
                "2026-08-23T00:04:30Z",
            )
            .await
            .expect("fetch latest active playback by token");
        assert_eq!(latest.id, "pbs-live");
        assert_eq!(latest.last_used_at, "2026-08-23T00:04:30Z");

        let reusable = database
            .find_reusable_live_playback_session(ReusableLivePlaybackSessionLookup {
                stream_id: "live-test",
                auth_session_id: Some("ses-live"),
                device_id: Some("device-a"),
                now: "2026-08-23T00:05:00Z",
            })
            .await
            .expect("lookup")
            .expect("reusable session");
        assert_eq!(reusable.id, "pbs-live");

        let rotated = database
            .rotate_reusable_live_playback_session(
                reusable,
                "playback-token-2",
                "2026-08-23T00:10:00Z",
                "2026-08-23T06:10:00Z",
                PlaybackSessionMetadataUpdate {
                    device_name: Some("Chrome"),
                    player_version: None,
                    capabilities_json: Some(r#"{"hls":true,"hevc":false}"#),
                },
            )
            .await
            .expect("rotate reusable");
        assert_eq!(rotated.expires_at, "2026-08-23T06:10:00Z");
        assert_eq!(rotated.last_used_at, "2026-08-23T00:10:00Z");

        let row = sqlx::query(
            "SELECT token_hash, device_name, player_version, capabilities_json FROM playback_sessions WHERE id = 'pbs-live'",
        )
        .fetch_one(&pool)
        .await
        .expect("row");
        assert_eq!(
            row.get::<String, _>("token_hash"),
            hash_token("playback-token-2")
        );
        assert_eq!(row.get::<String, _>("device_name"), "Chrome");
        assert_eq!(row.get::<String, _>("player_version"), "1.0.0");
        assert_eq!(
            row.get::<String, _>("capabilities_json"),
            r#"{"hls":true,"hevc":false}"#
        );

        database
            .rotate_playback_session_token(
                "pbs-live",
                "playback-token-2",
                "playback-token-3",
                "2026-08-23T00:20:00Z",
                "2026-08-23T06:20:00Z",
            )
            .await
            .expect("refresh rotation");
        let token_hash: String =
            sqlx::query("SELECT token_hash FROM playback_sessions WHERE id = 'pbs-live'")
                .fetch_one(&pool)
                .await
                .expect("token row")
                .get("token_hash");
        assert_eq!(token_hash, hash_token("playback-token-3"));

        let stale = database
            .rotate_playback_session_token(
                "pbs-live",
                "playback-token-2",
                "playback-token-4",
                "2026-08-23T00:30:00Z",
                "2026-08-23T06:30:00Z",
            )
            .await;
        assert!(matches!(stale, Err(AppError::Unauthorized)));
    }

    #[tokio::test]
    async fn sqlite_provider_searches_catalog_documents() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("sqlite pool");
        sqlx::raw_sql(
            r#"
            CREATE VIRTUAL TABLE search_documents USING fts5(
                entity_id UNINDEXED,
                kind UNINDEXED,
                slug UNINDEXED,
                title,
                body,
                tokenize = 'unicode61 remove_diacritics 2'
            );
            INSERT INTO search_documents (entity_id, kind, slug, title, body)
            VALUES
                ('ser-halcyon', 'series', 'halcyon-drift', 'Halcyon Drift', 'salvage crew sci-fi'),
                ('film-paper', 'film', 'paper-moon', 'Paper Moon', 'noir drama');
            "#,
        )
        .execute(&pool)
        .await
        .expect("search schema");

        let database = Database::from_sqlite(pool);
        let hits = database
            .search_catalog_documents("halcyon", 24)
            .await
            .expect("hits");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entity_id, "ser-halcyon");
        assert_eq!(hits[0].kind, "series");
        assert!(
            database
                .search_catalog_documents("   ", 24)
                .await
                .expect("empty")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn sqlite_provider_updates_user_profile_and_settings() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("sqlite pool");
        sqlx::raw_sql(
            r#"
            CREATE TABLE users (
                id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL
            );
            CREATE TABLE user_profiles (
                user_id TEXT PRIMARY KEY,
                email TEXT NOT NULL,
                mature_content_allowed INTEGER NOT NULL,
                default_audio TEXT NOT NULL,
                subtitle_preset TEXT NOT NULL,
                autoplay_trailers INTEGER NOT NULL,
                live_chat_filter TEXT NOT NULL
            );
            CREATE TABLE user_playback_settings (
                user_id TEXT PRIMARY KEY,
                default_quality TEXT NOT NULL,
                audio_language TEXT NOT NULL,
                subtitle_language TEXT NOT NULL,
                subtitle_style TEXT NOT NULL,
                autoplay_next_episode INTEGER NOT NULL,
                autoplay_trailers INTEGER NOT NULL,
                reduced_motion INTEGER NOT NULL,
                prefer_dubbed INTEGER NOT NULL,
                playback_speed TEXT NOT NULL
            );
            CREATE TABLE user_language_settings (
                user_id TEXT PRIMARY KEY,
                interface_language TEXT NOT NULL,
                subtitle_language TEXT NOT NULL,
                catalog_region TEXT NOT NULL,
                date_format TEXT NOT NULL,
                clock_format TEXT NOT NULL
            );
            INSERT INTO users (id, display_name) VALUES ('usr-test', 'Old Name');
            INSERT INTO user_profiles (
                user_id, email, mature_content_allowed, default_audio, subtitle_preset,
                autoplay_trailers, live_chat_filter
            ) VALUES ('usr-test', 'old@example.com', 0, 'English', 'Classic', 1, 'balanced');
            INSERT INTO user_playback_settings (
                user_id, default_quality, audio_language, subtitle_language, subtitle_style,
                autoplay_next_episode, autoplay_trailers, reduced_motion, prefer_dubbed,
                playback_speed
            ) VALUES ('usr-test', 'Auto', 'English', 'Off', 'Classic', 1, 1, 0, 0, '1x');
            INSERT INTO user_language_settings (
                user_id, interface_language, subtitle_language, catalog_region, date_format,
                clock_format
            ) VALUES ('usr-test', 'English', 'Off', 'US', 'MM/DD/YYYY', '12h');
            "#,
        )
        .execute(&pool)
        .await
        .expect("schema");

        let database = Database::from_sqlite(pool.clone());
        database
            .update_user_profile(
                "usr-test",
                &UpdateProfileRequest {
                    display_name: Some("New Name".to_string()),
                    email: Some("new@example.com".to_string()),
                    mature_content_allowed: Some(true),
                    default_audio: None,
                    subtitle_preset: None,
                    autoplay_trailers: Some(false),
                    live_chat_filter: Some("strict".to_string()),
                },
            )
            .await
            .expect("profile update");
        database
            .update_user_settings(
                "usr-test",
                &UpdateSettingsRequest {
                    playback: Some(PlaybackSettings {
                        default_quality: "4K".to_string(),
                        audio_language: "Japanese".to_string(),
                        subtitle_language: "English".to_string(),
                        subtitle_style: "Cinema".to_string(),
                        autoplay_next_episode: false,
                        autoplay_trailers: false,
                        reduced_motion: true,
                        prefer_dubbed: true,
                        playback_speed: "1.25x".to_string(),
                    }),
                    notifications: None,
                    privacy: None,
                    parental: None,
                    downloads: None,
                    language: Some(LanguageSettings {
                        interface_language: "Spanish".to_string(),
                        subtitle_language: "Spanish".to_string(),
                        catalog_region: "MX".to_string(),
                        date_format: "DD/MM/YYYY".to_string(),
                        clock_format: "24h".to_string(),
                    }),
                },
            )
            .await
            .expect("settings update");

        let user_row = sqlx::query("SELECT display_name FROM users WHERE id = 'usr-test'")
            .fetch_one(&pool)
            .await
            .expect("user row");
        assert_eq!(user_row.get::<String, _>("display_name"), "New Name");

        let profile_row = sqlx::query(
            r#"
            SELECT email, mature_content_allowed, default_audio, autoplay_trailers, live_chat_filter
            FROM user_profiles
            WHERE user_id = 'usr-test'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("profile row");
        assert_eq!(profile_row.get::<String, _>("email"), "new@example.com");
        assert_eq!(profile_row.get::<i64, _>("mature_content_allowed"), 1);
        assert_eq!(profile_row.get::<String, _>("default_audio"), "English");
        assert_eq!(profile_row.get::<i64, _>("autoplay_trailers"), 0);
        assert_eq!(profile_row.get::<String, _>("live_chat_filter"), "strict");

        let playback_row = sqlx::query(
            r#"
            SELECT default_quality, autoplay_next_episode, reduced_motion, prefer_dubbed
            FROM user_playback_settings
            WHERE user_id = 'usr-test'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("playback row");
        assert_eq!(playback_row.get::<String, _>("default_quality"), "4K");
        assert_eq!(playback_row.get::<i64, _>("autoplay_next_episode"), 0);
        assert_eq!(playback_row.get::<i64, _>("reduced_motion"), 1);
        assert_eq!(playback_row.get::<i64, _>("prefer_dubbed"), 1);

        let language_row = sqlx::query(
            r#"
            SELECT interface_language, catalog_region, clock_format
            FROM user_language_settings
            WHERE user_id = 'usr-test'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("language row");
        assert_eq!(
            language_row.get::<String, _>("interface_language"),
            "Spanish"
        );
        assert_eq!(language_row.get::<String, _>("catalog_region"), "MX");
        assert_eq!(language_row.get::<String, _>("clock_format"), "24h");
    }

    #[tokio::test]
    async fn sqlite_provider_manages_viewer_activity_mutations() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("sqlite pool");
        sqlx::raw_sql(
            r#"
            CREATE TABLE series (
                id TEXT PRIMARY KEY
            );
            CREATE TABLE films (
                id TEXT PRIMARY KEY,
                duration_sec INTEGER NOT NULL
            );
            CREATE TABLE live_streams (
                id TEXT PRIMARY KEY
            );
            CREATE TABLE streamers (
                id TEXT PRIMARY KEY
            );
            CREATE TABLE episodes (
                id TEXT PRIMARY KEY,
                series_id TEXT NOT NULL,
                duration_sec INTEGER NOT NULL
            );
            CREATE TABLE user_watchlist (
                user_id TEXT NOT NULL,
                content_id TEXT NOT NULL,
                PRIMARY KEY (user_id, content_id)
            );
            CREATE TABLE user_following (
                user_id TEXT NOT NULL,
                streamer_id TEXT NOT NULL,
                PRIMARY KEY (user_id, streamer_id)
            );
            CREATE TABLE continue_watching (
                user_id TEXT NOT NULL,
                content_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                episode_id TEXT,
                progress_sec INTEGER NOT NULL,
                duration_sec INTEGER NOT NULL,
                last_watched_at TEXT NOT NULL,
                PRIMARY KEY (user_id, content_id)
            );
            CREATE TABLE user_watch_history (
                user_id TEXT NOT NULL,
                content_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                episode_id TEXT,
                progress_sec INTEGER NOT NULL,
                duration_sec INTEGER NOT NULL,
                completed INTEGER NOT NULL,
                completed_at TEXT,
                last_watched_at TEXT NOT NULL,
                PRIMARY KEY (user_id, content_id)
            );
            INSERT INTO series (id) VALUES ('ser-test');
            INSERT INTO films (id, duration_sec) VALUES ('film-test', 120);
            INSERT INTO live_streams (id) VALUES ('live-test');
            INSERT INTO streamers (id) VALUES ('str-test');
            INSERT INTO episodes (id, series_id, duration_sec) VALUES ('ep-test', 'ser-test', 60);
            "#,
        )
        .execute(&pool)
        .await
        .expect("schema");

        let database = Database::from_sqlite(pool.clone());
        database
            .add_watchlist_item("usr-test", "film-test")
            .await
            .expect("add watchlist");
        database
            .add_watchlist_item("usr-test", "film-test")
            .await
            .expect("duplicate watchlist");
        let watchlist_count: i64 =
            sqlx::query("SELECT COUNT(*) AS count FROM user_watchlist WHERE user_id = 'usr-test'")
                .fetch_one(&pool)
                .await
                .expect("watchlist count")
                .get("count");
        assert_eq!(watchlist_count, 1);
        assert!(
            matches!(
                database.add_watchlist_item("usr-test", "live-test").await,
                Err(AppError::BadRequest(_))
            ),
            "live streams should not be watchlistable"
        );

        database
            .add_following("usr-test", "str-test")
            .await
            .expect("add following");
        assert!(
            matches!(
                database.add_following("usr-test", "missing-streamer").await,
                Err(AppError::NotFound)
            ),
            "missing streamer should not be followable"
        );
        let following_count: i64 =
            sqlx::query("SELECT COUNT(*) AS count FROM user_following WHERE user_id = 'usr-test'")
                .fetch_one(&pool)
                .await
                .expect("following count")
                .get("count");
        assert_eq!(following_count, 1);

        database
            .record_progress(
                "usr-test",
                &ProgressInput {
                    content_id: "film-test".to_string(),
                    kind: "film".to_string(),
                    episode_id: None,
                    progress_sec: 30,
                    _duration_sec: 999,
                },
                "2026-08-23T10:00:00Z",
            )
            .await
            .expect("partial progress");
        let progress_row = sqlx::query(
            "SELECT progress_sec, duration_sec FROM continue_watching WHERE user_id = 'usr-test' AND content_id = 'film-test'",
        )
        .fetch_one(&pool)
        .await
        .expect("continue row");
        assert_eq!(progress_row.get::<i64, _>("progress_sec"), 30);
        assert_eq!(progress_row.get::<i64, _>("duration_sec"), 120);

        database
            .record_progress(
                "usr-test",
                &ProgressInput {
                    content_id: "film-test".to_string(),
                    kind: "film".to_string(),
                    episode_id: None,
                    progress_sec: 120,
                    _duration_sec: 999,
                },
                "2026-08-23T10:05:00Z",
            )
            .await
            .expect("complete progress");
        let continue_count: i64 = sqlx::query(
            "SELECT COUNT(*) AS count FROM continue_watching WHERE user_id = 'usr-test' AND content_id = 'film-test'",
        )
        .fetch_one(&pool)
        .await
        .expect("continue count")
        .get("count");
        assert_eq!(continue_count, 0);
        let history_row = sqlx::query(
            "SELECT progress_sec, duration_sec, completed, completed_at FROM user_watch_history WHERE user_id = 'usr-test' AND content_id = 'film-test'",
        )
        .fetch_one(&pool)
        .await
        .expect("history row");
        assert_eq!(history_row.get::<i64, _>("progress_sec"), 120);
        assert_eq!(history_row.get::<i64, _>("duration_sec"), 120);
        assert_eq!(history_row.get::<i64, _>("completed"), 1);
        assert_eq!(
            history_row.get::<String, _>("completed_at"),
            "2026-08-23T10:05:00Z"
        );

        database
            .record_progress(
                "usr-test",
                &ProgressInput {
                    content_id: "ser-test".to_string(),
                    kind: "series".to_string(),
                    episode_id: Some("ep-test".to_string()),
                    progress_sec: 20,
                    _duration_sec: 999,
                },
                "2026-08-23T10:10:00Z",
            )
            .await
            .expect("series progress");
        let series_progress = sqlx::query(
            "SELECT episode_id, duration_sec FROM continue_watching WHERE user_id = 'usr-test' AND content_id = 'ser-test'",
        )
        .fetch_one(&pool)
        .await
        .expect("series continue row");
        assert_eq!(series_progress.get::<String, _>("episode_id"), "ep-test");
        assert_eq!(series_progress.get::<i64, _>("duration_sec"), 60);

        database
            .remove_watchlist_item("usr-test", "film-test")
            .await
            .expect("remove watchlist");
        database
            .remove_following("usr-test", "str-test")
            .await
            .expect("remove following");
        database
            .remove_progress("usr-test", "ser-test")
            .await
            .expect("remove progress");
        database
            .remove_history_entry("usr-test", "film-test")
            .await
            .expect("remove film history");
        database
            .remove_history_entry("usr-test", "ser-test")
            .await
            .expect("remove series history");
        let remaining: i64 = sqlx::query(
            r#"
            SELECT
                (SELECT COUNT(*) FROM user_watchlist) +
                (SELECT COUNT(*) FROM user_following) +
                (SELECT COUNT(*) FROM continue_watching) +
                (SELECT COUNT(*) FROM user_watch_history) AS count
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("remaining rows")
        .get("count");
        assert_eq!(remaining, 0);
    }

    #[tokio::test]
    async fn sqlite_provider_marks_user_notification_read_once() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("sqlite pool");
        sqlx::raw_sql(
            r#"
            CREATE TABLE notification_deliveries (
                id TEXT PRIMARY KEY,
                recipient_user_id TEXT NOT NULL,
                read_at TEXT
            );
            INSERT INTO notification_deliveries (id, recipient_user_id, read_at)
            VALUES
                ('del-unread', 'usr-test', NULL),
                ('del-other', 'usr-other', NULL);
            "#,
        )
        .execute(&pool)
        .await
        .expect("schema");

        let database = Database::from_sqlite(pool.clone());
        let rows = database
            .mark_user_notification_read("usr-test", "del-unread", "2026-08-23T11:00:00Z")
            .await
            .expect("mark read");
        assert_eq!(rows, 1);
        let read_at: String =
            sqlx::query("SELECT read_at FROM notification_deliveries WHERE id = 'del-unread'")
                .fetch_one(&pool)
                .await
                .expect("read row")
                .get("read_at");
        assert_eq!(read_at, "2026-08-23T11:00:00Z");

        let rows = database
            .mark_user_notification_read("usr-test", "del-unread", "2026-08-23T11:05:00Z")
            .await
            .expect("mark read again");
        assert_eq!(rows, 1);
        let read_at: String =
            sqlx::query("SELECT read_at FROM notification_deliveries WHERE id = 'del-unread'")
                .fetch_one(&pool)
                .await
                .expect("read row")
                .get("read_at");
        assert_eq!(read_at, "2026-08-23T11:00:00Z");

        let rows = database
            .mark_user_notification_read("usr-test", "del-other", "2026-08-23T11:10:00Z")
            .await
            .expect("wrong user");
        assert_eq!(rows, 0);
    }
}
