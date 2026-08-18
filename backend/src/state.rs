use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::Arc,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use axum::http::HeaderValue;
use sqlx::SqlitePool;
use tokio::sync::{Mutex, broadcast};

use crate::models::WsEvent;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub media_root: PathBuf,
    pub cors_allowed_origins: Vec<HeaderValue>,
    pub realtime: RealtimeHub,
    pub rate_limits: RateLimitStore,
    pub metrics: MetricsStore,
    pub media_processing_jobs: MediaProcessingJobStore,
    pub background_worker: BackgroundWorkerHealthStore,
    started_at: Instant,
}

impl AppState {
    pub fn new(
        pool: SqlitePool,
        media_root: PathBuf,
        cors_allowed_origins: Vec<HeaderValue>,
    ) -> Self {
        Self {
            pool,
            media_root,
            cors_allowed_origins,
            realtime: RealtimeHub::default(),
            rate_limits: RateLimitStore::default(),
            metrics: MetricsStore::default(),
            media_processing_jobs: MediaProcessingJobStore::default(),
            background_worker: BackgroundWorkerHealthStore::default(),
            started_at: Instant::now(),
        }
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    pub fn allows_origin(&self, origin: &HeaderValue) -> bool {
        self.cors_allowed_origins
            .iter()
            .any(|allowed| allowed == origin)
    }
}

#[derive(Clone)]
pub struct RealtimeHub {
    inner: Arc<Mutex<HashMap<String, ChannelHub>>>,
    total_connections: Arc<AtomicUsize>,
}

#[derive(Clone, Default)]
pub struct RateLimitStore {
    inner: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
}

#[derive(Clone, Default)]
pub struct MetricsStore {
    total_requests: Arc<AtomicU64>,
    in_flight_requests: Arc<AtomicU64>,
    total_rate_limits: Arc<AtomicU64>,
    status_counts: Arc<Mutex<HashMap<u16, u64>>>,
}

#[derive(Clone, Default)]
pub struct MediaProcessingJobStore {
    inner: Arc<Mutex<HashSet<String>>>,
}

#[derive(Clone, Default)]
pub struct BackgroundWorkerHealthStore {
    inner: Arc<Mutex<BackgroundWorkerHealthState>>,
}

#[derive(Clone, Debug)]
pub struct BackgroundWorkerHealthSnapshot {
    pub last_tick_age_seconds: Option<u64>,
    pub last_success_age_seconds: Option<u64>,
    pub consecutive_failures: u64,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct BackgroundWorkerHealthState {
    last_tick: Option<Instant>,
    last_success: Option<Instant>,
    consecutive_failures: u64,
    last_error: Option<String>,
}

#[derive(Clone)]
struct ChannelHub {
    sender: broadcast::Sender<WsEvent>,
    connections: usize,
}

impl Default for RealtimeHub {
    fn default() -> Self {
        Self {
            inner: Arc::default(),
            total_connections: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl RealtimeHub {
    pub async fn join(&self, channel_id: &str) -> (broadcast::Receiver<WsEvent>, usize) {
        let mut guard = self.inner.lock().await;
        let entry = guard.entry(channel_id.to_string()).or_insert_with(|| {
            let (sender, _) = broadcast::channel(256);
            ChannelHub {
                sender,
                connections: 0,
            }
        });
        entry.connections += 1;
        self.total_connections.fetch_add(1, Ordering::Relaxed);
        (entry.sender.subscribe(), entry.connections)
    }

    pub async fn leave(&self, channel_id: &str) -> usize {
        let mut guard = self.inner.lock().await;
        let mut remove_channel = false;

        let connections = if let Some(entry) = guard.get_mut(channel_id) {
            if entry.connections > 0 {
                entry.connections -= 1;
            }
            remove_channel = entry.connections == 0;
            entry.connections
        } else {
            0
        };

        if remove_channel {
            guard.remove(channel_id);
        }

        if connections > 0 || remove_channel {
            self.total_connections.fetch_sub(1, Ordering::Relaxed);
        }

        connections
    }

    pub async fn publish(&self, channel_id: &str, event: WsEvent) {
        let sender = {
            let mut guard = self.inner.lock().await;
            guard
                .entry(channel_id.to_string())
                .or_insert_with(|| {
                    let (sender, _) = broadcast::channel(256);
                    ChannelHub {
                        sender,
                        connections: 0,
                    }
                })
                .sender
                .clone()
        };

        let _ = sender.send(event);
    }

    pub async fn active_streams(&self) -> usize {
        self.active_channels_with_prefix("stream:").await
    }

    pub async fn active_collaboration_sessions(&self) -> usize {
        self.active_channels_with_prefix("collab:").await
    }

    async fn active_channels_with_prefix(&self, prefix: &str) -> usize {
        self.inner
            .lock()
            .await
            .keys()
            .filter(|key| key.starts_with(prefix))
            .count()
    }

    pub fn total_connections(&self) -> usize {
        self.total_connections.load(Ordering::Relaxed)
    }
}

impl RateLimitStore {
    pub async fn check(&self, key: &str, limit: usize, window: Duration) -> Result<(), ()> {
        let now = Instant::now();
        let mut guard = self.inner.lock().await;
        let bucket = guard.entry(key.to_string()).or_default();

        while let Some(front) = bucket.front() {
            if now.duration_since(*front) > window {
                bucket.pop_front();
            } else {
                break;
            }
        }

        if bucket.len() >= limit {
            return Err(());
        }

        bucket.push_back(now);
        Ok(())
    }
}

impl MetricsStore {
    pub fn begin_request(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.in_flight_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub async fn finish_request(&self, status: u16) {
        self.in_flight_requests.fetch_sub(1, Ordering::Relaxed);
        let mut guard = self.status_counts.lock().await;
        *guard.entry(status).or_insert(0) += 1;
    }

    pub fn increment_rate_limit(&self) {
        self.total_rate_limits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }

    pub fn in_flight_requests(&self) -> u64 {
        self.in_flight_requests.load(Ordering::Relaxed)
    }

    pub fn total_rate_limits(&self) -> u64 {
        self.total_rate_limits.load(Ordering::Relaxed)
    }

    pub async fn status_counts(&self) -> HashMap<u16, u64> {
        self.status_counts.lock().await.clone()
    }
}

impl MediaProcessingJobStore {
    pub async fn try_acquire(&self, job_id: &str) -> bool {
        let mut guard = self.inner.lock().await;
        guard.insert(job_id.to_string())
    }

    pub async fn release(&self, job_id: &str) {
        self.inner.lock().await.remove(job_id);
    }
}

impl BackgroundWorkerHealthStore {
    pub async fn mark_tick(&self) {
        let mut guard = self.inner.lock().await;
        guard.last_tick = Some(Instant::now());
    }

    pub async fn mark_success(&self) {
        let now = Instant::now();
        let mut guard = self.inner.lock().await;
        guard.last_tick = Some(now);
        guard.last_success = Some(now);
        guard.consecutive_failures = 0;
        guard.last_error = None;
    }

    pub async fn mark_failure(&self, error: String) {
        let now = Instant::now();
        let mut guard = self.inner.lock().await;
        guard.last_tick = Some(now);
        guard.consecutive_failures = guard.consecutive_failures.saturating_add(1);
        guard.last_error = Some(error);
    }

    pub async fn snapshot(&self) -> BackgroundWorkerHealthSnapshot {
        let guard = self.inner.lock().await;
        let now = Instant::now();
        BackgroundWorkerHealthSnapshot {
            last_tick_age_seconds: guard
                .last_tick
                .map(|instant| now.saturating_duration_since(instant).as_secs()),
            last_success_age_seconds: guard
                .last_success
                .map(|instant| now.saturating_duration_since(instant).as_secs()),
            consecutive_failures: guard.consecutive_failures,
            last_error: guard.last_error.clone(),
        }
    }
}
