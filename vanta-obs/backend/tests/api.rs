use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio_tungstenite::{accept_async, connect_async, tungstenite::Message};
use tower::ServiceExt;
use vanta_obs_backend::{
    app::{AppState, app_state_from_stores, build_app, connect_stores},
    media::{service::MediaService, store::MediaStore},
    native::{service::NativeService, store::NativeStore},
    obs::{
        bridge::{
            ObsBridgeAudioInput, ObsBridgeClient, ObsBridgeCommand, ObsBridgeCommandResult,
            ObsBridgeError, ObsBridgeProfile, ObsBridgeScene, ObsBridgeSceneItem,
            ObsBridgeSnapshot, ObsBridgeSource, ObsBridgeTransition, bridge_warning,
            websocket::LocalObsWebSocketClient,
        },
        export::{ObsExportInput, build_obs_export_package},
        runtime::stream_snapshot,
        service::ObsService,
        store::ObsStore,
        vendor::{validate_vendored_obs_approval, vendored_obs_policy},
    },
};

#[tokio::test]
async fn sqlite_migrations_are_idempotent_across_all_durable_stores() {
    let database_path = std::env::temp_dir().join(format!(
        "vanta-obs-migration-{}.sqlite",
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));

    let (obs, _native, _media) = connect_stores(&database_path).await.unwrap();
    obs.seed().await.unwrap();
    let pool = obs.pool();
    assert_migrated_table_family(
        &pool,
        &[
            "obs_scene_collections",
            "obs_scenes",
            "obs_sources",
            "obs_guest_rooms",
            "obs_guest_participants",
            "obs_guest_webrtc_sessions",
            "obs_guest_media_relays",
            "obs_guest_rtp_packets",
            "obs_guest_media_worker_frames",
            "obs_guest_decoded_media_frames",
            "obs_guest_media_route_frames",
            "obs_guest_media_sync_pairs",
            "obs_guest_compositor_frames",
            "obs_guest_compositor_playout_frames",
            "obs_runtime_live_feed_sessions",
            "obs_guest_return_feed_sessions",
            "obs_guest_isolated_recordings",
            "obs_recording_jobs",
            "obs_runtime_bindings",
            "vanta_live_ingest_sessions",
            "vanta_live_runtime_outputs",
            "vanta_live_authoritative_bindings",
            "vanta_live_authoritative_events",
            "vanta_media_assets",
        ],
    )
    .await;
    assert_migrated_table_family(
        &pool,
        &[
            "native_helper_sessions",
            "native_helper_events",
            "native_helper_logs",
            "media_capture_sessions",
            "media_capture_frames",
            "media_capture_artifacts",
            "media_encode_jobs",
            "media_source_artifacts",
            "media_packages",
        ],
    )
    .await;
    assert_seed_counts(&pool, 1, 5, 3, 1).await;
    pool.close().await;

    let (obs, _native, _media) = connect_stores(&database_path).await.unwrap();
    obs.seed().await.unwrap();
    let reopened = obs.pool();
    assert_seed_counts(&reopened, 1, 5, 3, 1).await;
    assert_migrated_table_family(
        &reopened,
        &[
            "obs_scene_collections",
            "obs_scenes",
            "obs_sources",
            "obs_guest_rooms",
            "obs_guest_participants",
            "obs_guest_webrtc_sessions",
            "obs_guest_media_relays",
            "obs_guest_rtp_packets",
            "obs_guest_media_worker_frames",
            "obs_guest_decoded_media_frames",
            "obs_guest_media_route_frames",
            "obs_guest_media_sync_pairs",
            "obs_guest_compositor_frames",
            "obs_guest_compositor_playout_frames",
            "obs_guest_return_feed_sessions",
            "obs_guest_isolated_recordings",
            "obs_runtime_events",
            "vanta_live_authoritative_bindings",
            "vanta_live_authoritative_events",
            "native_helper_sessions",
            "media_capture_sessions",
        ],
    )
    .await;
    reopened.close().await;

    let _ = tokio::fs::remove_file(database_path).await;
}

