use sqlx::{PgPool, Row, SqlitePool, postgres::PgRow, sqlite::SqliteRow};

use crate::api::PlaybackSessionRecord;
use crate::auth::{RequestIdentity, hash_token};
use crate::config::DatabaseKind;
use crate::error::{AppError, AppResult};
use crate::models::{
    AdvertiserAccountResponse, AdvertiserCompany, AdvertiserInvite, AdvertiserPermissionPreset,
    AdvertiserSeat, AuthSession, ContinueWatchingEntry, CreatorApiKey, CreatorProfile,
    PlaybackSession, ProgressInput, UpdateAdvertiserCompanyRequest, UpdateProfileRequest,
    UpdateSettingsRequest, User, ViewerEventInput,
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

pub struct NewCreatorApiKey<'a> {
    pub id: &'a str,
    pub user_id: &'a str,
    pub creator_id: &'a str,
    pub name: &'a str,
    pub key_prefix: &'a str,
    pub access_token: &'a str,
    pub key_hash: &'a str,
    pub scopes: &'a [String],
    pub created_at: &'a str,
    pub expires_at: Option<&'a str>,
}

pub struct CreatorApiKeyIdentity {
    pub key_id: String,
    pub creator_id: String,
    pub scopes: Vec<String>,
}

pub struct CreatorApiProfileUpdate {
    pub display_name: String,
    pub avatar: String,
    pub banner: String,
    pub tagline: String,
    pub bio: String,
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
    pub slug: String,
    pub title: String,
    pub subtitle: String,
    pub image: Option<String>,
    pub href: String,
    pub metadata_json: String,
    pub score: f64,
    pub total_count: i64,
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
        offset: i64,
    ) -> AppResult<Vec<CatalogSearchHit>> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                let Some(fts_query) = build_sqlite_fts_query(query) else {
                    return Ok(Vec::new());
                };
                let normalized_query = search_tokens(query).join(" ");
                let rows = sqlx::query(
                    r#"
                    SELECT
                        entity_id,
                        kind,
                        slug,
                        title,
                        subtitle,
                        NULLIF(image, '') AS image,
                        href,
                        metadata_json,
                        score,
                        COUNT(*) OVER () AS total_count
                    FROM (
                        SELECT
                            entity_id,
                            kind,
                            slug,
                            title,
                            subtitle,
                            image,
                            href,
                            metadata_json,
                            CASE WHEN lower(title) = lower(?) OR lower(slug) = lower(?) THEN 1000.0 ELSE 0.0 END +
                                CASE WHEN lower(title) LIKE lower(?) || '%' THEN 250.0 ELSE 0.0 END +
                                (-bm25(search_documents, 8.0, 4.0, 1.0)) +
                                rank_boost +
                                (popularity * 0.02) AS score
                        FROM search_documents
                        WHERE search_documents MATCH ?
                    )
                    ORDER BY score DESC, title ASC
                    LIMIT ?
                    OFFSET ?
                    "#,
                )
                .bind(&normalized_query)
                .bind(&normalized_query)
                .bind(&normalized_query)
                .bind(&fts_query)
                .bind(limit.max(1))
                .bind(offset.max(0))
                .fetch_all(pool)
                .await?;
                Ok(rows
                    .into_iter()
                    .map(|row| CatalogSearchHit {
                        entity_id: row.get("entity_id"),
                        kind: row.get("kind"),
                        slug: row.get("slug"),
                        title: row.get("title"),
                        subtitle: row.get("subtitle"),
                        image: row.get("image"),
                        href: row.get("href"),
                        metadata_json: row.get("metadata_json"),
                        score: row.get("score"),
                        total_count: row.get("total_count"),
                    })
                    .collect())
            }
            DatabaseProvider::Postgres(pool) => {
                let tokens = search_tokens(query);
                if tokens.is_empty() {
                    return Ok(Vec::new());
                }
                let normalized_query = tokens.join(" ");
                let prefix_query = build_postgres_prefix_query(&tokens).unwrap_or_default();
                let rows = sqlx::query(
                    r#"
                    WITH input AS (
                        SELECT
                            websearch_to_tsquery('english', $1) AS web_query,
                            NULLIF($2, '')::tsquery AS prefix_query,
                            lower($1) AS raw_query
                    ),
                    ranked AS (
                        SELECT
                            d.entity_id,
                            d.kind,
                            d.slug,
                            d.title,
                            d.subtitle,
                            d.image,
                            d.href,
                            d.metadata_json::TEXT AS metadata_json,
                            (
                                CASE WHEN lower(d.title) = input.raw_query OR lower(d.slug) = input.raw_query THEN 1000.0 ELSE 0.0 END +
                                CASE WHEN lower(d.title) LIKE input.raw_query || '%' THEN 250.0 ELSE 0.0 END +
                                CASE WHEN lower(d.slug) LIKE input.raw_query || '%' THEN 200.0 ELSE 0.0 END +
                                CASE WHEN d.search_vector @@ input.web_query THEN ts_rank_cd(d.search_vector, input.web_query, 32) * 140.0 ELSE 0.0 END +
                                CASE WHEN input.prefix_query IS NOT NULL AND d.search_vector @@ input.prefix_query THEN ts_rank_cd(d.search_vector, input.prefix_query, 32) * 90.0 ELSE 0.0 END +
                                GREATEST(similarity(lower(d.title), input.raw_query), similarity(lower(d.slug), input.raw_query)) * 85.0 +
                                d.rank_boost +
                                LEAST(d.popularity, 300.0) * 0.2
                            ) AS score
                        FROM search_documents d
                        CROSS JOIN input
                        WHERE d.search_vector @@ input.web_query
                           OR (input.prefix_query IS NOT NULL AND d.search_vector @@ input.prefix_query)
                           OR lower(d.title) LIKE '%' || input.raw_query || '%'
                           OR lower(d.slug) LIKE '%' || input.raw_query || '%'
                           OR similarity(lower(d.title), input.raw_query) >= 0.18
                           OR similarity(lower(d.slug), input.raw_query) >= 0.18
                    )
                    SELECT
                        entity_id,
                        kind,
                        slug,
                        title,
                        subtitle,
                        image,
                        href,
                        metadata_json,
                        score,
                        COUNT(*) OVER () AS total_count
                    FROM ranked
                    ORDER BY
                        score DESC,
                        CASE kind
                            WHEN 'series' THEN 0
                            WHEN 'film' THEN 1
                            WHEN 'live' THEN 2
                            WHEN 'episode' THEN 3
                            WHEN 'creator' THEN 4
                            WHEN 'profile' THEN 5
                            WHEN 'category' THEN 6
                            ELSE 7
                        END,
                        title ASC
                    LIMIT $3
                    OFFSET $4
                    "#,
                )
                .bind(&normalized_query)
                .bind(&prefix_query)
                .bind(limit.max(1))
                .bind(offset.max(0))
                .fetch_all(pool)
                .await?;
                Ok(rows
                    .into_iter()
                    .map(|row| CatalogSearchHit {
                        entity_id: row.get("entity_id"),
                        kind: row.get("kind"),
                        slug: row.get("slug"),
                        title: row.get("title"),
                        subtitle: row.get("subtitle"),
                        image: row.get("image"),
                        href: row.get("href"),
                        metadata_json: row.get("metadata_json"),
                        score: row.get("score"),
                        total_count: row.get("total_count"),
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

    pub async fn record_viewer_event(
        &self,
        id: &str,
        user_id: Option<&str>,
        input: &ViewerEventInput,
        received_at: &str,
    ) -> AppResult<()> {
        let visitor_id = normalize_required_text(&input.visitor_id, 96, "visitorId")?;
        let event_type = normalize_required_text(&input.event_type, 96, "eventType")?;
        let content_id = normalize_optional_text(input.content_id.as_deref(), 128);
        let content_kind = normalize_optional_text(input.content_kind.as_deref(), 32);
        let episode_id = normalize_optional_text(input.episode_id.as_deref(), 128);
        let stream_id = normalize_optional_text(input.stream_id.as_deref(), 128);
        let session_id = normalize_optional_text(input.session_id.as_deref(), 128);
        let path = normalize_optional_text(input.path.as_deref(), 512);
        let url = normalize_optional_text(input.url.as_deref(), 2048);
        let referrer_url = normalize_optional_text(input.referrer_url.as_deref(), 2048);
        let landing_url = normalize_optional_text(input.landing_url.as_deref(), 2048);
        let initial_referrer_url =
            normalize_optional_text(input.initial_referrer_url.as_deref(), 2048);
        let utm_source = normalize_optional_text(input.utm_source.as_deref(), 160);
        let utm_medium = normalize_optional_text(input.utm_medium.as_deref(), 160);
        let utm_campaign = normalize_optional_text(input.utm_campaign.as_deref(), 160);
        let utm_term = normalize_optional_text(input.utm_term.as_deref(), 160);
        let utm_content = normalize_optional_text(input.utm_content.as_deref(), 160);
        let progress_sec = input.progress_sec.map(|value| value.max(0));
        let duration_sec = input.duration_sec.map(|value| value.max(0));
        let watch_time_ms = input.watch_time_ms.map(|value| value.max(0));
        let metadata_json = input
            .metadata
            .as_ref()
            .map(|value| serde_json::to_string(value))
            .transpose()?
            .map(|value| value.chars().take(8192).collect::<String>())
            .unwrap_or_else(|| "{}".to_string());
        let occurred_at = input
            .occurred_at
            .as_deref()
            .and_then(|value| normalize_optional_text(Some(value), 64))
            .unwrap_or_else(|| received_at.to_string());

        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO viewer_events (
                        id, visitor_id, user_id, event_type, content_id, content_kind, episode_id,
                        stream_id, session_id, path, url, referrer_url, landing_url,
                        initial_referrer_url, utm_source, utm_medium, utm_campaign, utm_term,
                        utm_content, progress_sec, duration_sec, watch_time_ms, metadata_json,
                        occurred_at, received_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind(id)
                .bind(&visitor_id)
                .bind(user_id)
                .bind(&event_type)
                .bind(content_id.as_deref())
                .bind(content_kind.as_deref())
                .bind(episode_id.as_deref())
                .bind(stream_id.as_deref())
                .bind(session_id.as_deref())
                .bind(path.as_deref())
                .bind(url.as_deref())
                .bind(referrer_url.as_deref())
                .bind(landing_url.as_deref())
                .bind(initial_referrer_url.as_deref())
                .bind(utm_source.as_deref())
                .bind(utm_medium.as_deref())
                .bind(utm_campaign.as_deref())
                .bind(utm_term.as_deref())
                .bind(utm_content.as_deref())
                .bind(progress_sec)
                .bind(duration_sec)
                .bind(watch_time_ms)
                .bind(&metadata_json)
                .bind(&occurred_at)
                .bind(received_at)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO viewer_events (
                        id, visitor_id, user_id, event_type, content_id, content_kind, episode_id,
                        stream_id, session_id, path, url, referrer_url, landing_url,
                        initial_referrer_url, utm_source, utm_medium, utm_campaign, utm_term,
                        utm_content, progress_sec, duration_sec, watch_time_ms, metadata_json,
                        occurred_at, received_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25)
                    "#,
                )
                .bind(id)
                .bind(&visitor_id)
                .bind(user_id)
                .bind(&event_type)
                .bind(content_id.as_deref())
                .bind(content_kind.as_deref())
                .bind(episode_id.as_deref())
                .bind(stream_id.as_deref())
                .bind(session_id.as_deref())
                .bind(path.as_deref())
                .bind(url.as_deref())
                .bind(referrer_url.as_deref())
                .bind(landing_url.as_deref())
                .bind(initial_referrer_url.as_deref())
                .bind(utm_source.as_deref())
                .bind(utm_medium.as_deref())
                .bind(utm_campaign.as_deref())
                .bind(utm_term.as_deref())
                .bind(utm_content.as_deref())
                .bind(progress_sec)
                .bind(duration_sec)
                .bind(watch_time_ms)
                .bind(&metadata_json)
                .bind(&occurred_at)
                .bind(received_at)
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

    pub async fn insert_creator_api_key(&self, key: NewCreatorApiKey<'_>) -> AppResult<()> {
        let scopes_json = serde_json::to_string(key.scopes)?;
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO creator_api_keys (
                        id, user_id, creator_id, name, key_prefix, access_token, key_hash, scopes_json,
                        created_at, expires_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind(key.id)
                .bind(key.user_id)
                .bind(key.creator_id)
                .bind(key.name)
                .bind(key.key_prefix)
                .bind(key.access_token)
                .bind(key.key_hash)
                .bind(scopes_json)
                .bind(key.created_at)
                .bind(key.expires_at)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO creator_api_keys (
                        id, user_id, creator_id, name, key_prefix, access_token, key_hash, scopes_json,
                        created_at, expires_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                    "#,
                )
                .bind(key.id)
                .bind(key.user_id)
                .bind(key.creator_id)
                .bind(key.name)
                .bind(key.key_prefix)
                .bind(key.access_token)
                .bind(key.key_hash)
                .bind(scopes_json)
                .bind(key.created_at)
                .bind(key.expires_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn list_creator_api_keys_for_user(
        &self,
        user_id: &str,
    ) -> AppResult<Vec<CreatorApiKey>> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                let rows = sqlx::query(creator_api_key_select("user_id = ?").as_str())
                    .bind(user_id)
                    .fetch_all(pool)
                    .await?;
                rows.into_iter()
                    .map(|row| creator_api_key_from_sqlite_row(&row))
                    .collect()
            }
            DatabaseProvider::Postgres(pool) => {
                let rows = sqlx::query(creator_api_key_select("user_id = $1").as_str())
                    .bind(user_id)
                    .fetch_all(pool)
                    .await?;
                rows.into_iter()
                    .map(|row| creator_api_key_from_postgres_row(&row))
                    .collect()
            }
        }
    }

    pub async fn get_creator_api_key_for_user(
        &self,
        id: &str,
        user_id: &str,
    ) -> AppResult<CreatorApiKey> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                let row = sqlx::query(creator_api_key_select("id = ? AND user_id = ?").as_str())
                    .bind(id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?
                    .ok_or(AppError::NotFound)?;
                creator_api_key_from_sqlite_row(&row)
            }
            DatabaseProvider::Postgres(pool) => {
                let row = sqlx::query(creator_api_key_select("id = $1 AND user_id = $2").as_str())
                    .bind(id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?
                    .ok_or(AppError::NotFound)?;
                creator_api_key_from_postgres_row(&row)
            }
        }
    }

    pub async fn revoke_creator_api_key(
        &self,
        id: &str,
        user_id: &str,
        now: &str,
    ) -> AppResult<u64> {
        let rows = match &self.provider {
            DatabaseProvider::Sqlite(pool) => sqlx::query(
                "UPDATE creator_api_keys SET revoked_at = ? WHERE id = ? AND user_id = ? AND revoked_at IS NULL",
            )
            .bind(now)
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected(),
            DatabaseProvider::Postgres(pool) => sqlx::query(
                "UPDATE creator_api_keys SET revoked_at = $1 WHERE id = $2 AND user_id = $3 AND revoked_at IS NULL",
            )
            .bind(now)
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected(),
        };
        Ok(rows)
    }

    pub async fn lookup_creator_api_key_identity(
        &self,
        key_hash: &str,
        now: &str,
    ) -> AppResult<CreatorApiKeyIdentity> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                let row = sqlx::query(
                    r#"
                    SELECT id, creator_id, scopes_json
                    FROM creator_api_keys
                    WHERE key_hash = ? AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > ?)
                    "#,
                )
                .bind(key_hash)
                .bind(now)
                .fetch_optional(pool)
                .await?
                .ok_or(AppError::Unauthorized)?;
                Ok(CreatorApiKeyIdentity {
                    key_id: row.get("id"),
                    creator_id: row.get("creator_id"),
                    scopes: serde_json::from_str(&row.get::<String, _>("scopes_json"))?,
                })
            }
            DatabaseProvider::Postgres(pool) => {
                let row = sqlx::query(
                    r#"
                    SELECT id, creator_id, scopes_json
                    FROM creator_api_keys
                    WHERE key_hash = $1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > $2)
                    "#,
                )
                .bind(key_hash)
                .bind(now)
                .fetch_optional(pool)
                .await?
                .ok_or(AppError::Unauthorized)?;
                Ok(CreatorApiKeyIdentity {
                    key_id: row.get("id"),
                    creator_id: row.get("creator_id"),
                    scopes: serde_json::from_str(&row.get::<String, _>("scopes_json"))?,
                })
            }
        }
    }

    pub async fn touch_creator_api_key(&self, id: &str, now: &str) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query("UPDATE creator_api_keys SET last_used_at = ? WHERE id = ?")
                    .bind(now)
                    .bind(id)
                    .execute(pool)
                    .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query("UPDATE creator_api_keys SET last_used_at = $1 WHERE id = $2")
                    .bind(now)
                    .bind(id)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn get_creator_profile_for_api(&self, creator_id: &str) -> AppResult<CreatorProfile> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                let row = sqlx::query(creator_profile_select("id = ?").as_str())
                    .bind(creator_id)
                    .fetch_optional(pool)
                    .await?
                    .ok_or(AppError::NotFound)?;
                creator_profile_from_sqlite_row(&row)
            }
            DatabaseProvider::Postgres(pool) => {
                let row = sqlx::query(creator_profile_select("id = $1").as_str())
                    .bind(creator_id)
                    .fetch_optional(pool)
                    .await?
                    .ok_or(AppError::NotFound)?;
                creator_profile_from_postgres_row(&row)
            }
        }
    }

    pub async fn update_creator_profile_for_api(
        &self,
        creator_id: &str,
        profile: CreatorApiProfileUpdate,
    ) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE creator_profiles SET display_name = ?, avatar = ?, banner = ?, tagline = ?, bio = ? WHERE id = ?",
                )
                .bind(profile.display_name)
                .bind(profile.avatar)
                .bind(profile.banner)
                .bind(profile.tagline)
                .bind(profile.bio)
                .bind(creator_id)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    r#"
                    UPDATE creator_profiles
                    SET display_name = $1, avatar = $2, banner = $3, tagline = $4, bio = $5
                    WHERE id = $6
                    "#,
                )
                .bind(profile.display_name)
                .bind(profile.avatar)
                .bind(profile.banner)
                .bind(profile.tagline)
                .bind(profile.bio)
                .bind(creator_id)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn update_creator_stream_key(
        &self,
        creator_id: &str,
        stream_key: &str,
    ) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query("UPDATE creator_profiles SET stream_key = ? WHERE id = ?")
                    .bind(stream_key)
                    .bind(creator_id)
                    .execute(pool)
                    .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query("UPDATE creator_profiles SET stream_key = $1 WHERE id = $2")
                    .bind(stream_key)
                    .bind(creator_id)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn update_creator_live_defaults_for_api(
        &self,
        creator_id: &str,
        category: &str,
        tags_json: &str,
        title: Option<String>,
        is_mature: Option<bool>,
        current_broadcast_id: Option<&str>,
    ) -> AppResult<()> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE creator_profiles SET default_category = ?, default_tags_json = ? WHERE id = ?",
                )
                .bind(category)
                .bind(tags_json)
                .bind(creator_id)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    "UPDATE creator_profiles SET default_category = $1, default_tags_json = $2 WHERE id = $3",
                )
                .bind(category)
                .bind(tags_json)
                .bind(creator_id)
                .execute(pool)
                .await?;
                if let Some(current_id) = current_broadcast_id {
                    sqlx::query(
                        "UPDATE broadcasts SET title = COALESCE($1, title), category = $2, tags_json = $3, is_mature = COALESCE($4, is_mature) WHERE id = $5 AND creator_id = $6",
                    )
                    .bind(title)
                    .bind(category)
                    .bind(tags_json)
                    .bind(is_mature)
                    .bind(current_id)
                    .bind(creator_id)
                    .execute(pool)
                    .await?;
                }
            }
        }
        Ok(())
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

