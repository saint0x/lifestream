use serde_json::{Value, json};

#[derive(Debug, Clone, Copy)]
pub struct TransitionSpec {
    pub kind: &'static str,
    pub renderer: &'static str,
    pub default_duration_ms: i64,
    pub interruption_policy: &'static str,
}

pub const TRANSITIONS: &[TransitionSpec] = &[
    spec("cut", "instant_swap", 0, "replace_running"),
    spec("fade", "crossfade", 300, "replace_running"),
    spec("dip_to_black", "dip_color", 500, "replace_running"),
    spec("swipe", "directional_wipe", 420, "replace_running"),
    spec("stinger", "stinger_overlay", 900, "replace_running"),
];

const fn spec(
    kind: &'static str,
    renderer: &'static str,
    default_duration_ms: i64,
    interruption_policy: &'static str,
) -> TransitionSpec {
    TransitionSpec {
        kind,
        renderer,
        default_duration_ms,
        interruption_policy,
    }
}

pub fn transition_kinds() -> Vec<&'static str> {
    TRANSITIONS.iter().map(|spec| spec.kind).collect()
}

pub fn transition_plan(
    kind: &str,
    duration_ms: i64,
    from_scene_id: Option<&str>,
    to_scene_id: &str,
    preview: bool,
) -> Value {
    let spec = TRANSITIONS
        .iter()
        .copied()
        .find(|spec| spec.kind == kind)
        .unwrap_or(TRANSITIONS[1]);
    let duration_ms = if spec.kind == "cut" {
        0
    } else if duration_ms <= 0 {
        spec.default_duration_ms
    } else {
        duration_ms
    };
    json!({
        "kind": spec.kind,
        "renderer": spec.renderer,
        "duration_ms": duration_ms,
        "from_scene_id": from_scene_id,
        "to_scene_id": to_scene_id,
        "preview": preview,
        "applied_by_renderer": !preview,
        "interruption_policy": spec.interruption_policy,
        "phases": phases(spec.kind, duration_ms),
        "requires_media_asset": spec.kind == "stinger",
        "accessibility": {
            "reduced_motion_fallback": if spec.kind == "cut" { "cut" } else { "fade" },
            "flash_risk": false
        }
    })
}

fn phases(kind: &str, duration_ms: i64) -> Vec<Value> {
    match kind {
        "cut" => vec![json!({"at":0.0,"action":"swap_program","duration_ms":0})],
        "dip_to_black" => vec![
            json!({"at":0.0,"action":"fade_out_to_black","duration_ms":duration_ms / 2}),
            json!({"at":0.5,"action":"swap_program_under_black","duration_ms":0}),
            json!({"at":0.5,"action":"fade_in_from_black","duration_ms":duration_ms / 2}),
        ],
        "swipe" => vec![
            json!({"at":0.0,"action":"wipe_from_preview","direction":"left_to_right","duration_ms":duration_ms}),
            json!({"at":1.0,"action":"commit_program","duration_ms":0}),
        ],
        "stinger" => vec![
            json!({"at":0.0,"action":"play_stinger_overlay","duration_ms":duration_ms}),
            json!({"at":0.45,"action":"swap_program_at_cut_point","duration_ms":0}),
            json!({"at":1.0,"action":"clear_stinger_overlay","duration_ms":0}),
        ],
        _ => vec![
            json!({"at":0.0,"action":"crossfade_outgoing","duration_ms":duration_ms}),
            json!({"at":0.0,"action":"crossfade_incoming","duration_ms":duration_ms}),
            json!({"at":1.0,"action":"commit_program","duration_ms":0}),
        ],
    }
}