#[tokio::test]
async fn studio_api_flow_exercises_current_routes() {
    let app = test_app().await;

    let health = call_json(app.clone(), Method::GET, "/health", None).await;
    assert_eq!(health["status"], "ok");

    let dashboard = call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let broadcast_id = dashboard["broadcast"]["id"].as_str().unwrap().to_string();
    let collection_id = dashboard["collection"]["id"].as_str().unwrap().to_string();
    let seeded_scene_id = dashboard["scenes"][0]["id"].as_str().unwrap().to_string();
    let seeded_source_id = dashboard["sources"][0]["id"].as_str().unwrap().to_string();
    let sponsor_cue_id = dashboard["cues"][0]["id"].as_str().unwrap().to_string();

    assert_eq!(dashboard["runtime"]["stream_state"], "scheduled");
    assert_eq!(
        dashboard["runtime"]["runtime_status_json"]["source_validation"]["status"],
        "ready"
    );
    assert_eq!(
        dashboard["runtime"]["runtime_status_json"]["packaging"]["status"],
        "ready"
    );
    assert_eq!(
        dashboard["runtime"]["runtime_status_json"]["native_fallback"]["status"],
        "browser_preview_external_ingest"
    );
    assert_eq!(
        dashboard["runtime"]["runtime_status_json"]["native_fallback"]["external_ingest"]["available"],
        true
    );
    assert!(dashboard["scenes"].as_array().unwrap().len() >= 5);
    assert_eq!(dashboard["scene_templates"].as_array().unwrap().len(), 3);
    assert!(dashboard["sources"].as_array().unwrap().len() >= 8);
    let program_validated_scene = dashboard["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|scene| {
            scene["scene_validation_json"]["role"]
                .as_str()
                .unwrap()
                .contains("program")
        })
        .unwrap();
    assert_eq!(
        program_validated_scene["scene_validation_json"]["status"],
        "ready"
    );
    assert!(
        program_validated_scene["scene_validation_json"]["visible_instances"]
            .as_i64()
            .unwrap()
            > 0
    );
    let host_audio = dashboard["audio"]
        .as_array()
        .unwrap()
        .iter()
        .find(|channel| channel["id"] == "audio_host")
        .unwrap();
    assert_eq!(host_audio["audio_graph_json"]["buses"]["program"], true);
    assert_eq!(
        host_audio["audio_graph_json"]["filters"]["noise_gate"],
        true
    );
    assert!(
        host_audio["audio_graph_json"]["meter"]["level_percent"]
            .as_i64()
            .unwrap()
            > 0
    );
    let camera_source = dashboard["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["source_kind"] == "camera")
        .unwrap();
    assert_eq!(
        camera_source["source_contract_json"]["renderer"],
        "device_video"
    );
    assert_eq!(camera_source["source_permission_json"]["kind"], "camera");
    assert_eq!(camera_source["source_sync_json"]["status"], "ready");
    assert!(
        camera_source["filters_chain_json"]
            .as_array()
            .unwrap()
            .iter()
            .any(|filter| filter["filter_kind"] == "color_correction"
                && filter["filter_contract_json"]["obs_kind"] == "color_filter")
    );
    assert_eq!(dashboard["guests"]["room_mode"], "group");
    assert_eq!(dashboard["guests"]["max_participants"], 8);
    assert!(
        dashboard["guests"]["modes_supported_json"]
            .as_array()
            .unwrap()
            .iter()
            .any(|mode| mode == "shared_game")
    );
    assert_eq!(
        dashboard["guests"]["routing_policy_json"]["transport"],
        "selective_forwarding"
    );
    let seeded_guest = dashboard["guests"]["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == "guest_ike")
        .unwrap();
    assert_eq!(seeded_guest["status"], "backstage");
    assert_eq!(seeded_guest["return_feed_json"]["audio"], "mix_minus");
    assert_eq!(seeded_guest["connection_health_json"]["status"], "good");
    assert!(
        dashboard["hotkeys"]
            .as_array()
            .unwrap()
            .iter()
            .any(|hotkey| hotkey["action"] == "scene.send_program"
                && hotkey["binding"] == "Alt+Digit1")
    );
    let replay_hotkey_id = dashboard["hotkeys"]
        .as_array()
        .unwrap()
        .iter()
        .find(|hotkey| hotkey["action"] == "replay.save_30")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let scene_hotkey_id = dashboard["hotkeys"]
        .as_array()
        .unwrap()
        .iter()
        .find(|hotkey| hotkey["target_id"] == "scene_sponsor_read")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let patched_hotkey = call_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/obs/me/hotkeys/{scene_hotkey_id}"),
        Some(json!({
            "binding": "Alt+Digit3",
            "enabled": false
        })),
    )
    .await;
    assert_eq!(patched_hotkey["binding"], "Alt+Digit3");
    assert_eq!(patched_hotkey["enabled"], 0);
    let ignored_hotkey = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/hotkeys/{scene_hotkey_id}/trigger"),
        None,
    )
    .await;
    assert_eq!(ignored_hotkey["status"], "ignored");
    let patched_hotkey = call_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/obs/me/hotkeys/{scene_hotkey_id}"),
        Some(json!({
            "enabled": true
        })),
    )
    .await;
    assert_eq!(patched_hotkey["enabled"], 1);

    let collections = call_json(
        app.clone(),
        Method::GET,
        "/api/v1/obs/me/scene-collections",
        None,
    )
    .await;
    assert_eq!(collections.as_array().unwrap().len(), 1);

    let collection = call_json(
        app.clone(),
        Method::GET,
        &format!("/api/v1/obs/me/scene-collections/{collection_id}"),
        None,
    )
    .await;
    assert_eq!(collection["collection"]["id"], collection_id);

    let templates = call_json(
        app.clone(),
        Method::GET,
        "/api/v1/obs/me/scene-templates",
        None,
    )
    .await;
    assert!(
        templates
            .as_array()
            .unwrap()
            .iter()
            .any(|template| template["id"] == "template_screen_share")
    );

    let initial_scene_ids = dashboard["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|scene| scene["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    let reordered_ids = initial_scene_ids.iter().rev().cloned().collect::<Vec<_>>();
    let reordered = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/scene-collections/{collection_id}/scenes/reorder"),
        Some(json!({ "scene_ids": reordered_ids })),
    )
    .await;
    assert_eq!(reordered["scenes"][0]["id"], initial_scene_ids[4]);
    assert_eq!(reordered["scenes"][0]["order_index"], 1);
    assert!(reordered["events"].as_array().unwrap().iter().any(|event| {
        event["event_kind"] == "scene_reorder" && event["message"] == "Scene rail order updated"
    }));
    let (bad_reorder_status, _) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/scene-collections/{collection_id}/scenes/reorder"),
        Some(json!({ "scene_ids": [initial_scene_ids[0]] })),
    )
    .await;
    assert_eq!(bad_reorder_status, StatusCode::BAD_REQUEST);
    let restored = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/scene-collections/{collection_id}/scenes/reorder"),
        Some(json!({ "scene_ids": initial_scene_ids })),
    )
    .await;
    assert_eq!(restored["scenes"][0]["id"], "scene_starting_soon");

    let templated = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/obs/me/scene-templates/template_screen_share/create",
        Some(json!({
            "collection_id": collection_id,
            "name": "Route Screen Template"
        })),
    )
    .await;
    let templated_scene = templated["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|scene| scene["name"] == "Route Screen Template")
        .unwrap();
    let templated_scene_id = templated_scene["id"].as_str().unwrap().to_string();
    assert_eq!(templated_scene["transition_kind"], "cut");
    assert_eq!(templated_scene["scene_validation_json"]["status"], "ready");
    assert!(
        templated["instances"]
            .as_array()
            .unwrap()
            .iter()
            .any(|instance| instance["scene_id"] == templated_scene_id
                && instance["source_id"] == "source_screen")
    );
    assert!(templated["events"].as_array().unwrap().iter().any(|event| {
        event["event_kind"] == "scene_template"
            && event["message"] == "Screen Share template created"
    }));

    let (locked_delete_status, _) = call_status_json(
        app.clone(),
        Method::DELETE,
        "/api/v1/obs/me/scenes/scene_emergency_holding",
        None,
    )
    .await;
    assert_eq!(locked_delete_status, StatusCode::BAD_REQUEST);
    let (active_delete_status, _) = call_status_json(
        app.clone(),
        Method::DELETE,
        "/api/v1/obs/me/scenes/scene_host_camera",
        None,
    )
    .await;
    assert_eq!(active_delete_status, StatusCode::BAD_REQUEST);
    let (program_delete_status, _) = call_status_json(
        app.clone(),
        Method::DELETE,
        "/api/v1/obs/me/scenes/scene_product_demo",
        None,
    )
    .await;
    assert_eq!(program_delete_status, StatusCode::BAD_REQUEST);

    let deleteable = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/obs/me/scenes",
        Some(json!({
            "collection_id": collection_id,
            "name": "Deleteable Scene",
            "transition_kind": "fade",
            "transition_duration_ms": 200
        })),
    )
    .await;
    let deleteable_id = deleteable["id"].as_str().unwrap().to_string();
    let after_delete = call_json(
        app.clone(),
        Method::DELETE,
        &format!("/api/v1/obs/me/scenes/{deleteable_id}"),
        None,
    )
    .await;
    assert!(
        after_delete["scenes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|scene| scene["id"] != deleteable_id)
    );
    assert!(
        after_delete["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| {
                event["event_kind"] == "scene_delete"
                    && event["message"]
                        .as_str()
                        .unwrap()
                        .contains("Deleteable Scene deleted")
            })
    );

    let scene = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/obs/me/scenes",
        Some(json!({
            "collection_id": collection_id,
            "name": "Route Test Scene",
            "transition_kind": "fade",
            "transition_duration_ms": 250
        })),
    )
    .await;
    let scene_id = scene["id"].as_str().unwrap().to_string();
    let empty_scene_dashboard =
        call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let empty_scene = empty_scene_dashboard["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|scene| scene["id"] == scene_id)
        .unwrap();
    assert_eq!(empty_scene["scene_validation_json"]["status"], "blocked");
    assert!(
        empty_scene["scene_validation_json"]["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error == "no_visible_sources")
    );

    let patched_scene = call_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/obs/me/scenes/{scene_id}"),
        Some(json!({
            "name": "Route Test Scene Updated",
            "locked": false,
            "validation_state": "ready",
            "transition_kind": "cut",
            "transition_duration_ms": 0
        })),
    )
    .await;
    assert_eq!(patched_scene["name"], "Route Test Scene Updated");

    let cut_preview = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/scenes/{scene_id}/transition-preview"),
        Some(json!({ "from_scene_id": "scene_product_demo" })),
    )
    .await;
    assert_eq!(cut_preview["transition"]["kind"], "cut");
    assert_eq!(cut_preview["transition"]["renderer"], "instant_swap");
    assert_eq!(cut_preview["transition"]["duration_ms"], 0);
    assert_eq!(cut_preview["transition"]["preview"], true);
    assert_eq!(
        cut_preview["transition"]["phases"][0]["action"],
        "swap_program"
    );

    let fade_preview = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/obs/me/scenes/scene_product_demo/transition-preview",
        Some(json!({ "from_scene_id": "scene_host_camera" })),
    )
    .await;
    assert_eq!(fade_preview["transition"]["kind"], "fade");
    assert_eq!(fade_preview["transition"]["renderer"], "crossfade");
    assert_eq!(
        fade_preview["transition"]["phases"][0]["action"],
        "crossfade_outgoing"
    );

    let dip_preview = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/obs/me/scenes/scene_sponsor_read/transition-preview",
        Some(json!({ "from_scene_id": "scene_product_demo" })),
    )
    .await;
    assert_eq!(dip_preview["transition"]["kind"], "dip_to_black");
    assert_eq!(dip_preview["transition"]["renderer"], "dip_color");
    assert_eq!(
        dip_preview["transition"]["phases"][1]["action"],
        "swap_program_under_black"
    );

    let _swipe_scene = call_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/obs/me/scenes/{scene_id}"),
        Some(json!({
            "transition_kind": "swipe",
            "transition_duration_ms": 420
        })),
    )
    .await;
    let swipe_preview = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/scenes/{scene_id}/transition-preview"),
        Some(json!({ "from_scene_id": "scene_product_demo" })),
    )
    .await;
    assert_eq!(swipe_preview["transition"]["renderer"], "directional_wipe");
    assert_eq!(
        swipe_preview["transition"]["phases"][0]["direction"],
        "left_to_right"
    );

    let _stinger_scene = call_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/obs/me/scenes/{scene_id}"),
        Some(json!({
            "transition_kind": "stinger",
            "transition_duration_ms": 900
        })),
    )
    .await;
    let stinger_preview = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/scenes/{scene_id}/transition-preview"),
        Some(json!({ "from_scene_id": "scene_product_demo" })),
    )
    .await;
    assert_eq!(stinger_preview["transition"]["renderer"], "stinger_overlay");
    assert_eq!(stinger_preview["transition"]["requires_media_asset"], true);
    assert_eq!(
        stinger_preview["transition"]["phases"][1]["action"],
        "swap_program_at_cut_point"
    );

    let _cut_scene = call_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/obs/me/scenes/{scene_id}"),
        Some(json!({
            "transition_kind": "cut",
            "transition_duration_ms": 0
        })),
    )
    .await;

    let grouped = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/scenes/{scene_id}/groups"),
        Some(json!({
            "child_scene_id": "scene_product_demo",
            "label": "Route nested product",
            "x": 120.0,
            "y": 120.0,
            "width": 760.0,
            "height": 428.0,
            "opacity": 0.9
        })),
    )
    .await;
    let group_source = grouped["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| {
            source["source_kind"] == "scene_group"
                && source["display_name"] == "Route nested product"
        })
        .unwrap();
    let group_source_id = group_source["id"].as_str().unwrap().to_string();
    assert_eq!(
        group_source["default_settings_json"]["scene_id"],
        "scene_product_demo"
    );
    assert!(
        grouped["instances"]
            .as_array()
            .unwrap()
            .iter()
            .any(|instance| {
                instance["scene_id"] == scene_id && instance["source_id"] == group_source_id
            })
    );
    let grouped_scene = grouped["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|scene| scene["id"] == scene_id)
        .unwrap();
    assert_eq!(grouped_scene["scene_validation_json"]["status"], "ready");

    let patched_group = call_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/obs/me/scene-groups/{group_source_id}"),
        Some(json!({
            "child_scene_id": "scene_sponsor_read",
            "label": "Route nested sponsor"
        })),
    )
    .await;
    let patched_group_source = patched_group["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["id"] == group_source_id)
        .unwrap();
    assert_eq!(patched_group_source["display_name"], "Route nested sponsor");
    assert_eq!(
        patched_group_source["default_settings_json"]["scene_id"],
        "scene_sponsor_read"
    );
    let (self_group_status, _) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/scenes/{scene_id}/groups"),
        Some(json!({
            "child_scene_id": scene_id,
            "label": "Bad self group"
        })),
    )
    .await;
    assert_eq!(self_group_status, StatusCode::BAD_REQUEST);
    let (cycle_group_status, _) = call_status_json(
        app.clone(),
        Method::POST,
        "/api/v1/obs/me/scenes/scene_sponsor_read/groups",
        Some(json!({
            "child_scene_id": scene_id,
            "label": "Bad cycle group"
        })),
    )
    .await;
    assert_eq!(cycle_group_status, StatusCode::BAD_REQUEST);

    let duplicated = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/scenes/{seeded_scene_id}/duplicate"),
        None,
    )
    .await;
    assert!(
        duplicated["scenes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|scene| scene["name"].as_str().unwrap().ends_with(" Copy"))
    );

    let source = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/obs/me/sources",
        Some(json!({
            "source_kind": "browser_capture",
            "display_name": "Route Test Browser",
            "browser_url": "https://streamvanta.tv/test",
            "settings_json": { "width": 1280, "height": 720 }
        })),
    )
    .await;
    let source_id = source["id"].as_str().unwrap().to_string();

    let patched_source = call_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/obs/me/sources/{seeded_source_id}"),
        Some(json!({
            "display_name": "Patched seeded source",
            "permission_state": "granted",
            "health_state": "good",
            "settings_json": { "density": "test" }
        })),
    )
    .await;
    assert_eq!(patched_source["display_name"], "Patched seeded source");
    assert_eq!(
        patched_source["default_settings_json"]["vanta_source"]["validation"]["status"],
        "ready"
    );

    let filter = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/sources/{seeded_source_id}/filters"),
        Some(json!({
            "filter_kind": "chroma_key",
            "label": "Route green screen",
            "order_index": 3,
            "settings_json": { "key_color": "#00ff00", "similarity": 0.23 }
        })),
    )
    .await;
    assert_eq!(filter["filter_kind"], "chroma_key");
    assert_eq!(
        filter["filter_contract_json"]["obs_kind"],
        "chroma_key_filter"
    );
    assert_eq!(filter["validation_json"]["status"], "ready");
    let filter_id = filter["id"].as_str().unwrap().to_string();

    let patched_filter = call_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/obs/me/source-filters/{filter_id}"),
        Some(json!({
            "label": "Route green screen tuned",
            "enabled": true,
            "settings_json": { "key_color": "#00ff00", "similarity": 0.18 }
        })),
    )
    .await;
    assert_eq!(patched_filter["label"], "Route green screen tuned");
    assert_eq!(patched_filter["enabled"], 1);

    let disabled_filter = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/source-filters/{filter_id}/disable"),
        None,
    )
    .await;
    assert_eq!(disabled_filter["enabled"], 0);
    let dashboard_after_filter =
        call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let camera_after_filter = dashboard_after_filter["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["id"] == seeded_source_id)
        .unwrap();
    assert!(
        camera_after_filter["filters_chain_json"]
            .as_array()
            .unwrap()
            .iter()
            .any(|filter| filter["id"] == filter_id && filter["enabled"] == 0)
    );

    let instance = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/scenes/{scene_id}/source-instances"),
        Some(json!({
            "source_id": source_id,
            "order_index": 1,
            "x": 64.0,
            "y": 48.0,
            "width": 1280.0,
            "height": 720.0
        })),
    )
    .await;
    let instance_id = instance["id"].as_str().unwrap().to_string();
    let scene_with_source_dashboard =
        call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let scene_with_source = scene_with_source_dashboard["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|scene| scene["id"] == scene_id)
        .unwrap();
    assert_eq!(
        scene_with_source["scene_validation_json"]["status"],
        "warning"
    );
    assert!(
        scene_with_source["scene_validation_json"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning.as_str().unwrap().contains("source_pending"))
    );

    let patched_instance = call_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/obs/me/source-instances/{instance_id}"),
        Some(json!({
            "visible": true,
            "locked": false,
            "opacity": 0.72
        })),
    )
    .await;
    assert_eq!(patched_instance["opacity"], 0.72);

    let program = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/scenes/{scene_id}/send-to-program"),
        None,
    )
    .await;
    assert_eq!(program["runtime"]["program_scene_id"], scene_id);
    assert_eq!(
        program["runtime"]["latest_transition_json"]["to_scene_id"],
        scene_id
    );
    assert_eq!(
        program["runtime"]["latest_transition_json"]["transition_kind"],
        "cut"
    );
    assert_eq!(
        program["runtime"]["latest_transition_json"]["duration_ms"],
        0
    );
    assert_eq!(
        program["runtime"]["latest_transition_json"]["status"],
        "completed"
    );
    assert_eq!(
        program["runtime"]["latest_transition_json"]["preview_json"]["applied_by_renderer"],
        true
    );
    assert_eq!(
        program["runtime"]["latest_transition_json"]["preview_json"]["renderer"],
        "instant_swap"
    );
    assert_eq!(
        program["runtime"]["latest_transition_json"]["preview_json"]["phases"][0]["action"],
        "swap_program"
    );
    assert!(program["events"].as_array().unwrap().iter().any(|event| {
        event["event_kind"] == "scene_program"
            && event["message"]
                .as_str()
                .unwrap()
                .contains("with cut over 0ms")
    }));

    let hotkey_program = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/hotkeys/{scene_hotkey_id}/trigger"),
        None,
    )
    .await;
    assert_eq!(hotkey_program["status"], "executed");
    assert_eq!(
        hotkey_program["dashboard"]["runtime"]["program_scene_id"],
        "scene_sponsor_read"
    );
    assert!(
        hotkey_program["dashboard"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["event_kind"] == "hotkey_trigger")
    );

    let preflight = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/obs/me/preflight",
        Some(json!({
            "broadcast_id": broadcast_id,
            "collection_id": program["collection"]["id"]
        })),
    )
    .await;
    assert_eq!(preflight["ready"], true);
    assert!(
        preflight["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| {
                warning
                    .as_str()
                    .unwrap()
                    .contains("browser preview plus external ingest fallback")
            })
    );

    let live = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/start"),
        None,
    )
    .await;
    assert_eq!(live["runtime"]["stream_state"], "live");
    assert_eq!(live["runtime"]["runtime_target_json"]["status"], "ready");
    assert!(
        ["srt", "rtmp", "webrtc"].contains(
            &live["runtime"]["runtime_target_json"]["protocol"]
                .as_str()
                .unwrap()
        )
    );
    assert_eq!(
        live["runtime"]["runtime_output_json"]["status"],
        "publishing"
    );
    let local_publish = &live["runtime"]["runtime_output_json"]["health_json"]["local_publish"];
    assert_eq!(local_publish["mode"], "ffmpeg_hls");
    assert_eq!(local_publish["status"], "publishing");
    assert_eq!(local_publish["validation"]["playable"], true);
    let publish_manifest = local_publish["manifest_path"].as_str().unwrap();
    assert!(
        std::path::Path::new(publish_manifest).is_file(),
        "expected stream manifest at {publish_manifest}"
    );
    let publish_segments = local_publish["segments"].as_array().unwrap();
    assert!(!publish_segments.is_empty());
    for segment in publish_segments {
        let path = segment["path"].as_str().unwrap();
        assert!(
            std::path::Path::new(path).is_file(),
            "expected stream segment at {path}"
        );
        assert!(segment["bytes"].as_u64().unwrap() > 0);
        assert_eq!(segment["sha256"].as_str().unwrap().len(), 64);
    }
    assert_eq!(
        live["runtime"]["playback_readiness_json"]["status"],
        "ready"
    );
    assert_eq!(live["health"]["viewer_playback_ready"], true);
    assert_eq!(live["health"]["reconnect_count"], 0);
    assert_eq!(
        live["runtime"]["runtime_status_json"]["reconnect"]["status"],
        "armed"
    );
    assert_eq!(
        live["runtime"]["runtime_status_json"]["playback_status"],
        "ready"
    );
    assert_eq!(
        live["runtime"]["runtime_status_json"]["native_fallback"]["browser_preview"]["available"],
        true
    );
    assert_eq!(
        live["runtime"]["authoritative_binding_json"]["authority"],
        "vanta_live"
    );
    assert_eq!(
        live["runtime"]["authoritative_binding_json"]["status"],
        "live"
    );
    assert_eq!(
        live["runtime"]["runtime_status_json"]["authoritative_vanta_live"]["source_of_truth"],
        "vanta_live_tables"
    );
    assert_eq!(
        live["runtime"]["authoritative_binding_json"]["last_snapshot_json"]["event_kind"],
        "stream_start"
    );
    assert!(
        live["runtime"]["authoritative_events_json"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["event_kind"] == "stream_start")
    );

    let degraded = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/runtime/errors"),
        Some(json!({
            "error_code": "ingest_jitter",
            "severity": "error",
            "message": "Primary ingest jitter exceeded live threshold",
            "source": "vanta_runtime",
            "operator_id": "runtime_monitor",
            "details_json": { "latency_ms": 2400, "dropped_frames": 94 }
        })),
    )
    .await;
    assert_eq!(degraded["runtime"]["runtime_state"], "degraded");
    assert_eq!(
        degraded["runtime"]["runtime_output_json"]["status"],
        "degraded"
    );
    assert_eq!(
        degraded["health"]["last_runtime_error"]["incident_kind"],
        "runtime_error"
    );
    assert_eq!(
        degraded["runtime"]["authoritative_binding_json"]["status"],
        "degraded"
    );
    assert_eq!(
        degraded["runtime"]["authoritative_binding_json"]["last_snapshot_json"]["event_kind"],
        "runtime_error"
    );
    assert!(
        degraded["runtime"]["authoritative_binding_json"]["version"]
            .as_i64()
            .unwrap()
            > live["runtime"]["authoritative_binding_json"]["version"]
                .as_i64()
                .unwrap()
    );
    assert!(
        degraded["runtime"]["authoritative_events_json"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["event_kind"] == "runtime_error"
                && event["payload_json"]["authority"] == "vanta_live")
    );
    assert!(degraded["events"].as_array().unwrap().iter().any(|event| {
        event["event_kind"] == "runtime_error"
            && event["severity"] == "error"
            && event["message"].as_str().unwrap().contains("ingest_jitter")
    }));
    let stream_payload = stream_snapshot(7, degraded.clone());
    assert_eq!(stream_payload["event_kind"], "runtime_snapshot");
    assert_eq!(stream_payload["sequence"], 7);
    assert_eq!(
        stream_payload["dashboard"]["runtime"]["runtime_state"],
        "degraded"
    );

    let recording = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/recording/start"),
        Some(json!({ "recording_mode": "program_plus_isolated_audio" })),
    )
    .await;
    assert_eq!(recording["status"], "recording");
    assert!(
        std::path::Path::new(recording["output_paths_json"]["manifest"].as_str().unwrap())
            .is_file()
    );

    let (blocked_stop_status, blocked_stop_body) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/recording/stop"),
        Some(json!({
            "operator_id": "creator_vanta_originals",
            "operator_role": "creator_owner",
            "acknowledged_risks": ["campaign_recording"]
        })),
    )
    .await;
    assert_eq!(blocked_stop_status, StatusCode::CONFLICT);
    assert!(
        blocked_stop_body["error"]
            .as_str()
            .unwrap()
            .contains("STOP RECORDING")
    );

    let stopped_recording = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/recording/stop"),
        Some(json!({
            "operator_id": "creator_vanta_originals",
            "operator_role": "creator_owner",
            "confirmation_text": "STOP RECORDING",
            "acknowledged_risks": ["campaign_recording"]
        })),
    )
    .await;
    assert_eq!(stopped_recording[0]["status"], "packaging");
    let recording_paths = &stopped_recording[0]["output_paths_json"];
    assert_eq!(recording_paths["integrity"]["status"], "verified");
    assert_eq!(recording_paths["integrity"]["segments_verified"], 2);
    assert_eq!(recording_paths["recovery"]["status"], "clean");
    assert_eq!(recording_paths["recovery"]["partial_cleanup"], "completed");
    assert_eq!(recording_paths["recovery"]["atomic_promotion"], true);
    assert_eq!(
        stopped_recording[0]["output_media_asset_id"],
        recording_paths["vanta_asset"]["asset_id"]
    );
    assert_eq!(recording_paths["vanta_asset"]["status"], "ready");
    assert_eq!(
        recording_paths["vanta_asset"]["asset_kind"],
        "recording_package"
    );
    let asset_manifest_path = recording_paths["vanta_asset"]["manifest_path"]
        .as_str()
        .unwrap();
    assert!(std::path::Path::new(asset_manifest_path).is_file());
    let manifest_path = recording_paths["manifest"].as_str().unwrap();
    assert!(std::path::Path::new(manifest_path).is_file());
    let manifest: Value = serde_json::from_slice(&std::fs::read(manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["integrity"]["status"], "verified");
    assert_eq!(manifest["segments"].as_array().unwrap().len(), 2);
    for segment in manifest["segments"].as_array().unwrap() {
        assert!(std::path::Path::new(segment["path"].as_str().unwrap()).is_file());
        assert_eq!(segment["verified"], true);
        assert_eq!(segment["sha256"].as_str().unwrap().len(), 64);
        assert_eq!(segment["validation"]["playable"], true);
        assert_eq!(segment["validation"]["has_audio"], true);
        if segment["feed"] == "isolated_audio" {
            assert_eq!(segment["validation"]["has_video"], false);
            assert!(segment["path"].as_str().unwrap().ends_with(".m4a"));
        } else {
            assert_eq!(segment["validation"]["has_video"], true);
            assert!(segment["path"].as_str().unwrap().ends_with(".mp4"));
        }
    }
    let asset_manifest: Value =
        serde_json::from_slice(&std::fs::read(asset_manifest_path).unwrap()).unwrap();
    assert_eq!(
        asset_manifest["asset_id"],
        recording_paths["vanta_asset"]["asset_id"]
    );
    for segment in asset_manifest["segments"].as_array().unwrap() {
        assert!(std::path::Path::new(segment["asset_path"].as_str().unwrap()).is_file());
    }

    let replay = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/replay-buffer/save"),
        Some(json!({
            "duration_seconds": 5,
            "label": "Route replay",
            "sponsor_proof": true
        })),
    )
    .await;
    assert_eq!(replay["status"], "clip_draft_ready");
    assert_eq!(replay["duration_seconds"], 5);
    assert_eq!(
        replay["clip_draft_json"]["manifest_json"]["validation"]["playable"],
        true
    );
    assert_eq!(
        replay["clip_draft_json"]["manifest_json"]["sponsor_proof"],
        true
    );
    assert_eq!(
        replay["clip_draft_json"]["manifest_json"]["buffer"]["kind"],
        "rolling_replay_buffer"
    );
    assert_eq!(
        replay["clip_draft_json"]["manifest_json"]["source"]["mode"],
        "native_live_media"
    );
    assert_eq!(
        replay["clip_draft_json"]["manifest_json"]["source"]["kind"],
        "recording_program_segment"
    );
    assert!(
        std::path::Path::new(
            replay["clip_draft_json"]["manifest_json"]["source"]["path"]
                .as_str()
                .unwrap()
        )
        .is_file()
    );
    assert_eq!(replay["clip_draft_json"]["buffer_json"]["status"], "ready");
    assert_eq!(
        replay["clip_draft_json"]["buffer_json"]["source"],
        "recording_program_segment"
    );
    assert!(
        replay["clip_draft_json"]["buffer_json"]["selected_segment_count"]
            .as_i64()
            .unwrap()
            >= 1
    );
    for segment in replay["clip_draft_json"]["buffer_json"]["segments"]
        .as_array()
        .unwrap()
    {
        assert!(std::path::Path::new(segment["artifact_path"].as_str().unwrap()).is_file());
        assert_eq!(segment["sha256"].as_str().unwrap().len(), 64);
        assert_eq!(segment["source_kind"], "recording_program_segment");
        assert_eq!(segment["native_live_source"], true);
    }
    assert_eq!(
        replay["clip_draft_json"]["pressure_json"]["retention_policy"]["eviction"],
        "oldest_first"
    );
    assert_eq!(
        replay["clip_draft_json"]["upload_queue_json"]["status"],
        "uploaded"
    );
    assert_eq!(
        replay["clip_draft_json"]["upload_queue_json"]["mode"],
        "instant_vanta_asset"
    );
    assert_eq!(
        replay["clip_draft_json"]["upload_queue_json"]["ready_for_upload"],
        true
    );
    assert_eq!(
        replay["clip_draft_json"]["vanta_asset_json"]["status"],
        "ready"
    );
    assert_eq!(
        replay["clip_draft_json"]["vanta_asset_json"]["asset_kind"],
        "replay_clip"
    );
    assert!(
        std::path::Path::new(
            replay["clip_draft_json"]["vanta_asset_json"]["asset_path"]
                .as_str()
                .unwrap()
        )
        .is_file()
    );
    assert!(
        std::path::Path::new(
            replay["clip_draft_json"]["vanta_asset_json"]["manifest_path"]
                .as_str()
                .unwrap()
        )
        .is_file()
    );
    assert_eq!(
        replay["clip_draft_json"]["vanta_asset_json"]["validation_json"]["playable"],
        true
    );
    assert_eq!(
        replay["clip_draft_json"]["vanta_asset_json"]["metadata_json"]["replay_source"]["kind"],
        "recording_program_segment"
    );
    assert!(
        replay["clip_draft_json"]["pressure_json"]["memory"]["estimated_uncompressed_bytes"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        std::path::Path::new(replay["clip_draft_json"]["output_path"].as_str().unwrap()).is_file()
    );

    let hotkey_replay = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/hotkeys/{replay_hotkey_id}/trigger"),
        None,
    )
    .await;
    assert_eq!(hotkey_replay["status"], "executed");
    assert!(
        hotkey_replay["dashboard"]["replays"]
            .as_array()
            .unwrap()
            .iter()
            .any(|replay| replay["duration_seconds"] == 30 && replay["sponsor_proof"] == 1)
    );

    let created_cue = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/cues"),
        Some(json!({
            "cue_kind": "lower_third",
            "label": "Route lower third",
            "scheduled_at_seconds": 120.0,
            "required_duration_seconds": 15.0,
            "campaign_id": "campaign_nova_run",
            "scene_id": scene_id,
            "source_id": source_id,
            "requirements_json": { "proof": "route test" }
        })),
    )
    .await;
    assert_eq!(created_cue["status"], "ready");

    let cue = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/live-cues/{sponsor_cue_id}/trigger"),
        None,
    )
    .await;
    assert_eq!(cue["status"], "shown_live");

    let guest_room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/guests/invite"),
        Some(json!({
            "display_name": "Route Guest",
            "role": "guest"
        })),
    )
    .await;
    let invited_guest = guest_room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["display_name"] == "Route Guest")
        .unwrap();
    assert_eq!(invited_guest["status"], "invited");
    assert!(
        invited_guest["invite_url"]
            .as_str()
            .unwrap()
            .contains("/guest/")
    );
    let participant_id = invited_guest["id"].as_str().unwrap().to_string();

    let dual_room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/guests/routing"),
        Some(json!({
            "room_mode": "dual",
            "max_participants": 2,
            "mirrored_channels": false,
            "latency_target_ms": 120
        })),
    )
    .await;
    assert_eq!(dual_room["room_mode"], "dual");
    assert_eq!(dual_room["max_participants"], 2);
    assert_eq!(
        dual_room["routing_policy_json"]["return_audio"],
        "mix_minus"
    );
    assert_eq!(
        dual_room["shared_program_context_json"]["layout_policy"]["layout"],
        "side_by_side"
    );

    let shared_game_room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/guests/routing"),
        Some(json!({
            "room_mode": "shared_game",
            "max_participants": 8,
            "shared_feed_source_id": "source_screen",
            "mirrored_channels": true,
            "latency_target_ms": 110
        })),
    )
    .await;
    assert_eq!(shared_game_room["room_mode"], "shared_game");
    assert_eq!(
        shared_game_room["shared_program_context_json"]["shared_feed_source_id"],
        "source_screen"
    );
    assert_eq!(
        shared_game_room["routing_policy_json"]["return_video"],
        "program_and_shared_feed"
    );
    assert_eq!(
        shared_game_room["routing_policy_json"]["mirrored_channels"],
        true
    );
    let routed_guest = shared_game_room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(
        routed_guest["return_feed_json"]["video"],
        "program_and_shared_feed"
    );
    assert_eq!(
        routed_guest["return_feed_json"]["shared_feed_source_id"],
        "source_screen"
    );

    let returned_room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/return-feed"),
        Some(json!({
            "audio_mode": "mix_minus",
            "video_mode": "shared_game",
            "transport": "vanta_realtime_sfu",
            "shared_feed_source_id": "source_screen",
            "target_latency_ms": 110,
            "audio_bitrate_kbps": 96,
            "video_bitrate_kbps": 3200
        })),
    )
    .await;
    let returned_guest = returned_room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(returned_guest["return_feed_json"]["status"], "ready");
    assert_eq!(returned_guest["return_feed_json"]["audio"], "mix_minus");
    assert_eq!(returned_guest["return_feed_json"]["video"], "shared_game");
    assert_eq!(
        returned_guest["return_feed_json"]["audio_track"]["codec"],
        "opus"
    );
    assert_eq!(
        returned_guest["return_feed_json"]["video_track"]["max_resolution"],
        "1080p60"
    );
    assert_eq!(
        returned_guest["return_feed_json"]["sync"]["priority"],
        "audio_continuity"
    );

    let isolated_started_room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/isolated-recording/start"),
        Some(json!({
            "recording_mode": "audio_video",
            "include_audio": true,
            "include_video": true
        })),
    )
    .await;
    let isolated_started_guest = isolated_started_room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(
        isolated_started_guest["isolated_recording_json"]["status"],
        "recording"
    );
    assert_eq!(
        isolated_started_guest["isolated_recording_json"]["tracks"]["audio"],
        true
    );
    assert_eq!(
        isolated_started_guest["isolated_recording_json"]["tracks"]["video"],
        true
    );

    let (duplicate_iso_status, duplicate_iso_body) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/isolated-recording/start"),
        Some(json!({
            "recording_mode": "audio_video",
            "include_audio": true,
            "include_video": true
        })),
    )
    .await;
    assert_eq!(duplicate_iso_status, StatusCode::BAD_REQUEST);
    assert!(
        duplicate_iso_body["error"]
            .as_str()
            .unwrap()
            .contains("already active")
    );

    let isolated_stopped_room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/isolated-recording/stop"),
        None,
    )
    .await;
    let isolated_stopped_guest = isolated_stopped_room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(
        isolated_stopped_guest["isolated_recording_json"]["status"],
        "ready"
    );
    assert_eq!(
        isolated_stopped_guest["isolated_recording_json"]["artifact"]["validation"]["playable"],
        true
    );
    assert_eq!(
        isolated_stopped_guest["isolated_recording_json"]["artifact"]["validation"]["has_audio"],
        true
    );
    assert_eq!(
        isolated_stopped_guest["isolated_recording_json"]["artifact"]["validation"]["has_video"],
        true
    );
    assert!(
        std::path::Path::new(
            isolated_stopped_guest["isolated_recording_json"]["artifact"]["path"]
                .as_str()
                .unwrap()
        )
        .is_file()
    );
    assert!(
        std::path::Path::new(
            isolated_stopped_guest["isolated_recording_json"]["artifact"]["manifest_path"]
                .as_str()
                .unwrap()
        )
        .is_file()
    );

    let (bad_iso_status, bad_iso_body) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/isolated-recording/start"),
        Some(json!({
            "recording_mode": "audio_video",
            "include_audio": false,
            "include_video": false
        })),
    )
    .await;
    assert_eq!(bad_iso_status, StatusCode::BAD_REQUEST);
    assert!(bad_iso_body["error"].as_str().unwrap().contains("tracks"));

    let (bad_return_status, bad_return_body) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/return-feed"),
        Some(json!({
            "audio_mode": "mix_minus",
            "video_mode": "shared_game",
            "target_latency_ms": 110
        })),
    )
    .await;
    assert_eq!(bad_return_status, StatusCode::BAD_REQUEST);
    assert!(
        bad_return_body["error"]
            .as_str()
            .unwrap()
            .contains("shared_feed_source_id")
    );

    let (bad_routing_status, bad_routing_body) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/guests/routing"),
        Some(json!({
            "room_mode": "shared_game",
            "max_participants": 8,
            "shared_feed_source_id": "source_camera_a"
        })),
    )
    .await;
    assert_eq!(bad_routing_status, StatusCode::BAD_REQUEST);
    assert!(
        bad_routing_body["error"]
            .as_str()
            .unwrap()
            .contains("shared feed source")
    );

    let checked_room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/device-check"),
        Some(json!({
            "camera_status": "ready",
            "microphone_status": "ready",
            "network_status": "ready",
            "browser_status": "ready",
            "bitrate_kbps": 2400,
            "round_trip_ms": 118,
            "packet_loss_percent": 0.4,
            "checks_json": {"surface": "api_test", "device_picker": "prejoin"}
        })),
    )
    .await;
    let checked_guest = checked_room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(checked_guest["device_check_json"]["status"], "ready");
    assert_eq!(
        checked_guest["device_check_json"]["checks"]["camera"],
        "ready"
    );
    assert_eq!(
        checked_guest["device_check_json"]["checks"]["thresholds"]["minimum_bitrate_kbps"],
        1200
    );
    assert_eq!(checked_guest["connection_health_json"]["status"], "good");
    assert_eq!(
        checked_guest["connection_health_json"]["recommended_layer"],
        "720p30"
    );

    let held_room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/moderation"),
        Some(json!({
            "action": "hold_backstage",
            "moderator_id": "producer_api",
            "reason": "Hold before talent approval"
        })),
    )
    .await;
    let held_guest = held_room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(held_guest["status"], "held");
    assert_eq!(held_guest["safety_disabled"], 1);
    assert_eq!(
        held_guest["moderator_control_json"]["action"],
        "hold_backstage"
    );
    assert_eq!(
        held_guest["moderator_control_json"]["moderator_id"],
        "producer_api"
    );

    let released_room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/moderation"),
        Some(json!({
            "action": "release_backstage",
            "moderator_id": "producer_api",
            "reason": "Device check cleared"
        })),
    )
    .await;
    let released_guest = released_room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(released_guest["status"], "backstage");
    assert_eq!(released_guest["safety_disabled"], 0);
    assert_eq!(
        released_guest["moderator_control_json"]["action"],
        "release_backstage"
    );

    let (bad_moderation_status, bad_moderation_body) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/moderation"),
        Some(json!({
            "action": "approve_live",
            "moderator_id": "producer_api",
            "reason": "Missing target scene"
        })),
    )
    .await;
    assert_eq!(bad_moderation_status, StatusCode::BAD_REQUEST);
    assert!(
        bad_moderation_body["error"]
            .as_str()
            .unwrap()
            .contains("target_scene_id")
    );

    let promoted_room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/promote/{scene_id}"),
        None,
    )
    .await;
    let promoted_guest = promoted_room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(promoted_guest["status"], "live");
    assert_eq!(promoted_guest["scene_id"], scene_id);
    assert_eq!(promoted_guest["return_feed_json"]["audio"], "mix_minus");
    assert_eq!(
        promoted_guest["connection_health_json"]["degrade_policy"],
        "guest_first"
    );
    assert_eq!(
        promoted_guest["isolated_recording_json"]["status"],
        "recording"
    );
    let guest_source_id = promoted_guest["source_id"].as_str().unwrap().to_string();

    let dashboard_after_guest =
        call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    assert!(
        dashboard_after_guest["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["id"] == guest_source_id && source["source_kind"] == "guest_feed")
    );
    assert!(
        dashboard_after_guest["instances"]
            .as_array()
            .unwrap()
            .iter()
            .any(|instance| instance["scene_id"] == scene_id
                && instance["source_id"] == guest_source_id)
    );

    let approved_room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/moderation"),
        Some(json!({
            "action": "approve_live",
            "moderator_id": "producer_api",
            "reason": "Producer approved live placement",
            "target_scene_id": scene_id
        })),
    )
    .await;
    let approved_guest = approved_room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(approved_guest["status"], "live");
    assert_eq!(approved_guest["scene_id"], scene_id);
    assert_eq!(
        approved_guest["moderator_control_json"]["action"],
        "approve_live"
    );
    assert_eq!(
        approved_guest["moderator_control_json"]["target_scene_id"],
        scene_id
    );

    let speaking_room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/media-telemetry"),
        Some(json!({
            "audio_level_db": -29.0,
            "speaking": true,
            "video_active": true,
            "round_trip_ms": 94,
            "packet_loss_percent": 0.3,
            "jitter_ms": 9,
            "dropped_frames": 0,
            "media_json": {"surface": "api_test", "track": "guest_microphone"}
        })),
    )
    .await;
    let speaking_guest = speaking_room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(speaking_guest["media_state_json"]["speaking"], true);
    assert_eq!(speaking_guest["media_state_json"]["active_speaker"], true);
    assert_eq!(
        speaking_guest["media_state_json"]["long_session"]["status"],
        "stable"
    );
    assert!(
        speaking_guest["media_state_json"]["long_session"]["sample_count"]
            .as_i64()
            .unwrap()
            >= 1
    );
    assert_eq!(
        speaking_room["shared_program_context_json"]["active_speaker"]["participant_id"],
        participant_id
    );
    assert_eq!(
        speaking_room["shared_program_context_json"]["active_speaker_policy"]["routing"],
        "layout_and_return_feed_priority"
    );

    let room_shifted_to_seeded_guest = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/obs/me/guests/guest_ike/media-telemetry",
        Some(json!({
            "audio_level_db": -18.0,
            "speaking": true,
            "video_active": true,
            "round_trip_ms": 88,
            "packet_loss_percent": 0.1,
            "jitter_ms": 5,
            "dropped_frames": 0,
            "media_json": {"surface": "api_test", "track": "seeded_guest_microphone"}
        })),
    )
    .await;
    assert_eq!(
        room_shifted_to_seeded_guest["shared_program_context_json"]["active_speaker"]["participant_id"],
        "guest_ike"
    );
    let shifted_guest = room_shifted_to_seeded_guest["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == "guest_ike")
        .unwrap();
    assert_eq!(shifted_guest["media_state_json"]["active_speaker"], true);

    let degraded_guest_room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/media-telemetry"),
        Some(json!({
            "audio_level_db": -34.0,
            "speaking": true,
            "video_active": true,
            "round_trip_ms": 1040,
            "packet_loss_percent": 11.5,
            "jitter_ms": 220,
            "dropped_frames": 144,
            "media_json": {"surface": "api_test", "track": "guest_camera_under_pressure"}
        })),
    )
    .await;
    let degraded_guest = degraded_guest_room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(
        degraded_guest["media_state_json"]["long_session"]["status"],
        "degrading"
    );
    assert_eq!(
        degraded_guest["media_state_json"]["long_session"]["cumulative_dropped_frames"],
        144
    );
    assert_eq!(
        degraded_guest["media_state_json"]["long_session"]["degradation_action"],
        "reduce_guest_video_layer_keep_audio_and_mix_minus"
    );

    let (bad_media_status, bad_media_body) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/media-telemetry"),
        Some(json!({
            "audio_level_db": 12.0,
            "speaking": true,
            "video_active": true,
            "round_trip_ms": 20,
            "packet_loss_percent": 0.0
        })),
    )
    .await;
    assert_eq!(bad_media_status, StatusCode::BAD_REQUEST);
    assert!(
        bad_media_body["error"]
            .as_str()
            .unwrap()
            .contains("audio_level_db")
    );

    let patched_guest_room = call_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/obs/me/guests/{participant_id}"),
        Some(json!({
            "muted": true,
            "solo": true
        })),
    )
    .await;
    let patched_guest = patched_guest_room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(patched_guest["muted"], 1);
    assert_eq!(patched_guest["solo"], 1);

    let disabled_guest_room = call_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/obs/me/guests/{participant_id}"),
        Some(json!({ "safety_disabled": true })),
    )
    .await;
    let disabled_guest = disabled_guest_room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(disabled_guest["status"], "disabled");
    assert_eq!(disabled_guest["safety_disabled"], 1);

    let blocked_check_room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/device-check"),
        Some(json!({
            "camera_status": "ready",
            "microphone_status": "denied",
            "network_status": "blocked",
            "browser_status": "ready",
            "bitrate_kbps": 400,
            "round_trip_ms": 650,
            "packet_loss_percent": 9.5,
            "checks_json": {"surface": "api_test", "reason": "mic_denied"}
        })),
    )
    .await;
    let blocked_guest = blocked_check_room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(blocked_guest["device_check_json"]["status"], "blocked");
    assert_eq!(blocked_guest["connection_health_json"]["status"], "blocked");
    assert_eq!(
        blocked_guest["connection_health_json"]["recommended_layer"],
        "hold_backstage"
    );

    let (bad_check_status, bad_check_body) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/device-check"),
        Some(json!({
            "camera_status": "ready",
            "microphone_status": "ready",
            "network_status": "ready",
            "browser_status": "ready",
            "bitrate_kbps": 1000,
            "round_trip_ms": 20,
            "packet_loss_percent": 101
        })),
    )
    .await;
    assert_eq!(bad_check_status, StatusCode::BAD_REQUEST);
    assert!(
        bad_check_body["error"]
            .as_str()
            .unwrap()
            .contains("packet_loss_percent")
    );

    let removed_guest_room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/remove"),
        None,
    )
    .await;
    let removed_guest = removed_guest_room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(removed_guest["status"], "removed");
    assert!(
        removed_guest["scene_id"]
            .as_str()
            .unwrap_or_default()
            .is_empty()
    );

    let ended = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/end"),
        Some(json!({
            "operator_id": "producer",
            "operator_role": "producer",
            "confirmation_text": "END STREAM",
            "acknowledged_risks": ["campaign_recording"]
        })),
    )
    .await;
    assert_eq!(ended["runtime"]["stream_state"], "ended");
    assert_eq!(ended["runtime"]["runtime_output_json"]["status"], "ended");
    assert_eq!(
        ended["runtime"]["runtime_status_json"]["packaging"]["status"],
        "packaging"
    );
    assert_eq!(
        ended["runtime"]["runtime_status_json"]["archive"]["status"],
        "ready"
    );
    assert_eq!(
        ended["runtime"]["playback_readiness_json"]["status"],
        "ended"
    );

    let runtime = call_json(
        app.clone(),
        Method::GET,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/runtime"),
        None,
    )
    .await;
    assert_eq!(runtime["stream_state"], "ended");
    assert_eq!(runtime["runtime_output_json"]["status"], "ended");
    assert_eq!(
        runtime["runtime_status_json"]["source_validation"]["blocked"],
        0
    );

    let health = call_json(
        app.clone(),
        Method::GET,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/health"),
        None,
    )
    .await;
    assert_eq!(health["packaging_status"], "packaging");
    assert_eq!(
        health["native_fallback_json"]["external_ingest"]["available"],
        true
    );

    let post_show = call_json(
        app.clone(),
        Method::GET,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/post-show"),
        None,
    )
    .await;
    assert_eq!(post_show["status"], "packaging");
    for key in [
        "archive_manifest",
        "clip_pack",
        "sponsor_proof_export",
        "highlights_publish",
        "captions_vtt",
        "transcript",
    ] {
        assert!(
            std::path::Path::new(post_show["output_paths_json"][key].as_str().unwrap()).is_file(),
            "expected post-show artifact {key} to exist"
        );
    }
    assert_eq!(post_show["metrics_json"]["archive_integrity"], "ready");
    assert_eq!(
        post_show["metrics_json"]["archive_asset_status"],
        "published"
    );
    assert_eq!(post_show["metrics_json"]["highlights_status"], "published");
    assert!(
        post_show["metrics_json"]["thumbnail_count"]
            .as_i64()
            .unwrap()
            >= 1
    );
    let archive_manifest: Value = serde_json::from_slice(
        &std::fs::read(
            post_show["output_paths_json"]["archive_manifest"]
                .as_str()
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        archive_manifest["encoded_timeline"]["markers"]
            .as_array()
            .unwrap()
            .len(),
        post_show["metrics_json"]["clip_pack_count"]
            .as_i64()
            .unwrap() as usize
    );
    let clip_pack: Value = serde_json::from_slice(
        &std::fs::read(
            post_show["output_paths_json"]["clip_pack"]
                .as_str()
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(clip_pack["publish_ready"], true);
    assert!(!clip_pack["social_tags"].as_array().unwrap().is_empty());
    for clip in clip_pack["clips"].as_array().unwrap() {
        assert_eq!(clip["timeline"]["encoded_timeline_status"], "marked");
        assert_eq!(clip["publish"]["archive_attachment"], "attached");
        assert_eq!(clip["publish"]["status"], "published");
        assert!(std::path::Path::new(clip["thumbnail"]["path"].as_str().unwrap()).is_file());
        assert_eq!(clip["thumbnail"]["sha256"].as_str().unwrap().len(), 64);
    }
    let highlights: Value = serde_json::from_slice(
        &std::fs::read(
            post_show["output_paths_json"]["highlights_publish"]
                .as_str()
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(highlights["status"], "published");
    assert!(
        highlights["highlights"].as_array().unwrap().len()
            >= clip_pack["clips"].as_array().unwrap().len()
    );
    for asset_key in ["archive_asset", "highlights_asset"] {
        let asset = &post_show["output_paths_json"][asset_key];
        assert_eq!(asset["status"], "ready");
        assert!(std::path::Path::new(asset["manifest_path"].as_str().unwrap()).is_file());
    }
    assert_eq!(
        post_show["sponsor_proofs_json"]["kind"],
        "vanta_sponsor_proof_export"
    );
    assert_eq!(
        post_show["sponsor_proofs_json"]["review_status"],
        "ready_for_ad_ops"
    );

    let handoff = call_json(
        app,
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/send-to-editor"),
        None,
    )
    .await;
    assert_eq!(handoff["status"], "sent_to_editor");
    assert!(
        std::path::Path::new(
            handoff["output_paths_json"]["editor_handoff"]
                .as_str()
                .unwrap()
        )
        .is_file()
    );

    let fresh_broadcast = call_json(
        test_app().await,
        Method::POST,
        "/api/v1/obs/me/broadcasts",
        Some(json!({
            "title": "Route Created Broadcast",
            "category": "Technology",
            "visibility": "unlisted",
            "latency_profile": "low",
            "recording_policy": "program",
            "archive_policy": "archive_to_vanta_asset",
            "scheduled_start": null,
            "sponsor_campaign_id": null
        })),
    )
    .await;
    assert_eq!(fresh_broadcast["status"], "scheduled");
}

#[tokio::test]
async fn guest_multi_participant_shared_game_routing_has_dedicated_coverage() {
    let app = test_app().await;
    let dashboard = call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let broadcast_id = dashboard["broadcast"]["id"].as_str().unwrap().to_string();

    let mut participant_ids = vec!["guest_ike".to_string()];
    for display_name in ["Nova", "Rin", "Sol"] {
        let room = call_json(
            app.clone(),
            Method::POST,
            &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/guests/invite"),
            Some(json!({
                "display_name": display_name,
                "role": "guest"
            })),
        )
        .await;
        let participant_id = room["participants_json"]
            .as_array()
            .unwrap()
            .iter()
            .find(|participant| participant["display_name"] == display_name)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        participant_ids.push(participant_id);
    }

    let group_room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/guests/routing"),
        Some(json!({
            "room_mode": "group",
            "max_participants": 4,
            "latency_target_ms": 150
        })),
    )
    .await;
    assert_eq!(group_room["room_mode"], "group");
    assert_eq!(group_room["participants_json"].as_array().unwrap().len(), 4);

    let shared_room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/guests/routing"),
        Some(json!({
            "room_mode": "shared_game",
            "max_participants": 4,
            "shared_feed_source_id": "source_screen",
            "latency_target_ms": 110
        })),
    )
    .await;
    assert_eq!(shared_room["room_mode"], "shared_game");
    assert_eq!(
        shared_room["shared_program_context_json"]["shared_feed_source_id"],
        "source_screen"
    );
    assert_eq!(
        shared_room["routing_policy_json"]["room_mode"],
        "shared_game"
    );
    assert_eq!(
        shared_room["routing_policy_json"]["transport"],
        "selective_forwarding"
    );
    assert_eq!(
        shared_room["shared_program_context_json"]["media_transport"]["transport"],
        "selective_forwarding"
    );
    assert_eq!(
        shared_room["shared_program_context_json"]["media_transport"]["shared_feed"]["source_id"],
        "source_screen"
    );
    assert_eq!(
        shared_room["routing_policy_json"]["media_plan"]["degradation"]["host_program_protected"],
        true
    );
    assert_eq!(
        shared_room["shared_program_context_json"]["media_transport"]["participant_plans"]
            .as_array()
            .unwrap()
            .len(),
        4
    );

    for participant_id in &participant_ids {
        let room = call_json(
            app.clone(),
            Method::POST,
            &format!("/api/v1/obs/me/guests/{participant_id}/return-feed"),
            Some(json!({
                "audio_mode": "mix_minus",
                "video_mode": "shared_game",
                "transport": "vanta_realtime_sfu",
                "shared_feed_source_id": "source_screen",
                "target_latency_ms": 110,
                "audio_bitrate_kbps": 96,
                "video_bitrate_kbps": 2200
            })),
        )
        .await;
        let participant = room["participants_json"]
            .as_array()
            .unwrap()
            .iter()
            .find(|participant| participant["id"] == participant_id.as_str())
            .unwrap();
        assert_eq!(participant["return_feed_json"]["status"], "ready");
        assert_eq!(participant["return_feed_json"]["video"], "shared_game");
        assert_eq!(
            participant["return_feed_json"]["shared_feed_source_id"],
            "source_screen"
        );
        assert_eq!(
            participant["return_feed_json"]["video_track"]["codec"],
            "h264"
        );
        assert_eq!(
            participant["return_feed_json"]["transport_plan"]["receive_policy"],
            "program_plus_shared_feed"
        );
        assert_eq!(
            participant["return_feed_json"]["transport_plan"]["video"]["shared_feed_layer"],
            "1080p60"
        );
    }

    for (participant_id, audio_level_db) in participant_ids.iter().zip([-36.0, -30.0, -9.0, -24.0])
    {
        call_json(
            app.clone(),
            Method::POST,
            &format!("/api/v1/obs/me/guests/{participant_id}/media-telemetry"),
            Some(json!({
                "audio_level_db": audio_level_db,
                "speaking": true,
                "video_active": true,
                "round_trip_ms": 65,
                "packet_loss_percent": 0.4,
                "jitter_ms": 12,
                "dropped_frames": 0
            })),
        )
        .await;
    }

    let dashboard = call_json(app, Method::GET, "/api/v1/obs/me/dashboard", None).await;
    assert_eq!(
        dashboard["guests"]["shared_program_context_json"]["active_speaker"]["participant_id"],
        participant_ids[2]
    );
    assert_eq!(
        dashboard["guests"]["shared_program_context_json"]["active_speaker_policy"]["routing"],
        "layout_and_return_feed_priority"
    );
    assert_eq!(
        dashboard["guests"]["shared_program_context_json"]["media_transport"]["active_speaker"]["participant_id"],
        participant_ids[2]
    );
    assert_eq!(
        dashboard["guests"]["shared_program_context_json"]["media_transport"]["degradation"]["weak_guest_policy"],
        "reduce_guest_layer_before_host_program"
    );
}

#[tokio::test]
async fn guest_webrtc_signaling_persists_offer_ice_answer_and_dashboard_state() {
    let app = test_app().await;
    let dashboard = call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let participant_id = dashboard["guests"]["participants_json"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let offer_sdp = "v=0\r\n\
o=- 461173305936321 2 IN IP4 127.0.0.1\r\n\
s=Vanta Guest\r\n\
t=0 0\r\n\
m=audio 9 UDP/TLS/RTP/SAVPF 111\r\n\
c=IN IP4 0.0.0.0\r\n\
a=rtpmap:111 opus/48000/2\r\n\
m=video 9 UDP/TLS/RTP/SAVPF 96\r\n\
a=rtpmap:96 H264/90000\r\n";
    let room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/webrtc/offer"),
        Some(json!({
            "session_role": "guest_publish",
            "direction": "sendrecv",
            "offer_sdp": offer_sdp,
            "audio": true,
            "video": true,
            "preferred_video_layer": "720p30",
            "tracks_json": {
                "audio": true,
                "video": true,
                "source": "browser_peer_connection"
            }
        })),
    )
    .await;
    let participant = room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    let session_id = participant["webrtc_session_json"]["id"].as_str().unwrap();
    assert!(session_id.starts_with("guest_webrtc_"));
    assert_eq!(
        participant["webrtc_session_json"]["status"],
        "awaiting_sfu_answer"
    );
    assert_eq!(
        participant["media_state_json"]["webrtc_session"]["status"],
        "awaiting_sfu_answer"
    );

    let room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/webrtc/{session_id}/ice"),
        Some(json!({
            "candidate": "candidate:0 1 UDP 2122252543 192.0.2.10 54400 typ host",
            "sdp_mid": "0",
            "sdp_mline_index": 0,
            "candidate_json": {"protocol": "udp", "source": "browser"}
        })),
    )
    .await;
    let participant = room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(
        participant["webrtc_session_json"]["health_json"]["ice_candidate_count"],
        1
    );

    let answer_sdp = "v=0\r\n\
o=- 461173305936322 2 IN IP4 127.0.0.1\r\n\
s=Vanta SFU\r\n\
t=0 0\r\n\
m=audio 9 UDP/TLS/RTP/SAVPF 111\r\n\
a=rtpmap:111 opus/48000/2\r\n\
m=video 9 UDP/TLS/RTP/SAVPF 96\r\n\
a=rtpmap:96 H264/90000\r\n";
    let room = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/webrtc/{session_id}/answer"),
        Some(json!({
            "answer_sdp": answer_sdp,
            "selected_video_layer": "720p30",
            "media_json": {"runtime": "vanta_realtime_sfu", "relay": "primary"}
        })),
    )
    .await;
    let participant = room["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(participant["webrtc_session_json"]["status"], "connected");
    assert_eq!(
        participant["media_state_json"]["webrtc_session"]["status"],
        "connected"
    );
    assert_eq!(participant["media_state_json"]["video_active"], true);

    call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/return-feed"),
        Some(json!({
            "audio_mode": "mix_minus",
            "video_mode": "program_return",
            "transport": "vanta_realtime_sfu",
            "target_latency_ms": 110,
            "audio_bitrate_kbps": 96,
            "video_bitrate_kbps": 1800
        })),
    )
    .await;
    let relay_result = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/obs/me/broadcasts/broadcast_prime_launch/guests/relays/reconcile",
        None,
    )
    .await;
    assert_eq!(relay_result["status"], "ready");
    assert_eq!(relay_result["relays_json"].as_array().unwrap().len(), 1);
    let relay = &relay_result["relays_json"][0];
    assert_eq!(relay["status"], "relaying");
    assert_eq!(relay["relay_kind"], "webrtc_sfu");
    assert_eq!(
        relay["route_json"]["program_composition"]["track_selector"],
        "remote_guest_av"
    );
    assert_eq!(relay["archive_manifest_json"]["status"], "armed");
    assert_eq!(
        relay_result["runtime"]["runtime_status_json"]["guest_media_relays"][0]["status"],
        "relaying"
    );
    let participant = relay_result["guest_room"]["participants_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|participant| participant["id"] == participant_id)
        .unwrap();
    assert_eq!(participant["media_relay_json"]["status"], "relaying");
    assert_eq!(
        participant["media_state_json"]["media_relay"]["transport"],
        "webrtc_sfu"
    );
    assert_eq!(
        participant["return_feed_json"]["media_ingress"],
        "webrtc_sfu"
    );
    let dashboard = call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let guest_source = dashboard["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["id"] == "source_guest")
        .unwrap();
    assert_eq!(
        guest_source["default_settings_json"]["relay_route"]["program_composition"]["track_selector"],
        "remote_guest_av"
    );
    let relay_id = relay["id"].as_str().unwrap();
    let first_packet = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/relays/{relay_id}/rtp"),
        Some(json!({
            "payload_kind": "video",
            "packet_base64": sample_rtp_packet_base64(120, 90000, 0x11223344, true),
            "received_at_ms": 1000,
            "metadata_json": {"codec": "h264", "frame": "idr"}
        })),
    )
    .await;
    assert_eq!(first_packet["status"], "accepted");
    assert_eq!(first_packet["packet"]["sequence_number"], 120);
    assert_eq!(first_packet["packet"]["payload_bytes"], 4);
    assert_eq!(first_packet["packet"]["packet_order"], "first");
    assert_eq!(first_packet["packet"]["playout_at_ms"], 1070);
    assert_eq!(first_packet["frame"]["status"], "ready_for_playout");
    assert_eq!(
        first_packet["frame"]["depacketizer"]["mode"],
        "rtp_marker_delimited_access_unit"
    );
    assert_eq!(first_packet["frame"]["playout"]["target_buffer_ms"], 70);
    assert_eq!(
        first_packet["frame"]["access_unit"]["format"],
        "h264_annex_b"
    );
    assert_eq!(
        first_packet["frame"]["access_unit"]["ready_for_decode"],
        true
    );
    assert_eq!(first_packet["frame"]["access_unit"]["packet_count"], 1);
    assert_eq!(
        first_packet["frame"]["access_unit"]["base64"],
        "AAAAAWWIhCE="
    );
    assert_eq!(
        first_packet["frame"]["access_unit"]["nal_units"][0]["packetization"],
        "single_nal"
    );
    assert_eq!(
        first_packet["frame"]["decoded_frame"]["status"],
        "decode_failed"
    );
    assert_eq!(
        first_packet["frame"]["decoded_frame"]["reason"],
        "ffmpeg_rejected_access_unit"
    );
    assert_eq!(first_packet["relay"]["health_json"]["rtp_packet_count"], 1);
    assert_eq!(
        first_packet["relay"]["health_json"]["media_worker"]["stage"],
        "rtp_jitter_buffer"
    );
    assert_eq!(
        first_packet["relay"]["health_json"]["media_worker"]["target_buffer_ms"],
        70
    );
    assert_eq!(
        first_packet["relay"]["health_json"]["last_depacketized_frame"]["id"],
        first_packet["frame"]["id"]
    );
    assert_eq!(
        first_packet["relay"]["health_json"]["last_decoded_media_frame"]["status"],
        "decode_failed"
    );
    assert_eq!(
        first_packet["runtime"]["runtime_status_json"]["guest_media_relays"][0]["health_json"]["frame_clock"]
            ["program_sync"],
        "program_clock"
    );
    let second_packet = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/relays/{relay_id}/rtp"),
        Some(json!({
            "payload_kind": "video",
            "packet_base64": sample_rtp_packet_base64(123, 90300, 0x11223344, true),
            "received_at_ms": 1033,
            "metadata_json": {"codec": "h264", "frame": "p"}
        })),
    )
    .await;
    assert_eq!(second_packet["relay"]["health_json"]["rtp_packet_count"], 2);
    assert_eq!(second_packet["relay"]["health_json"]["dropped_packets"], 2);
    assert_eq!(second_packet["packet"]["packet_order"], "gap");
    assert_eq!(second_packet["packet"]["dropped_since_last"], 2);
    assert_eq!(second_packet["frame"]["playout"]["target_buffer_ms"], 110);
    assert_eq!(
        second_packet["relay"]["health_json"]["media_worker"]["ready_frames"],
        2
    );
    assert_eq!(
        second_packet["relay"]["health_json"]["media_worker"]["last_packet_order"],
        "gap"
    );
    assert_eq!(
        second_packet["runtime"]["authoritative_binding_json"]["last_snapshot_json"]["event_kind"],
        "guest_rtp_packet"
    );
    assert_eq!(
        second_packet["runtime"]["authoritative_binding_json"]["last_snapshot_json"]["payload"]["relay_id"],
        relay_id
    );
    assert_eq!(
        second_packet["guest_room"]["participants_json"][0]["media_state_json"]["media_relay"]["status"],
        "relaying"
    );
    let reordered_packet = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/relays/{relay_id}/rtp"),
        Some(json!({
            "payload_kind": "video",
            "packet_base64": sample_rtp_packet_base64(122, 90300, 0x11223344, true),
            "received_at_ms": 1040,
            "metadata_json": {"codec": "h264", "frame": "late-p"}
        })),
    )
    .await;
    assert_eq!(reordered_packet["status"], "accepted");
    assert_eq!(reordered_packet["packet"]["packet_order"], "out_of_order");
    assert_eq!(
        reordered_packet["relay"]["health_json"]["dropped_packets"],
        2
    );
    assert_eq!(
        reordered_packet["relay"]["health_json"]["last_sequence_number"],
        123
    );
    assert_eq!(
        reordered_packet["relay"]["health_json"]["media_worker"]["status"],
        "reordering"
    );
    assert_eq!(
        reordered_packet["relay"]["health_json"]["media_worker"]["reordered_packets"],
        1
    );
    assert_eq!(
        reordered_packet["relay"]["health_json"]["media_worker"]["target_buffer_ms"],
        140
    );
    let fu_a_start = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/relays/{relay_id}/rtp"),
        Some(json!({
            "payload_kind": "video",
            "packet_base64": sample_h264_fu_a_packet_base64(
                124,
                90600,
                0x11223344,
                true,
                false,
                &[0x88, 0x84]
            ),
            "received_at_ms": 1050,
            "metadata_json": {"codec": "h264", "frame": "idr-fragment-start"}
        })),
    )
    .await;
    assert_eq!(fu_a_start["packet"]["packet_order"], "in_order");
    assert_eq!(fu_a_start["frame"], Value::Null);
    let fu_a_end = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/relays/{relay_id}/rtp"),
        Some(json!({
            "payload_kind": "video",
            "packet_base64": sample_h264_fu_a_packet_base64(
                125,
                90600,
                0x11223344,
                false,
                true,
                &[0x21]
            ),
            "received_at_ms": 1060,
            "metadata_json": {"codec": "h264", "frame": "idr-fragment-end"}
        })),
    )
    .await;
    assert_eq!(fu_a_end["packet"]["packet_order"], "in_order");
    assert_eq!(fu_a_end["frame"]["access_unit"]["format"], "h264_annex_b");
    assert_eq!(fu_a_end["frame"]["access_unit"]["packet_count"], 2);
    assert_eq!(fu_a_end["frame"]["access_unit"]["base64"], "AAAAAWWIhCE=");
    assert_eq!(
        fu_a_end["frame"]["access_unit"]["nal_units"][0]["packetization"],
        "fu_a"
    );
    assert_eq!(
        fu_a_end["frame"]["access_unit"]["nal_units"][0]["fragment_start"],
        true
    );
    assert_eq!(
        fu_a_end["frame"]["access_unit"]["nal_units"][1]["fragment_end"],
        true
    );
    assert_eq!(
        fu_a_end["frame"]["decoded_frame"]["status"],
        "decode_failed"
    );
    let valid_nals = generated_h264_annex_b_nals().await;
    assert!(
        !valid_nals.is_empty(),
        "ffmpeg-generated H.264 fixture should include NAL payloads"
    );
    let valid_timestamp = 90900;
    let valid_start_sequence = 126;
    let mut decoded_packet = Value::Null;
    for (index, nal) in valid_nals.iter().enumerate() {
        decoded_packet = call_json(
            app.clone(),
            Method::POST,
            &format!("/api/v1/obs/me/guests/relays/{relay_id}/rtp"),
            Some(json!({
                "payload_kind": "video",
                "packet_base64": sample_rtp_payload_packet_base64(
                    valid_start_sequence + index as u16,
                    valid_timestamp,
                    0x11223344,
                    index + 1 == valid_nals.len(),
                    nal
                ),
                "received_at_ms": 1080 + index as i64,
                "metadata_json": {"codec": "h264", "frame": "ffmpeg-idr"}
            })),
        )
        .await;
    }
    assert_eq!(
        decoded_packet["frame"]["decoded_frame"]["status"],
        "decoded"
    );
    assert_eq!(
        decoded_packet["frame"]["decoded_frame"]["artifact_kind"],
        "guest_decoded_video_png"
    );
    assert_eq!(decoded_packet["frame"]["decoded_frame"]["decodeable"], true);
    assert_eq!(decoded_packet["frame"]["decoded_frame"]["width"], 16);
    assert_eq!(decoded_packet["frame"]["decoded_frame"]["height"], 16);
    let route_frames = decoded_packet["frame"]["decoded_frame"]["route_frames"]
        .as_array()
        .unwrap();
    assert_eq!(route_frames.len(), 3);
    assert!(route_frames.iter().any(|route| route["route_kind"] == "program_composition" && route["status"] == "ready"));
    assert!(
        route_frames
            .iter()
            .any(|route| route["route_kind"] == "return_feed" && route["status"] == "ready")
    );
    assert!(
        route_frames
            .iter()
            .any(|route| route["route_kind"] == "archive" && route["status"] == "ready")
    );
    assert!(
        route_frames
            .iter()
            .all(|route| route["program_sync"]["sync_policy"] == "decoded_frame_playout")
    );
    assert_eq!(
        decoded_packet["frame"]["decoded_frame"]["sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert!(
        std::path::Path::new(
            decoded_packet["frame"]["decoded_frame"]["artifact_path"]
                .as_str()
                .unwrap()
        )
        .is_file()
    );
    assert_eq!(
        decoded_packet["relay"]["health_json"]["last_decoded_media_frame"]["status"],
        "decoded"
    );
    let opus_payloads = generated_opus_packets().await;
    assert!(
        !opus_payloads.is_empty(),
        "ffmpeg-generated Opus fixture should include audio packets"
    );
    let mut decoded_audio_packet = Value::Null;
    for (index, payload) in opus_payloads.iter().take(2).enumerate() {
        decoded_audio_packet = call_json(
            app.clone(),
            Method::POST,
            &format!("/api/v1/obs/me/guests/relays/{relay_id}/rtp"),
            Some(json!({
                "payload_kind": "audio",
                "packet_base64": sample_rtp_audio_payload_packet_base64(
                    300 + index as u16,
                    48_000 + (index as u32 * 960),
                    0x55667788,
                    true,
                    payload
                ),
                "received_at_ms": 1120 + index as i64 * 20,
                "metadata_json": {"codec": "opus", "clock_rate": 48000}
            })),
        )
        .await;
    }
    assert_eq!(
        decoded_audio_packet["frame"]["decoded_frame"]["status"],
        "decoded"
    );
    assert_eq!(
        decoded_audio_packet["frame"]["decoded_frame"]["artifact_kind"],
        "guest_decoded_audio_wav"
    );
    assert_eq!(
        decoded_audio_packet["frame"]["decoded_frame"]["sample_rate"],
        48000
    );
    assert_eq!(
        decoded_audio_packet["frame"]["decoded_frame"]["channels"],
        1
    );
    assert!(
        decoded_audio_packet["frame"]["decoded_frame"]["duration_samples"]
            .as_i64()
            .unwrap()
            > 0
    );
    assert_eq!(
        decoded_audio_packet["frame"]["decoded_frame"]["sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert!(
        std::path::Path::new(
            decoded_audio_packet["frame"]["decoded_frame"]["artifact_path"]
                .as_str()
                .unwrap()
        )
        .is_file()
    );
    let audio_routes = decoded_audio_packet["frame"]["decoded_frame"]["route_frames"]
        .as_array()
        .unwrap();
    assert_eq!(audio_routes.len(), 3);
    assert!(
        audio_routes
            .iter()
            .all(|route| route["payload_kind"] == "audio" && route["status"] == "ready")
    );
    let sync_pairs = decoded_audio_packet["frame"]["decoded_frame"]["sync_pairs"]
        .as_array()
        .unwrap();
    assert_eq!(sync_pairs.len(), 3);
    assert!(sync_pairs.iter().all(|pair| {
        pair["program_sync"]["sync_policy"] == "audio_video_route_pair"
            && pair["audio_route_frame_id"]
                .as_str()
                .unwrap()
                .starts_with("guest_route_frame_")
            && pair["video_route_frame_id"]
                .as_str()
                .unwrap()
                .starts_with("guest_route_frame_")
            && pair["absolute_drift_ms"].as_i64().unwrap()
                <= pair["resync_threshold_ms"].as_i64().unwrap()
    }));
    assert!(
        sync_pairs
            .iter()
            .any(|pair| pair["route_kind"] == "program_composition")
    );
    let program_sync_pair = sync_pairs
        .iter()
        .find(|pair| pair["route_kind"] == "program_composition")
        .expect("program composition sync pair");
    assert_eq!(program_sync_pair["compositor_frame"]["status"], "ready");
    assert_eq!(
        program_sync_pair["compositor_frame"]["artifact_kind"],
        "guest_program_compositor_png"
    );
    assert_eq!(program_sync_pair["compositor_frame"]["width"], 1920);
    assert_eq!(program_sync_pair["compositor_frame"]["height"], 1080);
    assert_eq!(
        program_sync_pair["compositor_frame"]["layout"]["transform"]["fit"],
        "contain"
    );
    assert_eq!(
        program_sync_pair["compositor_frame"]["compositor"]["engine"],
        "ffmpeg_software_fallback"
    );
    assert!(
        program_sync_pair["compositor_frame"]["playout"]["program_frame_sequence"]
            .as_i64()
            .unwrap()
            >= 1
    );
    assert!(matches!(
        program_sync_pair["compositor_frame"]["playout"]["playout_status"]
            .as_str()
            .unwrap(),
        "paced" | "jitter_warning"
    ));
    assert_eq!(
        program_sync_pair["compositor_frame"]["playout"]["dropped_frames"],
        0
    );
    assert_eq!(
        program_sync_pair["compositor_frame"]["playout"]["runtime_delivery"]["playout_artifact"]["status"],
        "ready"
    );
    assert_eq!(
        program_sync_pair["compositor_frame"]["playout"]["runtime_delivery"]["playout_artifact"]["artifact_kind"],
        "guest_runtime_program_playout_mp4"
    );
    assert_eq!(
        program_sync_pair["compositor_frame"]["playout"]["runtime_delivery"]["playout_artifact"]["validation"]
            ["playable"],
        true
    );
    assert_eq!(
        program_sync_pair["compositor_frame"]["playout"]["runtime_delivery"]["playout_artifact"]["transport_contract"]
            ["fragmented_mp4"],
        true
    );
    assert_eq!(
        program_sync_pair["compositor_frame"]["playout"]["runtime_delivery"]["live_feed_session"]["continuity"]
            ["mode"],
        "sustained_runtime_gpu_live_feed"
    );
    assert!(
        program_sync_pair["compositor_frame"]["playout"]["runtime_delivery"]["live_feed_session"]["continuity"]
            ["delivered_chunks"]
            .as_i64()
            .unwrap()
            >= 1
    );
    assert_eq!(
        program_sync_pair["compositor_frame"]["playout"]["runtime_delivery"]["live_feed_session"]["continuity"]
            ["program_clock_paced"],
        true
    );
    assert!(
        std::path::Path::new(
            program_sync_pair["compositor_frame"]["playout"]["runtime_delivery"]
                ["playout_artifact"]["artifact_path"]
                .as_str()
                .unwrap()
        )
        .is_file()
    );
    assert_eq!(
        program_sync_pair["compositor_frame"]["sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert!(
        std::path::Path::new(
            program_sync_pair["compositor_frame"]["artifact_path"]
                .as_str()
                .unwrap()
        )
        .is_file()
    );
    assert_eq!(
        decoded_audio_packet["relay"]["health_json"]["last_media_sync_pair"]["program_sync"]["sync_policy"],
        "audio_video_route_pair"
    );
    assert_eq!(
        decoded_audio_packet["relay"]["health_json"]["last_compositor_frame"]["status"],
        "ready"
    );
    assert!(matches!(
            decoded_audio_packet["relay"]["health_json"]["last_compositor_playout_frame"]
                ["playout_status"]
                .as_str()
                .unwrap(),
            "paced" | "jitter_warning"
        ));

    let delayed_timestamp = 99_000;
    let delayed_start_sequence = 400;
    let mut delayed_video_packet = Value::Null;
    for (index, nal) in valid_nals.iter().enumerate() {
        delayed_video_packet = call_json(
            app.clone(),
            Method::POST,
            &format!("/api/v1/obs/me/guests/relays/{relay_id}/rtp"),
            Some(json!({
                "payload_kind": "video",
                "packet_base64": sample_rtp_payload_packet_base64(
                    delayed_start_sequence + index as u16,
                    delayed_timestamp,
                    0x11223344,
                    index + 1 == valid_nals.len(),
                    nal
                ),
                "received_at_ms": 2400 + index as i64,
                "metadata_json": {"codec": "h264", "frame": "ffmpeg-idr-delayed"}
            })),
        )
        .await;
    }
    assert_eq!(
        delayed_video_packet["frame"]["decoded_frame"]["status"],
        "decoded"
    );
    let delayed_audio_packet = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/relays/{relay_id}/rtp"),
        Some(json!({
            "payload_kind": "audio",
            "packet_base64": sample_rtp_audio_payload_packet_base64(
                500,
                96_000,
                0x55667788,
                true,
                &opus_payloads[0]
            ),
            "received_at_ms": 2420,
            "metadata_json": {"codec": "opus", "clock_rate": 48000}
        })),
    )
    .await;
    let delayed_playout =
        &delayed_audio_packet["relay"]["health_json"]["last_compositor_playout_frame"];
    assert!(matches!(
        delayed_playout["playout_status"].as_str().unwrap(),
        "dropped_frames" | "jitter_warning"
    ));
    assert!(
        delayed_playout["cumulative_dropped_frames"]
            .as_i64()
            .unwrap()
            > 0,
        "delayed guest playout should account for dropped frames"
    );
    assert_eq!(delayed_playout["pressure"]["level"], "high");
    assert_eq!(
        delayed_playout["pressure"]["degradation_action"],
        "request_lower_sfu_layer_and_hold_last_good_frame"
    );
    assert_eq!(
        delayed_playout["runtime_delivery"]["transport"],
        "vanta_realtime_sfu"
    );
    assert_eq!(
        delayed_playout["runtime_delivery"]["live_feed_session"]["status"],
        "degraded"
    );
    assert_eq!(
        delayed_playout["runtime_delivery"]["live_feed_session"]["pressure"]["cumulative_dropped_frames"],
        delayed_playout["cumulative_dropped_frames"]
    );
    assert_eq!(
        delayed_playout["runtime_delivery"]["live_feed_session"]["continuity"]["last_program_frame_sequence"],
        delayed_playout["program_frame_sequence"]
    );
    assert!(
        delayed_playout["runtime_delivery"]["live_feed_session"]["continuity"]["delivered_chunks"]
            .as_i64()
            .unwrap()
            >= 2
    );

    let (bad_rtp_status, _) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/relays/{relay_id}/rtp"),
        Some(json!({
            "payload_kind": "video",
            "packet_base64": "bm90LXJ0cA=="
        })),
    )
    .await;
    assert_eq!(bad_rtp_status, StatusCode::BAD_REQUEST);

    let (bad_offer_status, _) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/guests/{participant_id}/webrtc/offer"),
        Some(json!({
            "session_role": "guest_publish",
            "direction": "sendrecv",
            "offer_sdp": "not-sdp",
            "audio": true,
            "video": true
        })),
    )
    .await;
    assert_eq!(bad_offer_status, StatusCode::BAD_REQUEST);

    let (bad_candidate_status, _) = call_status_json(
        app,
        Method::POST,
        &format!("/api/v1/obs/me/guests/webrtc/{session_id}/ice"),
        Some(json!({
            "candidate": "not-a-candidate",
            "sdp_mline_index": 0
        })),
    )
    .await;
    assert_eq!(bad_candidate_status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn recording_modes_create_real_media_assets_without_streaming() {
    let app = test_app().await;
    let dashboard = call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let broadcast_id = dashboard["broadcast"]["id"].as_str().unwrap().to_string();
    assert_eq!(dashboard["runtime"]["stream_state"], "scheduled");

    for (mode, expected_feed, exercise_pause) in [
        ("program", "program", true),
        ("clean_feed", "clean_feed", false),
    ] {
        let recording = call_json(
            app.clone(),
            Method::POST,
            &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/recording/start"),
            Some(json!({ "recording_mode": mode })),
        )
        .await;
        assert_eq!(recording["status"], "recording");
        assert_eq!(recording["recording_mode"], mode);
        if exercise_pause {
            let (duplicate_status, duplicate_body) = call_status_json(
                app.clone(),
                Method::POST,
                &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/recording/start"),
                Some(json!({ "recording_mode": mode })),
            )
            .await;
            assert_eq!(duplicate_status, StatusCode::BAD_REQUEST);
            assert!(
                duplicate_body["error"]
                    .as_str()
                    .unwrap()
                    .contains("active recording")
            );
            let paused = call_json(
                app.clone(),
                Method::POST,
                &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/recording/pause"),
                None,
            )
            .await;
            assert_eq!(paused["status"], "paused");
            assert_eq!(paused["output_paths_json"]["timeline"]["status"], "paused");
            assert!(
                paused["output_paths_json"]["paused_at"]
                    .as_str()
                    .unwrap()
                    .contains('T')
            );
            let resumed = call_json(
                app.clone(),
                Method::POST,
                &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/recording/resume"),
                None,
            )
            .await;
            assert_eq!(resumed["status"], "recording");
            assert_eq!(
                resumed["output_paths_json"]["timeline"]["status"],
                "recording"
            );
            assert_eq!(
                resumed["output_paths_json"]["pause_ranges"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
        }

        let stopped = call_json(
            app.clone(),
            Method::POST,
            &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/recording/stop"),
            Some(json!({
                "operator_id": "creator_vanta_originals",
                "operator_role": "creator_owner",
                "confirmation_text": "STOP RECORDING",
                "acknowledged_risks": ["campaign_recording"]
            })),
        )
        .await;
        let current = &stopped[0];
        assert_eq!(current["status"], "packaging");
        assert_eq!(current["recording_mode"], mode);
        assert!(
            current["output_media_asset_id"]
                .as_str()
                .unwrap()
                .starts_with("media_asset_recording_")
        );
        let paths = &current["output_paths_json"];
        assert_eq!(paths["integrity"]["status"], "verified");
        assert_eq!(paths["recovery"]["status"], "clean");
        assert_eq!(paths["vanta_asset"]["status"], "ready");
        assert_eq!(paths["vanta_asset"]["asset_kind"], "recording_package");
        assert_eq!(
            paths["vanta_asset"]["asset_id"],
            current["output_media_asset_id"]
        );
        if exercise_pause {
            assert_eq!(paths["timeline"]["status"], "packaging");
            assert_eq!(paths["timeline"]["pause_count"], 1);
            assert_eq!(paths["pause_ranges"].as_array().unwrap().len(), 1);
            assert!(
                paths["timeline"]["media_duration_seconds"]
                    .as_i64()
                    .unwrap()
                    >= 1
            );
        }
        assert_eq!(paths["segments"].as_array().unwrap().len(), 1);
        let segment = &paths["segments"][0];
        assert_eq!(segment["feed"], expected_feed);
        assert_eq!(segment["validation"]["playable"], true);
        assert_eq!(segment["validation"]["has_video"], true);
        assert_eq!(segment["validation"]["has_audio"], true);
        assert!(std::path::Path::new(segment["path"].as_str().unwrap()).is_file());
        let asset_manifest_path = paths["vanta_asset"]["manifest_path"].as_str().unwrap();
        assert!(std::path::Path::new(asset_manifest_path).is_file());
        let asset_manifest: Value =
            serde_json::from_slice(&std::fs::read(asset_manifest_path).unwrap()).unwrap();
        assert_eq!(asset_manifest["segments"].as_array().unwrap().len(), 1);
        assert!(
            std::path::Path::new(
                asset_manifest["segments"][0]["asset_path"]
                    .as_str()
                    .unwrap()
            )
            .is_file()
        );
        let participant_archives = paths["participant_archives"].as_array().unwrap();
        assert_eq!(participant_archives.len(), 1);
        let participant_archive = &participant_archives[0];
        assert_eq!(participant_archive["participant_id"], "guest_ike");
        assert_eq!(participant_archive["display_name"], "Ike Backstage");
        assert_eq!(participant_archive["status"], "ready");
        assert_eq!(participant_archive["source_feed"], expected_feed);
        assert_eq!(
            participant_archive["source_mode"],
            "program_reference_until_guest_media_transport"
        );
        assert_eq!(participant_archive["validation"]["playable"], true);
        assert_eq!(participant_archive["validation"]["has_video"], true);
        assert_eq!(participant_archive["validation"]["has_audio"], true);
        assert_eq!(participant_archive["sha256"].as_str().unwrap().len(), 64);
        assert!(std::path::Path::new(participant_archive["path"].as_str().unwrap()).is_file());
        assert_eq!(
            paths["vanta_asset"]["participant_archives"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let dashboard_after_stop =
            call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
        let archived_guest = dashboard_after_stop["guests"]["participants_json"]
            .as_array()
            .unwrap()
            .iter()
            .find(|participant| participant["id"] == "guest_ike")
            .unwrap();
        assert_eq!(
            archived_guest["isolated_recording_json"]["status"],
            "archived_participant_package"
        );
        assert_eq!(
            archived_guest["isolated_recording_json"]["archive"]["status"],
            "ready"
        );
        assert_eq!(
            archived_guest["isolated_recording_json"]["archive"]["sha256"],
            participant_archive["sha256"]
        );
    }
}

#[tokio::test]
async fn recording_stop_persists_long_session_runtime_ledger() {
    let (app, pool) = test_app_with_pool().await;
    let dashboard = call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let broadcast_id = dashboard["broadcast"]["id"].as_str().unwrap().to_string();

    let recording = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/recording/start"),
        Some(json!({ "recording_mode": "program" })),
    )
    .await;
    let recording_id = recording["id"].as_str().unwrap();
    let long_session_started_at =
        (chrono::Utc::now() - chrono::Duration::seconds(3_665)).to_rfc3339();
    sqlx::query("UPDATE obs_recording_jobs SET started_at = ?, updated_at = ? WHERE id = ?")
        .bind(&long_session_started_at)
        .bind(&long_session_started_at)
        .bind(recording_id)
        .execute(&pool)
        .await
        .unwrap();

    let stopped = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/recording/stop"),
        Some(json!({
            "operator_id": "creator_vanta_originals",
            "operator_role": "creator_owner",
            "confirmation_text": "STOP RECORDING",
            "acknowledged_risks": ["campaign_recording"]
        })),
    )
    .await;
    let paths = &stopped[0]["output_paths_json"];
    let timeline = &paths["timeline"];
    let runtime_recording = &paths["runtime_recording"];
    assert_eq!(runtime_recording["status"], "chunked");
    assert_eq!(runtime_recording["chunk_target_seconds"], 1_800);
    assert_eq!(runtime_recording["logical_chunk_count"], 3);
    assert_eq!(runtime_recording["validation_window_capped"], true);
    assert_eq!(
        runtime_recording["output_strategy"],
        "chunked_runtime_ledger_with_validated_media_window"
    );
    assert!(
        timeline["captured_duration_seconds"].as_i64().unwrap() >= 3_600,
        "captured duration should reflect the long session"
    );
    assert_eq!(timeline["media_duration_seconds"], 30);
    assert_eq!(paths["segments"][0]["duration_seconds"], 30);
    assert_eq!(paths["segments"][0]["validation"]["playable"], true);
    assert_eq!(
        paths["vanta_asset"]["runtime_recording"]["logical_chunk_count"],
        3
    );
}

#[tokio::test]
async fn recording_discard_requires_confirmation_and_removes_artifacts() {
    let app = test_app().await;
    let dashboard = call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let broadcast_id = dashboard["broadcast"]["id"].as_str().unwrap().to_string();

    let recording = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/recording/start"),
        Some(json!({ "recording_mode": "program" })),
    )
    .await;
    assert_eq!(recording["status"], "recording");

    let stopped = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/recording/stop"),
        Some(json!({
            "operator_id": "creator_vanta_originals",
            "operator_role": "creator_owner",
            "confirmation_text": "STOP RECORDING",
            "acknowledged_risks": ["campaign_recording"]
        })),
    )
    .await;
    let packaged = &stopped[0];
    let recording_dir = packaged["output_paths_json"]["recording_dir"]
        .as_str()
        .unwrap()
        .to_string();
    let asset_dir = packaged["output_paths_json"]["vanta_asset"]["asset_dir"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(std::path::Path::new(&recording_dir).is_dir());
    assert!(std::path::Path::new(&asset_dir).is_dir());

    let (blocked_status, blocked_body) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/recording/discard"),
        Some(json!({
            "operator_id": "creator_vanta_originals",
            "operator_role": "creator_owner",
            "acknowledged_risks": ["campaign_recording"]
        })),
    )
    .await;
    assert_eq!(blocked_status, StatusCode::CONFLICT);
    assert!(
        blocked_body["error"]
            .as_str()
            .unwrap()
            .contains("DISCARD RECORDING")
    );
    assert!(std::path::Path::new(&recording_dir).is_dir());
    assert!(std::path::Path::new(&asset_dir).is_dir());

    let discarded = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/recording/discard"),
        Some(json!({
            "operator_id": "creator_vanta_originals",
            "operator_role": "creator_owner",
            "confirmation_text": "DISCARD RECORDING",
            "acknowledged_risks": ["campaign_recording"]
        })),
    )
    .await;
    assert_eq!(discarded["status"], "discarded");
    assert_eq!(discarded["output_paths_json"]["status"], "discarded");
    assert_eq!(
        discarded["output_paths_json"]["deleted"]["recording_dir"],
        true
    );
    assert_eq!(discarded["output_paths_json"]["deleted"]["asset_dir"], true);
    assert_eq!(
        discarded["output_paths_json"]["integrity"]["status"],
        "discarded"
    );
    assert!(!std::path::Path::new(&recording_dir).exists());
    assert!(!std::path::Path::new(&asset_dir).exists());

    let dashboard = call_json(app, Method::GET, "/api/v1/obs/me/dashboard", None).await;
    assert_eq!(dashboard["runtime"]["recording_state"], "discarded");
    assert_eq!(
        dashboard["runtime"]["latest_recording_json"]["status"],
        "discarded"
    );
    assert!(dashboard["events"].as_array().unwrap().iter().any(|event| {
        event["event_kind"] == "recording_discard" && event["severity"] == "warning"
    }));
}

