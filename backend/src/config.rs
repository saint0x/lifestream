use std::{env, net::SocketAddr, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub max_db_connections: u32,
    pub media_root: PathBuf,
    pub allowed_origins: Vec<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let bind_addr = env::var("LIFESTREAM_BIND")
            .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
            .parse()?;

        let backend_root = detect_backend_root()?;
        let database_url = env::var("LIFESTREAM_DATABASE_URL")
            .unwrap_or_else(|_| default_database_url(&backend_root));

        let max_db_connections = env::var("LIFESTREAM_DB_MAX_CONNECTIONS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(12);

        Ok(Self {
            bind_addr,
            database_url,
            max_db_connections,
            media_root: env::var("LIFESTREAM_MEDIA_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|_| backend_root.join("media")),
            allowed_origins: parse_allowed_origins(),
        })
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
    let path = backend_root.join("lifestream.db");
    format!("sqlite://{}?mode=rwc", path.to_string_lossy())
}

fn parse_allowed_origins() -> Vec<String> {
    env::var("LIFESTREAM_ALLOWED_ORIGINS")
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
    use super::{default_database_url, detect_backend_root, parse_allowed_origins};
    use std::path::PathBuf;

    #[test]
    fn parses_allowed_origins_from_env_list() {
        unsafe {
            std::env::set_var(
                "LIFESTREAM_ALLOWED_ORIGINS",
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
            std::env::remove_var("LIFESTREAM_ALLOWED_ORIGINS");
        }
    }

    #[test]
    fn defaults_database_url_into_backend_root() {
        let url = default_database_url(&PathBuf::from("/tmp/lifestream/backend"));
        assert_eq!(url, "sqlite:///tmp/lifestream/backend/lifestream.db?mode=rwc");
    }

    #[test]
    fn detects_backend_root_from_current_backend_directory() {
        let cwd = std::env::current_dir().expect("current dir");
        let backend_root = detect_backend_root().expect("backend root");
        assert_eq!(backend_root, cwd);
    }
}
