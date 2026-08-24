use std::{env, fmt, net::SocketAddr, path::PathBuf, str::FromStr};

#[derive(Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_kind: DatabaseKind,
    pub database_url: String,
    pub max_db_connections: u32,
    pub storage_kind: StorageKind,
    pub media_root: PathBuf,
    pub media_scratch_root: PathBuf,
    pub object_storage_bucket: Option<String>,
    pub object_storage_cdn_base_url: Option<String>,
    pub cdn_cookie_domain: Option<String>,
    pub admin_api_enabled: bool,
    pub token_hash_secret: Option<String>,
    pub allowed_origins: Vec<String>,
    pub environment: RuntimeEnvironment,
}

impl fmt::Debug for Config {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Config")
            .field("bind_addr", &self.bind_addr)
            .field("database_kind", &self.database_kind)
            .field("database_url", &self.database_url)
            .field("max_db_connections", &self.max_db_connections)
            .field("storage_kind", &self.storage_kind)
            .field("media_root", &self.media_root)
            .field("media_scratch_root", &self.media_scratch_root)
            .field("object_storage_bucket", &self.object_storage_bucket)
            .field(
                "object_storage_cdn_base_url",
                &self.object_storage_cdn_base_url,
            )
            .field("cdn_cookie_domain", &self.cdn_cookie_domain)
            .field("admin_api_enabled", &self.admin_api_enabled)
            .field(
                "token_hash_secret",
                &self.token_hash_secret.as_ref().map(|_| "<redacted>"),
            )
            .field("allowed_origins", &self.allowed_origins)
            .field("environment", &self.environment)
            .finish()
    }
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let bind_addr = runtime_bind_addr()?.parse()?;

        let backend_root = detect_backend_root()?;
        let environment = parse_env("VANTA_ENV", RuntimeEnvironment::Development)?;
        let database_kind = parse_env("VANTA_DATABASE_KIND", DatabaseKind::Sqlite)?;
        let storage_kind = parse_env("VANTA_STORAGE_KIND", StorageKind::Local)?;
        let database_url = database_url_from_env(database_kind, &backend_root);

        let max_db_connections = env::var("VANTA_DB_MAX_CONNECTIONS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(32);

        let media_root = env::var("VANTA_MEDIA_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| backend_root.join("media"));
        let config = Self {
            bind_addr,
            database_kind,
            database_url,
            max_db_connections,
            storage_kind,
            media_scratch_root: env::var("VANTA_MEDIA_SCRATCH_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|_| backend_root.join("media-scratch")),
            media_root,
            object_storage_bucket: env::var("VANTA_OBJECT_STORAGE_BUCKET").ok(),
            object_storage_cdn_base_url: env::var("VANTA_OBJECT_STORAGE_CDN_BASE_URL").ok(),
            cdn_cookie_domain: env::var("VANTA_CDN_COOKIE_DOMAIN").ok(),
            admin_api_enabled: parse_bool_env("VANTA_ADMIN_API_ENABLED")
                .unwrap_or(environment != RuntimeEnvironment::Production),
            token_hash_secret: env::var("VANTA_TOKEN_HASH_SECRET").ok(),
            allowed_origins: parse_allowed_origins(),
            environment,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.environment == RuntimeEnvironment::Production {
            if self.database_kind != DatabaseKind::Postgres {
                return Err(ConfigError::new(
                    "production requires VANTA_DATABASE_KIND=postgres",
                ));
            }
            if self.storage_kind != StorageKind::Object {
                return Err(ConfigError::new(
                    "production requires VANTA_STORAGE_KIND=object",
                ));
            }
            if self.allowed_origins.iter().any(|origin| {
                origin.starts_with("http://localhost") || origin.starts_with("http://127.0.0.1")
            }) {
                return Err(ConfigError::new(
                    "production requires explicit non-local VANTA_ALLOWED_ORIGINS",
                ));
            }
            if self.token_hash_secret.as_deref().unwrap_or("").trim().len() < 32 {
                return Err(ConfigError::new(
                    "production requires VANTA_TOKEN_HASH_SECRET with at least 32 characters",
                ));
            }
        }
        match self.database_kind {
            DatabaseKind::Sqlite if !self.database_url.starts_with("sqlite:") => Err(
                ConfigError::new("sqlite database kind requires a sqlite:// URL"),
            ),
            DatabaseKind::Postgres
                if !self.database_url.starts_with("postgres://")
                    && !self.database_url.starts_with("postgresql://") =>
            {
                Err(ConfigError::new(
                    "postgres database kind requires VANTA_DATABASE_URL or VANTA_POSTGRES_URL",
                ))
            }
            _ => Ok(()),
        }?;
        if self.storage_kind == StorageKind::Object {
            if self
                .object_storage_bucket
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
            {
                return Err(ConfigError::new(
                    "object storage requires VANTA_OBJECT_STORAGE_BUCKET",
                ));
            }
            if self
                .object_storage_cdn_base_url
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
            {
                return Err(ConfigError::new(
                    "object storage requires VANTA_OBJECT_STORAGE_CDN_BASE_URL",
                ));
            }
            if self
                .cdn_cookie_domain
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
            {
                return Err(ConfigError::new(
                    "object storage requires VANTA_CDN_COOKIE_DOMAIN",
                ));
            }
        }
        Ok(())
    }
}

fn runtime_bind_addr() -> Result<String, ConfigError> {
    if let Ok(bind) = env::var("VANTA_BIND") {
        if !bind.trim().is_empty() {
            return Ok(bind);
        }
    }

    if let Ok(port) = env::var("PORT") {
        let port = port.trim();
        if !port.is_empty() {
            return Ok(format!("0.0.0.0:{port}"));
        }
    }

    Ok("127.0.0.1:8080".to_string())
}

fn database_url_from_env(database_kind: DatabaseKind, backend_root: &PathBuf) -> String {
    env::var("VANTA_DATABASE_URL")
        .or_else(|_| env::var(database_kind.url_env_name()))
        .or_else(|_| match database_kind {
            DatabaseKind::Postgres => env::var("DATABASE_URL"),
            DatabaseKind::Sqlite => Err(env::VarError::NotPresent),
        })
        .unwrap_or_else(|_| default_database_url(backend_root))
}

fn parse_bool_env(name: &str) -> Option<bool> {
    env::var(name)
        .ok()
        .and_then(|value| parse_bool_env_value(&value))
}

fn parse_bool_env_value(value: &str) -> Option<bool> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeEnvironment {
    Development,
    Production,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatabaseKind {
    Sqlite,
    Postgres,
}

impl DatabaseKind {
    pub fn url_env_name(self) -> &'static str {
        match self {
            Self::Sqlite => "VANTA_SQLITE_URL",
            Self::Postgres => "VANTA_POSTGRES_URL",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageKind {
    Local,
    Object,
}

#[derive(Debug)]
pub struct ConfigError(String);

impl ConfigError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

fn parse_env<T>(name: &str, default: T) -> Result<T, ConfigError>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => value
            .parse()
            .map_err(|error| ConfigError::new(format!("invalid {name}: {error}"))),
        _ => Ok(default),
    }
}

impl FromStr for RuntimeEnvironment {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dev" | "development" | "local" => Ok(Self::Development),
            "prod" | "production" => Ok(Self::Production),
            _ => Err("expected development or production"),
        }
    }
}

impl FromStr for DatabaseKind {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "sqlite" => Ok(Self::Sqlite),
            "postgres" | "postgresql" => Ok(Self::Postgres),
            _ => Err("expected sqlite or postgres"),
        }
    }
}