#[tokio::test]
async fn audio_graph_patch_updates_buses_filters_meters_and_warnings() {
    let app = test_app().await;

    let patched = call_json(
        app.clone(),
        Method::PATCH,
        "/api/v1/obs/me/audio/channels/audio_guest",
        Some(json!({
            "muted": false,
            "solo": true,
            "gain_db": 24.0,
            "monitor_enabled": true,
            "program_enabled": true,
            "delay_ms": 120,
            "filters_json": {
                "noise_suppression": true,
                "noise_gate": true,
                "compressor": false,
                "limiter": false
            },
            "route_json": {
                "program": true,
                "monitor": true,
                "mix_minus": false,
                "isolated": true,
                "drift_correction": false
            }
        })),
    )
    .await;
    assert_eq!(patched["solo"], 1);
    assert_eq!(patched["gain_db"], 24.0);
    assert_eq!(patched["route_json"]["isolated"], true);

    let dashboard = call_json(app, Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let guest = dashboard["audio"]
        .as_array()
        .unwrap()
        .iter()
        .find(|channel| channel["id"] == "audio_guest")
        .unwrap();
    assert_eq!(guest["audio_graph_json"]["buses"]["mix_minus"], false);
    assert_eq!(
        guest["audio_graph_json"]["drift_correction"]["status"],
        "warning"
    );
    assert_eq!(
        guest["audio_graph_json"]["drift_correction"]["correction_active"],
        false
    );
    assert_eq!(
        guest["audio_graph_json"]["drift_correction"]["residual_drift_ms"],
        138
    );
    assert_eq!(guest["audio_graph_json"]["meter"]["clipping"], true);
    assert!(
        guest["audio_graph_json"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning == "clipping")
    );
    assert!(
        guest["audio_graph_json"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning == "guest_mix_minus_disabled")
    );
    assert!(
        guest["audio_graph_json"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning == "drift_correction_unlocked")
    );
    assert_eq!(guest["audio_mix_json"]["status"], "warning");
    assert_eq!(
        guest["audio_mix_json"]["drift_correction"]["status"],
        "warning"
    );
    assert_eq!(
        guest["audio_mix_json"]["drift_correction"]["uncorrected_channels"],
        1
    );

    let response = test_app()
        .await
        .oneshot(
            Request::builder()
                .method(Method::PATCH)
                .uri("/api/v1/obs/me/audio/channels/audio_host")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "gain_db": 40.0 }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn source_contracts_validate_permissions_and_sync_metadata() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/obs/me/sources")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_kind": "browser_capture",
                        "display_name": "Broken browser",
                        "settings_json": { "width": 1280, "height": 720 }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let source = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/obs/me/sources",
        Some(json!({
            "source_kind": "text",
            "display_name": "Lower scoreboard",
            "settings_json": { "text": "Quarterfinals" }
        })),
    )
    .await;
    assert_eq!(
        source["default_settings_json"]["vanta_source"]["contract"]["renderer"],
        "text_overlay"
    );
    assert_eq!(
        source["default_settings_json"]["vanta_source"]["permission"]["required"],
        false
    );
    assert_eq!(
        source["default_settings_json"]["vanta_source"]["local_sync"]["transport"],
        "inline"
    );

    let patched = call_json(
        app,
        Method::PATCH,
        &format!("/api/v1/obs/me/sources/{}", source["id"].as_str().unwrap()),
        Some(json!({
            "permission_state": "granted",
            "health_state": "good",
            "settings_json": { "text": "Finals" }
        })),
    )
    .await;
    assert_eq!(
        patched["default_settings_json"]["vanta_source"]["local_sync"]["status"],
        "ready"
    );
}

