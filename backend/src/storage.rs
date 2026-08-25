use std::path::{Path, PathBuf};

use crate::config::{Config, StorageKind};
use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct Storage {
    provider: StorageProvider,
    http_client: reqwest::Client,
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
            object_storage_endpoint_url: Some("https://r2.example.com".to_string()),
            object_storage_access_key_id: Some("access".to_string()),
            object_storage_secret_access_key: Some("secret".to_string()),
            object_storage_region: "auto".to_string(),
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
    endpoint_url: String,
    access_key_id: String,
    secret_access_key: String,
    region: String,
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
                http_client: reqwest::Client::new(),
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
                let endpoint_url = config
                    .object_storage_endpoint_url
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .trim_end_matches('/')
                    .to_string();
                let access_key_id = config
                    .object_storage_access_key_id
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let secret_access_key = config
                    .object_storage_secret_access_key
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let region = config.object_storage_region.trim().to_string();
                let cdn_cookie_domain = config
                    .cdn_cookie_domain
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if bucket.is_empty()
                    || cdn_base_url.is_empty()
                    || cdn_cookie_domain.is_empty()
                    || endpoint_url.is_empty()
                    || access_key_id.is_empty()
                    || secret_access_key.is_empty()
                    || region.is_empty()
                {
                    return Err(AppError::Internal(
                        "object storage requires bucket, endpoint, credentials, region, CDN base URL, and CDN cookie domain"
                            .to_string(),
                    ));
                }
                Ok(Self {
                    provider: StorageProvider::Object(ObjectStorage {
                        bucket,
                        endpoint_url,
                        access_key_id,
                        secret_access_key,
                        region,
                        cdn_base_url,
                        cdn_cookie_domain,
                        scratch_root: config.media_scratch_root.clone(),
                    }),
                    http_client: reqwest::Client::new(),
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

    pub async fn publish_file(
        &self,
        relative_path: &str,
        local_path: &Path,
        content_type: &str,
    ) -> AppResult<()> {
        let StorageProvider::Object(object) = &self.provider else {
            return Ok(());
        };

        let file = tokio::fs::File::open(local_path).await?;
        let metadata = file.metadata().await?;
        let stream = tokio_util::io::ReaderStream::new(file);
        let request = object.signed_request(
            reqwest::Method::PUT,
            relative_path,
            Some(content_type),
            Some(metadata.len()),
        )?;
        let response = self
            .http_client
            .put(request.url)
            .headers(request.headers)
            .body(reqwest::Body::wrap_stream(stream))
            .send()
            .await
            .map_err(|error| {
                AppError::Internal(format!("object storage upload failed: {error}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "object storage upload rejected {status}: {body}"
            )));
        }

        Ok(())
    }

    pub async fn restore_file_if_missing(
        &self,
        relative_path: &str,
        local_path: &Path,
    ) -> AppResult<()> {
        let StorageProvider::Object(object) = &self.provider else {
            return Ok(());
        };
        if tokio::fs::try_exists(local_path).await? {
            return Ok(());
        }
        if let Some(parent) = local_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let request = object.signed_request(reqwest::Method::GET, relative_path, None, None)?;
        let response = self
            .http_client
            .get(request.url)
            .headers(request.headers)
            .send()
            .await
            .map_err(|error| {
                AppError::Internal(format!("object storage download failed: {error}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "object storage download rejected {status}: {body}"
            )));
        }

        let mut file = tokio::fs::File::create(local_path).await?;
        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| {
                AppError::Internal(format!("object storage download stream failed: {error}"))
            })?;
            file.write_all(&chunk).await?;
        }
        file.flush().await?;

        Ok(())
    }
}

struct SignedObjectRequest {
    url: String,
    headers: reqwest::header::HeaderMap,
}

