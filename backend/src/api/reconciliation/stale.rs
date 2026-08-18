use super::*;

pub(crate) fn stale_media_processing_cutoff() -> String {
    (Utc::now() - ChronoDuration::minutes(5)).to_rfc3339()
}

pub(crate) fn is_upload_job_stale(job: &UploadJob) -> bool {
    job.status == "processing" && job.updated_at < stale_media_processing_cutoff()
}

pub(crate) fn stale_live_ingest_cutoff() -> String {
    (Utc::now() - ChronoDuration::seconds(20)).to_rfc3339()
}

pub(crate) fn is_live_ingest_session_stale(session: &LiveIngestSession) -> bool {
    session.status == "connected" && session.last_heartbeat_at < stale_live_ingest_cutoff()
}
