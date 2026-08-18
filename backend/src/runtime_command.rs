use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{auth::hash_token, error::AppError};

pub(crate) enum RuntimeCommand {
    Serve,
    ProvisionUser(ProvisionUserCommand),
    ProvisionCreator(ProvisionCreatorCommand),
    IssueSession(IssueSessionCommand),
}

pub(crate) struct ProvisionUserCommand {
    pub(crate) user_id: String,
    pub(crate) handle: String,
    pub(crate) display_name: String,
    pub(crate) avatar_url: String,
    pub(crate) tier: String,
}

pub(crate) struct ProvisionCreatorCommand {
    pub(crate) creator_id: String,
    pub(crate) user_id: String,
    pub(crate) handle: String,
    pub(crate) display_name: String,
    pub(crate) avatar_url: String,
    pub(crate) banner_url: String,
    pub(crate) tagline: String,
    pub(crate) bio: String,
    pub(crate) partner_status: String,
    pub(crate) stream_key: String,
    pub(crate) rtmp_url: String,
    pub(crate) default_category: String,
    pub(crate) default_tags: Vec<String>,
}

pub(crate) struct IssueSessionCommand {
    pub(crate) user_id: String,
    pub(crate) label: String,
    pub(crate) scopes: Vec<String>,
    pub(crate) expires_in_days: Option<i64>,
}

impl RuntimeCommand {
    pub(crate) fn from_args(
        mut args: impl Iterator<Item = String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let Some(command) = args.next() else {
            return Ok(Self::Serve);
        };

        match command.as_str() {
            "serve" => Ok(Self::Serve),
            "provision-user" => Ok(Self::ProvisionUser(ProvisionUserCommand::from_args(args)?)),
            "provision-creator" => Ok(Self::ProvisionCreator(
                ProvisionCreatorCommand::from_args(args)?,
            )),
            "issue-session" => Ok(Self::IssueSession(IssueSessionCommand::from_args(args)?)),
            flag => Err(format!(
                "unknown command `{flag}`; supported commands: `serve`, `provision-user`, `provision-creator`, `issue-session`"
            )
            .into()),
        }
    }
}

impl ProvisionUserCommand {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Self, Box<dyn std::error::Error>> {
        let options = parse_options(args)?;
        Ok(Self {
            user_id: required_option(&options, "--user-id")?,
            handle: required_option(&options, "--handle")?,
            display_name: required_option(&options, "--display-name")?,
            avatar_url: option_with_default(
                &options,
                "--avatar-url",
                "https://cdn.lifestream.local/avatar/default.jpg",
            ),
            tier: option_with_default(&options, "--tier", "free"),
        })
    }

    pub(crate) async fn execute(self, pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
        if self.user_id.trim().is_empty() || self.handle.trim().is_empty() {
            return Err(AppError::BadRequest("user id and handle are required".to_string()).into());
        }
        if self.display_name.trim().is_empty() {
            return Err(AppError::BadRequest("display name is required".to_string()).into());
        }

        let now = Utc::now().to_rfc3339();
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
        .bind(self.user_id.trim())
        .bind(self.handle.trim())
        .bind(self.display_name.trim())
        .bind(self.avatar_url.trim())
        .bind(self.tier.trim())
        .bind(&now)
        .execute(pool)
        .await?;

        println!(
            "provisioned user {}\nhandle: {}\ndisplay_name: {}",
            self.user_id.trim(),
            self.handle.trim(),
            self.display_name.trim()
        );
        Ok(())
    }
}

impl ProvisionCreatorCommand {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Self, Box<dyn std::error::Error>> {
        let options = parse_options(args)?;
        let handle = required_option(&options, "--handle")?;

        Ok(Self {
            creator_id: required_option(&options, "--creator-id")?,
            user_id: required_option(&options, "--user-id")?,
            display_name: required_option(&options, "--display-name")?,
            avatar_url: option_with_default(
                &options,
                "--avatar-url",
                &format!("https://cdn.lifestream.local/avatar/{handle}.jpg"),
            ),
            banner_url: option_with_default(
                &options,
                "--banner-url",
                &format!("https://cdn.lifestream.local/banner/{handle}.jpg"),
            ),
            tagline: option_with_default(&options, "--tagline", "Live now on Lifestream"),
            bio: option_with_default(&options, "--bio", "Lifestream creator"),
            partner_status: option_with_default(&options, "--partner-status", "affiliate"),
            stream_key: option_with_default(
                &options,
                "--stream-key",
                &format!("sk_{handle}_{}", Uuid::new_v4().simple()),
            ),
            rtmp_url: option_with_default(
                &options,
                "--rtmp-url",
                "rtmp://ingest.lifestream.local/live",
            ),
            default_category: option_with_default(&options, "--default-category", "Gaming"),
            default_tags: parse_csv_option(
                options.get("--default-tags").map(String::as_str),
                &["live".to_string()],
            ),
            handle,
        })
    }