fn normalize_optional_text(value: Option<&str>, max_len: usize) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(max_len).collect())
}

fn normalize_required_text(value: &str, max_len: usize, field: &str) -> AppResult<String> {
    normalize_optional_text(Some(value), max_len)
        .ok_or_else(|| AppError::BadRequest(format!("{field} is required")))
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
            if ch.is_ascii_alphanumeric() {
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

fn build_postgres_prefix_query(tokens: &[String]) -> Option<String> {
    let parts = tokens
        .iter()
        .filter(|token| token.chars().all(|ch| ch.is_ascii_alphanumeric()))
        .map(|token| format!("{token}:*"))
        .collect::<Vec<_>>();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" & "))
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

fn creator_api_key_select(where_clause: &str) -> String {
    format!(
        r#"
        SELECT id, name, key_prefix, access_token, scopes_json, created_at, last_used_at, expires_at, revoked_at
        FROM creator_api_keys
        WHERE {where_clause}
        ORDER BY created_at DESC
        "#
    )
}

fn creator_api_key_from_sqlite_row(row: &SqliteRow) -> AppResult<CreatorApiKey> {
    Ok(CreatorApiKey {
        id: row.get("id"),
        name: row.get("name"),
        key_prefix: row.get("key_prefix"),
        access_token: row.get("access_token"),
        scopes: serde_json::from_str(&row.get::<String, _>("scopes_json"))?,
        created_at: row.get("created_at"),
        last_used_at: row.get("last_used_at"),
        expires_at: row.get("expires_at"),
        revoked_at: row.get("revoked_at"),
    })
}

fn creator_api_key_from_postgres_row(row: &PgRow) -> AppResult<CreatorApiKey> {
    Ok(CreatorApiKey {
        id: row.get("id"),
        name: row.get("name"),
        key_prefix: row.get("key_prefix"),
        access_token: row.get("access_token"),
        scopes: serde_json::from_str(&row.get::<String, _>("scopes_json"))?,
        created_at: row.get("created_at"),
        last_used_at: row.get("last_used_at"),
        expires_at: row.get("expires_at"),
        revoked_at: row.get("revoked_at"),
    })
}

fn creator_profile_select(where_clause: &str) -> String {
    format!(
        r#"
        SELECT id, user_id, handle, display_name, avatar, banner, tagline, bio,
               partner_status, joined_at, stream_key, rtmp_url, default_category,
               default_tags_json, followers, subscribers, monthly_viewers,
               total_watch_hours, live_status, current_broadcast_id
        FROM creator_profiles
        WHERE {where_clause}
        "#
    )
}

fn creator_profile_from_sqlite_row(row: &SqliteRow) -> AppResult<CreatorProfile> {
    Ok(CreatorProfile {
        id: row.get("id"),
        user_id: row.get("user_id"),
        handle: row.get("handle"),
        display_name: row.get("display_name"),
        avatar: row.get("avatar"),
        banner: row.get("banner"),
        tagline: row.get("tagline"),
        bio: row.get("bio"),
        partner_status: row.get("partner_status"),
        joined_at: row.get("joined_at"),
        stream_key: row.get("stream_key"),
        rtmp_url: row.get("rtmp_url"),
        default_category: row.get("default_category"),
        default_tags: serde_json::from_str(&row.get::<String, _>("default_tags_json"))?,
        followers: row.get("followers"),
        subscribers: row.get("subscribers"),
        monthly_viewers: row.get("monthly_viewers"),
        total_watch_hours: row.get("total_watch_hours"),
        live_status: row.get("live_status"),
        current_broadcast_id: row.get("current_broadcast_id"),
    })
}

fn creator_profile_from_postgres_row(row: &PgRow) -> AppResult<CreatorProfile> {
    Ok(CreatorProfile {
        id: row.get("id"),
        user_id: row.get("user_id"),
        handle: row.get("handle"),
        display_name: row.get("display_name"),
        avatar: row.get("avatar"),
        banner: row.get("banner"),
        tagline: row.get("tagline"),
        bio: row.get("bio"),
        partner_status: row.get("partner_status"),
        joined_at: row.get("joined_at"),
        stream_key: row.get("stream_key"),
        rtmp_url: row.get("rtmp_url"),
        default_category: row.get("default_category"),
        default_tags: serde_json::from_str(&row.get::<String, _>("default_tags_json"))?,
        followers: row.get("followers"),
        subscribers: row.get("subscribers"),
        monthly_viewers: row.get("monthly_viewers"),
        total_watch_hours: row.get("total_watch_hours"),
        live_status: row.get("live_status"),
        current_broadcast_id: row.get("current_broadcast_id"),
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

pub fn advertiser_permissions_for_role(role: &str) -> Option<Vec<String>> {
    let permissions = match role {
        "admin" => [
            "manage_account",
            "manage_team",
            "manage_billing",
            "buy_media",
            "approve_work",
            "view_reports",
        ]
        .as_slice(),
        "buyer" => ["buy_media", "approve_work", "view_reports"].as_slice(),
        "analyst" => ["view_reports"].as_slice(),
        "reviewer" => ["approve_work"].as_slice(),
        _ => return None,
    };
    Some(
        permissions
            .iter()
            .map(|permission| permission.to_string())
            .collect(),
    )
}

pub fn advertiser_permission_presets() -> Vec<AdvertiserPermissionPreset> {
    [
        ("admin", "Admin"),
        ("buyer", "Buyer"),
        ("analyst", "Analyst"),
        ("reviewer", "Reviewer"),
    ]
    .into_iter()
    .filter_map(|(role, label)| {
        advertiser_permissions_for_role(role).map(|permissions| AdvertiserPermissionPreset {
            role: role.to_string(),
            label: label.to_string(),
            permissions,
        })
    })
    .collect()
}

impl Database {
    pub async fn fetch_advertiser_account_for_auth_user(
        &self,
        auth_user_id: &str,
    ) -> AppResult<AdvertiserAccountResponse> {
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                fetch_sqlite_advertiser_account_for_auth_user(pool, auth_user_id).await
            }
            DatabaseProvider::Postgres(pool) => {
                fetch_postgres_advertiser_account_for_auth_user(pool, auth_user_id).await
            }
        }
    }

    pub async fn update_advertiser_company_for_auth_user(
        &self,
        auth_user_id: &str,
        input: &UpdateAdvertiserCompanyRequest,
        now: &str,
    ) -> AppResult<AdvertiserAccountResponse> {
        let account = self
            .fetch_advertiser_account_for_auth_user(auth_user_id)
            .await?;
        require_advertiser_permission(&account.current_seat, "manage_account")?;
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    r#"
                    UPDATE ad_marketplace_advertisers
                    SET name = ?, industry = ?, website_url = ?, updated_at = ?
                    WHERE id = ?
                    "#,
                )
                .bind(input.name.trim())
                .bind(input.industry.trim())
                .bind(
                    input
                        .website_url
                        .as_deref()
                        .filter(|value| !value.trim().is_empty()),
                )
                .bind(now)
                .bind(&account.company.id)
                .execute(pool)
                .await?;
                sqlx::query(
                    r#"
                    INSERT INTO advertiser_billing_profiles (
                        advertiser_id, billing_name, billing_email, status, created_at, updated_at
                    ) VALUES (?, ?, ?, 'active', ?, ?)
                    ON CONFLICT(advertiser_id) DO UPDATE SET
                        billing_name = excluded.billing_name,
                        billing_email = excluded.billing_email,
                        updated_at = excluded.updated_at
                    "#,
                )
                .bind(&account.company.id)
                .bind(input.billing_name.trim())
                .bind(input.billing_email.trim())
                .bind(now)
                .bind(now)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    r#"
                    UPDATE ad_marketplace_advertisers
                    SET name = $1, industry = $2, website_url = $3, updated_at = $4
                    WHERE id = $5
                    "#,
                )
                .bind(input.name.trim())
                .bind(input.industry.trim())
                .bind(
                    input
                        .website_url
                        .as_deref()
                        .filter(|value| !value.trim().is_empty()),
                )
                .bind(now)
                .bind(&account.company.id)
                .execute(pool)
                .await?;
                sqlx::query(
                    r#"
                    INSERT INTO advertiser_billing_profiles (
                        advertiser_id, billing_name, billing_email, status, created_at, updated_at
                    ) VALUES ($1, $2, $3, 'active', $4, $5)
                    ON CONFLICT(advertiser_id) DO UPDATE SET
                        billing_name = excluded.billing_name,
                        billing_email = excluded.billing_email,
                        updated_at = excluded.updated_at
                    "#,
                )
                .bind(&account.company.id)
                .bind(input.billing_name.trim())
                .bind(input.billing_email.trim())
                .bind(now)
                .bind(now)
                .execute(pool)
                .await?;
            }
        }
        self.fetch_advertiser_account_for_auth_user(auth_user_id)
            .await
    }

    pub async fn create_advertiser_invite_for_auth_user(
        &self,
        auth_user_id: &str,
        invite_id: &str,
        email: &str,
        role: &str,
        token_hash: &str,
        now: &str,
        expires_at: &str,
    ) -> AppResult<AdvertiserAccountResponse> {
        let permissions = advertiser_permissions_for_role(role)
            .ok_or_else(|| AppError::BadRequest(format!("unsupported advertiser role `{role}`")))?;
        let permissions_json = serde_json::to_string(&permissions)?;
        let account = self
            .fetch_advertiser_account_for_auth_user(auth_user_id)
            .await?;
        require_advertiser_permission(&account.current_seat, "manage_team")?;
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO advertiser_invites (
                        id, advertiser_id, email, role, permissions_json, status,
                        invited_by_user_id, token_hash, created_at, expires_at
                    ) VALUES (?, ?, ?, ?, ?, 'pending', ?, ?, ?, ?)
                    "#,
                )
                .bind(invite_id)
                .bind(&account.company.id)
                .bind(email.trim().to_lowercase())
                .bind(role)
                .bind(&permissions_json)
                .bind(&account.current_seat.user_id)
                .bind(token_hash)
                .bind(now)
                .bind(expires_at)
                .execute(pool)
                .await?;
            }
            DatabaseProvider::Postgres(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO advertiser_invites (
                        id, advertiser_id, email, role, permissions_json, status,
                        invited_by_user_id, token_hash, created_at, expires_at
                    ) VALUES ($1, $2, $3, $4, $5, 'pending', $6, $7, $8, $9)
                    "#,
                )
                .bind(invite_id)
                .bind(&account.company.id)
                .bind(email.trim().to_lowercase())
                .bind(role)
                .bind(&permissions_json)
                .bind(&account.current_seat.user_id)
                .bind(token_hash)
                .bind(now)
                .bind(expires_at)
                .execute(pool)
                .await?;
            }
        }
        self.fetch_advertiser_account_for_auth_user(auth_user_id)
            .await
    }

    pub async fn update_advertiser_seat_for_auth_user(
        &self,
        auth_user_id: &str,
        target_user_id: &str,
        role: &str,
        status: Option<&str>,
        now: &str,
    ) -> AppResult<AdvertiserAccountResponse> {
        let permissions = advertiser_permissions_for_role(role)
            .ok_or_else(|| AppError::BadRequest(format!("unsupported advertiser role `{role}`")))?;
        let permissions_json = serde_json::to_string(&permissions)?;
        let account = self
            .fetch_advertiser_account_for_auth_user(auth_user_id)
            .await?;
        require_advertiser_permission(&account.current_seat, "manage_team")?;
        let status = status.unwrap_or("active");
        match &self.provider {
            DatabaseProvider::Sqlite(pool) => {
                let result = sqlx::query(
                    r#"
                    UPDATE advertiser_memberships
                    SET role = ?, permissions_json = ?, status = ?, updated_at = ?
                    WHERE advertiser_id = ? AND user_id = ?
                    "#,
                )
                .bind(role)
                .bind(&permissions_json)
                .bind(status)
                .bind(now)
                .bind(&account.company.id)
                .bind(target_user_id)
                .execute(pool)
                .await?;
                if result.rows_affected() == 0 {
                    return Err(AppError::NotFound);
                }
            }
            DatabaseProvider::Postgres(pool) => {
                let result = sqlx::query(
                    r#"
                    UPDATE advertiser_memberships
                    SET role = $1, permissions_json = $2, status = $3, updated_at = $4
                    WHERE advertiser_id = $5 AND user_id = $6
                    "#,
                )
                .bind(role)
                .bind(&permissions_json)
                .bind(status)
                .bind(now)
                .bind(&account.company.id)
                .bind(target_user_id)
                .execute(pool)
                .await?;
                if result.rows_affected() == 0 {
                    return Err(AppError::NotFound);
                }
            }
        }
        self.fetch_advertiser_account_for_auth_user(auth_user_id)
            .await
    }
}

