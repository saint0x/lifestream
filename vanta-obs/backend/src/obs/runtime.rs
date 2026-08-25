use serde_json::{Value, json};

pub fn stream_snapshot(sequence: u64, dashboard: Value) -> Value {
    let broadcast_id = dashboard["broadcast"]["id"].as_str().unwrap_or_default();
    let runtime_state = dashboard["runtime"]["runtime_state"]
        .as_str()
        .unwrap_or("unknown");
    let stream_state = dashboard["runtime"]["stream_state"]
        .as_str()
        .unwrap_or("unknown");
    let severity = dashboard["events"]
        .as_array()
        .and_then(|events| events.first())
        .and_then(|event| event["severity"].as_str())
        .unwrap_or("info");
    json!({
        "id": format!("runtime_stream_{}_{}", broadcast_id, sequence),
        "sequence": sequence,
        "event_kind": "runtime_snapshot",
        "severity": severity,
        "broadcast_id": broadcast_id,
        "runtime_state": runtime_state,
        "stream_state": stream_state,
        "dashboard": dashboard
    })
}
