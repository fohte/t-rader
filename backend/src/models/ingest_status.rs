use chrono::{DateTime, FixedOffset, NaiveDate};
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct IngestStatusResponse {
    pub jobs: Vec<IngestJobStatusResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IngestJobStatusResponse {
    pub job: String,
    pub last_run: Option<IngestRunResponse>,
    pub last_succeeded_at: Option<DateTime<FixedOffset>>,
    pub latest_data_date: Option<NaiveDate>,
    pub expected_data_date: Option<NaiveDate>,
    pub worker_jobs: Vec<IngestWorkerJobResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IngestRunResponse {
    pub id: Uuid,
    pub started_at: DateTime<FixedOffset>,
    pub finished_at: Option<DateTime<FixedOffset>>,
    pub status: String,
    pub stats: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IngestWorkerJobResponse {
    pub id: i64,
    pub task_identifier: String,
    pub state: String,
    pub queue_name: Option<String>,
    pub run_at: DateTime<FixedOffset>,
    pub attempts: i32,
    pub max_attempts: i32,
    pub last_error: Option<String>,
}

impl From<core_application::ingest_status::IngestStatus> for IngestStatusResponse {
    fn from(status: core_application::ingest_status::IngestStatus) -> Self {
        Self {
            jobs: status.jobs.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<core_application::ingest_status::IngestJobStatus> for IngestJobStatusResponse {
    fn from(status: core_application::ingest_status::IngestJobStatus) -> Self {
        Self {
            job: status.job,
            last_run: status.latest_run.map(Into::into),
            last_succeeded_at: status.last_succeeded_at,
            latest_data_date: status.latest_data_date,
            expected_data_date: status.expected_data_date,
            worker_jobs: status.worker_jobs.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<core_application::ingest_status::IngestRun> for IngestRunResponse {
    fn from(run: core_application::ingest_status::IngestRun) -> Self {
        Self {
            id: run.id,
            started_at: run.started_at,
            finished_at: run.finished_at,
            status: run.status,
            stats: run.stats,
            error: run.error,
        }
    }
}

impl From<core_application::ingest_status::IngestWorkerJob> for IngestWorkerJobResponse {
    fn from(job: core_application::ingest_status::IngestWorkerJob) -> Self {
        Self {
            id: job.id,
            task_identifier: job.job,
            state: job.state,
            queue_name: job.queue_name,
            run_at: job.run_at,
            attempts: job.attempts,
            max_attempts: job.max_attempts,
            last_error: job.last_error,
        }
    }
}
