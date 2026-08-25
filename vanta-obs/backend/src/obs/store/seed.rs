use serde_json::json;

use super::{
    ObsStore, ObsStoreError,
    row::{id, now},
};
use crate::obs::domain::{BroadcastInput, CueInput, PreflightInput};

impl ObsStore {
    pub async fn seed(&self) -> Result<(), ObsStoreError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM obs_scene_collections")
            .fetch_one(&self.pool)
            .await?;
        if count > 0 {
            self.seed_scene_templates().await?;
            self.seed_audience_if_missing("broadcast_prime_launch")
                .await?;
            self.seed_engagement_if_missing("broadcast_prime_launch")
                .await?;
            return Ok(());
        }

        let now = now();
        let collection = "obs_collection_prime_live";
        let broadcast = "broadcast_prime_launch";
        let scenes = [
            (
                "scene_starting_soon",
                "Starting Soon",
                1,
                "fade",
                450,
                0,
                "ready",
            ),
            ("scene_host_camera", "Host Camera", 2, "cut", 0, 0, "ready"),
            (
                "scene_product_demo",
                "Product Demo",
                3,
                "fade",
                320,
                0,
                "ready",
            ),
            (
                "scene_sponsor_read",
                "Sponsor Read",
                4,
                "dip_to_black",
                500,
                0,
                "ready",
            ),
            (
                "scene_emergency_holding",
                "Emergency Holding",
                5,
                "cut",
                0,
                1,
                "ready",
            ),
        ];
        sqlx::query(
            "INSERT INTO obs_scene_collections
            (id, creator_id, name, description, canvas_width, canvas_height, frame_rate, default_transition, active_scene_id, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', 'Prime Live Kit', 'Reusable live package for sponsor-backed shows.', 1920, 1080, 30, 'fade', 'scene_host_camera', ?, ?)",
        )
        .bind(collection)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        for (id, name, index, transition, duration, locked, state) in scenes {
            sqlx::query(
                "INSERT INTO obs_scenes
                (id, collection_id, creator_id, name, order_index, transition_kind, transition_duration_ms, hotkey, locked, validation_state, created_at, updated_at)
                VALUES (?, ?, 'creator_vanta_originals', ?, ?, ?, ?, NULL, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(collection)
            .bind(name)
            .bind(index)
            .bind(transition)
            .bind(duration)
            .bind(locked)
            .bind(state)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
                .await?;
        }
        self.seed_scene_templates().await?;

        for (id, kind, name, device, media, url, permission, health, settings) in [
            (
                "source_camera_a",
                "camera",
                "Sony FX3 Camera A",
                Some("camera:fx3-a"),
                None,
                None,
                "granted",
                "good",
                json!({"resolution":"1080p","frame_rate":30}),
            ),
            (
                "source_mic_host",
                "microphone",
                "Host lav",
                Some("audio:lav-host"),
                None,
                None,
                "granted",
                "good",
                json!({"noise_suppression":true,"gate":true}),
            ),
            (
                "source_screen",
                "screen_capture",
                "Demo display",
                Some("display:main"),
                None,
                None,
                "granted",
                "good",
                json!({"cursor":true,"fit":"contain"}),
            ),
            (
                "source_sponsor_card",
                "sponsor_card",
                "Nova sponsor card",
                None,
                Some("media_asset_nova_logo"),
                None,
                "granted",
                "good",
                json!({"promo_code":"VANTA20","tracking":"streamvanta.tv/r/nova"}),
            ),
            (
                "source_countdown",
                "countdown_timer",
                "Live countdown",
                None,
                None,
                None,
                "granted",
                "good",
                json!({"seconds":180}),
            ),
            (
                "source_guest",
                "guest_feed",
                "Backstage guest",
                Some("guest:ike"),
                None,
                None,
                "granted",
                "warning",
                json!({"return_audio":"mix_minus"}),
            ),
            (
                "source_chat",
                "chat_overlay",
                "Audience chat lower rail",
                None,
                None,
                None,
                "granted",
                "good",
                json!({"density":"compact"}),
            ),
            (
                "source_browser",
                "browser_capture",
                "Launch page browser",
                None,
                None,
                Some("https://streamvanta.tv/r/nova"),
                "granted",
                "good",
                json!({"width":1280,"height":720}),
            ),
        ] {
            sqlx::query(
                "INSERT INTO obs_sources
                (id, creator_id, source_kind, display_name, device_id, media_asset_id, browser_url, default_settings_json, permission_state, health_state, created_at, updated_at)
                VALUES (?, 'creator_vanta_originals', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(kind)
            .bind(name)
            .bind(device)
            .bind(media)
            .bind(url)
            .bind(settings.to_string())
            .bind(permission)
            .bind(health)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }

        for (scene, source, order, x, y, w, h, opacity) in [
            (
                "scene_starting_soon",
                "source_countdown",
                1,
                760.0,
                470.0,
                400.0,
                120.0,
                1.0,
            ),
            (
                "scene_host_camera",
                "source_camera_a",
                1,
                0.0,
                0.0,
                1920.0,
                1080.0,
                1.0,
            ),
            (
                "scene_host_camera",
                "source_chat",
                2,
                1280.0,
                720.0,
                560.0,
                220.0,
                0.85,
            ),
            (
                "scene_product_demo",
                "source_screen",
                1,
                80.0,
                80.0,
                1300.0,
                820.0,
                1.0,
            ),
            (
                "scene_product_demo",
                "source_camera_a",
                2,
                1420.0,
                620.0,
                380.0,
                214.0,
                1.0,
            ),
            (
                "scene_sponsor_read",
                "source_camera_a",
                1,
                0.0,
                0.0,
                1920.0,
                1080.0,
                1.0,
            ),
            (
                "scene_sponsor_read",
                "source_sponsor_card",
                2,
                1040.0,
                120.0,
                680.0,
                380.0,
                0.96,
            ),
            (
                "scene_sponsor_read",
                "source_browser",
                3,
                1040.0,
                540.0,
                680.0,
                382.0,
                0.92,
            ),
        ] {
            self.create_instance_raw(scene, source, order, x, y, w, h, opacity)
                .await?;
        }
        self.seed_source_filters().await?;

        self.create_broadcast_with_id(
            broadcast,
            BroadcastInput {
                title: "Prime Launch Control Room".to_string(),
                category: "Technology".to_string(),
                visibility: "public".to_string(),
                latency_profile: "low".to_string(),
                recording_policy: "program_plus_isolated_audio".to_string(),
                archive_policy: "archive_to_vanta_asset".to_string(),
                scheduled_start: Some("2026-09-05T22:00:00Z".to_string()),
                sponsor_campaign_id: Some("campaign_nova_run".to_string()),
            },
        )
        .await?;
        self.create_binding(
            broadcast,
            collection,
            "scene_host_camera",
            "scene_product_demo",
        )
        .await?;
        self.seed_audio(broadcast).await?;
        self.seed_guest_room(broadcast).await?;
        self.seed_hotkeys().await?;
        self.seed_moderation(broadcast).await?;
        self.seed_audience_if_missing(broadcast).await?;
        self.seed_engagement_if_missing(broadcast).await?;
        self.create_cue_for_broadcast(broadcast, CueInput {
            cue_kind: "sponsor_read".to_string(),
            label: "Nova 30s read".to_string(),
            scheduled_at_seconds: Some(900.0),
            required_duration_seconds: Some(30.0),
            campaign_id: Some("campaign_nova_run".to_string()),
            scene_id: Some("scene_sponsor_read".to_string()),
            source_id: Some("source_sponsor_card".to_string()),
            requirements_json: Some(json!({"advertiser":"Nova","required_claims":["Use code VANTA20"],"prohibited_claims":["guaranteed results"],"proof":"capture card and live read marker"})),
        }).await?;
        self.save_preflight(PreflightInput {
            broadcast_id: broadcast.to_string(),
            collection_id: collection.to_string(),
        })
        .await?;
        Ok(())
    }

    async fn seed_scene_templates(&self) -> Result<(), ObsStoreError> {
        let now = now();
        for (id, label, kind, description, transition, duration, layout, requirements) in [
            (
                "template_dual_host_guest",
                "Dual Stream",
                "dual_stream",
                "Host and guest side-by-side with compact chat rail.",
                "fade",
                260,
                json!([
                    {"source_kind":"camera","order_index":1,"x":80.0,"y":120.0,"width":820.0,"height":462.0,"opacity":1.0},
                    {"source_kind":"guest_feed","order_index":2,"x":1020.0,"y":120.0,"width":820.0,"height":462.0,"opacity":1.0},
                    {"source_kind":"chat_overlay","order_index":3,"x":1240.0,"y":690.0,"width":560.0,"height":230.0,"opacity":0.86}
                ]),
                json!({"source_kinds":["camera","guest_feed"],"use_case":"dual_collaboration","value":"guest_production"}),
            ),
            (
                "template_screen_share",
                "Screen Share",
                "screen_share",
                "Shared screen or game feed with host picture-in-picture.",
                "cut",
                0,
                json!([
                    {"source_kind":"screen_capture","order_index":1,"x":80.0,"y":70.0,"width":1380.0,"height":860.0,"opacity":1.0},
                    {"source_kind":"camera","order_index":2,"x":1500.0,"y":650.0,"width":340.0,"height":192.0,"opacity":1.0}
                ]),
                json!({"source_kinds":["screen_capture","camera"],"use_case":"shared_screen_game","value":"collaboration"}),
            ),
            (
                "template_sponsor_read",
                "Sponsor Read",
                "sponsor_read",
                "Host camera with sponsor card and tracked browser proof surface.",
                "dip_to_black",
                420,
                json!([
                    {"source_kind":"camera","order_index":1,"x":0.0,"y":0.0,"width":1920.0,"height":1080.0,"opacity":1.0},
                    {"source_kind":"sponsor_card","order_index":2,"x":1030.0,"y":110.0,"width":700.0,"height":392.0,"opacity":0.96},
                    {"source_kind":"browser_capture","order_index":3,"x":1030.0,"y":540.0,"width":700.0,"height":392.0,"opacity":0.92}
                ]),
                json!({"source_kinds":["camera","sponsor_card"],"use_case":"sponsor_proof","value":"ad_inventory"}),
            ),
        ] {
            sqlx::query(
                "INSERT OR IGNORE INTO obs_scene_templates
                (id, creator_id, label, template_kind, description, transition_kind, transition_duration_ms, layout_json, requirements_json, created_at, updated_at)
                VALUES (?, 'creator_vanta_originals', ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(label)
            .bind(kind)
            .bind(description)
            .bind(transition)
            .bind(duration)
            .bind(layout.to_string())
            .bind(requirements.to_string())
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    async fn seed_source_filters(&self) -> Result<(), ObsStoreError> {
        let now = now();
        for (id, source, kind, label, enabled, index, settings, obs_kind) in [
            (
                "filter_camera_color",
                "source_camera_a",
                "color_correction",
                "Camera color balance",
                1,
                1,
                json!({"exposure":0.1,"contrast":1.05,"saturation":1.08}),
                "color_filter",
            ),
            (
                "filter_camera_sharpness",
                "source_camera_a",
                "sharpness",
                "Lens detail",
                1,
                2,
                json!({"amount":0.18}),
                "sharpness_filter_v2",
            ),
            (
                "filter_sponsor_crop",
                "source_sponsor_card",
                "crop_pad",
                "Sponsor safe crop",
                1,
                1,
                json!({"left":0,"top":0,"right":0,"bottom":0}),
                "crop_filter",
            ),
        ] {
            sqlx::query(
                "INSERT INTO obs_source_filters
                (id, creator_id, source_id, filter_kind, label, enabled, order_index, settings_json, obs_mapping_json, validation_json, created_at, updated_at)
                VALUES (?, 'creator_vanta_originals', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(source)
            .bind(kind)
            .bind(label)
            .bind(enabled)
            .bind(index)
            .bind(settings.to_string())
            .bind(json!({"obs_kind":obs_kind,"renderer_stage": if kind == "crop_pad" { "layout" } else { "video" }}).to_string())
            .bind(json!({"status":"ready","errors":[],"warnings":[]}).to_string())
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    async fn seed_hotkeys(&self) -> Result<(), ObsStoreError> {
        let now = now();
        for (id, scope, action, target, binding, guard) in [
            (
                "hotkey_send_product_demo",
                "scene",
                "scene.send_program",
                Some("scene_product_demo"),
                "Alt+Digit1",
                json!({"requires_preflight":false,"surface":"studio"}),
            ),
            (
                "hotkey_send_sponsor",
                "scene",
                "scene.send_program",
                Some("scene_sponsor_read"),
                "Alt+Digit2",
                json!({"requires_preflight":false,"surface":"studio"}),
            ),
            (
                "hotkey_replay_30",
                "runtime",
                "replay.save_30",
                None,
                "Alt+KeyR",
                json!({"duration_seconds":30,"sponsor_proof":true,"surface":"studio"}),
            ),
            (
                "hotkey_record",
                "recording",
                "recording.start",
                None,
                "Alt+KeyD",
                json!({"recording_mode":"program_plus_isolated_audio","surface":"studio"}),
            ),
            (
                "hotkey_go_live",
                "runtime",
                "broadcast.start",
                None,
                "Alt+KeyG",
                json!({"requires_preflight":true,"surface":"studio"}),
            ),
            (
                "hotkey_hold",
                "safety",
                "safety.hold",
                None,
                "Alt+KeyH",
                json!({"reason":"Hotkey emergency hold","surface":"studio"}),
            ),
        ] {
            sqlx::query(
                "INSERT INTO obs_hotkeys
                (id, creator_id, scope, action, target_id, binding, enabled, guard_json, created_at, updated_at)
                VALUES (?, 'creator_vanta_originals', ?, ?, ?, ?, 1, ?, ?, ?)",
            )
            .bind(id)
            .bind(scope)
            .bind(action)
            .bind(target)
            .bind(binding)
            .bind(guard.to_string())
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    async fn seed_guest_room(&self, broadcast: &str) -> Result<(), ObsStoreError> {
        let now = now();
        sqlx::query(
            "INSERT INTO obs_guest_rooms
            (id, broadcast_id, status, room_mode, max_participants, shared_program_context_json, routing_policy_json, created_at, updated_at)
            VALUES ('guest_room_prime_launch', ?, 'backstage_open', 'group', 8, ?, ?, ?, ?)",
        )
        .bind(broadcast)
        .bind(json!({"program_feed":"canonical","preview_scene":"scene_product_demo","shared_game_feed":true,"latency_target_ms":180}).to_string())
        .bind(json!({"transport":"selective_forwarding","bandwidth_policy":"preserve_host_program","degrade_guest_first":true,"mix_minus":true}).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO obs_guest_participants
            (id, room_id, broadcast_id, display_name, role, source_id, status, muted, solo, safety_disabled, invite_url, scene_id, return_feed_json, connection_health_json, isolated_recording_json, device_check_json, moderator_control_json, media_state_json, created_at, updated_at)
            VALUES ('guest_ike', 'guest_room_prime_launch', ?, 'Ike Backstage', 'guest', 'source_guest', 'backstage', 0, 0, 0, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(broadcast)
        .bind(format!("https://studio.vanta.local/guest/{broadcast}/guest_ike"))
        .bind(json!({"video":"program_return","audio":"mix_minus","shared_game_feed":"low_latency"}).to_string())
        .bind(json!({"status":"good","latency_ms":94,"packet_loss_percent":0.4,"recommended_layer":"720p30"}).to_string())
        .bind(json!({"status":"armed","audio":true,"video":true,"storage":"local_then_archive"}).to_string())
        .bind(json!({"status":"pending","camera":"pending","microphone":"pending","network":"pending","browser":"pending"}).to_string())
        .bind(json!({"status":"clear","last_action":"none","moderator_id":null}).to_string())
        .bind(json!({"speaking":false,"active_speaker":false,"audio_level_db":-80.0,"video_active":true,"score":0.0}).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn create_binding(
        &self,
        broadcast_id: &str,
        collection_id: &str,
        program_scene: &str,
        preview_scene: &str,
    ) -> Result<(), ObsStoreError> {
        let now = now();
        sqlx::query(
            "INSERT INTO obs_runtime_bindings
            (id, creator_id, broadcast_id, live_ingest_session_id, scene_collection_id, active_scene_id, program_scene_id, preview_scene_id, runtime_state, stream_state, recording_state, last_heartbeat_at, created_at, updated_at)
            VALUES (?, 'creator_vanta_originals', ?, 'ingest_prime_launch', ?, ?, ?, ?, 'ready', 'scheduled', 'armed', ?, ?, ?)",
        )
        .bind(id())
        .bind(broadcast_id)
        .bind(collection_id)
        .bind(program_scene)
        .bind(program_scene)
        .bind(preview_scene)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn seed_audio(&self, broadcast: &str) -> Result<(), ObsStoreError> {
        let now = now();
        for (id, source, label, kind, gain, muted, monitor) in [
            (
                "audio_host",
                Some("source_mic_host"),
                "Host lav",
                "microphone",
                -3.0,
                0,
                1,
            ),
            (
                "audio_desktop",
                Some("source_screen"),
                "Desktop",
                "screen",
                -9.0,
                0,
                0,
            ),
            (
                "audio_guest",
                Some("source_guest"),
                "Guest return",
                "guest",
                -5.0,
                0,
                1,
            ),
            ("audio_music", None, "Music bed", "media", -18.0, 1, 0),
            (
                "audio_program",
                None,
                "Program output",
                "program",
                0.0,
                0,
                1,
            ),
        ] {
            sqlx::query(
                "INSERT INTO obs_audio_channels
                (id, creator_id, source_id, broadcast_id, label, channel_kind, muted, solo, gain_db, monitor_enabled, program_enabled, delay_ms, filters_json, route_json, created_at, updated_at)
                VALUES (?, 'creator_vanta_originals', ?, ?, ?, ?, ?, 0, ?, ?, 1, 0, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(source)
            .bind(broadcast)
            .bind(label)
            .bind(kind)
            .bind(muted)
            .bind(gain)
            .bind(monitor)
            .bind(json!({"limiter":true,"noise_gate":kind == "microphone"}).to_string())
            .bind(json!({"program":true,"monitor":monitor == 1,"mix_minus":kind == "guest"}).to_string())
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    async fn seed_moderation(&self, broadcast: &str) -> Result<(), ObsStoreError> {
        let now = now();
        sqlx::query(
            "INSERT INTO obs_moderator_roles
            (id, broadcast_id, user_id, display_name, role, permissions_json, status, created_at, updated_at)
            VALUES ('moderator_primary', ?, 'user_producer_ike', 'Ike Producer', 'producer', ?, 'active', ?, ?)",
        )
        .bind(broadcast)
        .bind(json!(["queue", "pin", "terms"]).to_string())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO obs_blocked_terms (id, broadcast_id, term, action, created_at, updated_at)
            VALUES ('blocked_term_spam', ?, 'scam', 'hold', ?, ?)",
        )
        .bind(broadcast)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO obs_moderation_queue
            (id, broadcast_id, author_id, author_name, message, reason, status, moderator_id, created_at, updated_at)
            VALUES ('mod_queue_seed', ?, 'viewer_luna', 'Luna', 'Is the sponsor code live yet?', 'manual review', 'pending', NULL, ?, ?)",
        )
        .bind(broadcast)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO obs_pinned_messages
            (id, broadcast_id, author_name, message, status, pinned_at, unpinned_at, created_at, updated_at)
            VALUES ('pin_seed', ?, 'Vanta', 'Use code VANTA20 during the launch segment.', 'active', ?, NULL, ?, ?)",
        )
        .bind(broadcast)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn seed_audience_if_missing(&self, broadcast: &str) -> Result<(), ObsStoreError> {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM obs_audience_snapshots WHERE broadcast_id = ?",
        )
        .bind(broadcast)
        .fetch_one(&self.pool)
        .await?;
        if exists > 0 {
            return Ok(());
        }
        let now = now();
        sqlx::query(
            "INSERT INTO obs_audience_snapshots
            (id, broadcast_id, viewer_count, chat_messages_per_minute, tips_cents, subscriptions,
             revenue_cents, discovery_source, discovery_score, discovery_json, created_at)
            VALUES ('audience_seed', ?, 842, 96, 2499, 14, 4599, 'home_recommendation', 81.5, ?, ?)",
        )
        .bind(broadcast)
        .bind(json!({"tags":["launch","software","live-studio"],"surface":"vanta_live","ranking":"rising"}).to_string())
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn seed_engagement_if_missing(&self, broadcast: &str) -> Result<(), ObsStoreError> {
        let schedule_exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM obs_schedule_slots WHERE broadcast_id = ?")
                .bind(broadcast)
                .fetch_one(&self.pool)
                .await?;
        let now = now();
        if schedule_exists == 0 {
            sqlx::query(
                "INSERT INTO obs_schedule_slots
                (id, broadcast_id, title, starts_at, timezone, duration_minutes, status, reminder_json, created_at, updated_at)
                VALUES ('schedule_seed', ?, 'Prime Launch Live', '2026-08-26T20:00:00-04:00',
                'America/New_York', 90, 'scheduled', ?, ?, ?)",
            )
            .bind(broadcast)
            .bind(json!({"notify_followers":true,"reminder_minutes":[60,10],"surface":"vanta_live"}).to_string())
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }

        let poll_exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM obs_engagement_polls WHERE broadcast_id = ?")
                .bind(broadcast)
                .fetch_one(&self.pool)
                .await?;
        if poll_exists == 0 {
            sqlx::query(
                "INSERT INTO obs_engagement_polls
                (id, broadcast_id, poll_kind, question, options_json, status, duration_seconds,
                 opened_at, closed_at, created_at, updated_at)
                VALUES ('poll_seed', ?, 'poll', 'Which segment should we replay?', ?, 'open', 300, ?, NULL, ?, ?)",
            )
            .bind(broadcast)
            .bind(json!([
                {"id":"option_1","label":"Product demo","votes":0,"percent":0},
                {"id":"option_2","label":"Sponsor read","votes":0,"percent":0}
            ]).to_string())
            .bind(&now)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
            sqlx::query(
                "INSERT INTO obs_engagement_votes (id, poll_id, broadcast_id, option_id, voter_id, created_at)
                VALUES ('vote_seed_1', 'poll_seed', ?, 'option_1', 'viewer_luna', ?),
                       ('vote_seed_2', 'poll_seed', ?, 'option_2', 'viewer_ari', ?)",
            )
            .bind(broadcast)
            .bind(&now)
            .bind(broadcast)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }

        let alert_exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM obs_alert_events WHERE broadcast_id = ?")
                .bind(broadcast)
                .fetch_one(&self.pool)
                .await?;
        if alert_exists == 0 {
            sqlx::query(
                "INSERT INTO obs_alert_events
                (id, broadcast_id, alert_kind, title, message, severity, source_user, amount_cents,
                 status, metadata_json, created_at, updated_at)
                VALUES ('alert_seed', ?, 'subscription', 'New sub', 'Luna subscribed during the launch.',
                'success', 'Luna', 0, 'ready', ?, ?, ?)",
            )
            .bind(broadcast)
            .bind(json!({"tier":"founder","surface":"studio"}).to_string())
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }
}
