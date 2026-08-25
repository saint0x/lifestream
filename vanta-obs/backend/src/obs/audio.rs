use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct AudioChannelState {
    pub id: String,
    pub channel_kind: String,
    pub muted: bool,
    pub solo: bool,
    pub gain_db: f64,
    pub monitor_enabled: bool,
    pub program_enabled: bool,
    pub delay_ms: i64,
    pub filters: Value,
    pub route: Value,
}

pub fn channel_graph(channel: &AudioChannelState) -> Value {
    let meter = channel_meter(channel);
    let drift = drift_correction(channel);
    json!({
        "channel_id": channel.id,
        "source_kind": channel.channel_kind,
        "gain_db": channel.gain_db,
        "muted": channel.muted,
        "solo": channel.solo,
        "delay_ms": channel.delay_ms,
        "buses": {
            "program": channel.program_enabled && !channel.muted,
            "monitor": channel.monitor_enabled,
            "mix_minus": bool_setting(&channel.route, "mix_minus"),
            "isolated": bool_setting(&channel.route, "isolated")
        },
        "filters": {
            "noise_suppression": bool_setting(&channel.filters, "noise_suppression"),
            "noise_gate": bool_setting(&channel.filters, "noise_gate"),
            "compressor": bool_setting(&channel.filters, "compressor"),
            "limiter": bool_setting(&channel.filters, "limiter")
        },
        "drift_correction": drift,
        "meter": meter,
        "warnings": warnings(channel, &meter, &drift)
    })
}

pub fn mix_graph(channels: &[Value]) -> Value {
    let program = channels
        .iter()
        .filter(|channel| {
            channel
                .pointer("/audio_graph_json/buses/program")
                .and_then(Value::as_bool)
                == Some(true)
        })
        .count();
    let monitor = channels
        .iter()
        .filter(|channel| {
            channel
                .pointer("/audio_graph_json/buses/monitor")
                .and_then(Value::as_bool)
                == Some(true)
        })
        .count();
    let mix_minus = channels
        .iter()
        .filter(|channel| {
            channel
                .pointer("/audio_graph_json/buses/mix_minus")
                .and_then(Value::as_bool)
                == Some(true)
        })
        .count();
    let clipping = channels
        .iter()
        .filter(|channel| {
            channel
                .pointer("/audio_graph_json/meter/clipping")
                .and_then(Value::as_bool)
                == Some(true)
        })
        .count();
    let program_drift = channels
        .iter()
        .filter(|channel| {
            channel
                .pointer("/audio_graph_json/buses/program")
                .and_then(Value::as_bool)
                == Some(true)
        })
        .filter_map(|channel| channel.pointer("/audio_graph_json/drift_correction"))
        .collect::<Vec<_>>();
    let drift_unlocked = program_drift
        .iter()
        .filter(|drift| drift.get("status").and_then(Value::as_str) != Some("locked"))
        .count();
    let correction_active = program_drift
        .iter()
        .filter(|drift| drift.get("correction_active").and_then(Value::as_bool) == Some(true))
        .count();
    let max_residual_drift_ms = program_drift
        .iter()
        .filter_map(|drift| drift.get("residual_drift_ms").and_then(Value::as_i64))
        .max()
        .unwrap_or_default();
    json!({
        "program_bus_channels": program,
        "monitor_bus_channels": monitor,
        "mix_minus_channels": mix_minus,
        "clipping_channels": clipping,
        "drift_correction": {
            "status": if drift_unlocked == 0 { "locked" } else { "warning" },
            "reference_clock": "program_bus",
            "correction_active_channels": correction_active,
            "uncorrected_channels": drift_unlocked,
            "max_residual_drift_ms": max_residual_drift_ms,
            "long_session_ready": drift_unlocked == 0,
            "algorithm": "program_clock_aresample_async"
        },
        "status": if clipping == 0 && drift_unlocked == 0 { "ready" } else { "warning" }
    })
}

pub fn merged_filters(current: Value, patch: Option<Value>) -> Value {
    merge_object(
        json!({
            "noise_suppression": false,
            "noise_gate": false,
            "compressor": true,
            "limiter": true
        }),
        current,
        patch,
    )
}

pub fn merged_route(current: Value, patch: Option<Value>) -> Value {
    merge_object(
        json!({
            "program": true,
            "monitor": false,
            "mix_minus": false,
            "isolated": false,
            "drift_correction": true,
            "sync_anchor": "program_bus"
        }),
        current,
        patch,
    )
}