impl ObjectStorage {
    fn signed_request(
        &self,
        method: reqwest::Method,
        relative_path: &str,
        content_type: Option<&str>,
        content_length: Option<u64>,
    ) -> AppResult<SignedObjectRequest> {
        let object_key = relative_path.trim_start_matches('/');
        if object_key.is_empty() || object_key.contains("..") {
            return Err(AppError::BadRequest(
                "object storage key must be a safe relative path".to_string(),
            ));
        }

        let encoded_key = encode_object_key(object_key);
        let canonical_uri = format!("/{}/{}", self.bucket, encoded_key);
        let url = format!("{}{}", self.endpoint_url, canonical_uri);
        let parsed = reqwest::Url::parse(&url)
            .map_err(|error| AppError::Internal(format!("invalid object storage URL: {error}")))?;
        let host = parsed
            .host_str()
            .ok_or_else(|| {
                AppError::Internal("object storage endpoint requires a host".to_string())
            })
            .map(|host| {
                parsed
                    .port()
                    .map(|port| format!("{host}:{port}"))
                    .unwrap_or_else(|| host.to_string())
            })?;

        let now = chrono::Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let datestamp = now.format("%Y%m%d").to_string();
        let payload_hash = "UNSIGNED-PAYLOAD";
        let canonical_headers =
            format!("host:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n");
        let signed_headers = "host;x-amz-content-sha256;x-amz-date";
        let canonical_request = format!(
            "{}\n{}\n\n{}\n{}\n{}",
            method.as_str(),
            canonical_uri,
            canonical_headers,
            signed_headers,
            payload_hash
        );
        let credential_scope = format!("{datestamp}/{}/s3/aws4_request", self.region);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
            sha256_hex(canonical_request.as_bytes())
        );
        let signing_key = signing_key(&self.secret_access_key, &datestamp, &self.region);
        let signature = hmac_sha256_hex(&signing_key, string_to_sign.as_bytes());

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::HOST,
            reqwest::header::HeaderValue::from_str(&host).map_err(|error| {
                AppError::Internal(format!("invalid object storage host header: {error}"))
            })?,
        );
        headers.insert(
            "x-amz-date",
            reqwest::header::HeaderValue::from_str(&amz_date).map_err(|error| {
                AppError::Internal(format!("invalid object storage date header: {error}"))
            })?,
        );
        headers.insert(
            "x-amz-content-sha256",
            reqwest::header::HeaderValue::from_static(payload_hash),
        );
        headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!(
                "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
                self.access_key_id, credential_scope, signed_headers, signature
            ))
            .map_err(|error| {
                AppError::Internal(format!(
                    "invalid object storage authorization header: {error}"
                ))
            })?,
        );
        if let Some(content_type) = content_type {
            headers.insert(
                reqwest::header::CONTENT_TYPE,
                reqwest::header::HeaderValue::from_str(content_type).map_err(|error| {
                    AppError::Internal(format!("invalid object storage content type: {error}"))
                })?,
            );
        }
        if let Some(content_length) = content_length {
            headers.insert(
                reqwest::header::CONTENT_LENGTH,
                reqwest::header::HeaderValue::from_str(&content_length.to_string()).map_err(
                    |error| {
                        AppError::Internal(format!(
                            "invalid object storage content length: {error}"
                        ))
                    },
                )?,
            );
        }

        Ok(SignedObjectRequest { url, headers })
    }
}

type HmacSha256 = hmac::Hmac<sha2::Sha256>;

fn signing_key(secret: &str, datestamp: &str, region: &str) -> Vec<u8> {
    let date_key = hmac_sha256(format!("AWS4{secret}").as_bytes(), datestamp.as_bytes());
    let date_region_key = hmac_sha256(&date_key, region.as_bytes());
    let date_region_service_key = hmac_sha256(&date_region_key, b"s3");
    hmac_sha256(&date_region_service_key, b"aws4_request")
}

fn hmac_sha256(key: &[u8], value: &[u8]) -> Vec<u8> {
    use hmac::Mac;

    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts keys of any size");
    mac.update(value);
    mac.finalize().into_bytes().to_vec()
}

fn hmac_sha256_hex(key: &[u8], value: &[u8]) -> String {
    hex_string(&hmac_sha256(key, value))
}

fn sha256_hex(value: &[u8]) -> String {
    use sha2::Digest;

    hex_string(&sha2::Sha256::digest(value))
}

fn hex_string(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn encode_object_key(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}
