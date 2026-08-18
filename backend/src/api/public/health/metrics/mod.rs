use super::*;

mod live_ingest;
mod response;
mod runtime;

use live_ingest::write_live_ingest_metrics;
use response::{finish_metrics_response, write_counter, write_gauge, write_optional_gauge};
use runtime::write_runtime_metrics;

pub(crate) async fn metrics(State(state): State<SharedState>) -> AppResult<Response> {
    let mut body = String::new();
    let status_counts = state.metrics.status_counts().await;
    let active_streams = state.realtime.active_streams().await;
    let active_collaboration_sessions = state.realtime.active_collaboration_sessions().await;
    reconcile_stale_creator_live_socket_sessions_for_read(&state.pool, None, None).await?;
    let active_viewer_presence = count_all_active_live_viewer_sessions(&state.pool).await?;
    let active_collaboration_presence =
        count_all_active_collaboration_socket_sessions(&state.pool).await?;
    let active_creator_live_presence =
        count_all_active_creator_live_socket_sessions(&state.pool).await?;
    let live_ingest_overview = fetch_admin_live_ingest_overview(&state.pool, None).await?;
    let worker = state.background_worker.snapshot().await;

    write_counter(
        &mut body,
        "lifestream_http_requests_total",
        state.metrics.total_requests(),
    );
    write_gauge(
        &mut body,
        "lifestream_http_requests_in_flight",
        state.metrics.in_flight_requests(),
    );
    write_counter(
        &mut body,
        "lifestream_http_rate_limited_total",
        state.metrics.total_rate_limits(),
    );

    let _ = writeln!(body, "# TYPE lifestream_http_responses_total counter");
    for (status, count) in status_counts {
        let _ = writeln!(
            body,
            "lifestream_http_responses_total{{status=\"{status}\"}} {count}"
        );
    }

    write_gauge(
        &mut body,
        "lifestream_uptime_seconds",
        state.uptime_seconds(),
    );
    write_gauge(
        &mut body,
        "lifestream_realtime_collaboration_sessions",
        active_collaboration_sessions,
    );
    write_gauge(
        &mut body,
        "lifestream_presence_live_viewers",
        active_viewer_presence,
    );
    write_gauge(
        &mut body,
        "lifestream_presence_collaboration_participants",
        active_collaboration_presence,
    );
    write_gauge(
        &mut body,
        "lifestream_presence_creator_live_sockets",
        active_creator_live_presence,
    );
    write_gauge(
        &mut body,
        "lifestream_db_pool_connections",
        state.pool.size(),
    );
    write_gauge(
        &mut body,
        "lifestream_db_pool_idle_connections",
        state.pool.num_idle(),
    );
    write_gauge(
        &mut body,
        "lifestream_ws_connections",
        state.realtime.total_connections(),
    );
    write_gauge(&mut body, "lifestream_ws_active_streams", active_streams);

    write_runtime_metrics(&mut body, &worker);
    write_live_ingest_metrics(&mut body, &live_ingest_overview);

    finish_metrics_response(body)
}