#[tokio::test]
async fn missing_resources_return_404_json_errors() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/obs/me/scenes/missing/send-to-program")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["error"], "not found");
}

#[tokio::test]
async fn service_boundary_rejects_non_vanta_source_kinds() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/obs/me/sources")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_kind": "obs_noise_effect_factory",
                        "display_name": "Not useful here"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        payload["error"],
        "invalid source_kind: is not supported by Vanta OBS"
    );
}

#[tokio::test]
async fn obs_bridge_sync_and_commands_are_mockable_and_value_filtered() {
    let bridge = Arc::new(MockBridgeClient::default());
    let app = test_app_with_bridge(bridge.clone()).await;

    let connection = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/obs/me/bridge/connections",
        Some(json!({
            "label": "Local OBS",
            "websocket_url": "ws://127.0.0.1:4455",
            "password": "secret",
            "auto_sync": true
        })),
    )
    .await;
    let connection_id = connection["id"].as_str().unwrap().to_string();
    assert_eq!(connection["sync_status"], "created");

    let fetched_connection = call_json(
        app.clone(),
        Method::GET,
        &format!("/api/v1/obs/me/bridge/connections/{connection_id}"),
        None,
    )
    .await;
    assert_eq!(fetched_connection["id"], connection_id);

    let connections = call_json(
        app.clone(),
        Method::GET,
        "/api/v1/obs/me/bridge/connections",
        None,
    )
    .await;
    assert_eq!(connections.as_array().unwrap().len(), 1);

    let synced = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/bridge/connections/{connection_id}/sync"),
        None,
    )
    .await;
    assert_eq!(synced["sync_status"], "synced");
    assert_eq!(
        synced["last_snapshot_json"]["current_program_scene"],
        "Host Camera"
    );
    assert_eq!(
        synced["last_snapshot_json"]["unsupported"][0]["code"],
        "unsupported_source_kind"
    );

    let command = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/bridge/connections/{connection_id}/program-scene"),
        Some(json!({ "scene_name": "Sponsor Read" })),
    )
    .await;
    assert_eq!(command["command"], "set_program_scene");
    assert_eq!(command["accepted"], true);

    call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/bridge/connections/{connection_id}/stream/start"),
        None,
    )
    .await;
    call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/bridge/connections/{connection_id}/stream/stop"),
        None,
    )
    .await;
    call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/bridge/connections/{connection_id}/recording/start"),
        None,
    )
    .await;
    call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/bridge/connections/{connection_id}/recording/stop"),
        None,
    )
    .await;
    call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/bridge/connections/{connection_id}/replay-buffer/save"),
        None,
    )
    .await;

    let events = call_json(
        app.clone(),
        Method::GET,
        &format!("/api/v1/obs/me/bridge/connections/{connection_id}/events"),
        None,
    )
    .await;
    assert!(events.as_array().unwrap().len() >= 4);

    let commands = bridge.commands.lock().unwrap().clone();
    assert!(commands.contains(&"set_program_scene:Sponsor Read".to_string()));
    assert!(commands.contains(&"start_stream".to_string()));
    assert!(commands.contains(&"stop_stream".to_string()));
    assert!(commands.contains(&"start_recording".to_string()));
    assert!(commands.contains(&"stop_recording".to_string()));
    assert!(commands.contains(&"save_replay_buffer".to_string()));
}

