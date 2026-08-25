use serde_json::Value;

use crate::obs::domain::{
    GuestDeviceCheckInput, GuestInviteInput, GuestIsolatedRecordingInput, GuestMediaTelemetryInput,
    GuestModerationInput, GuestPatchInput, GuestReturnFeedInput, GuestRoomRoutingInput,
    GuestRtpPacketInput, GuestWebrtcAnswerInput, GuestWebrtcIceInput, GuestWebrtcOfferInput,
};

use super::{ObsService, ObsServiceError, ObsServiceResult, require_one_of, require_text};

impl ObsService {
    pub async fn invite_guest(
        &self,
        broadcast_id: &str,
        input: GuestInviteInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        require_text(&input.display_name, "display_name")?;
        Ok(self.store.invite_guest(broadcast_id, input).await?)
    }

    pub async fn promote_guest(
        &self,
        participant_id: &str,
        scene_id: &str,
    ) -> ObsServiceResult<Value> {
        require_text(participant_id, "participant_id")?;
        require_text(scene_id, "scene_id")?;
        Ok(self.store.promote_guest(participant_id, scene_id).await?)
    }

    pub async fn patch_guest(
        &self,
        participant_id: &str,
        input: GuestPatchInput,
    ) -> ObsServiceResult<Value> {
        require_text(participant_id, "participant_id")?;
        Ok(self.store.patch_guest(participant_id, input).await?)
    }

    pub async fn run_guest_device_check(
        &self,
        participant_id: &str,
        input: GuestDeviceCheckInput,
    ) -> ObsServiceResult<Value> {
        require_text(participant_id, "participant_id")?;
        validate_guest_device_check(&input)?;
        Ok(self
            .store
            .run_guest_device_check(participant_id, input)
            .await?)
    }

    pub async fn moderate_guest(
        &self,
        participant_id: &str,
        input: GuestModerationInput,
    ) -> ObsServiceResult<Value> {
        require_text(participant_id, "participant_id")?;
        require_text(&input.moderator_id, "moderator_id")?;
        require_text(&input.reason, "reason")?;
        require_one_of(
            &input.action,
            "action",
            &["hold_backstage", "release_backstage", "approve_live"],
        )?;
        if input.action == "approve_live" {
            let scene_id = input.target_scene_id.as_deref().unwrap_or_default();
            require_text(scene_id, "target_scene_id")?;
        }
        Ok(self.store.moderate_guest(participant_id, input).await?)
    }

    pub async fn report_guest_media_telemetry(
        &self,
        participant_id: &str,
        input: GuestMediaTelemetryInput,
    ) -> ObsServiceResult<Value> {
        require_text(participant_id, "participant_id")?;
        validate_guest_media_telemetry(&input)?;
        Ok(self
            .store
            .report_guest_media_telemetry(participant_id, input)
            .await?)
    }

    pub async fn configure_guest_room_routing(
        &self,
        broadcast_id: &str,
        input: GuestRoomRoutingInput,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        validate_guest_room_routing(&input)?;
        Ok(self
            .store
            .configure_guest_room_routing(broadcast_id, input)
            .await?)
    }

    pub async fn create_guest_webrtc_offer(
        &self,
        participant_id: &str,
        input: GuestWebrtcOfferInput,
    ) -> ObsServiceResult<Value> {
        require_text(participant_id, "participant_id")?;
        validate_guest_webrtc_offer(&input)?;
        Ok(self
            .store
            .create_guest_webrtc_offer(participant_id, input)
            .await?)
    }

    pub async fn apply_guest_webrtc_answer(
        &self,
        session_id: &str,
        input: GuestWebrtcAnswerInput,
    ) -> ObsServiceResult<Value> {
        require_text(session_id, "session_id")?;
        validate_sdp(&input.answer_sdp, "answer_sdp")?;
        validate_optional_layer(input.selected_video_layer.as_deref())?;
        Ok(self
            .store
            .apply_guest_webrtc_answer(session_id, input)
            .await?)
    }

    pub async fn add_guest_webrtc_ice_candidate(
        &self,
        session_id: &str,
        input: GuestWebrtcIceInput,
    ) -> ObsServiceResult<Value> {
        require_text(session_id, "session_id")?;
        validate_guest_webrtc_ice(&input)?;
        Ok(self
            .store
            .add_guest_webrtc_ice_candidate(session_id, input)
            .await?)
    }

    pub async fn reconcile_guest_media_relays(
        &self,
        broadcast_id: &str,
    ) -> ObsServiceResult<Value> {
        require_text(broadcast_id, "broadcast_id")?;
        Ok(self
            .store
            .reconcile_guest_media_relays(broadcast_id)
            .await?)
    }

