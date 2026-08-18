use super::*;

pub(super) fn maybe_insert_output_path(
    detail: &mut serde_json::Map<String, Value>,
    key: &str,
    value: Option<String>,
) {
    if !detail.contains_key(key) {
        detail.insert(
            key.to_string(),
            value.map(Value::String).unwrap_or(Value::Null),
        );
    }
}

pub(super) fn build_session_detail(session: &LiveIngestSession, sample_kind: &str) -> Value {
    json!({
        "sampleKind": sample_kind,
        "protocol": session.protocol.clone(),
        "contributionClass": session.contribution_class.clone(),
        "contributionState": session.contribution_state.clone(),
        "reconnectSession": session.previous_session_id.is_some(),
        "previousSessionId": session.previous_session_id.clone(),
        "status": session.status.clone(),
        "ingestServer": session.ingest_server.clone(),
        "ingestLatencyMs": session.ingest_latency_ms,
        "sourceProbePresent": session.source_probe.is_some(),
        "sourceProbe": session.source_probe.clone(),
        "sourceValidation": session.source_validation.clone(),
    })
}

pub(super) fn build_runtime_output_detail(
    runtime_state: &str,
    packaging_status: &str,
    archive_status: &str,
    output: Option<&LiveRuntimeOutput>,
) -> Value {
    json!({
        "state": runtime_state,
        "packagingStatus": packaging_status,
        "archiveStatus": archive_status,
        "runtimeClass": output.map(|item| item.runtime_class.clone()),
        "latencyProfile": output.map(|item| item.latency_profile.clone()),
        "segmentFormat": output.map(|item| item.segment_format.clone()),
        "partialSegmentsEnabled": output.map(|item| item.partial_segments_enabled),
        "blockingReloadEnabled": output.map(|item| item.blocking_reload_enabled),
        "targetSegmentDurationSec": output.map(|item| item.target_segment_duration_sec),
        "holdBackSegments": output.map(|item| item.hold_back_segments),
        "discontinuitySequence": output.map(|item| item.discontinuity_sequence),
        "ladderPolicy": output.map(|item| item.ladder_policy.clone()),
        "contentClass": output.map(|item| item.content_class.clone()),
        "manifestRelativePath": output.and_then(|item| item.manifest_relative_path.clone()),
        "archiveRelativePath": output.and_then(|item| item.archive_relative_path.clone()),
        "lastError": output.and_then(|item| item.last_error.clone()),
        "updatedAt": output.map(|item| item.updated_at.clone()),
    })
}

pub(super) fn build_metric_detail(
    session: &LiveIngestSession,
    cpu_percent: Option<i64>,
    free_disk_gb: Option<f64>,
) -> Value {
    json!({
        "bitrateKbps": session.bitrate_kbps,
        "viewers": session.viewers,
        "droppedFrames": session.dropped_frames,
        "cpuPercent": cpu_percent,
        "freeDiskGb": free_disk_gb,
    })
}

pub(super) fn build_delivery_detail(output: Option<&LiveRuntimeOutput>) -> Value {
    json!({
        "runtimeClass": output.map(|item| item.runtime_class.clone()),
        "latencyProfile": output.map(|item| item.latency_profile.clone()),
        "segmentFormat": output.map(|item| item.segment_format.clone()),
        "partialSegmentsEnabled": output.map(|item| item.partial_segments_enabled).unwrap_or(false),
        "blockingReloadEnabled": output.map(|item| item.blocking_reload_enabled).unwrap_or(false),
        "targetSegmentDurationSec": output.map(|item| item.target_segment_duration_sec),
        "holdBackSegments": output.map(|item| item.hold_back_segments),
        "discontinuitySequence": output.map(|item| item.discontinuity_sequence).unwrap_or(0),
        "ladderPolicy": output.map(|item| item.ladder_policy.clone()),
        "contentClass": output.map(|item| item.content_class.clone()),
    })
}