#[tokio::test]
async fn obs_bridge_rejects_non_websocket_profiles() {
    let app = test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/obs/me/bridge/connections")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "label": "Bad OBS",
                        "websocket_url": "http://127.0.0.1:4455"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn api_route_coverage_smokes_read_endpoints_and_runtime_stream() {
    let app = test_app().await;
    let dashboard = call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let broadcast_id = dashboard["broadcast"]["id"].as_str().unwrap().to_string();

    let capture_sessions = call_json(
        app.clone(),
        Method::GET,
        "/api/v1/media/capture/sessions",
        None,
    )
    .await;
    assert_eq!(capture_sessions.as_array().unwrap().len(), 0);

    let encode_jobs = call_json(app.clone(), Method::GET, "/api/v1/media/encode/jobs", None).await;
    assert_eq!(encode_jobs.as_array().unwrap().len(), 0);

    let media_packages = call_json(app.clone(), Method::GET, "/api/v1/media/packages", None).await;
    assert_eq!(media_packages.as_array().unwrap().len(), 0);

    let helper_sessions = call_json(
        app.clone(),
        Method::GET,
        "/api/v1/native/helpers/sessions",
        None,
    )
    .await;
    assert_eq!(helper_sessions.as_array().unwrap().len(), 0);

    let bridge_connections = call_json(
        app.clone(),
        Method::GET,
        "/api/v1/obs/me/bridge/connections",
        None,
    )
    .await;
    assert_eq!(bridge_connections.as_array().unwrap().len(), 0);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let (mut socket, _) = connect_async(format!(
        "ws://127.0.0.1:{}/api/v1/obs/me/broadcasts/{broadcast_id}/runtime/stream",
        addr.port()
    ))
    .await
    .unwrap();
    let message = tokio::time::timeout(std::time::Duration::from_secs(2), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let payload: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
    assert_eq!(payload["event_kind"], "runtime_snapshot");
    assert_eq!(payload["sequence"], 1);
    assert_eq!(payload["broadcast_id"], "broadcast_prime_launch");
    socket.close(None).await.unwrap();
    server.abort();
}

#[tokio::test]
async fn obs_scene_collection_import_persists_report_and_vanta_collection() {
    let app = test_app().await;
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/obs/prime-live.obs.json")).unwrap();

    let report = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/obs/me/imports/scene-collections",
        Some(json!({
            "label": "Prime OBS Fixture",
            "collection_json": fixture,
            "allow_partial": true
        })),
    )
    .await;
    let report_id = report["id"].as_str().unwrap().to_string();
    let collection_id = report["collection_id"].as_str().unwrap().to_string();

    assert_eq!(report["status"], "partial");
    assert_eq!(report["report_json"]["imported_scene_count"], 2);
    assert_eq!(report["report_json"]["imported_source_count"], 6);
    assert_eq!(report["report_json"]["imported_instance_count"], 4);
    assert!(
        report["report_json"]["omissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["code"] == "unsupported_source_kind")
    );
    assert!(
        report["report_json"]["omissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["code"] == "unsupported_filter")
    );
    assert!(
        report["report_json"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["code"] == "audio_filter_preserved")
    );

    let fetched = call_json(
        app.clone(),
        Method::GET,
        &format!("/api/v1/obs/me/imports/scene-collections/{report_id}"),
        None,
    )
    .await;
    assert_eq!(fetched["id"], report_id);

    let reports = call_json(
        app.clone(),
        Method::GET,
        "/api/v1/obs/me/imports/scene-collections",
        None,
    )
    .await;
    assert_eq!(reports.as_array().unwrap().len(), 1);

    let bundle = call_json(
        app,
        Method::GET,
        &format!("/api/v1/obs/me/scene-collections/{collection_id}"),
        None,
    )
    .await;
    assert_eq!(bundle["collection"]["name"], "Prime Live OBS Collection");
    assert_eq!(bundle["scenes"].as_array().unwrap().len(), 2);
    assert!(
        bundle["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["source_kind"] == "browser_capture")
    );
}

#[tokio::test]
async fn vendored_obs_policy_blocks_source_copy_without_approval_evidence() {
    let policy = vendored_obs_policy();
    assert_eq!(policy["status"], "blocked_without_explicit_approval");
    assert!(
        policy["allowed_before_approval"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "obs_websocket_interop")
    );
    assert!(
        policy["blocked_before_approval"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "vendored_obs_studio_source")
    );

    let denied = validate_vendored_obs_approval(&json!({
        "gpl_legal_approval": true,
        "open_source_distribution_posture": true,
        "build_isolation_plan": true
    }))
    .unwrap_err();
    assert!(denied.contains(&"upstream_patch_strategy".to_string()));
    assert!(denied.contains(&"commercial_removal_plan".to_string()));

    let approved = validate_vendored_obs_approval(&json!({
        "gpl_legal_approval": true,
        "open_source_distribution_posture": true,
        "build_isolation_plan": true,
        "upstream_patch_strategy": true,
        "security_update_workflow": true,
        "reproducible_macos_build": true,
        "reproducible_windows_build": true,
        "commercial_removal_plan": true
    }))
    .unwrap();
    assert_eq!(
        approved["allowed_scope"],
        "isolated_optional_obs_vendor_track"
    );
    assert_eq!(approved["product_shell"], "vanta_native_web_studio");
}

#[tokio::test]
async fn obs_scene_collection_import_requires_explicit_partial_import() {
    let app = test_app().await;
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/obs/prime-live.obs.json")).unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/obs/me/imports/scene-collections")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "label": "Strict Fixture",
                        "collection_json": fixture,
                        "allow_partial": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn obs_websocket_mock_sequences_events_between_rpc_responses() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        socket
            .send(Message::Text(
                json!({
                    "op": 0,
                    "d": {
                        "obsWebSocketVersion": "5.6.0",
                        "rpcVersion": 1
                    }
                })
                .to_string()
                .into(),
            ))
            .await
            .unwrap();
        let identify = socket.next().await.unwrap().unwrap();
        assert!(identify.to_text().unwrap().contains("\"op\":1"));
        socket
            .send(Message::Text(json!({"op": 2, "d": {}}).to_string().into()))
            .await
            .unwrap();

        while let Some(message) = socket.next().await {
            let Ok(message) = message else {
                break;
            };
            let Ok(text) = message.to_text() else {
                continue;
            };
            let request: Value = serde_json::from_str(text).unwrap();
            if request["op"].as_i64() != Some(6) {
                continue;
            }
            let data = &request["d"];
            let request_id = data["requestId"].as_str().unwrap();
            let request_type = data["requestType"].as_str().unwrap();
            socket
                .send(Message::Text(
                    json!({
                        "op": 5,
                        "d": {
                            "eventType": "CurrentProgramSceneChanged",
                            "eventData": { "sceneName": "Interleaved Event" }
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .unwrap();
            socket
                .send(Message::Text(
                    obs_rpc_response(request_id, request_type)
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
        }
    });

    let client = LocalObsWebSocketClient::default();
    let snapshot = client
        .snapshot(&ObsBridgeProfile {
            id: "profile_ws".to_string(),
            label: "Sequenced OBS".to_string(),
            websocket_url: format!("ws://{addr}"),
            password: None,
            auto_sync: true,
        })
        .await
        .unwrap();

    assert_eq!(snapshot.obs_version, "32.0.0");
    assert_eq!(snapshot.websocket_version, "5.6.0");
    assert_eq!(
        snapshot.current_program_scene.as_deref(),
        Some("Program Scene")
    );
    assert_eq!(snapshot.stream_state, "live");
    assert_eq!(snapshot.recording_state, "recording");
    assert_eq!(snapshot.replay_buffer_state, "active");
    assert_eq!(snapshot.scenes[0].items[0].source_name, "Sony FX3");
    assert_eq!(snapshot.scenes[0].items[0].transform["positionX"], 32.0);
    assert!(snapshot.sources.iter().any(|source| {
        source.name == "Display" && source.vanta_kind.as_deref() == Some("display_capture")
    }));
    assert!(snapshot.unsupported.iter().any(|warning| {
        warning.code == "unsupported_source_kind" && warning.subject == "Novelty Shader"
    }));
    server.await.unwrap();
}

#[tokio::test]
async fn obs_scene_group_import_export_preserves_nested_group_graphs() {
    let app = test_app().await;
    let nested_fixture = json!({
        "name": "Nested OBS Group Fixture",
        "transition": "fade",
        "transition_duration": 300,
        "video": {"base_width": 1920, "base_height": 1080, "fps_num": 60, "fps_den": 1},
        "scene_order": [{"name": "Program"}],
        "sources": [
            {
                "name": "Program",
                "id": "scene",
                "settings": {
                    "items": [
                        {
                            "name": "Product Group",
                            "visible": true,
                            "locked": true,
                            "pos": {"x": 100.0, "y": 120.0},
                            "bounds": {"x": 640.0, "y": 360.0},
                            "crop": {"top": 4, "right": 8, "bottom": 12, "left": 16}
                        }
                    ]
                }
            },
            {
                "name": "Product Group",
                "id": "group",
                "settings": {
                    "items": [
                        {
                            "name": "Product Browser",
                            "visible": true,
                            "locked": false,
                            "pos": {"x": 0.0, "y": 0.0},
                            "bounds": {"x": 640.0, "y": 360.0}
                        },
                        {
                            "name": "Product Copy",
                            "visible": false,
                            "locked": true,
                            "pos": {"x": 24.0, "y": 300.0},
                            "bounds": {"x": 420.0, "y": 80.0}
                        }
                    ]
                }
            },
            {
                "name": "Product Browser",
                "id": "browser_source",
                "settings": {"url": "https://streamvanta.tv/product", "width": 640, "height": 360}
            },
            {
                "name": "Product Copy",
                "id": "text_ft2_source",
                "settings": {"text": "Live demo"}
            }
        ]
    });

    let report = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/obs/me/imports/scene-collections",
        Some(json!({
            "label": "Nested OBS Fixture",
            "collection_json": nested_fixture,
            "allow_partial": false
        })),
    )
    .await;
    assert_eq!(report["status"], "ready");
    assert_eq!(report["report_json"]["imported_scene_count"], 2);
    assert_eq!(report["report_json"]["imported_source_count"], 3);
    assert_eq!(report["report_json"]["imported_instance_count"], 3);
    assert!(
        report["report_json"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning["code"] == "obs_group_materialized_as_nested_scene")
    );
    let collection_id = report["collection_id"].as_str().unwrap().to_string();
    let bundle = call_json(
        app.clone(),
        Method::GET,
        &format!("/api/v1/obs/me/scene-collections/{collection_id}"),
        None,
    )
    .await;
    let group_source = bundle["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["source_kind"] == "scene_group")
        .unwrap();
    assert_eq!(group_source["display_name"], "Product Group");
    assert_eq!(
        group_source["default_settings_json"]["group_kind"],
        "obs_group"
    );
    assert_eq!(group_source["source_validation_json"]["status"], "ready");
    let nested_scene_id = group_source["default_settings_json"]["scene_id"]
        .as_str()
        .unwrap();
    assert!(!nested_scene_id.is_empty());
    assert!(
        bundle["scenes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|scene| scene["id"] == nested_scene_id && scene["locked"] == 1)
    );
    let product_browser_source_id = bundle["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["display_name"] == "Product Browser")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        bundle["instances"]
            .as_array()
            .unwrap()
            .iter()
            .any(|instance| instance["scene_id"] == nested_scene_id
                && instance["source_id"] == product_browser_source_id)
    );

    let export = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/obs/me/exports/scene-collections",
        Some(json!({
            "label": "Nested OBS Export",
            "collection_id": collection_id,
            "include_setup_instructions": false
        })),
    )
    .await;
    let exported_group = export["scene_collection_json"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["id"] == "group" && source["name"] == "Product Group")
        .unwrap();
    let items = exported_group["settings"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().any(|item| item["name"] == "Product Browser"));
    assert!(items.iter().any(|item| item["name"] == "Product Copy"));
    let program_scene = export["scene_collection_json"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["id"] == "scene" && source["name"] == "Program")
        .unwrap();
    assert!(
        program_scene["settings"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["name"] == "Product Group"
                && item["locked"] == true
                && item["crop"]["left"] == 16)
    );
}

#[tokio::test]
async fn obs_scene_collection_export_persists_manifest_warnings_and_layout() {
    let app = test_app().await;
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/obs/prime-live.obs.json")).unwrap();

    let report = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/obs/me/imports/scene-collections",
        Some(json!({
            "label": "Prime OBS Fixture",
            "collection_json": fixture,
            "allow_partial": true
        })),
    )
    .await;
    let collection_id = report["collection_id"].as_str().unwrap().to_string();

    let export = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/obs/me/exports/scene-collections",
        Some(json!({
            "label": "Creator OBS Export",
            "collection_id": collection_id,
            "include_setup_instructions": true
        })),
    )
    .await;
    let job_id = export["id"].as_str().unwrap().to_string();
    let scene_collection = &export["scene_collection_json"];

    assert_eq!(export["status"], "ready");
    assert_eq!(scene_collection["name"], "Prime Live OBS Collection");
    assert_eq!(scene_collection["video"]["base_width"], 1920);
    assert_eq!(scene_collection["video"]["base_height"], 1080);
    assert_eq!(scene_collection["scene_order"][0]["name"], "Host Camera");
    assert!(
        export["setup_instructions_json"]
            .as_array()
            .unwrap()
            .iter()
            .any(|line| line.as_str().unwrap().contains("asset_manifest"))
    );
    assert!(
        export["asset_manifest_json"]["assets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|asset| asset["source_name"] == "Nova Logo"
                && asset["bundle_path"] == "assets/images/nova-logo.bin")
    );
    assert!(
        export["warnings_json"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning["code"] == "vanta_overlay_exported_as_browser_source")
    );
    assert!(
        scene_collection["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["id"] == "image_source" && source["name"] == "Nova Logo")
    );
    assert!(
        scene_collection["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["id"] == "browser_source" && source["name"] == "Audience Chat")
    );

    let host_scene = scene_collection["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["id"] == "scene" && source["name"] == "Host Camera")
        .unwrap();
    let camera_item = host_scene["settings"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "Sony FX3")
        .unwrap();
    assert_eq!(camera_item["pos"]["x"], 0.0);
    assert_eq!(camera_item["bounds"]["x"], 1920.0);
    assert_eq!(camera_item["opacity"], 1.0);

    let fetched = call_json(
        app.clone(),
        Method::GET,
        &format!("/api/v1/obs/me/exports/scene-collections/{job_id}"),
        None,
    )
    .await;
    assert_eq!(fetched["id"], job_id);

    let exports = call_json(
        app,
        Method::GET,
        "/api/v1/obs/me/exports/scene-collections",
        None,
    )
    .await;
    assert_eq!(exports.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn obs_export_harness_covers_native_source_packages_and_transform_parity() {
    let matrix: Value =
        serde_json::from_str(include_str!("fixtures/obs/compatibility-matrix.json")).unwrap();
    assert_eq!(matrix["obs_versions"].as_array().unwrap().len(), 3);
    assert_eq!(matrix["platforms"]["macos"]["camera"], "av_capture_input");
    assert_eq!(
        matrix["platforms"]["windows"]["microphone"],
        "wasapi_input_capture"
    );
    assert_eq!(
        matrix["platforms"]["linux"]["display_capture"],
        "pipewire-desktop-capture-source"
    );

    let source_specs = [
        (
            "source_camera",
            "camera",
            "Camera",
            Some("camera:fx3"),
            None,
            None,
        ),
        (
            "source_mic",
            "microphone",
            "Host Mic",
            Some("audio:host"),
            None,
            None,
        ),
        (
            "source_screen",
            "screen_capture",
            "Screen",
            Some("display:1"),
            None,
            None,
        ),
        (
            "source_window",
            "window_capture",
            "Window",
            Some("window:vanta"),
            None,
            None,
        ),
        (
            "source_browser",
            "browser_capture",
            "Browser",
            None,
            Some("https://streamvanta.tv/live"),
            None,
        ),
        (
            "source_media",
            "media_file",
            "Bumper",
            None,
            None,
            Some("asset_bumper"),
        ),
        (
            "source_image",
            "image",
            "Logo",
            None,
            None,
            Some("asset_logo"),
        ),
        ("source_text", "text", "Lower Copy", None, None, None),
        (
            "source_lower",
            "lower_third",
            "Lower Third",
            None,
            None,
            None,
        ),
        (
            "source_sponsor",
            "sponsor_card",
            "Sponsor",
            None,
            None,
            Some("asset_sponsor"),
        ),
        (
            "source_countdown",
            "countdown_timer",
            "Countdown",
            None,
            None,
            None,
        ),
        ("source_chat", "chat_overlay", "Chat", None, None, None),
        ("source_alert", "alert_overlay", "Alert", None, None, None),
        ("source_guest", "guest_feed", "Guest", None, None, None),
        (
            "source_remote",
            "remote_contribution",
            "Remote",
            None,
            None,
            None,
        ),
        (
            "source_vanta_asset",
            "vanta_video_asset",
            "Vanta Asset",
            None,
            None,
            Some("asset_vanta"),
        ),
        (
            "source_clip",
            "vanta_clip",
            "Clip",
            None,
            None,
            Some("asset_clip"),
        ),
        ("source_color", "color_matte", "Matte", None, None, None),
        ("source_safe", "safe_area_guide", "Safe", None, None, None),
        (
            "source_group",
            "scene_group",
            "Nested Scene",
            None,
            None,
            None,
        ),
    ];
    let sources = source_specs
        .iter()
        .map(|(id, kind, name, device, url, media)| {
            json!({
                "id": id,
                "source_kind": kind,
                "display_name": name,
                "device_id": device.unwrap_or_default(),
                "browser_url": url.unwrap_or_default(),
                "media_asset_id": media.unwrap_or_default(),
                "default_settings_json": {
                    "text": "Use code VANTA20",
                    "width": 1280,
                    "height": 720,
                    "color": 4278190335u64,
                    "scene_id": "scene_nested"
                }
            })
        })
        .collect::<Vec<_>>();
    let instances = vec![
        json!({
            "id": "instance_camera",
            "scene_id": "scene_main",
            "source_id": "source_camera",
            "visible": 1,
            "locked": 0,
            "x": 10.5,
            "y": 20.25,
            "width": 1280.0,
            "height": 720.0,
            "opacity": 0.72,
            "crop_json": {"top": 8, "right": 16, "bottom": 24, "left": 32},
            "transform_json": {"rotation": 12.5}
        }),
        json!({
            "id": "instance_group",
            "scene_id": "scene_main",
            "source_id": "source_group",
            "visible": 1,
            "locked": 1,
            "x": 90.0,
            "y": 120.0,
            "width": 640.0,
            "height": 360.0,
            "opacity": 1.0,
            "crop_json": {"top": 0, "right": 0, "bottom": 0, "left": 0},
            "transform_json": {"rotation": 0.0}
        }),
        json!({
            "id": "instance_safe",
            "scene_id": "scene_nested",
            "source_id": "source_safe",
            "visible": 1,
            "locked": 1,
            "x": 0.0,
            "y": 0.0,
            "width": 1920.0,
            "height": 1080.0,
            "opacity": 0.5,
            "crop_json": {"top": 0, "right": 0, "bottom": 0, "left": 0},
            "transform_json": {"rotation": 0.0}
        }),
    ];
    let package = build_obs_export_package(
        ObsExportInput {
            collection_id: "collection_compat".to_string(),
            label: "Compatibility Golden".to_string(),
            include_setup_instructions: Some(true),
        },
        json!({
            "collection": {
                "id": "collection_compat",
                "name": "Compatibility Golden",
                "canvas_width": 1920,
                "canvas_height": 1080,
                "frame_rate": 60
            },
            "scenes": [
                {"id": "scene_main", "name": "Main", "transition_kind": "fade", "transition_duration_ms": 300},
                {"id": "scene_nested", "name": "Nested", "transition_kind": "cut", "transition_duration_ms": 0}
            ],
            "sources": sources,
            "instances": instances
        }),
    )
    .unwrap();

    let exported_sources = package.scene_collection_json["sources"].as_array().unwrap();
    let kinds = exported_sources
        .iter()
        .map(|source| source["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    for expected in [
        "av_capture_input",
        "coreaudio_input_capture",
        "display_capture",
        "window_capture",
        "browser_source",
        "ffmpeg_source",
        "image_source",
        "text_ft2_source",
        "color_source",
        "group",
    ] {
        assert!(
            kinds.contains(&expected),
            "missing OBS source kind {expected}"
        );
    }
    assert!(
        !exported_sources
            .iter()
            .any(|source| source["name"] == "Safe" && source["id"] == "safe_area_guide")
    );
    assert!(package.asset_manifest.assets.iter().any(|asset| {
        asset.source_kind == "vanta_video_asset"
            && asset.bundle_path == "assets/vanta-media/vanta-asset.bin"
    }));
    assert!(package.warnings.iter().any(|warning| {
        warning.code == "live_participant_exported_as_browser_source" && warning.subject == "Guest"
    }));
    assert!(
        package.warnings.iter().any(|warning| {
            warning.code == "safe_area_guide_omitted" && warning.subject == "Safe"
        })
    );

    let main_scene = exported_sources
        .iter()
        .find(|source| source["id"] == "scene" && source["name"] == "Main")
        .unwrap();
    let items = main_scene["settings"]["items"].as_array().unwrap();
    assert_eq!(items[0]["name"], "Camera");
    assert_eq!(items[1]["name"], "Nested Scene");
    assert_eq!(items[0]["pos"]["x"], 10.5);
    assert_eq!(items[0]["pos"]["y"], 20.25);
    assert_eq!(items[0]["bounds"]["x"], 1280.0);
    assert_eq!(items[0]["bounds"]["y"], 720.0);
    assert_eq!(items[0]["crop"]["top"], 8);
    assert_eq!(items[0]["crop"]["right"], 16);
    assert_eq!(items[0]["crop"]["bottom"], 24);
    assert_eq!(items[0]["crop"]["left"], 32);
    assert_eq!(items[0]["rot"], 12.5);
    assert_eq!(items[0]["opacity"], 0.72);
    assert_eq!(items[1]["locked"], true);
}

#[tokio::test]
async fn native_package_cli_reports_release_readiness_blockers() {
    let output = Command::new(env!("CARGO_BIN_EXE_vanta-native-package"))
        .arg("release-readiness")
        .output()
        .await
        .unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "blocked");
    assert_eq!(report["release_kind"], "vanta_obs_desktop_distribution");
    assert!(
        report["blockers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|blocker| blocker["gate"] == "installer_signature_unverified")
    );

    let output = Command::new(env!("CARGO_BIN_EXE_vanta-native-package"))
        .arg("verify-distribution")
        .output()
        .await
        .unwrap();
    assert!(output.status.success());
    let reports: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(reports.as_array().unwrap().iter().any(|report| {
        report["status"] == "blocked"
            && report["helper_production_signature_verified"] == false
            && report["installer_production_signature_verified"] == false
    }));

    let output = Command::new(env!("CARGO_BIN_EXE_vanta-native-package"))
        .arg("release-readiness")
        .arg("--strict")
        .output()
        .await
        .unwrap();
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "blocked");

    let output = Command::new(env!("CARGO_BIN_EXE_vanta-native-package"))
        .arg("verify-distribution")
        .arg("--strict")
        .output()
        .await
        .unwrap();
    assert!(!output.status.success());
    let reports: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        reports
            .as_array()
            .unwrap()
            .iter()
            .any(|report| report["status"] == "blocked")
    );
}

#[tokio::test]
async fn native_helper_protocol_persists_sessions_events_and_shutdown() {
    let app = test_app().await;

    let packages = call_json(
        app.clone(),
        Method::GET,
        "/api/v1/native/helpers/packages",
        None,
    )
    .await;
    let packages = packages.as_array().unwrap();
    assert_eq!(packages.len(), 8);
    assert!(packages.iter().any(|package| {
        package["helper_kind"] == "capture"
            && package["platform"] == "macos"
            && package["signing_required"] == true
            && package["notarization_required"] == true
            && package
                .get("build_manifest_path")
                .and_then(Value::as_str)
                .is_some()
            && package
                .get("helper_signature_verified")
                .and_then(Value::as_bool)
                .is_some()
            && package
                .get("installer_present")
                .and_then(Value::as_bool)
                .is_some()
            && package
                .get("installer_signature_verified")
                .and_then(Value::as_bool)
                .is_some()
            && package
                .get("notarization_verified")
                .and_then(Value::as_bool)
                .is_some()
            && package["transports"]
                .as_array()
                .unwrap()
                .iter()
                .any(|transport| transport == "stdio")
            && package["permissions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|permission| permission == "screen-recording")
    }));
    assert!(packages.iter().any(|package| {
        package["helper_kind"] == "capture"
            && package["platform"] == "windows"
            && package["signing_required"] == true
    }));
    assert!(packages.iter().any(|package| {
        package["helper_kind"] == "audio"
            && package["platform"] == "macos"
            && package["system_audio_validation_required"] == true
            && package["system_audio_validation_verified"]
                .as_bool()
                .is_some()
            && package["system_audio_validation_artifact"]
                .as_str()
                .is_some()
            && package["permissions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|permission| permission == "screen-recording")
            && package["permissions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|permission| permission == "system-audio")
            && package["permissions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|permission| permission == "application-audio")
    }));
    let release = call_json(app.clone(), Method::GET, "/api/v1/release/readiness", None).await;
    assert_eq!(release["status"], "blocked");
    assert_eq!(release["vendor_track"]["status"], "not_requested");
    assert!(
        release["blockers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|blocker| {
                blocker["gate"] == "installer_signature_unverified"
                    && blocker["platform"] == "macos"
                    && blocker["helper_kind"] == "capture"
            })
    );
    assert!(
        release["blockers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|blocker| {
                blocker["gate"] == "system_audio_validation_missing"
                    && blocker["helper_kind"] == "audio"
            })
    );

    let session = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/native/helpers/sessions",
        Some(json!({
            "helper_kind": "capture",
            "launch_mode": "managed"
        })),
    )
    .await;
    let session_id = session["id"].as_str().unwrap().to_string();

    assert_eq!(session["helper_kind"], "capture");
    assert_eq!(session["protocol_version"], "vanta-native-helper.v1");
    assert_eq!(session["status"], "ready");
    assert_eq!(session["capabilities_json"]["camera"], true);
    assert_eq!(session["capabilities_json"]["hotplug_events"], true);
    assert_eq!(session["health_json"]["package"]["platform"], "macos");
    assert_eq!(session["health_json"]["package"]["signing_required"], true);
    assert!(
        session["health_json"]["package"]["status"] == "missing_artifact"
            || session["health_json"]["package"]["status"] == "missing_signing_identity"
            || session["health_json"]["package"]["status"] == "ready"
    );

    let heartbeat = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/native/helpers/sessions/{session_id}/heartbeat"),
        None,
    )
    .await;
    assert_eq!(heartbeat["status"], "ready");
    assert_eq!(heartbeat["health_json"]["command"], "heartbeat");

    let command = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/native/helpers/sessions/{session_id}/command"),
        Some(json!({
            "command_kind": "prepare_capture",
            "payload_json": {
                "source_id": "source_camera_a",
                "width": 1920,
                "height": 1080,
                "output_path": std::env::temp_dir().join("vanta-obs-media/capture-preview.raw")
            }
        })),
    )
    .await;
    assert_eq!(
        command["health_json"]["detail"]["source_id"],
        "source_camera_a"
    );
    assert_eq!(command["health_json"]["sandbox"]["allowed"], true);
    assert_eq!(
        command["health_json"]["sandbox"]["checked_paths"][0]
            .as_str()
            .unwrap()
            .ends_with("vanta-obs-media/capture-preview.raw"),
        true
    );

    let blocked_path = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!(
                    "/api/v1/native/helpers/sessions/{session_id}/command"
                ))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "command_kind": "prepare_capture",
                        "payload_json": { "output_path": "/etc/passwd" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(blocked_path.status(), StatusCode::BAD_REQUEST);

    let fetched = call_json(
        app.clone(),
        Method::GET,
        &format!("/api/v1/native/helpers/sessions/{session_id}"),
        None,
    )
    .await;
    assert_eq!(fetched["id"], session_id);

    let events = call_json(
        app.clone(),
        Method::GET,
        &format!("/api/v1/native/helpers/sessions/{session_id}/events"),
        None,
    )
    .await;
    assert!(events.as_array().unwrap().len() >= 3);

    let crashed = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/native/helpers/sessions/{session_id}/command"),
        Some(json!({
            "command_kind": "report_crash",
            "payload_json": {
                "reason": "capture_device_driver_exit",
                "trace_event": "native.helper.capture.crash.test"
            }
        })),
    )
    .await;
    assert_eq!(crashed["status"], "crashed");
    assert_eq!(crashed["crash_count"], 1);

    let recovered = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/native/helpers/sessions/{session_id}/heartbeat"),
        None,
    )
    .await;
    assert_eq!(recovered["status"], "ready");
    assert_eq!(recovered["health_json"]["recovered"], true);
    assert_eq!(
        recovered["health_json"]["recovery_reason"],
        "auto_restart_after_crashed"
    );
    assert_eq!(recovered["crash_count"], 1);

    let logs = call_json(
        app.clone(),
        Method::GET,
        &format!("/api/v1/native/helpers/sessions/{session_id}/logs"),
        None,
    )
    .await;
    assert!(logs.as_array().unwrap().iter().any(|log| {
        log["message"] == "Native helper recovered"
            && log["trace_event_id"] == format!("native.helper.{session_id}.recovered")
    }));

    let manual_recovery = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/native/helpers/sessions/{session_id}/recover"),
        Some(json!({ "reason": "operator_requested" })),
    )
    .await;
    assert_eq!(manual_recovery["status"], "ready");
    assert_eq!(
        manual_recovery["health_json"]["recovery_reason"],
        "operator_requested"
    );

    let degraded = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/native/helpers/sessions/{session_id}/command"),
        Some(json!({
            "command_kind": "report_degraded",
            "payload_json": {
                "reason": "preview_frame_timeout",
                "trace_event": "native.helper.capture.degraded.test"
            }
        })),
    )
    .await;
    assert_eq!(degraded["status"], "degraded");
    assert_eq!(degraded["crash_count"], 1);

    let recovered_from_degraded = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/native/helpers/sessions/{session_id}/heartbeat"),
        None,
    )
    .await;
    assert_eq!(recovered_from_degraded["status"], "ready");
    assert_eq!(
        recovered_from_degraded["health_json"]["recovery_reason"],
        "auto_restart_after_degraded"
    );

    let stopped = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/native/helpers/sessions/{session_id}/shutdown"),
        None,
    )
    .await;
    assert_eq!(stopped["status"], "stopped");

    let sessions = call_json(app, Method::GET, "/api/v1/native/helpers/sessions", None).await;
    assert_eq!(sessions.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn native_helper_stdio_transport_executes_external_helper_binary() {
    let app = test_app().await;
    let helper_binary = env!("CARGO_BIN_EXE_vanta-native-helper");

    let session = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/native/helpers/sessions",
        Some(json!({
            "helper_kind": "audio",
            "launch_mode": "stdio",
            "binary_path": helper_binary
        })),
    )
    .await;
    let session_id = session["id"].as_str().unwrap().to_string();
    let process_id = session["process_id"].as_i64().unwrap();
    assert_eq!(session["launch_mode"], "stdio");
    assert_eq!(session["endpoint"], "stdio://vanta-native-helper");
    assert_eq!(session["health_json"]["lifecycle"], "long_lived");
    assert_eq!(session["health_json"]["process_id"], process_id);

    let heartbeat = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/native/helpers/sessions/{session_id}/heartbeat"),
        None,
    )
    .await;
    assert_eq!(heartbeat["status"], "ready");
    assert_eq!(heartbeat["health_json"]["transport"], "stdio");
    assert_eq!(heartbeat["health_json"]["helper_kind"], "audio");
    assert_eq!(heartbeat["health_json"]["command"], "heartbeat");
    assert_eq!(heartbeat["health_json"]["lifecycle"], "long_lived");
    assert_eq!(heartbeat["health_json"]["process_id"], process_id);
    assert_eq!(heartbeat["health_json"]["package"]["platform"], "macos");

    let second_heartbeat = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/native/helpers/sessions/{session_id}/heartbeat"),
        None,
    )
    .await;
    assert_eq!(second_heartbeat["status"], "ready");
    assert_eq!(second_heartbeat["health_json"]["process_id"], process_id);

    let stopped = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/native/helpers/sessions/{session_id}/shutdown"),
        None,
    )
    .await;
    assert_eq!(stopped["status"], "stopped");
    assert_eq!(stopped["health_json"]["process_id"], process_id);

    let after_shutdown = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!(
                    "/api/v1/native/helpers/sessions/{session_id}/heartbeat"
                ))
                .header("content-type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(after_shutdown.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn native_helper_localhost_transport_posts_to_loopback_helper() {
    let app = test_app().await;
    let helper_binary = env!("CARGO_BIN_EXE_vanta-native-helper");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let endpoint = format!("http://127.0.0.1:{}/command", addr.port());

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buffer = vec![0; 4096];
        let read = socket.read(&mut buffer).await.unwrap();
        let request = String::from_utf8_lossy(&buffer[..read]);
        assert!(request.starts_with("POST /command HTTP/1.1"));
        assert!(request.contains("\"command_kind\":\"heartbeat\""));
        let body = json!({
            "status": "ready",
            "health_json": {
                "state": "ready",
                "from_localhost": true
            }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    });

    let session = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/native/helpers/sessions",
        Some(json!({
            "helper_kind": "capture",
            "launch_mode": "localhost",
            "binary_path": helper_binary,
            "endpoint": endpoint
        })),
    )
    .await;
    let session_id = session["id"].as_str().unwrap().to_string();
    assert_eq!(session["launch_mode"], "localhost");
    assert_eq!(session["endpoint"], endpoint);

    let heartbeat = call_json(
        app,
        Method::POST,
        &format!("/api/v1/native/helpers/sessions/{session_id}/heartbeat"),
        None,
    )
    .await;
    server.await.unwrap();
    assert_eq!(heartbeat["status"], "ready");
    assert_eq!(heartbeat["health_json"]["transport"], "localhost");
    assert_eq!(heartbeat["health_json"]["from_localhost"], true);
    assert_eq!(heartbeat["health_json"]["helper_kind"], "capture");
}

