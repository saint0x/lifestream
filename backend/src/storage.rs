use std::path::{Path, PathBuf};

use crate::config::{Config, StorageKind};
use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct Storage {
    provider: StorageProvider,
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, path::PathBuf};

    use crate::config::{DatabaseKind, RuntimeEnvironment};

    use super::*;

    fn config(storage_kind: StorageKind) -> Config {
        Config {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            database_kind: DatabaseKind::Sqlite,
            database_url: "sqlite::memory:".to_string(),
            max_db_connections: 1,
            storage_kind,
            media_root: PathBuf::from("/tmp/vanta-media"),
            media_scratch_root: PathBuf::from("/tmp/vanta-scratch"),
            object_storage_bucket: Some("vanta-assets".to_string()),
            object_storage_cdn_base_url: Some("https://cdn.example.com/media/".to_string()),
            cdn_cookie_domain: Some(".example.com".to_string()),
            admin_api_enabled: true,
            token_hash_secret: None,
            allowed_origins: vec!["http://localhost:3000".to_string()],
            environment: RuntimeEnvironment::Development,
        }
    }

    #[test]
    fn local_storage_uses_backend_media_api_urls() {
        let storage = Storage::from_config(&config(StorageKind::Local)).expect("storage");

        assert_eq!(storage.kind(), StorageKind::Local);
        assert_eq!(
            storage.public_url("films/master.m3u8"),
            "/api/v1/media/films/master.m3u8"
        );
        assert_eq!(
            storage.local_artifact_path("films/master.m3u8"),
            PathBuf::from("/tmp/vanta-media/films/master.m3u8")
        );
        assert_eq!(
            storage.playback_manifest_url("ps-1", "films/master.m3u8", "pt-1"),
            "/api/v1/playback/sessions/ps-1/manifest?playbackToken=pt-1"
        );
        assert_eq!(
            storage.playback_media_url("films/segment.m4s", "pt-1"),
            "/api/v1/media/films/segment.m4s?playbackToken=pt-1"
        );
    }

    #[test]
    fn object_storage_uses_cdn_urls_and_scratch_artifact_paths() {
        let storage = Storage::from_config(&config(StorageKind::Object)).expect("storage");

        assert_eq!(storage.kind(), StorageKind::Object);
        assert_eq!(storage.object_bucket(), Some("vanta-assets"));
        assert_eq!(storage.cdn_cookie_domain(), Some(".example.com"));
        assert_eq!(
            storage.public_url("/films/master.m3u8"),
            "https://cdn.example.com/media/films/master.m3u8"
        );
        assert_eq!(
            storage.local_artifact_path("films/master.m3u8"),
            PathBuf::from("/tmp/vanta-scratch/films/master.m3u8")
        );
        assert_eq!(
            storage.playback_manifest_url("ps-1", "films/master.m3u8", "pt-1"),
            "https://cdn.example.com/media/films/master.m3u8"
        );
        assert_eq!(
            storage.playback_media_url("films/segment.m4s", "pt-1"),
            "https://cdn.example.com/media/films/segment.m4s"
        );
    }
}

#[derive(Clone)]
enum StorageProvider {
    Local(LocalStorage),
    Object(ObjectStorage),
}

#[derive(Clone)]
struct LocalStorage {
    media_root: PathBuf,
    scratch_root: PathBuf,
}

#[derive(Clone)]
struct ObjectStorage {
    bucket: String,
    cdn_base_url: String,
    cdn_cookie_domain: String,
    scratch_root: PathBuf,
}

impl Storage {
    pub fn from_config(config: &Config) -> AppResult<Self> {
        match config.storage_kind {
            StorageKind::Local => Ok(Self {
                provider: StorageProvider::Local(LocalStorage {
                    media_root: config.media_root.clone(),
                    scratch_root: config.media_scratch_root.clone(),
                }),
            }),
            StorageKind::Object => {
                let bucket = config
                    .object_storage_bucket
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let cdn_base_url = config
                    .object_storage_cdn_base_url
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .trim_end_matches('/')
                    .to_string();
                let cdn_cookie_domain = config
                    .cdn_cookie_domain
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if bucket.is_empty() || cdn_base_url.is_empty() || cdn_cookie_domain.is_empty() {
                    return Err(AppError::Internal(
                        "object storage requires bucket, CDN base URL, and CDN cookie domain"
                            .to_string(),
                    ));
                }
                Ok(Self {
                    provider: StorageProvider::Object(ObjectStorage {
                        bucket,
                        cdn_base_url,
                        cdn_cookie_domain,
                        scratch_root: config.media_scratch_root.clone(),
                    }),
                })
            }
        }
    }

    pub fn kind(&self) -> StorageKind {
        match &self.provider {
            StorageProvider::Local(_) => StorageKind::Local,
            StorageProvider::Object(_) => StorageKind::Object,
        }
    }

    pub async fn prepare(&self) -> AppResult<()> {
        tokio::fs::create_dir_all(self.scratch_root()).await?;
        if let StorageProvider::Local(local) = &self.provider {
            tokio::fs::create_dir_all(&local.media_root).await?;
        }
        Ok(())
    }

    pub fn scratch_root(&self) -> &Path {
        match &self.provider {
            StorageProvider::Local(local) => &local.scratch_root,
            StorageProvider::Object(object) => &object.scratch_root,
        }
    }

    pub fn local_media_root(&self) -> Option<&Path> {
        match &self.provider {
            StorageProvider::Local(local) => Some(&local.media_root),
            StorageProvider::Object(_) => None,
        }
    }

    pub fn local_artifact_path(&self, relative_path: &str) -> PathBuf {
        match &self.provider {
            StorageProvider::Local(local) => local.media_root.join(relative_path),
            StorageProvider::Object(object) => object.scratch_root.join(relative_path),
        }
    }

    pub fn public_url(&self, relative_path: &str) -> String {
        match &self.provider {
            StorageProvider::Local(_) => format!("/api/v1/media/{relative_path}"),
            StorageProvider::Object(object) => format!(
                "{}/{}",
                object.cdn_base_url,
                relative_path.trim_start_matches('/')
            ),
        }
    }

    pub fn playback_manifest_url(
        &self,
        session_id: &str,
        manifest_relative_path: &str,
        playback_token: &str,
    ) -> String {
        match &self.provider {
            StorageProvider::Local(_) => format!(
                "/api/v1/playback/sessions/{session_id}/manifest?playbackToken={playback_token}"
            ),
            StorageProvider::Object(_) => self.public_url(manifest_relative_path),
        }
    }

    pub fn playback_media_url(&self, relative_path: &str, playback_token: &str) -> String {
        match &self.provider {
            StorageProvider::Local(_) => {
                format!("/api/v1/media/{relative_path}?playbackToken={playback_token}")
            }
            StorageProvider::Object(_) => self.public_url(relative_path),
        }
    }

    pub fn object_bucket(&self) -> Option<&str> {
        match &self.provider {
            StorageProvider::Local(_) => None,
            StorageProvider::Object(object) => Some(&object.bucket),
        }
    }

    pub fn cdn_cookie_domain(&self) -> Option<&str> {
        match &self.provider {
            StorageProvider::Local(_) => None,
            StorageProvider::Object(object) => Some(&object.cdn_cookie_domain),
        }
    }
}
