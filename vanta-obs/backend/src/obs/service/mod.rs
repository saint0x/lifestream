use std::sync::Arc;

use thiserror::Error;

use super::{
    bridge::{ObsBridgeClient, ObsBridgeError, websocket::LocalObsWebSocketClient},
    store::{ObsStore, ObsStoreError},
};

mod audience;
mod audio;
mod bridge;
mod broadcast;
mod cue;
mod engagement;
mod export;
mod filter;
mod guest;
mod hotkey;
mod import;
mod moderation;
mod preflight;
mod recording;
mod replay;
mod runtime;
mod scene;
mod source;
mod sponsor;
mod studio;

#[derive(Clone)]
pub struct ObsService {
    store: Arc<ObsStore>,
    bridge: Arc<dyn ObsBridgeClient>,
}

impl ObsService {
    pub fn new(store: ObsStore) -> Self {
        Self {
            store: Arc::new(store),
            bridge: Arc::new(LocalObsWebSocketClient::default()),
        }
    }

    pub fn from_shared(store: Arc<ObsStore>) -> Self {
        Self {
            store,
            bridge: Arc::new(LocalObsWebSocketClient::default()),
        }
    }

    pub fn with_bridge_client(store: ObsStore, bridge: Arc<dyn ObsBridgeClient>) -> Self {
        Self {
            store: Arc::new(store),
            bridge,
        }
    }
}

#[derive(Debug, Error)]
pub enum ObsServiceError {
    #[error(transparent)]
    Store(#[from] ObsStoreError),
    #[error(transparent)]
    Bridge(#[from] ObsBridgeError),
    #[error(transparent)]
    Import(#[from] super::import::ObsImportError),
    #[error(transparent)]
    Export(#[from] super::export::ObsExportError),
    #[error(transparent)]
    ReplayMedia(#[from] super::replay_media::ReplayMediaError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("invalid {field}: {message}")]
    Invalid {
        field: &'static str,
        message: &'static str,
    },
}

pub type ObsServiceResult<T> = Result<T, ObsServiceError>;

fn require_text(value: &str, field: &'static str) -> ObsServiceResult<()> {
    if value.trim().is_empty() {
        return Err(ObsServiceError::Invalid {
            field,
            message: "must not be empty",
        });
    }
    Ok(())
}

fn require_one_of(
    value: &str,
    field: &'static str,
    accepted: &'static [&'static str],
) -> ObsServiceResult<()> {
    if !accepted.contains(&value) {
        return Err(ObsServiceError::Invalid {
            field,
            message: "is not supported by Vanta OBS",
        });
    }
    Ok(())
}

fn require_positive(value: f64, field: &'static str) -> ObsServiceResult<()> {
    if value <= 0.0 {
        return Err(ObsServiceError::Invalid {
            field,
            message: "must be greater than zero",
        });
    }
    Ok(())
}

const TRANSITION_KINDS: &[&str] = &["cut", "fade", "dip_to_black", "swipe", "stinger"];
const VALIDATION_STATES: &[&str] = &["ready", "needs_sources", "warning", "blocked"];
const PERMISSION_STATES: &[&str] = &["pending", "granted", "denied", "unsupported"];
const HEALTH_STATES: &[&str] = &["unknown", "good", "warning", "blocked"];
const VISIBILITY: &[&str] = &["public", "unlisted", "private"];
const LATENCY: &[&str] = &["ultra_low", "low", "normal"];
const RECORDING_POLICIES: &[&str] = &["none", "program", "program_plus_isolated_audio"];
const ARCHIVE_POLICIES: &[&str] = &["none", "archive_to_vanta_asset"];
const RECORDING_MODES: &[&str] = &["program", "clean_feed", "program_plus_isolated_audio"];
const CUE_KINDS: &[&str] = &[
    "sponsor_read",
    "lower_third",
    "branded_bumper",
    "pinned_cta",
    "qr_code",
    "promo_code",
    "proof_marker",
];