fn require_advertiser_permission(seat: &AdvertiserSeat, permission: &str) -> AppResult<()> {
    if seat.permissions.iter().any(|value| value == permission) {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

async fn fetch_sqlite_advertiser_account_for_auth_user(
    pool: &SqlitePool,
    auth_user_id: &str,
) -> AppResult<AdvertiserAccountResponse> {
    let current = sqlx::query(
        r#"
        SELECT au.id AS user_id, au.email, au.name, am.role, am.permissions_json, am.status,
               am.created_at, am.updated_at, adv.id AS advertiser_id
        FROM advertiser_users au
        JOIN advertiser_memberships am ON am.user_id = au.id
        JOIN ad_marketplace_advertisers adv ON adv.id = am.advertiser_id
        WHERE au.auth_user_id = ? AND au.status != 'disabled' AND am.status = 'active'
        ORDER BY am.created_at ASC
        LIMIT 1
        "#,
    )
    .bind(auth_user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Forbidden)?;
    let advertiser_id: String = current.get("advertiser_id");
    let current_seat = sqlite_advertiser_seat_from_row(&current)?;
    let company = fetch_sqlite_advertiser_company(pool, &advertiser_id).await?;
    let seats = fetch_sqlite_advertiser_seats(pool, &advertiser_id).await?;
    let invites = fetch_sqlite_advertiser_invites(pool, &advertiser_id).await?;
    Ok(AdvertiserAccountResponse {
        company,
        current_seat,
        seats,
        invites,
        permission_presets: advertiser_permission_presets(),
    })
}

async fn fetch_postgres_advertiser_account_for_auth_user(
    pool: &PgPool,
    auth_user_id: &str,
) -> AppResult<AdvertiserAccountResponse> {
    let current = sqlx::query(
        r#"
        SELECT au.id AS user_id, au.email, au.name, am.role, am.permissions_json, am.status,
               am.created_at, am.updated_at, adv.id AS advertiser_id
        FROM advertiser_users au
        JOIN advertiser_memberships am ON am.user_id = au.id
        JOIN ad_marketplace_advertisers adv ON adv.id = am.advertiser_id
        WHERE au.auth_user_id = $1 AND au.status != 'disabled' AND am.status = 'active'
        ORDER BY am.created_at ASC
        LIMIT 1
        "#,
    )
    .bind(auth_user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Forbidden)?;
    let advertiser_id: String = current.get("advertiser_id");
    let current_seat = postgres_advertiser_seat_from_row(&current)?;
    let company = fetch_postgres_advertiser_company(pool, &advertiser_id).await?;
    let seats = fetch_postgres_advertiser_seats(pool, &advertiser_id).await?;
    let invites = fetch_postgres_advertiser_invites(pool, &advertiser_id).await?;
    Ok(AdvertiserAccountResponse {
        company,
        current_seat,
        seats,
        invites,
        permission_presets: advertiser_permission_presets(),
    })
}

async fn fetch_sqlite_advertiser_company(
    pool: &SqlitePool,
    advertiser_id: &str,
) -> AppResult<AdvertiserCompany> {
    let row = sqlx::query(
        r#"
        SELECT adv.id, adv.name, adv.industry, adv.website_url, adv.status,
               COALESCE(bp.billing_name, adv.name) AS billing_name,
               COALESCE(bp.billing_email, '') AS billing_email,
               COALESCE(bp.status, 'missing') AS billing_status
        FROM ad_marketplace_advertisers adv
        LEFT JOIN advertiser_billing_profiles bp ON bp.advertiser_id = adv.id
        WHERE adv.id = ?
        "#,
    )
    .bind(advertiser_id)
    .fetch_one(pool)
    .await?;
    Ok(sqlite_advertiser_company_from_row(&row))
}

async fn fetch_postgres_advertiser_company(
    pool: &PgPool,
    advertiser_id: &str,
) -> AppResult<AdvertiserCompany> {
    let row = sqlx::query(
        r#"
        SELECT adv.id, adv.name, adv.industry, adv.website_url, adv.status,
               COALESCE(bp.billing_name, adv.name) AS billing_name,
               COALESCE(bp.billing_email, '') AS billing_email,
               COALESCE(bp.status, 'missing') AS billing_status
        FROM ad_marketplace_advertisers adv
        LEFT JOIN advertiser_billing_profiles bp ON bp.advertiser_id = adv.id
        WHERE adv.id = $1
        "#,
    )
    .bind(advertiser_id)
    .fetch_one(pool)
    .await?;
    Ok(postgres_advertiser_company_from_row(&row))
}

async fn fetch_sqlite_advertiser_seats(
    pool: &SqlitePool,
    advertiser_id: &str,
) -> AppResult<Vec<AdvertiserSeat>> {
    let rows = sqlx::query(
        r#"
        SELECT au.id AS user_id, au.email, au.name, am.role, am.permissions_json, am.status,
               am.created_at, am.updated_at
        FROM advertiser_memberships am
        JOIN advertiser_users au ON au.id = am.user_id
        WHERE am.advertiser_id = ?
        ORDER BY CASE am.role WHEN 'admin' THEN 0 WHEN 'buyer' THEN 1 WHEN 'analyst' THEN 2 ELSE 3 END, au.name ASC
        "#,
    )
    .bind(advertiser_id)
    .fetch_all(pool)
    .await?;
    rows.iter().map(sqlite_advertiser_seat_from_row).collect()
}

async fn fetch_postgres_advertiser_seats(
    pool: &PgPool,
    advertiser_id: &str,
) -> AppResult<Vec<AdvertiserSeat>> {
    let rows = sqlx::query(
        r#"
        SELECT au.id AS user_id, au.email, au.name, am.role, am.permissions_json, am.status,
               am.created_at, am.updated_at
        FROM advertiser_memberships am
        JOIN advertiser_users au ON au.id = am.user_id
        WHERE am.advertiser_id = $1
        ORDER BY CASE am.role WHEN 'admin' THEN 0 WHEN 'buyer' THEN 1 WHEN 'analyst' THEN 2 ELSE 3 END, au.name ASC
        "#,
    )
    .bind(advertiser_id)
    .fetch_all(pool)
    .await?;
    rows.iter().map(postgres_advertiser_seat_from_row).collect()
}

async fn fetch_sqlite_advertiser_invites(
    pool: &SqlitePool,
    advertiser_id: &str,
) -> AppResult<Vec<AdvertiserInvite>> {
    let rows = sqlx::query(
        r#"
        SELECT id, email, role, permissions_json, status, invited_by_user_id, created_at, expires_at
        FROM advertiser_invites
        WHERE advertiser_id = ? AND status = 'pending'
        ORDER BY created_at DESC
        "#,
    )
    .bind(advertiser_id)
    .fetch_all(pool)
    .await?;
    rows.iter().map(sqlite_advertiser_invite_from_row).collect()
}

async fn fetch_postgres_advertiser_invites(
    pool: &PgPool,
    advertiser_id: &str,
) -> AppResult<Vec<AdvertiserInvite>> {
    let rows = sqlx::query(
        r#"
        SELECT id, email, role, permissions_json, status, invited_by_user_id, created_at, expires_at
        FROM advertiser_invites
        WHERE advertiser_id = $1 AND status = 'pending'
        ORDER BY created_at DESC
        "#,
    )
    .bind(advertiser_id)
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(postgres_advertiser_invite_from_row)
        .collect()
}

fn permissions_from_json(value: String) -> AppResult<Vec<String>> {
    Ok(serde_json::from_str::<Vec<String>>(&value)?)
}

fn sqlite_advertiser_company_from_row(row: &SqliteRow) -> AdvertiserCompany {
    AdvertiserCompany {
        id: row.get("id"),
        name: row.get("name"),
        industry: row.get("industry"),
        website_url: row.get("website_url"),
        status: row.get("status"),
        billing_name: row.get("billing_name"),
        billing_email: row.get("billing_email"),
        billing_status: row.get("billing_status"),
    }
}

fn postgres_advertiser_company_from_row(row: &PgRow) -> AdvertiserCompany {
    AdvertiserCompany {
        id: row.get("id"),
        name: row.get("name"),
        industry: row.get("industry"),
        website_url: row.get("website_url"),
        status: row.get("status"),
        billing_name: row.get("billing_name"),
        billing_email: row.get("billing_email"),
        billing_status: row.get("billing_status"),
    }
}

fn sqlite_advertiser_seat_from_row(row: &SqliteRow) -> AppResult<AdvertiserSeat> {
    Ok(AdvertiserSeat {
        user_id: row.get("user_id"),
        email: row.get("email"),
        name: row.get("name"),
        role: row.get("role"),
        permissions: permissions_from_json(row.get("permissions_json"))?,
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn postgres_advertiser_seat_from_row(row: &PgRow) -> AppResult<AdvertiserSeat> {
    Ok(AdvertiserSeat {
        user_id: row.get("user_id"),
        email: row.get("email"),
        name: row.get("name"),
        role: row.get("role"),
        permissions: permissions_from_json(row.get("permissions_json"))?,
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn sqlite_advertiser_invite_from_row(row: &SqliteRow) -> AppResult<AdvertiserInvite> {
    Ok(AdvertiserInvite {
        id: row.get("id"),
        email: row.get("email"),
        role: row.get("role"),
        permissions: permissions_from_json(row.get("permissions_json"))?,
        status: row.get("status"),
        invited_by_user_id: row.get("invited_by_user_id"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
    })
}

fn postgres_advertiser_invite_from_row(row: &PgRow) -> AppResult<AdvertiserInvite> {
    Ok(AdvertiserInvite {
        id: row.get("id"),
        email: row.get("email"),
        role: row.get("role"),
        permissions: permissions_from_json(row.get("permissions_json"))?,
        status: row.get("status"),
        invited_by_user_id: row.get("invited_by_user_id"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
    })
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
                subtitle,
                body,
                image UNINDEXED,
                href UNINDEXED,
                metadata_json UNINDEXED,
                rank_boost UNINDEXED,
                popularity UNINDEXED,
                tokenize = 'unicode61 remove_diacritics 2'
            );
            INSERT INTO search_documents (
                entity_id, kind, slug, title, subtitle, body, image, href, metadata_json, rank_boost, popularity
            )
            VALUES
                ('ser-halcyon', 'series', 'halcyon-drift', 'Halcyon Drift', 'Sci-Fi', 'salvage crew sci-fi', '', '/series/halcyon-drift', '{}', 20, 90),
                ('film-paper', 'film', 'paper-moon', 'Paper Moon', 'Noir drama', 'noir drama', '', '/film/paper-moon', '{}', 10, 70);
            "#,
        )
        .execute(&pool)
        .await
        .expect("search schema");

        let database = Database::from_sqlite(pool);
        let hits = database
            .search_catalog_documents("halcyon", 24, 0)
            .await
            .expect("hits");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entity_id, "ser-halcyon");
        assert_eq!(hits[0].kind, "series");
        assert!(
            database
                .search_catalog_documents("   ", 24, 0)
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
    async fn sqlite_provider_records_viewer_events_for_anonymous_and_signed_in_viewers() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("sqlite pool");
        sqlx::raw_sql(include_str!("../migrations/0051_viewer_events.sql"))
            .execute(&pool)
            .await
            .expect("viewer event schema");
        sqlx::raw_sql(
            r#"
            CREATE TABLE users (
                id TEXT PRIMARY KEY
            );
            INSERT INTO users (id) VALUES ('usr-test');
            "#,
        )
        .execute(&pool)
        .await
        .expect("users schema");

        let database = Database::from_sqlite(pool.clone());
        database
            .record_viewer_event(
                "ve-anon",
                None,
                &ViewerEventInput {
                    visitor_id: "vis_test".to_string(),
                    event_type: "page_view".to_string(),
                    content_id: None,
                    content_kind: None,
                    episode_id: None,
                    stream_id: None,
                    session_id: None,
                    path: Some("/films".to_string()),
                    url: Some("https://streamvanta.tv/films".to_string()),
                    referrer_url: None,
                    landing_url: Some("https://streamvanta.tv/".to_string()),
                    initial_referrer_url: None,
                    utm_source: Some("newsletter".to_string()),
                    utm_medium: Some("email".to_string()),
                    utm_campaign: Some("launch".to_string()),
                    utm_term: None,
                    utm_content: None,
                    progress_sec: None,
                    duration_sec: None,
                    watch_time_ms: Some(-10),
                    metadata: Some(serde_json::json!({ "surface": "catalog" })),
                    occurred_at: Some("2026-08-24T12:00:00Z".to_string()),
                },
                "2026-08-24T12:00:01Z",
            )
            .await
            .expect("anonymous event");
        database
            .record_viewer_event(
                "ve-user",
                Some("usr-test"),
                &ViewerEventInput {
                    visitor_id: "vis_test".to_string(),
                    event_type: "playback_progress".to_string(),
                    content_id: Some("film-test".to_string()),
                    content_kind: Some("film".to_string()),
                    episode_id: None,
                    stream_id: None,
                    session_id: Some("pbs-test".to_string()),
                    path: Some("/watch/film/film-test".to_string()),
                    url: None,
                    referrer_url: None,
                    landing_url: None,
                    initial_referrer_url: None,
                    utm_source: None,
                    utm_medium: None,
                    utm_campaign: None,
                    utm_term: None,
                    utm_content: None,
                    progress_sec: Some(42),
                    duration_sec: Some(120),
                    watch_time_ms: Some(1000),
                    metadata: None,
                    occurred_at: None,
                },
                "2026-08-24T12:01:00Z",
            )
            .await
            .expect("signed event");

        let rows = sqlx::query(
            r#"
            SELECT id, user_id, event_type, watch_time_ms, metadata_json
            FROM viewer_events
            ORDER BY id
            "#,
        )
        .fetch_all(&pool)
        .await
        .expect("event rows");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get::<String, _>("id"), "ve-anon");
        assert!(rows[0].get::<Option<String>, _>("user_id").is_none());
        assert_eq!(rows[0].get::<i64, _>("watch_time_ms"), 0);
        assert_eq!(
            rows[0].get::<String, _>("metadata_json"),
            r#"{"surface":"catalog"}"#
        );
        assert_eq!(rows[1].get::<String, _>("event_type"), "playback_progress");
        assert_eq!(
            rows[1].get::<Option<String>, _>("user_id"),
            Some("usr-test".to_string())
        );
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
