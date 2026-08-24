#![allow(dead_code)]

use crate::{
    api::PlaybackSessionRecord,
    auth::RequestIdentity,
    db::{
        CatalogSearchHit, Database, NewAuthSession, NewPlaybackSession,
        PlaybackSessionMetadataUpdate, ProvisionedCreator, ProvisionedUser,
        ReusableLivePlaybackSessionLookup,
    },
    error::AppResult,
    models::{
        AuthSession, PlaybackSession, ProgressInput, UpdateProfileRequest, UpdateSettingsRequest,
    },
};

pub(crate) trait AuthRepository {
    async fn lookup_identity(&self, token_hash: &str, now: &str) -> AppResult<RequestIdentity>;
    async fn touch_auth_session(&self, session_id: &str, now: &str) -> AppResult<()>;
    async fn list_auth_sessions(
        &self,
        user_id: &str,
        current_session_id: &str,
        limit: Option<usize>,
    ) -> AppResult<Vec<AuthSession>>;
    async fn create_auth_session(&self, session: NewAuthSession<'_>) -> AppResult<()>;
    async fn revoke_auth_session(
        &self,
        session_id: &str,
        user_id: &str,
        now: &str,
    ) -> AppResult<u64>;
}

pub(crate) trait AccountProvisioningRepository {
    async fn ensure_user_exists(&self, user_id: &str) -> AppResult<()>;
    async fn provision_user(&self, user: ProvisionedUser<'_>) -> AppResult<()>;
    async fn provision_creator(&self, creator: ProvisionedCreator<'_>) -> AppResult<()>;
}

pub(crate) trait CatalogDiscoveryRepository {
    async fn search_catalog_documents(
        &self,
        query: &str,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<CatalogSearchHit>>;
}

pub(crate) trait ViewerRepository {
    async fn update_user_profile(
        &self,
        user_id: &str,
        input: &UpdateProfileRequest,
    ) -> AppResult<()>;
    async fn update_user_settings(
        &self,
        user_id: &str,
        input: &UpdateSettingsRequest,
    ) -> AppResult<()>;
    async fn add_watchlist_item(&self, user_id: &str, content_id: &str) -> AppResult<()>;
    async fn remove_watchlist_item(&self, user_id: &str, content_id: &str) -> AppResult<()>;
    async fn add_following(&self, user_id: &str, streamer_id: &str) -> AppResult<()>;
    async fn remove_following(&self, user_id: &str, streamer_id: &str) -> AppResult<()>;
    async fn record_progress(
        &self,
        user_id: &str,
        input: &ProgressInput,
        watched_at: &str,
    ) -> AppResult<()>;
    async fn remove_progress(&self, user_id: &str, content_id: &str) -> AppResult<()>;
    async fn remove_history_entry(&self, user_id: &str, content_id: &str) -> AppResult<()>;
}

pub(crate) trait NotificationRepository {
    async fn mark_user_notification_read(
        &self,
        user_id: &str,
        notification_id: &str,
        now: &str,
    ) -> AppResult<u64>;
}

pub(crate) trait PlaybackRepository {
    async fn find_reusable_live_playback_session(
        &self,
        lookup: ReusableLivePlaybackSessionLookup<'_>,
    ) -> AppResult<Option<PlaybackSession>>;
    async fn create_playback_session(&self, session: NewPlaybackSession<'_>) -> AppResult<()>;
    async fn rotate_reusable_live_playback_session(
        &self,
        session: PlaybackSession,
        refreshed_token: &str,
        now: &str,
        expires_at: &str,
        metadata: PlaybackSessionMetadataUpdate<'_>,
    ) -> AppResult<PlaybackSession>;
    async fn rotate_playback_session_token(
        &self,
        session_id: &str,
        current_playback_token: &str,
        next_playback_token: &str,
        now: &str,
        expires_at: &str,
    ) -> AppResult<()>;
    async fn fetch_active_playback_session_record(
        &self,
        session_id: &str,
        playback_token: &str,
        now: &str,
    ) -> AppResult<PlaybackSessionRecord>;
    async fn fetch_latest_active_playback_session_record_by_token(
        &self,
        playback_token: &str,
        now: &str,
    ) -> AppResult<PlaybackSessionRecord>;
    async fn expire_playback_sessions_for_auth_session(
        &self,
        auth_session_id: &str,
        now: &str,
    ) -> AppResult<()>;
}

pub(crate) trait CollaborationRuntimeRepository {
    async fn fetch_collaboration_launch_relative_path(&self, session_id: &str)
    -> AppResult<String>;
}

impl AuthRepository for Database {
    async fn lookup_identity(&self, token_hash: &str, now: &str) -> AppResult<RequestIdentity> {
        Database::lookup_identity(self, token_hash, now).await
    }

    async fn touch_auth_session(&self, session_id: &str, now: &str) -> AppResult<()> {
        Database::touch_auth_session(self, session_id, now).await
    }

    async fn list_auth_sessions(
        &self,
        user_id: &str,
        current_session_id: &str,
        limit: Option<usize>,
    ) -> AppResult<Vec<AuthSession>> {
        Database::list_auth_sessions(self, user_id, current_session_id, limit).await
    }

    async fn create_auth_session(&self, session: NewAuthSession<'_>) -> AppResult<()> {
        Database::create_auth_session(self, session).await
    }

    async fn revoke_auth_session(
        &self,
        session_id: &str,
        user_id: &str,
        now: &str,
    ) -> AppResult<u64> {
        Database::revoke_auth_session(self, session_id, user_id, now).await
    }
}

impl AccountProvisioningRepository for Database {
    async fn ensure_user_exists(&self, user_id: &str) -> AppResult<()> {
        Database::ensure_user_exists(self, user_id).await
    }

    async fn provision_user(&self, user: ProvisionedUser<'_>) -> AppResult<()> {
        Database::provision_user(self, user).await
    }

    async fn provision_creator(&self, creator: ProvisionedCreator<'_>) -> AppResult<()> {
        Database::provision_creator(self, creator).await
    }
}

impl CatalogDiscoveryRepository for Database {
    async fn search_catalog_documents(
        &self,
        query: &str,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<CatalogSearchHit>> {
        Database::search_catalog_documents(self, query, limit, offset).await
    }
}

impl ViewerRepository for Database {
    async fn update_user_profile(
        &self,
        user_id: &str,
        input: &UpdateProfileRequest,
    ) -> AppResult<()> {
        Database::update_user_profile(self, user_id, input).await
    }

    async fn update_user_settings(
        &self,
        user_id: &str,
        input: &UpdateSettingsRequest,
    ) -> AppResult<()> {
        Database::update_user_settings(self, user_id, input).await
    }

    async fn add_watchlist_item(&self, user_id: &str, content_id: &str) -> AppResult<()> {
        Database::add_watchlist_item(self, user_id, content_id).await
    }

    async fn remove_watchlist_item(&self, user_id: &str, content_id: &str) -> AppResult<()> {
        Database::remove_watchlist_item(self, user_id, content_id).await
    }

    async fn add_following(&self, user_id: &str, streamer_id: &str) -> AppResult<()> {
        Database::add_following(self, user_id, streamer_id).await
    }

    async fn remove_following(&self, user_id: &str, streamer_id: &str) -> AppResult<()> {
        Database::remove_following(self, user_id, streamer_id).await
    }

    async fn record_progress(
        &self,
        user_id: &str,
        input: &ProgressInput,
        watched_at: &str,
    ) -> AppResult<()> {
        Database::record_progress(self, user_id, input, watched_at).await
    }

    async fn remove_progress(&self, user_id: &str, content_id: &str) -> AppResult<()> {
        Database::remove_progress(self, user_id, content_id).await
    }

    async fn remove_history_entry(&self, user_id: &str, content_id: &str) -> AppResult<()> {
        Database::remove_history_entry(self, user_id, content_id).await
    }
}

impl NotificationRepository for Database {
    async fn mark_user_notification_read(
        &self,
        user_id: &str,
        notification_id: &str,
        now: &str,
    ) -> AppResult<u64> {
        Database::mark_user_notification_read(self, user_id, notification_id, now).await
    }
}

impl PlaybackRepository for Database {
    async fn find_reusable_live_playback_session(
        &self,
        lookup: ReusableLivePlaybackSessionLookup<'_>,
    ) -> AppResult<Option<PlaybackSession>> {
        Database::find_reusable_live_playback_session(self, lookup).await
    }

    async fn create_playback_session(&self, session: NewPlaybackSession<'_>) -> AppResult<()> {
        Database::create_playback_session(self, session).await
    }

    async fn rotate_reusable_live_playback_session(
        &self,
        session: PlaybackSession,
        refreshed_token: &str,
        now: &str,
        expires_at: &str,
        metadata: PlaybackSessionMetadataUpdate<'_>,
    ) -> AppResult<PlaybackSession> {
        Database::rotate_reusable_live_playback_session(
            self,
            session,
            refreshed_token,
            now,
            expires_at,
            metadata,
        )
        .await
    }

    async fn rotate_playback_session_token(
        &self,
        session_id: &str,
        current_playback_token: &str,
        next_playback_token: &str,
        now: &str,
        expires_at: &str,
    ) -> AppResult<()> {
        Database::rotate_playback_session_token(
            self,
            session_id,
            current_playback_token,
            next_playback_token,
            now,
            expires_at,
        )
        .await
    }

    async fn fetch_active_playback_session_record(
        &self,
        session_id: &str,
        playback_token: &str,
        now: &str,
    ) -> AppResult<PlaybackSessionRecord> {
        Database::fetch_active_playback_session_record(self, session_id, playback_token, now).await
    }

    async fn fetch_latest_active_playback_session_record_by_token(
        &self,
        playback_token: &str,
        now: &str,
    ) -> AppResult<PlaybackSessionRecord> {
        Database::fetch_latest_active_playback_session_record_by_token(self, playback_token, now)
            .await
    }

    async fn expire_playback_sessions_for_auth_session(
        &self,
        auth_session_id: &str,
        now: &str,
    ) -> AppResult<()> {
        Database::expire_playback_sessions_for_auth_session(self, auth_session_id, now).await
    }
}

impl CollaborationRuntimeRepository for Database {
    async fn fetch_collaboration_launch_relative_path(
        &self,
        session_id: &str,
    ) -> AppResult<String> {
        Database::fetch_collaboration_launch_relative_path(self, session_id).await
    }
}