impl FromStr for StorageKind {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "local" | "filesystem" | "fs" => Ok(Self::Local),
            "object" | "s3" | "r2" | "gcs" => Ok(Self::Object),
            _ => Err("expected local or object"),
        }
    }
}

fn detect_backend_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    if cwd.join("migrations").is_dir() && cwd.join("src").is_dir() {
        return Ok(cwd);
    }

    let nested_backend = cwd.join("backend");
    if nested_backend.join("migrations").is_dir() && nested_backend.join("src").is_dir() {
        return Ok(nested_backend);
    }

    Ok(cwd)
}

fn default_database_url(backend_root: &PathBuf) -> String {
    let path = backend_root.join("vanta.db");
    format!("sqlite://{}?mode=rwc", path.to_string_lossy())
}

fn parse_allowed_origins() -> Vec<String> {
    env::var("VANTA_ALLOWED_ORIGINS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|origins| !origins.is_empty())
        .unwrap_or_else(|| {
            vec![
                "http://127.0.0.1:3000".to_string(),
                "http://localhost:3000".to_string(),
                "http://127.0.0.1:5173".to_string(),
                "http://localhost:5173".to_string(),
            ]
        })
}

#[cfg(test)]
mod tests {
    use super::{
        Config, DatabaseKind, RuntimeEnvironment, StorageKind, database_url_from_env,
        default_database_url, detect_backend_root, parse_allowed_origins, parse_bool_env_value,
        runtime_bind_addr,
    };
    use std::{net::SocketAddr, path::PathBuf};

    #[test]
    fn parses_allowed_origins_from_env_list() {
        unsafe {
            std::env::set_var(
                "VANTA_ALLOWED_ORIGINS",
                "https://app.example.com, https://studio.example.com ,, ",
            );
        }
        let origins = parse_allowed_origins();
        assert_eq!(
            origins,
            vec![
                "https://app.example.com".to_string(),
                "https://studio.example.com".to_string()
            ]
        );
        unsafe {
            std::env::remove_var("VANTA_ALLOWED_ORIGINS");
        }
    }

    #[test]
    fn railway_port_sets_public_bind_address_when_bind_is_absent() {
        unsafe {
            std::env::remove_var("VANTA_BIND");
            std::env::set_var("PORT", "4242");
        }
        assert_eq!(runtime_bind_addr().expect("bind"), "0.0.0.0:4242");
        unsafe {
            std::env::remove_var("PORT");
        }
    }

    #[test]
    fn explicit_bind_overrides_railway_port() {
        unsafe {
            std::env::set_var("VANTA_BIND", "127.0.0.1:9191");
            std::env::set_var("PORT", "4242");
        }
        assert_eq!(runtime_bind_addr().expect("bind"), "127.0.0.1:9191");
        unsafe {
            std::env::remove_var("VANTA_BIND");
            std::env::remove_var("PORT");
        }
    }

    #[test]
    fn postgres_uses_railway_database_url_fallback() {
        unsafe {
            std::env::remove_var("VANTA_DATABASE_URL");
            std::env::remove_var("VANTA_POSTGRES_URL");
            std::env::set_var("DATABASE_URL", "postgresql://railway.internal/vanta");
        }
        assert_eq!(
            database_url_from_env(DatabaseKind::Postgres, &PathBuf::from("/tmp/backend")),
            "postgresql://railway.internal/vanta"
        );
        unsafe {
            std::env::remove_var("DATABASE_URL");
        }
    }

    #[test]
    fn defaults_database_url_into_backend_root() {
        let url = default_database_url(&PathBuf::from("/tmp/vanta/backend"));
        assert_eq!(url, "sqlite:///tmp/vanta/backend/vanta.db?mode=rwc");
    }

    #[test]
    fn detects_backend_root_from_current_backend_directory() {
        let cwd = std::env::current_dir().expect("current dir");
        let backend_root = detect_backend_root().expect("backend root");
        assert_eq!(backend_root, cwd);
    }

    fn base_config() -> Config {
        Config {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            database_kind: DatabaseKind::Sqlite,
            database_url: "sqlite::memory:".to_string(),
            max_db_connections: 1,
            storage_kind: StorageKind::Local,
            media_root: PathBuf::from("/tmp/vanta-media"),
            media_scratch_root: PathBuf::from("/tmp/vanta-scratch"),
            object_storage_bucket: None,
            object_storage_cdn_base_url: None,
            cdn_cookie_domain: None,
            admin_api_enabled: true,
            token_hash_secret: None,
            allowed_origins: vec!["http://localhost:3000".to_string()],
            environment: RuntimeEnvironment::Development,
        }
    }

    #[test]
    fn production_requires_postgres_object_storage_and_non_local_origins() {
        let mut config = base_config();
        config.environment = RuntimeEnvironment::Production;

        assert_eq!(
            config
                .validate()
                .expect_err("production sqlite rejected")
                .to_string(),
            "production requires VANTA_DATABASE_KIND=postgres"
        );

        config.database_kind = DatabaseKind::Postgres;
        config.database_url = "postgres://example/vanta".to_string();
        assert_eq!(
            config
                .validate()
                .expect_err("production local storage rejected")
                .to_string(),
            "production requires VANTA_STORAGE_KIND=object"
        );

        config.storage_kind = StorageKind::Object;
        config.object_storage_bucket = Some("vanta-assets".to_string());
        config.object_storage_cdn_base_url = Some("https://cdn.example.com".to_string());
        config.cdn_cookie_domain = Some(".example.com".to_string());
        assert_eq!(
            config
                .validate()
                .expect_err("production local origin rejected")
                .to_string(),
            "production requires explicit non-local VANTA_ALLOWED_ORIGINS"
        );

        config.allowed_origins = vec!["https://app.example.com".to_string()];
        assert_eq!(
            config
                .validate()
                .expect_err("production token hash secret rejected")
                .to_string(),
            "production requires VANTA_TOKEN_HASH_SECRET with at least 32 characters"
        );

        config.token_hash_secret = Some("0123456789abcdef0123456789abcdef".to_string());
        config.validate().expect("production config accepted");
    }

    #[test]
    fn object_storage_requires_bucket_and_cdn_url() {
        let mut config = base_config();
        config.storage_kind = StorageKind::Object;

        assert_eq!(
            config
                .validate()
                .expect_err("missing bucket rejected")
                .to_string(),
            "object storage requires VANTA_OBJECT_STORAGE_BUCKET"
        );

        config.object_storage_bucket = Some("vanta-assets".to_string());
        assert_eq!(
            config
                .validate()
                .expect_err("missing cdn url rejected")
                .to_string(),
            "object storage requires VANTA_OBJECT_STORAGE_CDN_BASE_URL"
        );

        config.object_storage_cdn_base_url = Some("https://cdn.example.com".to_string());
        assert_eq!(
            config
                .validate()
                .expect_err("missing cdn cookie domain rejected")
                .to_string(),
            "object storage requires VANTA_CDN_COOKIE_DOMAIN"
        );
    }

    #[test]
    fn parses_admin_api_enabled_boolean_values() {
        assert_eq!(parse_bool_env_value("true"), Some(true));
        assert_eq!(parse_bool_env_value("0"), Some(false));
        assert_eq!(parse_bool_env_value("off"), Some(false));
        assert_eq!(parse_bool_env_value("definitely"), None);
    }
}