fn merge_object(defaults: Value, current: Value, patch: Option<Value>) -> Value {
    let mut object = defaults.as_object().cloned().unwrap_or_default();
    if let Some(current) = current.as_object() {
        for (key, value) in current {
            object.insert(key.clone(), value.clone());
        }
    }
    if let Some(patch) = patch.and_then(|value| value.as_object().cloned()) {
        for (key, value) in patch {
            object.insert(key, value);
        }
    }
    Value::Object(object)
}

fn channel_meter(channel: &AudioChannelState) -> Value {
    let base_db = match channel.channel_kind.as_str() {
        "microphone" => -18.0,
        "screen" => -24.0,
        "guest" => -20.0,
        "media" => -28.0,
        "program" => -12.0,
        _ => -30.0,
    };
    let filter_reduction = if bool_setting(&channel.filters, "compressor") {
        2.0
    } else {
        0.0
    } + if bool_setting(&channel.filters, "limiter") {
        1.5
    } else {
        0.0
    };
    let peak_db = if channel.muted {
        -90.0
    } else {
        (base_db + channel.gain_db - filter_reduction).min(0.0)
    };
    let rms_db = if channel.muted {
        -90.0
    } else {
        (peak_db - 9.0).max(-90.0)
    };
    json!({
        "rms_db": round_db(rms_db),
        "peak_db": round_db(peak_db),
        "level_percent": db_to_percent(peak_db),
        "clipping": peak_db >= -1.0,
        "silent": peak_db <= -80.0
    })
}

fn drift_correction(channel: &AudioChannelState) -> Value {
    let program_active = channel.program_enabled && !channel.muted;
    let correction_enabled = bool_setting_default(&channel.route, "drift_correction", true);
    let source_drift_floor_ms = match channel.channel_kind.as_str() {
        "guest" => 18,
        "media" => 12,
        "desktop_audio" => 10,
        "microphone" => 6,
        "program" => 4,
        _ => 8,
    };
    let measured_drift_ms = (channel.delay_ms.abs() + source_drift_floor_ms).min(5000);
    let correction_applied_ms = if program_active && correction_enabled {
        measured_drift_ms
    } else {
        0
    };
    let residual_drift_ms = (measured_drift_ms - correction_applied_ms).max(0);
    let status = if !program_active {
        "standby"
    } else if residual_drift_ms <= 20 {
        "locked"
    } else {
        "warning"
    };
    json!({
        "status": status,
        "reference_clock": string_setting(&channel.route, "sync_anchor", "program_bus"),
        "correction_enabled": correction_enabled,
        "correction_active": program_active && correction_enabled,
        "measured_drift_ms": measured_drift_ms,
        "correction_applied_ms": correction_applied_ms,
        "residual_drift_ms": residual_drift_ms,
        "max_residual_drift_ms": 20,
        "long_session_ready": status == "locked" || status == "standby",
        "resample_filter": "aresample=async=1000:first_pts=0",
        "strategy": "program_clock_aresample_async"
    })
}

fn warnings(channel: &AudioChannelState, meter: &Value, drift: &Value) -> Vec<String> {
    let mut warnings = Vec::new();
    if meter.get("clipping").and_then(Value::as_bool) == Some(true) {
        warnings.push("clipping".to_string());
    }
    if meter.get("silent").and_then(Value::as_bool) == Some(true) && !channel.muted {
        warnings.push("silent".to_string());
    }
    if channel.channel_kind == "guest" && !bool_setting(&channel.route, "mix_minus") {
        warnings.push("guest_mix_minus_disabled".to_string());
    }
    if channel.channel_kind == "microphone" && !bool_setting(&channel.filters, "limiter") {
        warnings.push("limiter_disabled".to_string());
    }
    if drift.get("status").and_then(Value::as_str) == Some("warning") {
        warnings.push("drift_correction_unlocked".to_string());
    }
    warnings
}

fn bool_setting(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn bool_setting_default(value: &Value, key: &str, fallback: bool) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(fallback)
}

fn string_setting(value: &Value, key: &str, fallback: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn db_to_percent(db: f64) -> i64 {
    if db <= -60.0 {
        0
    } else {
        (((db + 60.0) / 60.0) * 100.0).round().clamp(0.0, 100.0) as i64
    }
}

fn round_db(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}