    pub(crate) async fn execute(self, pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
        ensure_user_exists(pool, &self.user_id).await?;

        if self.creator_id.trim().is_empty() || self.handle.trim().is_empty() {
            return Err(
                AppError::BadRequest("creator id and handle are required".to_string()).into(),
            );
        }
        if self.display_name.trim().is_empty() {
            return Err(AppError::BadRequest("display name is required".to_string()).into());
        }

        let now = Utc::now().to_rfc3339();
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
        .bind(self.creator_id.trim())
        .bind(self.user_id.trim())
        .bind(self.handle.trim())
        .bind(self.display_name.trim())
        .bind(self.avatar_url.trim())
        .bind(self.banner_url.trim())
        .bind(self.tagline.trim())
        .bind(self.bio.trim())
        .bind(self.partner_status.trim())
        .bind(&now)
        .bind(self.stream_key.trim())
        .bind(self.rtmp_url.trim())
        .bind(self.default_category.trim())
        .bind(serde_json::to_string(&self.default_tags)?)
        .execute(pool)
        .await?;

        println!(
            "provisioned creator {}\nuser_id: {}\nhandle: {}",
            self.creator_id.trim(),
            self.user_id.trim(),
            self.handle.trim()
        );
        Ok(())
    }
}

impl IssueSessionCommand {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Self, Box<dyn std::error::Error>> {
        let options = parse_options(args)?;
        let scopes = parse_csv_option(
            options.get("--scopes").map(String::as_str),
            &["viewer".to_string()],
        );
        let expires_in_days = match options.get("--expires-in-days") {
            Some(value) => Some(value.parse::<i64>().map_err(|_| {
                AppError::BadRequest("expires-in-days must be an integer".to_string())
            })?),
            None => None,
        };

        Ok(Self {
            user_id: required_option(&options, "--user-id")?,
            label: required_option(&options, "--label")?,
            scopes,
            expires_in_days,
        })
    }

    pub(crate) async fn execute(self, pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
        ensure_user_exists(pool, &self.user_id).await?;

        let label = self.label.trim();
        if label.is_empty() {
            return Err(AppError::BadRequest("label is required".to_string()).into());
        }
        if label.len() > 64 {
            return Err(
                AppError::BadRequest("label must be 64 characters or fewer".to_string()).into(),
            );
        }
        if self.scopes.is_empty() {
            return Err(AppError::BadRequest("at least one scope is required".to_string()).into());
        }
        if let Some(days) = self.expires_in_days {
            if !(1..=365).contains(&days) {
                return Err(AppError::BadRequest(
                    "expires-in-days must be between 1 and 365".to_string(),
                )
                .into());
            }
        }

        let session_id = Uuid::new_v4().to_string();
        let access_token = format!(
            "lst_{}_{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        );
        let created_at = Utc::now().to_rfc3339();
        let expires_at = self
            .expires_in_days
            .map(|days| (Utc::now() + chrono::Duration::days(days)).to_rfc3339());

        sqlx::query(
            r#"
            INSERT INTO auth_sessions (
                id, user_id, label, token_hash, scopes_json, created_at, expires_at, revoked_at, last_used_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, NULL, NULL)
            "#,
        )
        .bind(&session_id)
        .bind(self.user_id.trim())
        .bind(label)
        .bind(hash_token(&access_token))
        .bind(serde_json::to_string(&self.scopes)?)
        .bind(&created_at)
        .bind(&expires_at)
        .execute(pool)
        .await?;

        println!("session_id: {session_id}");
        println!("user_id: {}", self.user_id.trim());
        println!("scopes: {}", self.scopes.join(","));
        println!("expires_at: {}", expires_at.as_deref().unwrap_or("never"));
        println!("access_token: {access_token}");
        Ok(())
    }
}

async fn ensure_user_exists(pool: &SqlitePool, user_id: &str) -> Result<(), AppError> {
    let exists = sqlx::query("SELECT 1 FROM users WHERE id = ? LIMIT 1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .is_some();

    if !exists {
        return Err(AppError::BadRequest(format!(
            "user `{user_id}` does not exist; run `provision-user` first"
        )));
    }

    Ok(())
}

fn parse_options(
    mut args: impl Iterator<Item = String>,
) -> Result<std::collections::BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let mut options = std::collections::BTreeMap::new();

    while let Some(flag) = args.next() {
        if !flag.starts_with("--") {
            return Err(format!("unexpected positional argument `{flag}`").into());
        }
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for `{flag}`"))?;
        options.insert(flag, value);
    }

    Ok(options)
}

fn required_option(
    options: &std::collections::BTreeMap<String, String>,
    key: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    options
        .get(key)
        .cloned()
        .ok_or_else(|| format!("missing required option `{key}`").into())
}

fn option_with_default(
    options: &std::collections::BTreeMap<String, String>,
    key: &str,
    default: &str,
) -> String {
    options
        .get(key)
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn parse_csv_option(raw: Option<&str>, default: &[String]) -> Vec<String> {
    let parsed = raw
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if parsed.is_empty() {
        default.to_vec()
    } else {
        parsed
    }
}
