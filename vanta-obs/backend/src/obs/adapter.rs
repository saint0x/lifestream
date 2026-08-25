#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObsExportTarget {
    Source(ObsExportSource),
    Omit(ObsAdapterNotice),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObsExportSource {
    pub obs_kind: &'static str,
    pub asset_folder: Option<&'static str>,
    pub notice: Option<ObsAdapterNotice>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObsAdapterNotice {
    pub code: &'static str,
    pub detail: &'static str,
}

pub fn obs_kind_to_vanta_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "av_capture_input" | "dshow_input" | "v4l2_input" => Some("camera"),
        "wasapi_input_capture"
        | "coreaudio_input_capture"
        | "pulse_input_capture"
        | "alsa_input_capture" => Some("microphone"),
        "wasapi_output_capture" | "coreaudio_output_capture" | "pulse_output_capture" => {
            Some("desktop_audio")
        }
        "monitor_capture" | "display_capture" | "pipewire-desktop-capture-source" => {
            Some("display_capture")
        }
        "window_capture" | "xcomposite_input" => Some("window_capture"),
        "browser_source" => Some("browser_capture"),
        "ffmpeg_source" | "vlc_source" => Some("media_file"),
        "image_source" | "slideshow" => Some("image"),
        "text_gdiplus" | "text_ft2_source" => Some("text"),
        "color_source" => Some("color_matte"),
        "scene" | "group" => Some("scene_group"),
        _ => None,
    }
}

pub fn vanta_kind_to_obs_export(kind: &str) -> Option<ObsExportTarget> {
    let source = |obs_kind, asset_folder, notice| {
        Some(ObsExportTarget::Source(ObsExportSource {
            obs_kind,
            asset_folder,
            notice,
        }))
    };
    match kind {
        "camera" => source("av_capture_input", None, None),
        "microphone" => source("coreaudio_input_capture", None, None),
        "desktop_audio" => source("coreaudio_output_capture", None, None),
        "system_audio" => source(
            "coreaudio_output_capture",
            None,
            Some(ObsAdapterNotice {
                code: "system_audio_exported_as_obs_output_capture",
                detail: "Vanta native system audio uses ScreenCaptureKit without a loopback device; OBS export maps it to OBS output capture where available.",
            }),
        ),
        "screen_capture" | "display_capture" => source("display_capture", None, None),
        "window_capture" => source("window_capture", None, None),
        "browser_capture" => source("browser_source", None, None),
        "media_file" => source("ffmpeg_source", Some("media"), None),
        "image" => source("image_source", Some("images"), None),
        "text" => source("text_ft2_source", None, None),
        "color_matte" => source("color_source", None, None),
        "scene_group" => source("group", None, None),
        "chat_overlay" | "alert_overlay" | "sponsor_card" | "lower_third" | "countdown_timer" => {
            source(
                "browser_source",
                None,
                Some(ObsAdapterNotice {
                    code: "vanta_overlay_exported_as_browser_source",
                    detail: "Vanta runtime overlay is exported as an OBS browser source and will not carry Vanta runtime state.",
                }),
            )
        }
        "guest_feed" | "remote_contribution" => source(
            "browser_source",
            None,
            Some(ObsAdapterNotice {
                code: "live_participant_exported_as_browser_source",
                detail: "Guest/remote contribution requires Vanta runtime and is exported as a browser fallback.",
            }),
        ),
        "vanta_video_asset" | "vanta_clip" => source(
            "ffmpeg_source",
            Some("vanta-media"),
            Some(ObsAdapterNotice {
                code: "vanta_media_asset_requires_bundle",
                detail: "Vanta media asset is exported as a local OBS media source and requires the asset bundle.",
            }),
        ),
        "safe_area_guide" => Some(ObsExportTarget::Omit(ObsAdapterNotice {
            code: "safe_area_guide_omitted",
            detail: "Safe-area guides are editor/operator overlays and are omitted from OBS export.",
        })),
        _ => None,
    }
}

pub fn is_audio_obs_kind(kind: &str) -> bool {
    matches!(
        kind,
        "wasapi_input_capture"
            | "wasapi_output_capture"
            | "coreaudio_input_capture"
            | "coreaudio_output_capture"
            | "pulse_input_capture"
            | "pulse_output_capture"
            | "alsa_input_capture"
    )
}

#[cfg(test)]
mod tests {
    use super::{ObsExportTarget, obs_kind_to_vanta_kind, vanta_kind_to_obs_export};

    #[test]
    fn maps_only_value_filtered_vanta_sources_to_obs_targets() {
        assert_eq!(obs_kind_to_vanta_kind("av_capture_input"), Some("camera"));
        assert_eq!(
            obs_kind_to_vanta_kind("coreaudio_output_capture"),
            Some("desktop_audio")
        );
        assert_eq!(obs_kind_to_vanta_kind("text_ft2_source"), Some("text"));
        assert_eq!(obs_kind_to_vanta_kind("shader_filter"), None);

        let Some(ObsExportTarget::Source(camera)) = vanta_kind_to_obs_export("camera") else {
            panic!("camera should export to a native OBS capture source");
        };
        assert_eq!(camera.obs_kind, "av_capture_input");
        assert_eq!(camera.asset_folder, None);

        let Some(ObsExportTarget::Source(desktop_audio)) =
            vanta_kind_to_obs_export("desktop_audio")
        else {
            panic!("desktop audio should export to an OBS output capture source");
        };
        assert_eq!(desktop_audio.obs_kind, "coreaudio_output_capture");

        let Some(ObsExportTarget::Source(system_audio)) = vanta_kind_to_obs_export("system_audio")
        else {
            panic!("system audio should export to an OBS output capture source with a notice");
        };
        assert_eq!(system_audio.obs_kind, "coreaudio_output_capture");
        assert_eq!(
            system_audio.notice.map(|notice| notice.code),
            Some("system_audio_exported_as_obs_output_capture")
        );

        let Some(ObsExportTarget::Source(overlay)) = vanta_kind_to_obs_export("sponsor_card")
        else {
            panic!("sponsor cards should export with an OBS browser fallback");
        };
        assert_eq!(overlay.obs_kind, "browser_source");
        assert_eq!(
            overlay.notice.map(|notice| notice.code),
            Some("vanta_overlay_exported_as_browser_source"),
        );

        let Some(ObsExportTarget::Omit(guide)) = vanta_kind_to_obs_export("safe_area_guide") else {
            panic!("operator-only safe area guides should not export as program sources");
        };
        assert_eq!(guide.code, "safe_area_guide_omitted");
        assert!(vanta_kind_to_obs_export("noise_gate_visualizer").is_none());
    }
}
