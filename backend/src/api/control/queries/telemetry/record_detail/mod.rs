use super::record_collab::LiveRuntimeTelemetryCollaboration;
use super::*;
use crate::api::control::apply_collaboration_transport_gap;
use crate::models::LiveRuntimeTarget;

mod artifacts;
mod collab;
mod runtime;
mod targets;

use artifacts::build_live_runtime_telemetry_artifact_detail;
use collab::build_collaboration_detail;
use runtime::{
    build_delivery_detail, build_metric_detail, build_runtime_output_detail, build_session_detail,
    maybe_insert_output_path,
};
use targets::build_target_detail;

pub(super) fn build_live_runtime_telemetry_detail(
    session: &LiveIngestSession,
    sample_kind: &str,
    runtime_state: &str,
    packaging_status: &str,
    archive_status: &str,
    cpu_percent: Option<i64>,
    free_disk_gb: Option<f64>,
    output: Option<&LiveRuntimeOutput>,
    targets: &[LiveRuntimeTarget],
    collaboration: Option<&LiveRuntimeTelemetryCollaboration>,
    reported: Value,
) -> Value {
    let mut detail = match reported {
        Value::Object(map) => map,
        other => {
            let mut map = serde_json::Map::new();
            map.insert("reportedValue".to_string(), other);
            map
        }
    };

    maybe_insert_output_path(
        &mut detail,
        "manifestRelativePath",
        output.and_then(|item| item.manifest_relative_path.clone()),
    );
    maybe_insert_output_path(
        &mut detail,
        "archiveRelativePath",
        output.and_then(|item| item.archive_relative_path.clone()),
    );
    maybe_insert_output_path(
        &mut detail,
        "lastError",
        output.and_then(|item| item.last_error.clone()),
    );

    detail.insert(
        "session".to_string(),
        build_session_detail(session, sample_kind),
    );
    let advisory = apply_collaboration_transport_gap(
        session,
        crate::api::control::build_live_runtime_advisory(Some(session), output, None),
        collaboration
            .map(|item| item.transport_gap_present)
            .unwrap_or(false),
    );
    detail.insert("advisory".to_string(), json!(advisory));
    detail.insert(
        "artifacts".to_string(),
        build_live_runtime_telemetry_artifact_detail(
            session,
            packaging_status,
            archive_status,
            output,
        ),
    );
    detail.insert(
        "runtimeOutput".to_string(),
        build_runtime_output_detail(runtime_state, packaging_status, archive_status, output),
    );
    detail.insert(
        "metrics".to_string(),
        build_metric_detail(session, cpu_percent, free_disk_gb),
    );
    detail.insert("delivery".to_string(), build_delivery_detail(output));
    detail.insert(
        "collaboration".to_string(),
        build_collaboration_detail(collaboration),
    );
    detail.insert("targets".to_string(), build_target_detail(targets));
    detail.insert(
        "outputs".to_string(),
        json!({
            "activeRouteCount": collaboration.map(|item| item.active_route_count).unwrap_or(0),
            "armedArchiveRouteCount": collaboration
                .map(|item| item.armed_archive_route_count)
                .unwrap_or(0),
            "playbackReady": matches!(packaging_status, "ready" | "complete")
                && output.and_then(|item| item.manifest_relative_path.as_ref()).is_some(),
            "archiveReady": archive_status == "complete"
                && output.and_then(|item| item.archive_relative_path.as_ref()).is_some(),
        }),
    );

    Value::Object(detail)
}