    pub async fn ingest_guest_relay_rtp_packet(
        &self,
        relay_id: &str,
        input: GuestRtpPacketInput,
    ) -> ObsServiceResult<Value> {
        require_text(relay_id, "relay_id")?;
        validate_guest_rtp_packet(&input)?;
        Ok(self
            .store
            .ingest_guest_relay_rtp_packet(relay_id, input)
            .await?)
    }

    pub async fn negotiate_guest_return_feed(
        &self,
        participant_id: &str,
        input: GuestReturnFeedInput,
    ) -> ObsServiceResult<Value> {
        require_text(participant_id, "participant_id")?;
        validate_guest_return_feed(&input)?;
        Ok(self
            .store
            .negotiate_guest_return_feed(participant_id, input)
            .await?)
    }

    pub async fn start_guest_isolated_recording(
        &self,
        participant_id: &str,
        input: GuestIsolatedRecordingInput,
    ) -> ObsServiceResult<Value> {
        require_text(participant_id, "participant_id")?;
        validate_guest_isolated_recording(&input)?;
        Ok(self
            .store
            .start_guest_isolated_recording(participant_id, input)
            .await?)
    }

    pub async fn stop_guest_isolated_recording(
        &self,
        participant_id: &str,
    ) -> ObsServiceResult<Value> {
        require_text(participant_id, "participant_id")?;
        Ok(self
            .store
            .stop_guest_isolated_recording(participant_id)
            .await?)
    }

    pub async fn remove_guest(&self, participant_id: &str) -> ObsServiceResult<Value> {
        require_text(participant_id, "participant_id")?;
        Ok(self.store.remove_guest(participant_id).await?)
    }
}

fn validate_guest_device_check(input: &GuestDeviceCheckInput) -> ObsServiceResult<()> {
    for (field, value) in [
        ("camera_status", input.camera_status.as_str()),
        ("microphone_status", input.microphone_status.as_str()),
        ("browser_status", input.browser_status.as_str()),
    ] {
        require_one_of(
            value,
            field,
            &["ready", "missing", "denied", "unsupported", "warning"],
        )?;
    }
    require_one_of(
        &input.network_status,
        "network_status",
        &["ready", "warning", "blocked"],
    )?;
    if input.bitrate_kbps < 0 {
        return Err(ObsServiceError::Invalid {
            field: "bitrate_kbps",
            message: "must not be negative",
        });
    }
    if input.round_trip_ms < 0 {
        return Err(ObsServiceError::Invalid {
            field: "round_trip_ms",
            message: "must not be negative",
        });
    }
    if !(0.0..=100.0).contains(&input.packet_loss_percent) {
        return Err(ObsServiceError::Invalid {
            field: "packet_loss_percent",
            message: "must be between 0 and 100",
        });
    }
    Ok(())
}

fn validate_guest_isolated_recording(input: &GuestIsolatedRecordingInput) -> ObsServiceResult<()> {
    if let Some(mode) = input.recording_mode.as_deref() {
        require_one_of(
            mode,
            "recording_mode",
            &["audio_video", "isolated_audio", "isolated_video"],
        )?;
    }
    if input.include_audio == Some(false) && input.include_video == Some(false) {
        return Err(ObsServiceError::Invalid {
            field: "tracks",
            message: "must include audio or video",
        });
    }
    Ok(())
}

fn validate_guest_return_feed(input: &GuestReturnFeedInput) -> ObsServiceResult<()> {
    require_one_of(
        &input.audio_mode,
        "audio_mode",
        &["mix_minus", "program_audio", "muted"],
    )?;
    require_one_of(
        &input.video_mode,
        "video_mode",
        &["program_return", "shared_game", "camera_preview"],
    )?;
    require_one_of(
        input.transport.as_deref().unwrap_or("vanta_realtime_sfu"),
        "transport",
        &["vanta_realtime_sfu", "webrtc_sfu"],
    )?;
    if input.video_mode == "shared_game" {
        require_text(
            input.shared_feed_source_id.as_deref().unwrap_or_default(),
            "shared_feed_source_id",
        )?;
    }
    if input.target_latency_ms.unwrap_or(140) <= 0 {
        return Err(ObsServiceError::Invalid {
            field: "target_latency_ms",
            message: "must be greater than zero",
        });
    }
    if input.audio_bitrate_kbps.unwrap_or(96) <= 0 {
        return Err(ObsServiceError::Invalid {
            field: "audio_bitrate_kbps",
            message: "must be greater than zero",
        });
    }
    if input.video_bitrate_kbps.unwrap_or(1800) <= 0 {
        return Err(ObsServiceError::Invalid {
            field: "video_bitrate_kbps",
            message: "must be greater than zero",
        });
    }
    Ok(())
}