#[tokio::test]
async fn native_helper_rejects_missing_explicit_binary() {
    let app = test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/native/helpers/sessions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "helper_kind": "capture",
                        "launch_mode": "stdio",
                        "binary_path": "/definitely/missing/vanta-native-helper"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn native_helper_rejects_unsupported_helper_kind() {
    let app = test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/native/helpers/sessions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "helper_kind": "novelty_shader_engine",
                        "launch_mode": "managed"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn media_capture_and_encode_path_prepares_native_helpers() {
    let app = test_app().await;

    let devices = call_json(
        app.clone(),
        Method::GET,
        "/api/v1/media/capture/devices",
        None,
    )
    .await;
    assert_eq!(devices["platform"], "macos");
    assert!(
        devices["transport"]
            .as_str()
            .unwrap()
            .starts_with("ffmpeg_avfoundation")
    );
    assert!(devices["devices"].as_array().unwrap().iter().any(|device| {
        device["kind"] == "camera"
            || device["kind"] == "display"
            || device["kind"] == "microphone"
            || device["kind"] == "desktop_audio"
            || device["kind"] == "system_audio"
            || device["kind"] == "application_audio"
    }));
    assert!(devices["support"]["window"].is_boolean());
    assert!(devices["support"]["application_audio"].is_boolean());
    assert!(devices["support"]["desktop_audio"].is_boolean());
    assert!(devices["support"]["system_audio"].is_boolean());
    assert!(devices["permissions"]["system_audio"]["status"].is_string());
    assert_eq!(devices["permissions"]["system_audio"]["required"], true);
    assert!(devices["permissions"]["application_audio"]["status"].is_string());
    assert_eq!(
        devices["permissions"]["application_audio"]["required"],
        true
    );
    assert!(devices["permissions"]["camera"]["status"].is_string());
    assert_eq!(devices["permissions"]["camera"]["required"], true);
    assert!(devices["permissions"]["microphone"]["status"].is_string());
    assert_eq!(devices["permissions"]["microphone"]["required"], true);
    let camera_supported = devices["support"]["camera"].as_bool().unwrap_or(false);
    let display_kind = if devices["support"]["display"].as_bool().unwrap_or(false) {
        "display"
    } else {
        "program_canvas"
    };
    let mut encode_capture_id = String::new();
    let mut encode_duration_seconds = 5_i64;

    let program_canvas_capture = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/media/capture/sessions",
        Some(json!({
            "source_id": "source_program_canvas_runtime",
            "capture_kind": "program_canvas",
            "width": 320,
            "height": 180,
            "frame_rate": 30,
            "audio": false,
            "duration_seconds": 2
        })),
    )
    .await;
    assert_eq!(program_canvas_capture["status"], "capturing");
    let runtime_frame = call_json(
        app.clone(),
        Method::POST,
        &format!(
            "/api/v1/media/capture/sessions/{}/runtime-frame",
            program_canvas_capture["id"].as_str().unwrap()
        ),
        Some(json!({
            "image_data_url": generated_png_data_url(320, 180).await,
            "compositor_backend": "webgl_gpu",
            "frame_sequence": 1,
            "captured_at_ms": 1000
        })),
    )
    .await;
    assert_eq!(runtime_frame["status"], "ready");
    assert_eq!(runtime_frame["frame_kind"], "runtime_program_canvas_png");
    assert_eq!(
        runtime_frame["validation_json"]["runtime_backed_program_output"],
        true
    );
    assert_eq!(
        runtime_frame["validation_json"]["browser_preview_authoritative"],
        false
    );
    assert_eq!(
        runtime_frame["validation_json"]["compositor_backend"],
        "webgl_gpu"
    );
    assert_eq!(runtime_frame["validation_json"]["width"], 320);
    assert_eq!(runtime_frame["validation_json"]["height"], 180);
    assert_eq!(
        runtime_frame["validation_json"]["sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert!(
        tokio::fs::metadata(runtime_frame["artifact_path"].as_str().unwrap())
            .await
            .is_ok()
    );
    let sessions_after_program_canvas = call_json(
        app.clone(),
        Method::GET,
        "/api/v1/media/capture/sessions",
        None,
    )
    .await;
    let program_canvas_session = sessions_after_program_canvas
        .as_array()
        .unwrap()
        .iter()
        .find(|session| session["id"] == program_canvas_capture["id"])
        .unwrap();
    assert_eq!(
        program_canvas_session["health_json"]["runtime_compositor"]["coverage"],
        "all_live_capture_outputs"
    );
    assert_eq!(
        program_canvas_session["health_json"]["runtime_compositor"]["latest_output"]["source_kind"],
        "program_canvas"
    );
    assert_eq!(
        program_canvas_session["health_json"]["runtime_compositor"]["durable_frame_pacing"]["drop_policy"],
        "hold_last_good_frame_then_resync"
    );

    let browser_surface_capture = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/media/capture/sessions",
        Some(json!({
            "source_id": "source_browser",
            "capture_kind": "browser_surface",
            "width": 1280,
            "height": 720,
            "frame_rate": 60,
            "audio": false,
            "duration_seconds": 5
        })),
    )
    .await;
    assert_eq!(browser_surface_capture["status"], "capturing");
    assert_eq!(browser_surface_capture["capture_kind"], "browser_surface");
    assert_eq!(
        browser_surface_capture["settings_json"]["source_health"]["status"],
        "ready"
    );
    let browser_surface_frame = call_json(
        app.clone(),
        Method::POST,
        &format!(
            "/api/v1/media/capture/sessions/{}/source-frame",
            browser_surface_capture["id"].as_str().unwrap()
        ),
        Some(json!({
            "image_data_url": generated_png_data_url(1280, 720).await,
            "compositor_backend": "runtime_headless_browser",
            "frame_sequence": 1,
            "captured_at_ms": 1200,
            "surface_kind": "browser_source",
            "dropped_frames": 72,
            "reconnect_count": 1,
            "ingest_latency_ms": 1330
        })),
    )
    .await;
    assert_eq!(browser_surface_frame["status"], "ready");
    assert_eq!(
        browser_surface_frame["frame_kind"],
        "runtime_browser_surface_png"
    );
    assert_eq!(
        browser_surface_frame["validation_json"]["runtime_backed_source_output"],
        true
    );
    assert_eq!(
        browser_surface_frame["validation_json"]["sandboxed_iframe_pixels_read"],
        false
    );
    assert_eq!(
        browser_surface_frame["validation_json"]["source_kind"],
        "browser_capture"
    );
    assert_eq!(
        browser_surface_frame["validation_json"]["surface_kind"],
        "browser_source"
    );
    assert_eq!(
        browser_surface_frame["validation_json"]["long_session"]["status"],
        "watch"
    );
    assert_eq!(
        browser_surface_frame["validation_json"]["long_session"]["drop_status"],
        "warning"
    );
    assert_eq!(
        browser_surface_frame["validation_json"]["long_session"]["drift_status"],
        "watch"
    );
    assert!(
        tokio::fs::metadata(browser_surface_frame["artifact_path"].as_str().unwrap())
            .await
            .is_ok()
    );
    let second_browser_surface_frame = call_json(
        app.clone(),
        Method::POST,
        &format!(
            "/api/v1/media/capture/sessions/{}/source-frame",
            browser_surface_capture["id"].as_str().unwrap()
        ),
        Some(json!({
            "image_data_url": generated_png_data_url(1280, 720).await,
            "compositor_backend": "runtime_headless_browser",
            "frame_sequence": 2,
            "captured_at_ms": 1260,
            "surface_kind": "browser_source",
            "dropped_frames": 12,
            "reconnect_count": 0,
            "ingest_latency_ms": 2600
        })),
    )
    .await;
    assert_eq!(
        second_browser_surface_frame["validation_json"]["long_session"]["status"],
        "degrading"
    );
    let sessions_after_browser_surface = call_json(
        app.clone(),
        Method::GET,
        "/api/v1/media/capture/sessions",
        None,
    )
    .await;
    let browser_surface_session = sessions_after_browser_surface
        .as_array()
        .unwrap()
        .iter()
        .find(|session| session["id"] == browser_surface_capture["id"])
        .unwrap();
    assert_eq!(
        browser_surface_session["health_json"]["browser_surface"]["frames_received"],
        2
    );
    assert_eq!(
        browser_surface_session["health_json"]["browser_surface"]["cumulative_dropped_frames"],
        84
    );
    assert_eq!(
        browser_surface_session["health_json"]["browser_surface"]["max_reconnect_count"],
        1
    );
    assert_eq!(
        browser_surface_session["health_json"]["browser_surface"]["max_ingest_latency_ms"],
        2600
    );
    assert_eq!(
        browser_surface_session["health_json"]["browser_surface"]["latest_long_session"]["continuity_action"],
        "hold_last_good_source_frame_and_reduce_refresh_rate"
    );
    assert_eq!(
        browser_surface_session["health_json"]["runtime_compositor"]["coverage"],
        "all_live_capture_outputs"
    );
    assert_eq!(
        browser_surface_session["health_json"]["runtime_compositor"]["status"],
        "degraded"
    );
    assert_eq!(
        browser_surface_session["health_json"]["runtime_compositor"]["cumulative_dropped_frames"],
        84
    );
    assert_eq!(
        browser_surface_session["health_json"]["runtime_compositor"]["latest_output"]["source_kind"],
        "browser_capture"
    );
    let browser_surface_playout = call_json(
        app.clone(),
        Method::POST,
        &format!(
            "/api/v1/media/capture/sessions/{}/source-playout",
            browser_surface_capture["id"].as_str().unwrap()
        ),
        Some(json!({
            "target_frame_rate": 30,
            "frame_count": 2
        })),
    )
    .await;
    assert_eq!(browser_surface_playout["status"], "ready");
    assert_eq!(
        browser_surface_playout["artifact_kind"],
        "runtime_browser_surface_playout_mp4"
    );
    assert_eq!(
        browser_surface_playout["validation_json"]["sustained_runtime_loop"],
        true
    );
    assert_eq!(
        browser_surface_playout["validation_json"]["runtime_delivery"]["frame_source"],
        "runtime_browser_surface_playout_chunk"
    );
    assert_eq!(
        browser_surface_playout["validation_json"]["validation"]["playable"],
        true
    );
    assert_eq!(
        browser_surface_playout["validation_json"]["validation"]["expected_frames"],
        2
    );
    assert!(
        tokio::fs::metadata(browser_surface_playout["artifact_path"].as_str().unwrap())
            .await
            .is_ok()
    );
    let sessions_after_browser_playout = call_json(
        app.clone(),
        Method::GET,
        "/api/v1/media/capture/sessions",
        None,
    )
    .await;
    let browser_surface_session_after_playout = sessions_after_browser_playout
        .as_array()
        .unwrap()
        .iter()
        .find(|session| session["id"] == browser_surface_capture["id"])
        .unwrap();
    assert_eq!(
        browser_surface_session_after_playout["health_json"]["runtime_compositor"]["latest_output"]
            ["output_kind"],
        "runtime_browser_surface_playout_mp4"
    );
    assert_eq!(
        browser_surface_session_after_playout["health_json"]["runtime_compositor"]["latest_output"]
            ["pacing_mode"],
        "runtime_program_clock"
    );
    assert!(
        browser_surface_session_after_playout["health_json"]["runtime_compositor"]
            ["outputs_observed"]
            .as_i64()
            .unwrap()
            >= 3
    );

    let camera_surface_capture = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/media/capture/sessions",
        Some(json!({
            "source_id": "source_camera_a",
            "capture_kind": "browser_surface",
            "width": 320,
            "height": 180,
            "frame_rate": 30,
            "audio": false,
            "duration_seconds": 2
        })),
    )
    .await;
    let (bad_source_frame_status, bad_source_frame_body) = call_status_json(
        app.clone(),
        Method::POST,
        &format!(
            "/api/v1/media/capture/sessions/{}/source-frame",
            camera_surface_capture["id"].as_str().unwrap()
        ),
        Some(json!({
            "image_data_url": generated_png_data_url(320, 180).await,
            "compositor_backend": "runtime_headless_browser",
            "frame_sequence": 1,
            "surface_kind": "browser_source"
        })),
    )
    .await;
    assert_eq!(bad_source_frame_status, StatusCode::BAD_REQUEST);
    assert!(
        bad_source_frame_body["error"]
            .as_str()
            .unwrap()
            .contains("browser or remote web surface")
    );

    if camera_supported {
        let capture = call_json(
            app.clone(),
            Method::POST,
            "/api/v1/media/capture/sessions",
            Some(json!({
                "source_id": "source_camera_a",
                "capture_kind": "camera",
                "width": 1920,
                "height": 1080,
                "frame_rate": 60,
                "audio": true,
                "duration_seconds": 5
            })),
        )
        .await;
        assert_eq!(capture["status"], "capturing");
        assert_eq!(capture["capture_kind"], "camera");
        assert_eq!(capture["settings_json"]["low_latency_preview"], true);
        assert_eq!(capture["settings_json"]["long_capture_validation"], true);
        assert_eq!(capture["settings_json"]["duration_seconds"], 5);
        assert_eq!(
            capture["settings_json"]["source_health"]["capture_kind"],
            "camera"
        );
        assert_eq!(capture["settings_json"]["source_health"]["supported"], true);
        assert!(capture["settings_json"]["permission"]["status"].is_string());
        assert_eq!(capture["settings_json"]["permission"]["required"], true);
        assert_eq!(
            capture["health_json"]["source_health"]["capture_kind"],
            "camera"
        );
        assert!(capture["health_json"]["permission"]["status"].is_string());
        assert_eq!(
            capture["health_json"]["events"][0]["event_kind"],
            "source_health"
        );
        assert_eq!(
            capture["helper_command_json"]["health_json"]["detail"]["source_id"],
            "source_camera_a"
        );
        assert_eq!(
            capture["helper_command_json"]["health_json"]["detail"]["source_health"]["capture_kind"],
            "camera"
        );
        encode_capture_id = capture["id"].as_str().unwrap().to_string();

        let camera_frame = call_json(
            app.clone(),
            Method::POST,
            &format!(
                "/api/v1/media/capture/sessions/{}/preview-frame",
                capture["id"].as_str().unwrap()
            ),
            None,
        )
        .await;
        assert_eq!(camera_frame["status"], "ready");
        assert_eq!(camera_frame["validation_json"]["capture_kind"], "camera");
        assert_eq!(
            camera_frame["validation_json"]["native_camera_source_bridge"],
            true
        );
        assert_eq!(
            camera_frame["validation_json"]["source_bridge"]["browser_get_user_media"],
            false
        );
        assert!(
            tokio::fs::metadata(camera_frame["artifact_path"].as_str().unwrap())
                .await
                .is_ok()
        );

        let camera_segment = call_json(
            app.clone(),
            Method::POST,
            &format!(
                "/api/v1/media/capture/sessions/{}/segment",
                capture["id"].as_str().unwrap()
            ),
            None,
        )
        .await;
        assert_eq!(camera_segment["status"], "ready");
        assert_eq!(camera_segment["artifact_kind"], "live_camera_mp4");
        assert_eq!(camera_segment["validation_json"]["playable"], true);
        assert_eq!(camera_segment["validation_json"]["capture_kind"], "camera");
        assert_eq!(
            camera_segment["validation_json"]["native_camera_source_bridge"],
            true
        );
        assert_eq!(
            camera_segment["validation_json"]["frame_pacing"]["mode"],
            "native_camera_avfoundation"
        );
        assert!(
            camera_segment["validation_json"]["observed_video_frames"]
                .as_i64()
                .unwrap()
                >= camera_segment["validation_json"]["minimum_video_frames"]
                    .as_i64()
                    .unwrap()
        );
        assert!(
            tokio::fs::metadata(camera_segment["artifact_path"].as_str().unwrap())
                .await
                .is_ok()
        );
    } else {
        let (camera_status, camera_body) = call_status_json(
            app.clone(),
            Method::POST,
            "/api/v1/media/capture/sessions",
            Some(json!({
                "source_id": "source_camera_a",
                "capture_kind": "camera",
                "width": 1920,
                "height": 1080,
                "frame_rate": 60,
                "audio": true,
                "duration_seconds": 5
            })),
        )
        .await;
        assert_eq!(camera_status, StatusCode::BAD_REQUEST);
        assert!(
            camera_body["error"]
                .as_str()
                .unwrap()
                .contains("capture kind is not supported")
        );
    }

    let display_capture = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/media/capture/sessions",
        Some(json!({
                "source_id": "source_screen",
                "capture_kind": display_kind,
                "width": 1920,
                "height": 1080,
                "frame_rate": 30,
            "audio": false,
            "duration_seconds": 2
        })),
    )
    .await;
    assert_eq!(display_capture["status"], "capturing");
    assert_eq!(display_capture["capture_kind"], display_kind);
    let reconciled_capture = call_json(
        app.clone(),
        Method::POST,
        &format!(
            "/api/v1/media/capture/sessions/{}/reconcile",
            display_capture["id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(reconciled_capture["status"], "capturing");
    assert_eq!(
        reconciled_capture["helper_command_json"]["health_json"]["command"],
        "reconcile_capture"
    );
    assert_eq!(
        reconciled_capture["health_json"]["native_reconnect"]["status"],
        "recovered"
    );
    assert_eq!(
        reconciled_capture["health_json"]["source_health"]["capture_kind"],
        display_kind
    );
    assert!(
        reconciled_capture["health_json"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["event_kind"] == "native_reconnect")
    );
    if encode_capture_id.is_empty() {
        encode_capture_id = display_capture["id"].as_str().unwrap().to_string();
        encode_duration_seconds = 2;
    }

    let frame = call_json(
        app.clone(),
        Method::POST,
        &format!(
            "/api/v1/media/capture/sessions/{}/preview-frame",
            display_capture["id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(frame["status"], "ready");
    assert_eq!(frame["frame_kind"], "preview_png");
    assert_eq!(frame["validation_json"]["format"], "png");
    assert_eq!(frame["validation_json"]["low_latency_preview"], true);
    assert_eq!(frame["validation_json"]["permission"], "granted");
    assert!(frame["validation_json"]["width"].as_i64().unwrap() > 0);
    assert!(frame["validation_json"]["height"].as_i64().unwrap() > 0);
    assert!(frame["validation_json"]["byte_length"].as_i64().unwrap() > 0);
    assert_eq!(
        frame["validation_json"]["sha256"].as_str().unwrap().len(),
        64
    );
    assert!(
        tokio::fs::metadata(frame["artifact_path"].as_str().unwrap())
            .await
            .is_ok()
    );

    let frames = call_json(
        app.clone(),
        Method::GET,
        &format!(
            "/api/v1/media/capture/sessions/{}/frames",
            display_capture["id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(frames.as_array().unwrap().len(), 1);
    assert_eq!(frames[0]["id"], frame["id"]);

    let segment = call_json(
        app.clone(),
        Method::POST,
        &format!(
            "/api/v1/media/capture/sessions/{}/segment",
            display_capture["id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(segment["status"], "ready");
    assert_eq!(segment["artifact_kind"], "continuous_display_mp4");
    assert_eq!(segment["validation_json"]["playable"], true);
    assert_eq!(segment["validation_json"]["continuous_capture"], true);
    assert_eq!(segment["validation_json"]["format"], "mp4");
    assert_eq!(segment["validation_json"]["permission"], "granted");
    assert!(segment["validation_json"]["width"].as_i64().unwrap() > 0);
    assert!(segment["validation_json"]["height"].as_i64().unwrap() > 0);
    assert!(
        segment["validation_json"]["observed_video_frames"]
            .as_i64()
            .unwrap()
            >= segment["validation_json"]["minimum_video_frames"]
                .as_i64()
                .unwrap()
    );
    assert!(
        segment["validation_json"]["frame_coverage"]
            .as_f64()
            .unwrap()
            >= segment["validation_json"]["frame_coverage_threshold"]
                .as_f64()
                .unwrap()
    );
    assert_eq!(
        segment["validation_json"]["sha256"].as_str().unwrap().len(),
        64
    );
    assert!(
        tokio::fs::metadata(segment["artifact_path"].as_str().unwrap())
            .await
            .is_ok()
    );

    let artifacts = call_json(
        app.clone(),
        Method::GET,
        &format!(
            "/api/v1/media/capture/sessions/{}/artifacts",
            display_capture["id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(artifacts.as_array().unwrap().len(), 1);
    assert_eq!(artifacts[0]["id"], segment["id"]);

    let microphone_capture = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/media/capture/sessions",
        Some(json!({
            "source_id": "source_microphone_a",
            "capture_kind": "microphone",
            "width": 1920,
            "height": 1080,
            "frame_rate": 30,
            "audio": true,
            "duration_seconds": 2
        })),
    )
    .await;
    assert_eq!(microphone_capture["status"], "capturing");
    assert_eq!(microphone_capture["capture_kind"], "microphone");

    let audio_segment = call_json(
        app.clone(),
        Method::POST,
        &format!(
            "/api/v1/media/capture/sessions/{}/segment",
            microphone_capture["id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(audio_segment["status"], "ready");
    assert_eq!(audio_segment["artifact_kind"], "live_microphone_m4a");
    assert_eq!(audio_segment["validation_json"]["playable"], true);
    assert_eq!(audio_segment["validation_json"]["live_input_capture"], true);
    assert_eq!(audio_segment["validation_json"]["isolated_audio"], true);
    assert_eq!(
        audio_segment["validation_json"]["drift_correction_active"],
        true
    );
    assert_eq!(
        audio_segment["validation_json"]["drift_correction_filter"],
        "aresample=async=1000:first_pts=0"
    );
    assert_eq!(audio_segment["validation_json"]["format"], "m4a");
    assert_eq!(audio_segment["validation_json"]["permission"], "granted");
    assert!(
        audio_segment["validation_json"]["validated_duration_seconds"]
            .as_f64()
            .unwrap()
            >= 1.65
    );
    assert!(
        audio_segment["validation_json"]["sample_rate"]
            .as_i64()
            .unwrap()
            > 0
    );
    assert!(
        audio_segment["validation_json"]["channels"]
            .as_i64()
            .unwrap()
            > 0
    );
    assert_eq!(
        audio_segment["validation_json"]["sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert!(
        tokio::fs::metadata(audio_segment["artifact_path"].as_str().unwrap())
            .await
            .is_ok()
    );

    let audio_artifacts = call_json(
        app.clone(),
        Method::GET,
        &format!(
            "/api/v1/media/capture/sessions/{}/artifacts",
            microphone_capture["id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(audio_artifacts.as_array().unwrap().len(), 1);
    assert_eq!(audio_artifacts[0]["id"], audio_segment["id"]);

    if devices["support"]["window"] == true {
        let window_capture = call_json(
            app.clone(),
            Method::POST,
            "/api/v1/media/capture/sessions",
            Some(json!({
                "source_id": "source_window_a",
                "capture_kind": "window",
                "width": 1280,
                "height": 720,
                "frame_rate": 30,
                "audio": false,
                "duration_seconds": 2
            })),
        )
        .await;
        assert_eq!(window_capture["status"], "capturing");
        assert_eq!(window_capture["capture_kind"], "window");
        assert_eq!(
            window_capture["settings_json"]["source_health"]["status"],
            "ready"
        );

        let window_frame = call_json(
            app.clone(),
            Method::POST,
            &format!(
                "/api/v1/media/capture/sessions/{}/preview-frame",
                window_capture["id"].as_str().unwrap()
            ),
            None,
        )
        .await;
        assert_eq!(window_frame["status"], "ready");
        assert_eq!(window_frame["validation_json"]["capture_kind"], "window");
        assert!(window_frame["validation_json"]["width"].as_i64().unwrap() > 0);
        assert!(window_frame["validation_json"]["height"].as_i64().unwrap() > 0);
        assert!(
            tokio::fs::metadata(window_frame["artifact_path"].as_str().unwrap())
                .await
                .is_ok()
        );

        let window_segment = call_json(
            app.clone(),
            Method::POST,
            &format!(
                "/api/v1/media/capture/sessions/{}/segment",
                window_capture["id"].as_str().unwrap()
            ),
            None,
        )
        .await;
        assert_eq!(window_segment["status"], "ready");
        assert_eq!(window_segment["artifact_kind"], "continuous_window_mp4");
        assert_eq!(window_segment["validation_json"]["playable"], true);
        assert_eq!(window_segment["validation_json"]["capture_kind"], "window");
        let window_mode = window_segment["validation_json"]["frame_pacing"]["mode"]
            .as_str()
            .unwrap();
        if window_mode == "screencapturekit_window_stream" {
            assert_eq!(
                window_segment["validation_json"]["native_api"],
                "ScreenCaptureKit"
            );
            assert_eq!(
                window_segment["validation_json"]["runtime_authoritative"],
                true
            );
            assert_eq!(
                window_segment["validation_json"]["sampled_frame_capture"],
                false
            );
            assert!(
                window_segment["validation_json"]["captured_window_frames"]
                    .as_i64()
                    .unwrap()
                    >= 60
            );
            assert_eq!(
                window_segment["validation_json"]["source_bridge"]["transport"],
                "screencapturekit"
            );
        } else {
            assert_eq!(window_mode, "native_window_frame_sampling_fallback");
            assert_eq!(
                window_segment["validation_json"]["native_api"],
                "CoreGraphics"
            );
            assert_eq!(
                window_segment["validation_json"]["runtime_authoritative"],
                false
            );
            assert_eq!(
                window_segment["validation_json"]["screencapturekit_attempted"],
                true
            );
            assert!(
                window_segment["validation_json"]["captured_window_samples"]
                    .as_i64()
                    .unwrap()
                    >= 2
            );
        }
        assert!(
            window_segment["validation_json"]["dropped_frames"]
                .as_i64()
                .unwrap()
                >= 0
        );
        assert!(
            tokio::fs::metadata(window_segment["artifact_path"].as_str().unwrap())
                .await
                .is_ok()
        );
    } else {
        let unsupported_window = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/media/capture/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "source_id": "source_window_a",
                            "capture_kind": "window",
                            "width": 1280,
                            "height": 720,
                            "frame_rate": 30,
                            "audio": false
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unsupported_window.status(), StatusCode::BAD_REQUEST);
    }

    if devices["support"]["desktop_audio"] == false {
        let unsupported_desktop_audio = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/media/capture/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "source_id": "source_desktop_audio",
                            "capture_kind": "desktop_audio",
                            "width": 1920,
                            "height": 1080,
                            "frame_rate": 30,
                            "audio": true,
                            "duration_seconds": 2
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unsupported_desktop_audio.status(), StatusCode::BAD_REQUEST);
    }

    if devices["support"]["system_audio"] == true {
        let system_audio_capture = call_json(
            app.clone(),
            Method::POST,
            "/api/v1/media/capture/sessions",
            Some(json!({
                "source_id": "source_system_audio",
                "capture_kind": "system_audio",
                "width": 1920,
                "height": 1080,
                "frame_rate": 30,
                "audio": true,
                "duration_seconds": 2
            })),
        )
        .await;
        assert_eq!(system_audio_capture["status"], "capturing");
        assert_eq!(system_audio_capture["capture_kind"], "system_audio");
        assert_eq!(
            system_audio_capture["settings_json"]["source_health"]["supported"],
            true
        );
        assert_eq!(
            system_audio_capture["settings_json"]["source_health"]["permission"]["required"],
            true
        );
        assert_eq!(
            system_audio_capture["helper_command_json"]["health_json"]["detail"]["source_health"]["capture_kind"],
            "system_audio"
        );
    } else {
        let unsupported_system_audio = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/media/capture/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "source_id": "source_system_audio",
                            "capture_kind": "system_audio",
                            "width": 1920,
                            "height": 1080,
                            "frame_rate": 30,
                            "audio": true,
                            "duration_seconds": 2
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unsupported_system_audio.status(), StatusCode::BAD_REQUEST);
    }

    if devices["support"]["application_audio"] == true {
        let application_audio_capture = call_json(
            app.clone(),
            Method::POST,
            "/api/v1/media/capture/sessions",
            Some(json!({
                "source_id": "source_application_audio",
                "capture_kind": "application_audio",
                "width": 1920,
                "height": 1080,
                "frame_rate": 30,
                "audio": true,
                "duration_seconds": 2
            })),
        )
        .await;
        assert_eq!(application_audio_capture["status"], "capturing");
        assert_eq!(
            application_audio_capture["capture_kind"],
            "application_audio"
        );
        assert_eq!(
            application_audio_capture["settings_json"]["source_health"]["supported"],
            true
        );
        assert_eq!(
            application_audio_capture["settings_json"]["source_health"]["permission"]["required"],
            true
        );
        assert_eq!(
            application_audio_capture["helper_command_json"]["health_json"]["detail"]["source_health"]
                ["capture_kind"],
            "application_audio"
        );
    } else {
        let unsupported_application_audio = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/media/capture/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "source_id": "source_application_audio",
                            "capture_kind": "application_audio",
                            "width": 1920,
                            "height": 1080,
                            "frame_rate": 30,
                            "audio": true,
                            "duration_seconds": 2
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            unsupported_application_audio.status(),
            StatusCode::BAD_REQUEST
        );
    }

    let capabilities =
        call_json(app.clone(), Method::GET, "/api/v1/media/capabilities", None).await;
    assert!(capabilities["h265"].is_boolean());
    assert!(capabilities["av1"].is_boolean());
    assert!(capabilities["opus"].is_boolean());
    assert_eq!(capabilities["containers"]["mkv"], true);

    let dashboard = call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let encode = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/media/encode/jobs",
        Some(json!({
                "broadcast_id": dashboard["broadcast"]["id"],
                "capture_session_id": encode_capture_id.clone(),
                "codec": "h264",
                "audio_codec": "aac",
                "container": "fragmented_mp4",
            "bitrate_kbps": 6200,
            "keyframe_interval_seconds": 2,
            "latency_profile": "low"
        })),
    )
    .await;
    assert_eq!(encode["status"], "encoding");
    assert_eq!(encode["codec"], "h264");
    assert_eq!(encode["profile_json"]["hardware_encoder"], "auto");
    assert_eq!(
        encode["helper_command_json"]["health_json"]["detail"]["codec"],
        "h264"
    );

    let jobs = call_json(app.clone(), Method::GET, "/api/v1/media/encode/jobs", None).await;
    assert_eq!(jobs.as_array().unwrap().len(), 1);

    let rendered = call_json(
        app.clone(),
        Method::POST,
        &format!(
            "/api/v1/media/encode/jobs/{}/render",
            encode["id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(rendered["status"], "playable");
    assert_eq!(rendered["health_json"]["playable_validation"], "passed");
    assert_eq!(rendered["health_json"]["validation"]["has_video"], true);
    assert_eq!(rendered["health_json"]["validation"]["has_audio"], true);
    assert_eq!(
        rendered["health_json"]["validation"]["requested_duration_seconds"],
        encode_duration_seconds
    );
    assert_eq!(
        rendered["health_json"]["validation"]["long_capture_validation"],
        true
    );
    assert!(
        rendered["health_json"]["validation"]["validated_duration_seconds"]
            .as_f64()
            .unwrap()
            >= if encode_duration_seconds == 5 {
                4.65
            } else {
                1.65
            }
    );
    assert!(
        rendered["health_json"]["validation"]["observed_video_frames"]
            .as_i64()
            .unwrap()
            >= if encode_duration_seconds == 5 {
                285
            } else {
                105
            }
    );
    assert!(
        rendered["health_json"]["validation"]["frame_coverage"]
            .as_f64()
            .unwrap()
            >= 0.95
    );
    assert!(
        rendered["health_json"]["validation"]["selected_encoder"]
            .as_str()
            .unwrap()
            .contains("264")
    );
    assert_eq!(
        rendered["health_json"]["validation"]["latency_profile"],
        "low"
    );
    assert!(
        rendered["health_json"]["validation"]["attempted_video_encoders"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str().unwrap().contains("264"))
    );
    assert_eq!(
        rendered["health_json"]["validation"]["muxer_recovery"]["committed_atomically"],
        true
    );
    let output_path = rendered["output_path"].as_str().unwrap();
    assert!(
        std::path::Path::new(output_path).is_file(),
        "rendered output missing at {output_path}"
    );
    let partial_path = rendered["health_json"]["validation"]["muxer_recovery"]["partial_path"]
        .as_str()
        .unwrap();
    assert!(
        !std::path::Path::new(partial_path).exists(),
        "partial render artifact should not remain at {partial_path}"
    );

    let package = call_json(
        app.clone(),
        Method::POST,
        &format!(
            "/api/v1/media/encode/jobs/{}/package",
            rendered["id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(package["status"], "ready");
    assert_eq!(package["package_kind"], "hls_cmaf");
    assert_eq!(package["package_json"]["playback_ready"], true);
    assert!(
        std::path::Path::new(package["manifest_path"].as_str().unwrap()).is_file(),
        "package manifest missing"
    );
    let segment_count = package["package_json"]["segment_count"].as_i64().unwrap();
    assert!(segment_count > 0, "expected at least one CMAF segment");

    let packages = call_json(app.clone(), Method::GET, "/api/v1/media/packages", None).await;
    assert_eq!(packages.as_array().unwrap().len(), 1);

    let mkv = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/media/encode/jobs",
        Some(json!({
                "broadcast_id": dashboard["broadcast"]["id"],
                "capture_session_id": encode_capture_id.clone(),
                "codec": "h264",
                "audio_codec": "opus",
            "container": "mkv",
            "bitrate_kbps": 1800,
            "keyframe_interval_seconds": 2,
            "latency_profile": "ultra_low"
        })),
    )
    .await;
    let rendered_mkv = call_json(
        app.clone(),
        Method::POST,
        &format!(
            "/api/v1/media/encode/jobs/{}/render",
            mkv["id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(rendered_mkv["status"], "playable");
    assert!(
        rendered_mkv["output_path"]
            .as_str()
            .unwrap()
            .ends_with(".mkv")
    );
    assert_eq!(
        rendered_mkv["health_json"]["validation"]["streams"]
            .as_array()
            .unwrap()
            .iter()
            .find(|stream| stream["codec_type"] == "audio")
            .unwrap()["codec_name"],
        "opus"
    );

    let finalizing = call_json(
        app.clone(),
        Method::POST,
        &format!(
            "/api/v1/media/encode/jobs/{}/stop",
            encode["id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(finalizing["status"], "finalizing");

    let stopped = call_json(
        app,
        Method::POST,
        &format!("/api/v1/media/capture/sessions/{}/stop", encode_capture_id),
        None,
    )
    .await;
    assert_eq!(stopped["status"], "stopped");
}

#[tokio::test]
async fn media_source_audio_ingest_validates_drift_and_sandbox() {
    let app = test_app().await;
    let input_path = create_media_source_fixture("media-source-audio").await;

    let artifact = call_json(
        app.clone(),
        Method::POST,
        "/api/v1/media/sources/audio",
        Some(json!({
            "source_id": "source_media_fixture",
            "input_path": input_path
        })),
    )
    .await;
    assert_eq!(artifact["status"], "ready");
    assert_eq!(artifact["artifact_kind"], "media_source_audio_m4a");
    assert_eq!(artifact["validation_json"]["playable"], true);
    assert_eq!(artifact["validation_json"]["media_source_audio"], true);
    assert_eq!(artifact["validation_json"]["drift_correction_ready"], true);
    assert_eq!(artifact["validation_json"]["drift_correction_active"], true);
    assert_eq!(
        artifact["validation_json"]["drift_correction_filter"],
        "aresample=async=1000:first_pts=0"
    );
    assert_eq!(artifact["validation_json"]["drift_status"], "synced");
    assert_eq!(artifact["validation_json"]["format"], "m4a");
    assert!(
        artifact["validation_json"]["validated_duration_seconds"]
            .as_f64()
            .unwrap()
            >= 1.65
    );
    assert!(
        artifact["validation_json"]["audio_video_drift_ms"]
            .as_f64()
            .unwrap()
            <= 120.0
    );
    assert!(artifact["validation_json"]["sample_rate"].as_i64().unwrap() > 0);
    assert!(artifact["validation_json"]["channels"].as_i64().unwrap() > 0);
    assert_eq!(
        artifact["validation_json"]["sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert!(
        tokio::fs::metadata(artifact["artifact_path"].as_str().unwrap())
            .await
            .is_ok()
    );

    let artifacts = call_json(
        app.clone(),
        Method::GET,
        "/api/v1/media/sources/source_media_fixture/artifacts",
        None,
    )
    .await;
    assert_eq!(artifacts.as_array().unwrap().len(), 1);
    assert_eq!(artifacts[0]["id"], artifact["id"]);

    let (outside_status, _) = call_status_json(
        app,
        Method::POST,
        "/api/v1/media/sources/audio",
        Some(json!({
            "source_id": "source_media_fixture",
            "input_path": "/etc/hosts"
        })),
    )
    .await;
    assert_eq!(outside_status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn media_encode_rejects_invalid_profiles() {
    let app = test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/media/encode/jobs")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "broadcast_id": "broadcast_prime_launch",
                        "capture_session_id": "missing",
                        "codec": "mpeg2",
                        "audio_codec": "aac",
                        "container": "fragmented_mp4",
                        "bitrate_kbps": 6200,
                        "keyframe_interval_seconds": 2,
                        "latency_profile": "low"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn safety_gate_blocks_start_and_emergency_routes_to_holding_scene() {
    let app = test_app().await;
    let dashboard = call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let broadcast_id = dashboard["broadcast"]["id"].as_str().unwrap().to_string();
    let camera_id = dashboard["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["source_kind"] == "camera")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    call_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/obs/me/sources/{camera_id}"),
        Some(json!({
            "permission_state": "denied",
            "health_state": "blocked"
        })),
    )
    .await;
    let (status, blocked) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/start"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        blocked["error"]
            .as_str()
            .unwrap()
            .contains("safety blocked action")
    );

    call_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/obs/me/sources/{camera_id}"),
        Some(json!({
            "permission_state": "granted",
            "health_state": "good"
        })),
    )
    .await;
    let live = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/start"),
        None,
    )
    .await;
    assert_eq!(live["runtime"]["stream_state"], "live");

    let held = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/emergency-disconnect"),
        Some(json!({
            "reason": "Route safety test",
            "operator_id": "operator_test"
        })),
    )
    .await;
    assert_eq!(held["runtime"]["stream_state"], "emergency_disconnected");
    assert_eq!(held["runtime"]["runtime_state"], "safe_mode");
    assert_eq!(
        held["runtime"]["program_scene_id"],
        "scene_emergency_holding"
    );
    assert_eq!(
        held["safety"]["latest_incident"]["incident_kind"],
        "emergency_disconnect"
    );
    assert_eq!(
        held["safety"]["latest_incident"]["holding_scene_id"],
        "scene_emergency_holding"
    );

    let bundle = call_json(
        app,
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/support-bundle"),
        None,
    )
    .await;
    assert_eq!(bundle["status"], "ready");
    assert_eq!(
        bundle["bundle_json"]["runtime"]["stream_state"],
        "emergency_disconnected"
    );
    assert!(
        bundle["bundle_json"]["incidents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|incident| incident["incident_kind"] == "emergency_disconnect")
    );
}

#[tokio::test]
async fn live_ops_overrides_are_audited_and_mutate_runtime_state() {
    let app = test_app().await;
    let dashboard = call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let broadcast_id = dashboard["broadcast"]["id"].as_str().unwrap().to_string();

    let live = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/start"),
        None,
    )
    .await;
    assert_eq!(live["runtime"]["stream_state"], "live");

    let (blocked_status, blocked_body) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/live-ops/override"),
        Some(json!({
            "action": "force_end",
            "reason": "Unconfirmed force end",
            "operator_id": "guest_operator"
        })),
    )
    .await;
    assert_eq!(blocked_status, StatusCode::CONFLICT);
    assert!(
        blocked_body["error"]
            .as_str()
            .unwrap()
            .contains("cannot run live_ops_force_end")
    );

    let held = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/live-ops/override"),
        Some(json!({
            "action": "safe_mode",
            "reason": "Live ops smoke hold",
            "operator_id": "ops_lead",
            "operator_role": "live_ops",
            "target_scene_id": "scene_emergency_holding"
        })),
    )
    .await;
    assert_eq!(held["runtime"]["runtime_state"], "safe_mode");
    assert_eq!(held["runtime"]["stream_state"], "live_ops_hold");
    assert_eq!(held["runtime"]["runtime_output_json"]["status"], "held");
    assert_eq!(
        held["safety"]["latest_incident"]["incident_kind"],
        "live_ops_override"
    );
    assert!(held["events"].as_array().unwrap().iter().any(|event| {
        event["event_kind"] == "live_ops_override" && event["severity"] == "warning"
    }));

    let cleared = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/live-ops/override"),
        Some(json!({
            "action": "clear_incidents",
            "reason": "Ops acknowledged hold",
            "operator_id": "ops_lead",
            "operator_role": "live_ops"
        })),
    )
    .await;
    assert_eq!(cleared["safety"]["incident_count"], 0);
    assert!(
        cleared["safety"]["resolved_incident_count"]
            .as_i64()
            .unwrap()
            >= 1
    );

    let forced = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/live-ops/override"),
        Some(json!({
            "action": "force_end",
            "reason": "Live ops forced end",
            "operator_id": "ops_lead",
            "operator_role": "live_ops",
            "confirmation_text": "FORCE END",
            "acknowledged_risks": ["campaign_recording"]
        })),
    )
    .await;
    assert_eq!(forced["runtime"]["stream_state"], "ended");
    assert_eq!(forced["runtime"]["runtime_state"], "live_ops_force_ended");
    assert_eq!(
        forced["runtime"]["runtime_output_json"]["status"],
        "force_ended"
    );
    assert_eq!(forced["post_show"]["status"], "packaging");
    assert!(forced["events"].as_array().unwrap().iter().any(|event| {
        event["event_kind"] == "live_ops_override" && event["severity"] == "critical"
    }));

    let (bad_status, _) = call_status_json(
        app,
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/live-ops/override"),
        Some(json!({
            "action": "unsupported",
            "reason": "bad"
        })),
    )
    .await;
    assert_eq!(bad_status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn runtime_telemetry_derives_health_and_adaptive_bitrate() {
    let app = test_app().await;
    let dashboard = call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let broadcast_id = dashboard["broadcast"]["id"].as_str().unwrap().to_string();

    call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/start"),
        None,
    )
    .await;

    let steady = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/runtime/telemetry"),
        Some(json!({
            "sample_kind": "runtime_sample",
            "bitrate_kbps": 6200,
            "upload_mbps": 18.0,
            "ingest_latency_ms": 700,
            "dropped_frames": 8,
            "cpu_percent": 42,
            "reconnect_count": 0,
            "details_json": {"network":"fiber"}
        })),
    )
    .await;
    assert_eq!(steady["runtime"]["runtime_state"], "healthy");
    assert_eq!(
        steady["runtime"]["runtime_status_json"]["stream_health"]["status"],
        "green"
    );
    assert_eq!(
        steady["runtime"]["runtime_status_json"]["stream_health"]["dynamic_bitrate"],
        "stable"
    );
    assert_eq!(
        steady["runtime"]["runtime_output_json"]["health_json"]["long_session"]["drop_status"],
        "nominal"
    );
    let steady_sample_count =
        steady["runtime"]["runtime_output_json"]["health_json"]["long_session"]["sample_count"]
            .as_i64()
            .unwrap();
    assert!(steady_sample_count >= 1);

    let constrained = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/runtime/telemetry"),
        Some(json!({
            "sample_kind": "runtime_sample",
            "bitrate_kbps": 6200,
            "upload_mbps": 3.2,
            "ingest_latency_ms": 2900,
            "dropped_frames": 240,
            "cpu_percent": 94,
            "reconnect_count": 4
        })),
    )
    .await;
    assert_eq!(constrained["runtime"]["runtime_state"], "reconnecting");
    assert_eq!(
        constrained["runtime"]["runtime_output_json"]["status"],
        "reconnecting"
    );
    assert_eq!(
        constrained["runtime"]["runtime_status_json"]["stream_health"]["status"],
        "red"
    );
    assert_eq!(
        constrained["runtime"]["runtime_status_json"]["stream_health"]["adaptation"]["target_bitrate_kbps"],
        2500
    );
    assert_eq!(
        constrained["runtime"]["runtime_status_json"]["reconnect"]["ingest_status"],
        "reconnecting"
    );
    assert_eq!(
        constrained["runtime"]["runtime_output_json"]["health_json"]["reconnect_attempts"]["status"],
        "retrying"
    );
    assert_eq!(
        constrained["runtime"]["runtime_output_json"]["health_json"]["reconnect_attempts"]["next_backoff_ms"],
        4000
    );
    assert_eq!(
        constrained["runtime"]["runtime_output_json"]["health_json"]["local_publish"]["status"],
        "publishing"
    );
    let constrained_dropped_frames = constrained["runtime"]["runtime_output_json"]["health_json"]
        ["long_session"]["cumulative_dropped_frames"]
        .as_i64()
        .unwrap();
    assert!(constrained_dropped_frames >= 248);
    assert_eq!(
        constrained["runtime"]["runtime_output_json"]["health_json"]["long_session"]["drop_status"],
        "critical"
    );
    assert_eq!(
        constrained["runtime"]["runtime_output_json"]["health_json"]["long_session"]["drift_status"],
        "drift_warning"
    );
    assert_eq!(
        constrained["runtime"]["runtime_output_json"]["health_json"]["long_session"]["continuity_action"],
        "protect_audio_hold_last_good_frame_reduce_video_layer"
    );
    assert!(
        constrained["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| {
                event["event_kind"] == "runtime_telemetry" && event["severity"] == "critical"
            })
    );

    let recovered = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/runtime/telemetry"),
        Some(json!({
            "sample_kind": "runtime_sample",
            "bitrate_kbps": 6200,
            "upload_mbps": 16.5,
            "ingest_latency_ms": 640,
            "dropped_frames": 12,
            "cpu_percent": 48,
            "reconnect_count": 0
        })),
    )
    .await;
    assert_eq!(recovered["runtime"]["runtime_state"], "healthy");
    assert_eq!(
        recovered["runtime"]["runtime_output_json"]["status"],
        "publishing"
    );
    assert_eq!(
        recovered["runtime"]["runtime_status_json"]["reconnect"]["ingest_status"],
        "active"
    );
    let recovered_publish =
        &recovered["runtime"]["runtime_output_json"]["health_json"]["local_publish"];
    assert_eq!(recovered_publish["status"], "publishing");
    assert_eq!(
        recovered["runtime"]["runtime_output_json"]["health_json"]["reconnect_attempts"]["status"],
        "recovered"
    );
    assert_eq!(
        recovered["runtime"]["runtime_output_json"]["health_json"]["long_session"]["sample_count"]
            .as_i64()
            .unwrap(),
        steady_sample_count + 2
    );
    assert_eq!(
        recovered["runtime"]["runtime_output_json"]["health_json"]["long_session"]["reconnect_status"],
        "recovered"
    );
    assert_eq!(
        recovered["runtime"]["runtime_output_json"]["health_json"]["long_session"]["cumulative_dropped_frames"],
        constrained_dropped_frames + 12
    );
    let recovered_manifest = recovered["runtime"]["runtime_output_json"]["health_json"]["reconnect_attempts"]["recovered_manifest_path"]
        .as_str()
        .unwrap();
    assert!(
        std::path::Path::new(recovered_manifest).is_file(),
        "expected recovered stream manifest at {recovered_manifest}"
    );
    assert!(
        recovered_publish["segments"]
            .as_array()
            .unwrap()
            .iter()
            .all(|segment| std::path::Path::new(segment["path"].as_str().unwrap()).is_file())
    );
    assert_eq!(
        recovered["runtime"]["runtime_status_json"]["stream_health"]["dynamic_bitrate"],
        "stable"
    );

    let (bad_status, _) = call_status_json(
        app,
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/runtime/telemetry"),
        Some(json!({
            "bitrate_kbps": -1,
            "upload_mbps": 1,
            "ingest_latency_ms": 1,
            "dropped_frames": 0,
            "cpu_percent": 30
        })),
    )
    .await;
    assert_eq!(bad_status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn channel_metadata_updates_are_bound_to_runtime_state() {
    let app = test_app().await;
    let dashboard = call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let broadcast_id = dashboard["broadcast"]["id"].as_str().unwrap().to_string();

    let updated = call_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}"),
        Some(json!({
            "title": "Late Night Build Room",
            "category": "Software",
            "tags": ["rust", "live coding", "launch"],
            "mature_content": true,
            "language": "en-US",
            "scheduled_start": "2026-09-05T23:00:00Z",
            "visibility": "public",
            "follower_notification": false,
            "chat_mode": "slow_mode"
        })),
    )
    .await;
    assert_eq!(updated["broadcast"]["title"], "Late Night Build Room");
    assert_eq!(updated["broadcast"]["category"], "Software");
    assert_eq!(updated["broadcast"]["mature_content"], 1);
    assert_eq!(updated["broadcast"]["follower_notification"], 0);
    assert_eq!(updated["broadcast"]["chat_mode"], "slow_mode");
    assert_eq!(
        updated["runtime"]["runtime_status_json"]["channel"]["title"],
        "Late Night Build Room"
    );
    assert_eq!(
        updated["runtime"]["runtime_status_json"]["channel"]["tags"][1],
        "live coding"
    );
    assert_eq!(
        updated["runtime"]["runtime_status_json"]["channel"]["chat_mode"],
        "slow_mode"
    );
    assert!(updated["events"].as_array().unwrap().iter().any(|event| {
        event["event_kind"] == "channel_update"
            && event["message"] == "Live channel metadata updated"
    }));

    let (bad_status, _) = call_status_json(
        app,
        Method::PATCH,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}"),
        Some(json!({
            "chat_mode": "chaos_mode"
        })),
    )
    .await;
    assert_eq!(bad_status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn moderation_contract_handles_queue_terms_pins_and_roles() {
    let app = test_app().await;
    let dashboard = call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let broadcast_id = dashboard["broadcast"]["id"].as_str().unwrap().to_string();
    assert_eq!(dashboard["moderation"]["pending_count"], 1);
    assert_eq!(
        dashboard["moderation"]["active_pin"]["message"],
        "Use code VANTA20 during the launch segment."
    );

    let with_term = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/moderation/blocked-terms"),
        Some(json!({
            "term": "spoiler",
            "action": "hold"
        })),
    )
    .await;
    assert!(
        with_term["moderation"]["blocked_terms_json"]
            .as_array()
            .unwrap()
            .iter()
            .any(|term| term["term"] == "spoiler")
    );

    let queued = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/moderation/queue"),
        Some(json!({
            "author_id": "viewer_bad",
            "author_name": "Spoiler Fan",
            "message": "Huge spoiler in chat",
            "reason": null
        })),
    )
    .await;
    assert_eq!(queued["moderation"]["pending_count"], 2);
    let queued_item = queued["moderation"]["queue_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["author_id"] == "viewer_bad")
        .unwrap();
    assert_eq!(queued_item["reason"], "blocked term: spoiler");
    let item_id = queued_item["id"].as_str().unwrap().to_string();

    let resolved = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/moderation/queue/{item_id}/resolve"),
        Some(json!({
            "status": "hidden",
            "moderator_id": "user_producer_ike"
        })),
    )
    .await;
    assert!(
        resolved["moderation"]["queue_json"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == item_id && item["status"] == "hidden")
    );

    let pinned = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/moderation/pins"),
        Some(json!({
            "author_name": "Vanta",
            "message": "Pinned launch reminder"
        })),
    )
    .await;
    assert_eq!(
        pinned["moderation"]["active_pin"]["message"],
        "Pinned launch reminder"
    );
    let pin_id = pinned["moderation"]["active_pin"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let unpinned = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/moderation/pins/{pin_id}/unpin"),
        None,
    )
    .await;
    assert!(unpinned["moderation"]["active_pin"].is_null());

    let with_mod = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/moderation/moderators"),
        Some(json!({
            "user_id": "user_new_mod",
            "display_name": "New Mod",
            "role": "moderator"
        })),
    )
    .await;
    assert!(
        with_mod["moderation"]["moderators_json"]
            .as_array()
            .unwrap()
            .iter()
            .any(|moderator| moderator["display_name"] == "New Mod")
    );
    assert!(with_mod["events"].as_array().unwrap().iter().any(|event| {
        event["event_kind"] == "moderator_role"
            || event["event_kind"] == "moderation_resolve"
            || event["event_kind"] == "pinned_message"
    }));

    let (bad_role_status, _) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/moderation/moderators"),
        Some(json!({
            "user_id": "user_bad",
            "display_name": "Bad Role",
            "role": "super_admin"
        })),
    )
    .await;
    assert_eq!(bad_role_status, StatusCode::BAD_REQUEST);

    let (bad_resolve_status, _) = call_status_json(
        app,
        Method::POST,
        &format!("/api/v1/obs/me/moderation/queue/{item_id}/resolve"),
        Some(json!({
            "status": "maybe",
            "moderator_id": "user_producer_ike"
        })),
    )
    .await;
    assert_eq!(bad_resolve_status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn audience_telemetry_tracks_live_metrics_discovery_and_revenue() {
    let app = test_app().await;
    let dashboard = call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let broadcast_id = dashboard["broadcast"]["id"].as_str().unwrap().to_string();

    assert_eq!(dashboard["audience"]["viewer_count"], 842);
    assert_eq!(dashboard["audience"]["peak_viewers"], 842);
    assert_eq!(
        dashboard["audience"]["discovery_source"],
        "home_recommendation"
    );

    let updated = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/audience/telemetry"),
        Some(json!({
            "viewer_count": 1200,
            "chat_messages_per_minute": 144,
            "tips_cents": 799,
            "subscriptions": 4,
            "revenue_cents": 2599,
            "discovery_source": "category_rank",
            "discovery_score": 88.2,
            "details_json": {"surface": "test"}
        })),
    )
    .await;
    assert_eq!(updated["audience"]["viewer_count"], 1200);
    assert_eq!(updated["audience"]["chat_messages_per_minute"], 144);
    assert_eq!(updated["audience"]["peak_viewers"], 1200);
    assert_eq!(updated["audience"]["average_viewers"], 1021.0);
    assert_eq!(updated["audience"]["revenue_cents"], 7198);
    assert_eq!(updated["audience"]["tips_cents"], 3298);
    assert_eq!(updated["audience"]["subscriptions"], 18);
    assert_eq!(updated["audience"]["discovery_source"], "category_rank");
    assert_eq!(
        updated["audience"]["latest_snapshot"]["discovery_json"]["surface"],
        "test"
    );
    assert!(
        updated["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| { event["event_kind"] == "audience_telemetry" })
    );

    let raided = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/audience/raid-redirects"),
        Some(json!({
            "target_channel_id": "creator_afterparty",
            "target_channel_name": "Afterparty Studio",
            "viewer_count": 1188,
            "execute_after_seconds": 30,
            "redirect_url": "https://streamvanta.tv/creator_afterparty/live",
            "safety_json": {"moderation_handoff": true}
        })),
    )
    .await;
    assert_eq!(
        raided["audience"]["latest_outbound_raid"]["direction"],
        "outbound"
    );
    assert_eq!(
        raided["audience"]["latest_outbound_raid"]["status"],
        "scheduled"
    );
    assert_eq!(
        raided["audience"]["latest_outbound_raid"]["target_channel_name"],
        "Afterparty Studio"
    );
    assert_eq!(
        raided["audience"]["latest_outbound_raid"]["safety_json"]["chat_notice_required"],
        true
    );

    let inbound = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/audience/raids/inbound"),
        Some(json!({
            "target_channel_id": "creator_luna",
            "target_channel_name": "Luna Live",
            "viewer_count": 312,
            "redirect_url": "https://streamvanta.tv/creator_luna/live",
            "safety_json": {"moderation_handoff": true}
        })),
    )
    .await;
    assert_eq!(
        inbound["audience"]["latest_inbound_raid"]["direction"],
        "inbound"
    );
    assert_eq!(
        inbound["audience"]["latest_inbound_raid"]["status"],
        "received"
    );
    assert_eq!(
        inbound["audience"]["latest_inbound_raid"]["viewer_count"],
        312
    );
    assert_eq!(inbound["audience"]["raid_count"], 2);
    assert!(inbound["events"].as_array().unwrap().iter().any(|event| {
        event["event_kind"] == "raid_redirect" || event["event_kind"] == "raid_inbound"
    }));

    let (bad_status, _) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/audience/telemetry"),
        Some(json!({
            "viewer_count": -1,
            "chat_messages_per_minute": 1
        })),
    )
    .await;
    assert_eq!(bad_status, StatusCode::BAD_REQUEST);

    let (bad_raid_status, bad_raid_body) = call_status_json(
        app,
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/audience/raid-redirects"),
        Some(json!({
            "target_channel_id": "creator_bad",
            "target_channel_name": "Bad Redirect",
            "viewer_count": 10,
            "execute_after_seconds": 2,
            "redirect_url": "http://not-allowed.test/live"
        })),
    )
    .await;
    assert_eq!(bad_raid_status, StatusCode::BAD_REQUEST);
    assert!(
        bad_raid_body["error"]
            .as_str()
            .unwrap()
            .contains("execute_after_seconds")
            || bad_raid_body["error"]
                .as_str()
                .unwrap()
                .contains("redirect_url")
    );
}

#[tokio::test]
async fn engagement_contract_manages_schedule_polls_predictions_and_alerts() {
    let app = test_app().await;
    let dashboard = call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let broadcast_id = dashboard["broadcast"]["id"].as_str().unwrap().to_string();
    assert_eq!(dashboard["engagement"]["schedule_count"], 1);
    assert_eq!(
        dashboard["engagement"]["active_poll"]["question"],
        "Which segment should we replay?"
    );
    assert_eq!(dashboard["engagement"]["active_poll"]["total_votes"], 2);
    assert_eq!(dashboard["engagement"]["ready_alert_count"], 1);

    let scheduled = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/schedule"),
        Some(json!({
            "title": "Late-night sponsor Q&A",
            "starts_at": "2026-08-27T22:00:00-04:00",
            "timezone": "America/New_York",
            "duration_minutes": 45,
            "reminder_json": {"notify_followers": true, "reminder_minutes": [30, 5]}
        })),
    )
    .await;
    let slot_id = scheduled["engagement"]["schedule_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|slot| slot["title"] == "Late-night sponsor Q&A")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let rescheduled = call_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/obs/me/schedule/{slot_id}"),
        Some(json!({
            "status": "rescheduled",
            "duration_minutes": 60
        })),
    )
    .await;
    let patched_slot = rescheduled["engagement"]["schedule_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|slot| slot["id"] == slot_id)
        .unwrap();
    assert_eq!(patched_slot["status"], "rescheduled");
    assert_eq!(patched_slot["duration_minutes"], 60);

    let prediction = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/engagement/polls"),
        Some(json!({
            "poll_kind": "prediction",
            "question": "Will the demo finish under five minutes?",
            "options": ["Yes", "No"],
            "duration_seconds": 120
        })),
    )
    .await;
    let poll = prediction["engagement"]["active_poll"].clone();
    let poll_id = poll["id"].as_str().unwrap().to_string();
    let option_id = poll["options_json"][0]["id"].as_str().unwrap().to_string();
    let voted = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/engagement/polls/{poll_id}/vote"),
        Some(json!({
            "option_id": option_id,
            "voter_id": "viewer_prediction_1"
        })),
    )
    .await;
    assert_eq!(voted["engagement"]["active_poll"]["total_votes"], 1);
    assert_eq!(voted["engagement"]["active_poll"]["is_prediction"], true);
    let closed = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/engagement/polls/{poll_id}/close"),
        None,
    )
    .await;
    assert_eq!(closed["engagement"]["polls_json"][0]["status"], "closed");

    let alerted = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/engagement/alerts"),
        Some(json!({
            "alert_kind": "tip",
            "title": "Tip received",
            "message": "Ari tipped during the sponsor read.",
            "severity": "success",
            "source_user": "Ari",
            "amount_cents": 2500,
            "metadata_json": {"source": "stripe_test"}
        })),
    )
    .await;
    assert_eq!(alerted["engagement"]["alerts_json"][0]["alert_kind"], "tip");
    assert_eq!(
        alerted["engagement"]["alerts_json"][0]["amount_cents"],
        2500
    );
    assert!(alerted["events"].as_array().unwrap().iter().any(|event| {
        event["event_kind"] == "engagement_alert"
            || event["event_kind"] == "engagement_poll"
            || event["event_kind"] == "schedule_slot"
    }));

    let (bad_poll_status, _) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/engagement/polls"),
        Some(json!({
            "poll_kind": "lottery",
            "question": "Bad",
            "options": ["Only one"],
            "duration_seconds": 120
        })),
    )
    .await;
    assert_eq!(bad_poll_status, StatusCode::BAD_REQUEST);

    let (bad_vote_status, _) = call_status_json(
        app,
        Method::POST,
        &format!("/api/v1/obs/me/engagement/polls/{poll_id}/vote"),
        Some(json!({
            "option_id": "missing",
            "voter_id": "viewer_bad"
        })),
    )
    .await;
    assert_eq!(bad_vote_status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn sponsor_inventory_tracks_campaign_creatives_proof_review_and_handoff() {
    let app = test_app().await;
    let dashboard = call_json(app.clone(), Method::GET, "/api/v1/obs/me/dashboard", None).await;
    let broadcast_id = dashboard["broadcast"]["id"].as_str().unwrap().to_string();

    let attached = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/sponsor/campaigns"),
        Some(json!({
            "campaign_id": "campaign_test_nova",
            "advertiser": "Nova",
            "title": "Nova Launch Run",
            "flight_json": {"source": "vanta_backend", "spots": 2},
            "claims_json": {
                "required": ["Use code VANTA20"],
                "prohibited": ["guaranteed results"]
            },
            "performance_json": {"handoff": "ad_ops_ready", "deal_id": "deal_123"}
        })),
    )
    .await;
    assert_eq!(
        attached["broadcast"]["sponsor_campaign_id"],
        "campaign_test_nova"
    );
    assert_eq!(attached["sponsor"]["active_campaign"]["advertiser"], "Nova");
    assert_eq!(
        attached["sponsor"]["performance_handoff_json"]["handoff"],
        "ad_ops_ready"
    );

    let inventory = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/sponsor/inventory"),
        Some(json!({
            "campaign_id": "campaign_test_nova",
            "creative_kind": "qr_code",
            "label": "Nova QR CTA",
            "scheduled_at_seconds": 42.0,
            "required_duration_seconds": 20.0,
            "scene_id": "scene_sponsor_read",
            "required_claims": ["Scan the QR code"],
            "prohibited_claims": ["guaranteed results"],
            "settings_json": {
                "target_url": "https://streamvanta.tv/r/nova",
                "tracking": "streamvanta.tv/r/nova"
            }
        })),
    )
    .await;
    let item = inventory["sponsor"]["inventory_json"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["label"] == "Nova QR CTA")
        .unwrap();
    let inventory_id = item["id"].as_str().unwrap().to_string();
    assert_eq!(item["source_kind"], "qr_code");
    assert_eq!(item["renderer_json"]["renderer"], "qr_code");
    assert_eq!(item["renderer_json"]["clock_bound"], true);
    assert_eq!(inventory["sponsor"]["missed_count"], 1);
    assert!(
        inventory["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["id"] == item["source_id"]
                && source["source_contract_json"]["renderer"] == "qr_code")
    );
    assert!(
        inventory["cues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|cue| cue["id"] == item["cue_id"] && cue["cue_kind"] == "qr_code")
    );

    let proof_replay = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/replay-buffer/save"),
        Some(json!({
            "duration_seconds": 5,
            "label": "Sponsor proof replay",
            "sponsor_proof": true
        })),
    )
    .await;
    assert!(
        std::path::Path::new(
            proof_replay["clip_draft_json"]["output_path"]
                .as_str()
                .unwrap()
        )
        .is_file()
    );

    let proofed = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/sponsor/inventory/{inventory_id}/proof"),
        Some(json!({
            "proof_kind": "media_segment",
            "media_time_seconds": 1.0
        })),
    )
    .await;
    assert_eq!(proofed["sponsor"]["proof_count"], 1);
    assert_eq!(
        proofed["sponsor"]["inventory_json"][0]["status"],
        "proof_captured"
    );
    assert_eq!(
        proofed["sponsor"]["proofs_json"][0]["artifact_json"]["source_kind"],
        "captured_program_media"
    );
    assert_eq!(
        proofed["sponsor"]["proofs_json"][0]["artifact_json"]["validation"]["playable"],
        true
    );
    assert_eq!(
        proofed["sponsor"]["proofs_json"][0]["artifact_json"]["validation"]["has_video"],
        true
    );
    assert_eq!(
        proofed["sponsor"]["proofs_json"][0]["artifact_json"]["clip_sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert_eq!(
        proofed["sponsor"]["proofs_json"][0]["artifact_json"]["thumbnail_sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    for key in ["clip_path", "thumbnail_path", "manifest_path"] {
        assert!(
            std::path::Path::new(
                proofed["sponsor"]["proofs_json"][0]["artifact_json"][key]
                    .as_str()
                    .unwrap()
            )
            .is_file(),
            "expected sponsor proof {key} to exist"
        );
    }
    assert!(
        std::path::Path::new(
            proofed["sponsor"]["proofs_json"][0]["artifact_json"]["source_media_path"]
                .as_str()
                .unwrap()
        )
        .is_file()
    );
    assert_eq!(
        proofed["sponsor"]["proofs_json"][0]["vanta_asset_json"]["asset_kind"],
        "sponsor_proof"
    );
    assert_eq!(
        proofed["sponsor"]["proofs_json"][0]["vanta_asset_json"]["status"],
        "ready"
    );
    assert!(
        std::path::Path::new(
            proofed["sponsor"]["proofs_json"][0]["vanta_asset_json"]["manifest_path"]
                .as_str()
                .unwrap()
        )
        .is_file()
    );
    let proof_id = proofed["sponsor"]["proofs_json"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let reviewed = call_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/sponsor/proofs/{proof_id}/review"),
        Some(json!({
            "status": "approved",
            "reviewer_id": "ad_ops_1",
            "notes": "Proof matches contracted claims."
        })),
    )
    .await;
    assert_eq!(reviewed["sponsor"]["approved_proof_count"], 1);
    assert_eq!(
        reviewed["sponsor"]["inventory_json"][0]["review_status"],
        "approved"
    );
    assert!(reviewed["events"].as_array().unwrap().iter().any(|event| {
        event["event_kind"] == "sponsor_review"
            || event["event_kind"] == "sponsor_proof"
            || event["event_kind"] == "sponsor_inventory"
    }));

    let (bad_creative_status, _) = call_status_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/obs/me/broadcasts/{broadcast_id}/sponsor/inventory"),
        Some(json!({
            "campaign_id": "campaign_test_nova",
            "creative_kind": "novelty_effect",
            "label": "Nope",
            "scheduled_at_seconds": 10.0,
            "required_duration_seconds": 20.0
        })),
    )
    .await;
    assert_eq!(bad_creative_status, StatusCode::BAD_REQUEST);

    let (bad_review_status, _) = call_status_json(
        app,
        Method::POST,
        &format!("/api/v1/obs/me/sponsor/proofs/{proof_id}/review"),
        Some(json!({
            "status": "maybe",
            "reviewer_id": "ad_ops_1"
        })),
    )
    .await;
    assert_eq!(bad_review_status, StatusCode::BAD_REQUEST);
}

async fn create_media_source_fixture(label: &str) -> String {
    let dir = std::env::temp_dir()
        .join("vanta-obs-media")
        .join("test-fixtures")
        .join(format!(
            "{}-{}",
            label,
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let path = dir.join("source.mp4");
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("testsrc2=size=640x360:rate=30:duration=2")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("sine=frequency=440:sample_rate=48000:duration=2")
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("1:a:0")
        .arg("-c:v")
        .arg("libx264")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("aac")
        .arg("-shortest")
        .arg(&path)
        .output()
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "fixture generation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    path.to_string_lossy().to_string()
}

async fn test_app() -> Router {
    let (app, _) = test_app_with_pool().await;
    app
}

async fn test_app_with_pool() -> (Router, sqlx::SqlitePool) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().in_memory(true))
        .await
        .unwrap();
    let store = ObsStore::connect(pool.clone()).await.unwrap();
    let native = NativeStore::connect(pool.clone()).await.unwrap();
    let media = MediaStore::connect(pool.clone()).await.unwrap();
    store.seed().await.unwrap();
    (build_app(app_state_from_stores(store, native, media)), pool)
}

async fn test_app_with_bridge(bridge: Arc<dyn ObsBridgeClient>) -> Router {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().in_memory(true))
        .await
        .unwrap();
    let store = ObsStore::connect(pool.clone()).await.unwrap();
    let native = NativeStore::connect(pool.clone()).await.unwrap();
    let media = MediaStore::connect(pool).await.unwrap();
    store.seed().await.unwrap();
    let native = Arc::new(NativeService::new(native));
    build_app(AppState {
        obs: Arc::new(ObsService::with_bridge_client(store, bridge)),
        media: Arc::new(MediaService::new(media, native.clone())),
        native,
    })
}

async fn assert_migrated_table_family(pool: &sqlx::SqlitePool, tables: &[&str]) {
    for table in tables {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
        )
        .bind(table)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|error| panic!("failed to inspect sqlite_master for {table}: {error}"));
        assert_eq!(exists, 1, "missing migrated table {table}");

        let row_count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(pool)
            .await
            .unwrap_or_else(|error| panic!("failed to query migrated table {table}: {error}"));
        assert!(row_count >= 0, "table {table} should be queryable");
    }
}

async fn assert_seed_counts(
    pool: &sqlx::SqlitePool,
    collections: i64,
    scenes: i64,
    templates: i64,
    guest_rooms: i64,
) {
    let actual_collections: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM obs_scene_collections")
        .fetch_one(pool)
        .await
        .unwrap();
    let actual_scenes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM obs_scenes")
        .fetch_one(pool)
        .await
        .unwrap();
    let actual_templates: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM obs_scene_templates")
        .fetch_one(pool)
        .await
        .unwrap();
    let actual_guest_rooms: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM obs_guest_rooms")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(
        actual_collections, collections,
        "scene collection seed count drifted"
    );
    assert_eq!(actual_scenes, scenes, "scene seed count drifted");
    assert_eq!(
        actual_templates, templates,
        "scene template seed count drifted"
    );
    assert_eq!(
        actual_guest_rooms, guest_rooms,
        "guest room seed count drifted"
    );
}

async fn call_json(app: Router, method: Method, uri: &str, body: Option<Value>) -> Value {
    let request_body = body
        .map(|value| Body::from(value.to_string()))
        .unwrap_or_else(Body::empty);
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(request_body)
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(
        status.is_success(),
        "unexpected status {} for {}: {}",
        status,
        uri,
        String::from_utf8_lossy(&body)
    );
    serde_json::from_slice(&body).unwrap()
}

async fn call_status_json(
    app: Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let request_body = body
        .map(|value| Body::from(value.to_string()))
        .unwrap_or_else(Body::empty);
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(request_body)
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

fn sample_rtp_packet_base64(sequence: u16, timestamp: u32, ssrc: u32, marker: bool) -> String {
    sample_rtp_payload_packet_base64(sequence, timestamp, ssrc, marker, &[0x65, 0x88, 0x84, 0x21])
}

fn sample_rtp_payload_packet_base64(
    sequence: u16,
    timestamp: u32,
    ssrc: u32,
    marker: bool,
    payload: &[u8],
) -> String {
    use base64::{Engine as _, engine::general_purpose};
    let mut packet = Vec::new();
    packet.push(0x80);
    packet.push(if marker { 0x80 | 96 } else { 96 });
    packet.extend_from_slice(&sequence.to_be_bytes());
    packet.extend_from_slice(&timestamp.to_be_bytes());
    packet.extend_from_slice(&ssrc.to_be_bytes());
    packet.extend_from_slice(payload);
    general_purpose::STANDARD.encode(packet)
}

fn sample_h264_fu_a_packet_base64(
    sequence: u16,
    timestamp: u32,
    ssrc: u32,
    start: bool,
    end: bool,
    fragment_payload: &[u8],
) -> String {
    use base64::{Engine as _, engine::general_purpose};
    let mut packet = Vec::new();
    packet.push(0x80);
    packet.push(if end { 0x80 | 96 } else { 96 });
    packet.extend_from_slice(&sequence.to_be_bytes());
    packet.extend_from_slice(&timestamp.to_be_bytes());
    packet.extend_from_slice(&ssrc.to_be_bytes());
    packet.push(0x7c);
    packet.push((if start { 0x80 } else { 0 }) | (if end { 0x40 } else { 0 }) | 0x05);
    packet.extend_from_slice(fragment_payload);
    general_purpose::STANDARD.encode(packet)
}

fn sample_rtp_audio_payload_packet_base64(
    sequence: u16,
    timestamp: u32,
    ssrc: u32,
    marker: bool,
    payload: &[u8],
) -> String {
    sample_rtp_payload_packet_base64(sequence, timestamp, ssrc, marker, payload)
}

async fn generated_png_data_url(width: i64, height: i64) -> String {
    use base64::{Engine as _, engine::general_purpose};
    let dir = std::env::temp_dir().join(format!(
        "vanta-obs-png-fixture-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let output_path = dir.join("program-frame.png");
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg(format!("testsrc2=size={width}x{height}:rate=1"))
        .arg("-frames:v")
        .arg("1")
        .arg(&output_path)
        .output()
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "ffmpeg generated PNG fixture: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let png = tokio::fs::read(output_path).await.unwrap();
    format!(
        "data:image/png;base64,{}",
        general_purpose::STANDARD.encode(png)
    )
}

async fn generated_h264_annex_b_nals() -> Vec<Vec<u8>> {
    let dir = std::env::temp_dir().join(format!(
        "vanta-obs-h264-fixture-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let output_path = dir.join("fixture.h264");
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("color=c=red:s=16x16:d=0.04")
        .arg("-frames:v")
        .arg("1")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("ultrafast")
        .arg("-tune")
        .arg("zerolatency")
        .arg("-x264-params")
        .arg("keyint=1")
        .arg("-f")
        .arg("h264")
        .arg(&output_path)
        .output()
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "ffmpeg H.264 fixture generation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = tokio::fs::read(&output_path).await.unwrap();
    let _ = tokio::fs::remove_dir_all(&dir).await;
    annex_b_nal_payloads(&bytes)
}

fn annex_b_nal_payloads(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut starts = Vec::new();
    let mut index = 0;
    while index + 3 < bytes.len() {
        if bytes[index..].starts_with(&[0, 0, 1]) {
            starts.push((index, 3));
            index += 3;
        } else if index + 4 <= bytes.len() && bytes[index..].starts_with(&[0, 0, 0, 1]) {
            starts.push((index, 4));
            index += 4;
        } else {
            index += 1;
        }
    }
    starts
        .iter()
        .enumerate()
        .filter_map(|(position, (start, prefix_len))| {
            let payload_start = start + prefix_len;
            let payload_end = starts
                .get(position + 1)
                .map(|(next_start, _)| *next_start)
                .unwrap_or(bytes.len());
            if payload_start < payload_end {
                Some(bytes[payload_start..payload_end].to_vec())
            } else {
                None
            }
        })
        .collect()
}

async fn generated_opus_packets() -> Vec<Vec<u8>> {
    let dir = std::env::temp_dir().join(format!(
        "vanta-obs-opus-fixture-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let output_path = dir.join("fixture.ogg");
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("sine=frequency=440:sample_rate=48000:duration=0.08")
        .arg("-ac")
        .arg("1")
        .arg("-c:a")
        .arg("libopus")
        .arg("-application")
        .arg("lowdelay")
        .arg("-frame_duration")
        .arg("20")
        .arg(&output_path)
        .output()
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "ffmpeg Opus fixture generation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = tokio::fs::read(&output_path).await.unwrap();
    let _ = tokio::fs::remove_dir_all(&dir).await;
    ogg_audio_packets(&bytes)
}

fn ogg_audio_packets(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut packets = Vec::new();
    let mut offset = 0;
    while offset + 27 <= bytes.len() {
        if &bytes[offset..offset + 4] != b"OggS" {
            break;
        }
        let segment_count = bytes[offset + 26] as usize;
        let segment_table_start = offset + 27;
        let body_start = segment_table_start + segment_count;
        if body_start > bytes.len() {
            break;
        }
        let body_len = bytes[segment_table_start..body_start]
            .iter()
            .map(|value| *value as usize)
            .sum::<usize>();
        let body_end = body_start + body_len;
        if body_end > bytes.len() {
            break;
        }
        let mut cursor = body_start;
        let mut packet = Vec::new();
        for lace in &bytes[segment_table_start..body_start] {
            let next = cursor + *lace as usize;
            packet.extend_from_slice(&bytes[cursor..next]);
            cursor = next;
            if *lace < 255 {
                if !packet.starts_with(b"OpusHead") && !packet.starts_with(b"OpusTags") {
                    packets.push(std::mem::take(&mut packet));
                } else {
                    packet.clear();
                }
            }
        }
        offset = body_end;
    }
    packets
}

fn obs_rpc_response(request_id: &str, request_type: &str) -> Value {
    let response_data = match request_type {
        "GetVersion" => json!({
            "obsVersion": "32.0.0",
            "obsWebSocketVersion": "5.6.0"
        }),
        "GetSceneList" => json!({
            "currentProgramSceneName": "Program Scene",
            "currentPreviewSceneName": "Preview Scene",
            "scenes": [
                {"sceneName": "Program Scene"},
                {"sceneName": "Preview Scene"}
            ]
        }),
        "GetInputList" => json!({
            "inputs": [
                {"inputName": "Sony FX3", "inputKind": "av_capture_input"},
                {"inputName": "Host Mic", "inputKind": "coreaudio_input_capture"},
                {"inputName": "Display", "inputKind": "display_capture"},
                {"inputName": "Novelty Shader", "inputKind": "obs_shader_filter_source"}
            ]
        }),
        "GetSceneTransitionList" => json!({
            "transitions": [
                {"transitionName": "Fade", "transitionKind": "fade_transition"}
            ]
        }),
        "GetStreamStatus" => json!({
            "outputActive": true,
            "outputReconnecting": true
        }),
        "GetRecordStatus" => json!({
            "outputActive": true
        }),
        "GetReplayBufferStatus" => json!({
            "outputActive": true
        }),
        "GetSceneItemList" => json!({
            "sceneItems": [
                {
                    "sceneItemId": 7,
                    "sourceName": "Sony FX3",
                    "sourceType": "av_capture_input",
                    "sceneItemEnabled": true,
                    "sceneItemLocked": false,
                    "sceneItemIndex": 0,
                    "sceneItemTransform": {
                        "positionX": 32.0,
                        "positionY": 48.0,
                        "width": 1280.0,
                        "height": 720.0,
                        "rotation": 8.0
                    }
                }
            ]
        }),
        _ => json!({}),
    };
    json!({
        "op": 7,
        "d": {
            "requestId": request_id,
            "requestStatus": {
                "result": true,
                "code": 100
            },
            "responseData": response_data
        }
    })
}

#[derive(Default)]
struct MockBridgeClient {
    commands: Mutex<Vec<String>>,
}

#[async_trait]
impl ObsBridgeClient for MockBridgeClient {
    async fn snapshot(
        &self,
        profile: &ObsBridgeProfile,
    ) -> Result<ObsBridgeSnapshot, ObsBridgeError> {
        assert_eq!(profile.label, "Local OBS");
        assert_eq!(profile.id.is_empty(), false);
        assert_eq!(profile.password.as_deref(), Some("secret"));
        assert_eq!(profile.auto_sync, true);
        Ok(ObsBridgeSnapshot {
            obs_version: "32.0.0".to_string(),
            websocket_version: "5.6.0".to_string(),
            current_program_scene: Some("Host Camera".to_string()),
            current_preview_scene: Some("Sponsor Read".to_string()),
            stream_state: "idle".to_string(),
            recording_state: "idle".to_string(),
            replay_buffer_state: "active".to_string(),
            scenes: vec![ObsBridgeScene {
                name: "Host Camera".to_string(),
                index: 0,
                items: vec![ObsBridgeSceneItem {
                    id: 1,
                    source_name: "Sony FX3".to_string(),
                    source_kind: "av_capture_input".to_string(),
                    enabled: true,
                    locked: false,
                    index: 0,
                    transform: json!({"positionX":0,"positionY":0,"width":1920,"height":1080}),
                }],
            }],
            sources: vec![
                ObsBridgeSource {
                    name: "Sony FX3".to_string(),
                    kind: "av_capture_input".to_string(),
                    vanta_kind: Some("camera".to_string()),
                    configurable: true,
                },
                ObsBridgeSource {
                    name: "Novelty Shader".to_string(),
                    kind: "obs_shader_filter_source".to_string(),
                    vanta_kind: None,
                    configurable: false,
                },
            ],
            transitions: vec![ObsBridgeTransition {
                name: "Fade".to_string(),
                kind: "fade_transition".to_string(),
            }],
            audio_inputs: vec![ObsBridgeAudioInput {
                name: "Host Mic".to_string(),
                kind: "coreaudio_input_capture".to_string(),
                muted: false,
                volume_db: -3.0,
            }],
            unsupported: vec![bridge_warning(
                "unsupported_source_kind",
                "Novelty Shader",
                "obs_shader_filter_source is not mapped into Vanta OBS",
            )],
        })
    }

    async fn execute(
        &self,
        _profile: &ObsBridgeProfile,
        command: ObsBridgeCommand,
    ) -> Result<ObsBridgeCommandResult, ObsBridgeError> {
        let label = match command {
            ObsBridgeCommand::SetProgramScene { scene_name } => {
                format!("set_program_scene:{scene_name}")
            }
            ObsBridgeCommand::StartStream => "start_stream".to_string(),
            ObsBridgeCommand::StopStream => "stop_stream".to_string(),
            ObsBridgeCommand::StartRecording => "start_recording".to_string(),
            ObsBridgeCommand::StopRecording => "stop_recording".to_string(),
            ObsBridgeCommand::SaveReplayBuffer => "save_replay_buffer".to_string(),
        };
        self.commands.lock().unwrap().push(label.clone());
        Ok(ObsBridgeCommandResult {
            command: label.split(':').next().unwrap().to_string(),
            accepted: true,
            detail: "mock accepted".to_string(),
        })
    }
}