fn validate_guest_room_routing(input: &GuestRoomRoutingInput) -> ObsServiceResult<()> {
    require_one_of(
        &input.room_mode,
        "room_mode",
        &["solo", "dual", "group", "shared_game"],
    )?;
    if let Some(max_participants) = input.max_participants {
        let allowed = match input.room_mode.as_str() {
            "solo" => max_participants == 1,
            "dual" => max_participants == 2,
            "group" | "shared_game" => [4, 6, 8].contains(&max_participants),
            _ => false,
        };
        if !allowed {
            return Err(ObsServiceError::Invalid {
                field: "max_participants",
                message: "does not match the selected collaboration mode",
            });
        }
    }
    if input.room_mode == "shared_game" {
        require_text(
            input.shared_feed_source_id.as_deref().unwrap_or_default(),
            "shared_feed_source_id",
        )?;
    } else if input
        .shared_feed_source_id
        .as_deref()
        .unwrap_or_default()
        .trim()
        .len()
        > 0
    {
        return Err(ObsServiceError::Invalid {
            field: "shared_feed_source_id",
            message: "is only valid for shared_game mode",
        });
    }
    if input.latency_target_ms.unwrap_or(140) <= 0 {
        return Err(ObsServiceError::Invalid {
            field: "latency_target_ms",
            message: "must be greater than zero",
        });
    }
    Ok(())
}

fn validate_guest_webrtc_offer(input: &GuestWebrtcOfferInput) -> ObsServiceResult<()> {
    require_one_of(
        &input.session_role,
        "session_role",
        &["guest_publish", "guest_return", "shared_feed_return"],
    )?;
    require_one_of(
        &input.direction,
        "direction",
        &["sendrecv", "sendonly", "recvonly"],
    )?;
    if !input.audio && !input.video {
        return Err(ObsServiceError::Invalid {
            field: "media",
            message: "must include audio or video",
        });
    }
    validate_sdp(&input.offer_sdp, "offer_sdp")?;
    validate_optional_layer(input.preferred_video_layer.as_deref())?;
    Ok(())
}

fn validate_guest_webrtc_ice(input: &GuestWebrtcIceInput) -> ObsServiceResult<()> {
    require_text(&input.candidate, "candidate")?;
    if !input.candidate.starts_with("candidate:") {
        return Err(ObsServiceError::Invalid {
            field: "candidate",
            message: "must be a WebRTC ICE candidate",
        });
    }
    if input.sdp_mline_index.unwrap_or_default() < 0 {
        return Err(ObsServiceError::Invalid {
            field: "sdp_mline_index",
            message: "must not be negative",
        });
    }
    Ok(())
}

fn validate_guest_rtp_packet(input: &GuestRtpPacketInput) -> ObsServiceResult<()> {
    require_one_of(&input.payload_kind, "payload_kind", &["audio", "video"])?;
    require_text(&input.packet_base64, "packet_base64")?;
    if input.packet_base64.len() > 262_144 {
        return Err(ObsServiceError::Invalid {
            field: "packet_base64",
            message: "must be a bounded RTP packet payload",
        });
    }
    if input.received_at_ms.unwrap_or_default() < 0 {
        return Err(ObsServiceError::Invalid {
            field: "received_at_ms",
            message: "must not be negative",
        });
    }
    Ok(())
}

fn validate_sdp(value: &str, field: &'static str) -> ObsServiceResult<()> {
    require_text(value, field)?;
    if value.len() > 128_000 || !value.contains("v=0") || !value.contains("m=") {
        return Err(ObsServiceError::Invalid {
            field,
            message: "must be a bounded SDP payload",
        });
    }
    Ok(())
}

fn validate_optional_layer(value: Option<&str>) -> ObsServiceResult<()> {
    let Some(layer) = value.filter(|layer| !layer.trim().is_empty()) else {
        return Ok(());
    };
    require_one_of(
        layer,
        "video_layer",
        &["1080p60", "720p30", "480p30", "360p30", "180p15"],
    )
}

fn validate_guest_media_telemetry(input: &GuestMediaTelemetryInput) -> ObsServiceResult<()> {
    if !(-120.0..=6.0).contains(&input.audio_level_db) {
        return Err(ObsServiceError::Invalid {
            field: "audio_level_db",
            message: "must be between -120 dB and 6 dB",
        });
    }
    if input.round_trip_ms < 0 {
        return Err(ObsServiceError::Invalid {
            field: "round_trip_ms",
            message: "must not be negative",
        });
    }
    if !(0.0..=100.0).contains(&input.packet_loss_percent) {
        return Err(ObsServiceError::Invalid {
            field: "packet_loss_percent",
            message: "must be between 0 and 100",
        });
    }
    if input.jitter_ms.unwrap_or_default() < 0 {
        return Err(ObsServiceError::Invalid {
            field: "jitter_ms",
            message: "must not be negative",
        });
    }
    if input.dropped_frames.unwrap_or_default() < 0 {
        return Err(ObsServiceError::Invalid {
            field: "dropped_frames",
            message: "must not be negative",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guest_device_check_validation_rejects_bad_status_and_metrics() {
        assert!(
            validate_guest_device_check(&GuestDeviceCheckInput {
                camera_status: "ready".to_string(),
                microphone_status: "ready".to_string(),
                network_status: "ready".to_string(),
                browser_status: "ready".to_string(),
                bitrate_kbps: 2400,
                round_trip_ms: 90,
                packet_loss_percent: 0.5,
                checks_json: None,
            })
            .is_ok()
        );

        assert_invalid_field(
            validate_guest_device_check(&GuestDeviceCheckInput {
                camera_status: "prompting".to_string(),
                microphone_status: "ready".to_string(),
                network_status: "ready".to_string(),
                browser_status: "ready".to_string(),
                bitrate_kbps: 2400,
                round_trip_ms: 90,
                packet_loss_percent: 0.5,
                checks_json: None,
            }),
            "camera_status",
        );
        assert_invalid_field(
            validate_guest_device_check(&GuestDeviceCheckInput {
                camera_status: "ready".to_string(),
                microphone_status: "ready".to_string(),
                network_status: "ready".to_string(),
                browser_status: "ready".to_string(),
                bitrate_kbps: 2400,
                round_trip_ms: 90,
                packet_loss_percent: 101.0,
                checks_json: None,
            }),
            "packet_loss_percent",
        );
    }

    #[test]
    fn guest_routing_and_return_feed_validation_enforce_shared_game_contracts() {
        assert_invalid_field(
            validate_guest_room_routing(&GuestRoomRoutingInput {
                room_mode: "dual".to_string(),
                max_participants: Some(4),
                shared_feed_source_id: None,
                mirrored_channels: None,
                latency_target_ms: Some(140),
            }),
            "max_participants",
        );
        assert_invalid_field(
            validate_guest_room_routing(&GuestRoomRoutingInput {
                room_mode: "shared_game".to_string(),
                max_participants: Some(4),
                shared_feed_source_id: None,
                mirrored_channels: None,
                latency_target_ms: Some(140),
            }),
            "shared_feed_source_id",
        );
        assert!(
            validate_guest_room_routing(&GuestRoomRoutingInput {
                room_mode: "shared_game".to_string(),
                max_participants: Some(4),
                shared_feed_source_id: Some("source_screen".to_string()),
                mirrored_channels: Some(false),
                latency_target_ms: Some(120),
            })
            .is_ok()
        );

        assert_invalid_field(
            validate_guest_return_feed(&GuestReturnFeedInput {
                audio_mode: "mix_minus".to_string(),
                video_mode: "shared_game".to_string(),
                transport: Some("vanta_realtime_sfu".to_string()),
                shared_feed_source_id: None,
                target_latency_ms: Some(120),
                audio_bitrate_kbps: Some(96),
                video_bitrate_kbps: Some(1800),
            }),
            "shared_feed_source_id",
        );
        assert_invalid_field(
            validate_guest_return_feed(&GuestReturnFeedInput {
                audio_mode: "mix_minus".to_string(),
                video_mode: "program_return".to_string(),
                transport: Some("rtmp".to_string()),
                shared_feed_source_id: None,
                target_latency_ms: Some(120),
                audio_bitrate_kbps: Some(96),
                video_bitrate_kbps: Some(1800),
            }),
            "transport",
        );
    }

    #[test]
    fn isolated_recording_and_media_telemetry_validation_reject_impossible_payloads() {
        assert_invalid_field(
            validate_guest_isolated_recording(&GuestIsolatedRecordingInput {
                recording_mode: Some("multitrack".to_string()),
                include_video: Some(true),
                include_audio: Some(true),
            }),
            "recording_mode",
        );
        assert_invalid_field(
            validate_guest_isolated_recording(&GuestIsolatedRecordingInput {
                recording_mode: Some("audio_video".to_string()),
                include_video: Some(false),
                include_audio: Some(false),
            }),
            "tracks",
        );
        assert_invalid_field(
            validate_guest_media_telemetry(&GuestMediaTelemetryInput {
                audio_level_db: -12.0,
                speaking: true,
                video_active: true,
                round_trip_ms: 40,
                packet_loss_percent: 0.5,
                jitter_ms: Some(-1),
                dropped_frames: Some(0),
                media_json: None,
            }),
            "jitter_ms",
        );
    }

    fn assert_invalid_field(result: ObsServiceResult<()>, expected: &'static str) {
        match result {
            Err(ObsServiceError::Invalid { field, .. }) => assert_eq!(field, expected),
            other => panic!("expected invalid field {expected}, got {other:?}"),
        }
    }
}
