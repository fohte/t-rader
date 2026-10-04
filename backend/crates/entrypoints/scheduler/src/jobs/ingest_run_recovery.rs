use chrono::{DateTime, Duration as ChronoDuration, FixedOffset, Utc};
use core_application::ingest_run_log::IngestRunLog;
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use crate::{scheduler::RECOVERABLE_INGEST_JOBS, state::SchedulerState};

#[derive(Debug, Deserialize, Serialize)]
pub struct IngestRunRecovery;

impl TaskHandler for IngestRunRecovery {
    const IDENTIFIER: &'static str = "ingest_run_recovery";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        let Some(state) = context.get_ext::<SchedulerState>() else {
            return Err("scheduler state is not configured".to_string());
        };

        recover_interrupted_runs(
            state.dependencies.ingest_run_log.as_ref(),
            Utc::now().fixed_offset(),
        )
        .await
        .map(|_| ())
    }
}

async fn recover_interrupted_runs(
    log: &dyn IngestRunLog,
    now: DateTime<FixedOffset>,
) -> Result<u64, String> {
    let mut recovered = 0_u64;

    for (job, timeout) in RECOVERABLE_INGEST_JOBS {
        let cutoff = now - ChronoDuration::seconds(timeout.as_secs() as i64);
        recovered = recovered.saturating_add(
            log.fail_interrupted_before(job, cutoff)
                .await
                .map_err(|error| error.to_string())?,
        );
    }

    if recovered > 0 {
        tracing::info!(recovered, "interrupted ingest runs recovered");
    }
    Ok(recovered)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::{DateTime, Duration, FixedOffset};
    use core_application::{ingest_run_log::IngestRunLog, persistence::PersistenceError};
    use serde_json::Value;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use super::recover_interrupted_runs;

    #[derive(Default)]
    struct FakeIngestRunLog {
        calls: Mutex<Vec<(String, DateTime<FixedOffset>)>>,
    }

    #[async_trait]
    impl IngestRunLog for FakeIngestRunLog {
        async fn start(&self, _job: &str) -> Result<Uuid, PersistenceError> {
            Ok(Uuid::from_u128(1))
        }

        async fn finish(
            &self,
            _run_id: Uuid,
            _result: Result<Value, String>,
        ) -> Result<(), PersistenceError> {
            Ok(())
        }

        async fn fail_interrupted_before(
            &self,
            job: &str,
            started_before: DateTime<FixedOffset>,
        ) -> Result<u64, PersistenceError> {
            self.calls
                .lock()
                .await
                .push((job.to_string(), started_before));
            Ok(1)
        }
    }

    #[tokio::test]
    async fn uses_each_registered_jobs_timeout_for_recovery() {
        let now = DateTime::parse_from_rfc3339("2026-10-01T12:00:00+00:00")
            .expect("valid fixed timestamp");
        let log = Arc::new(FakeIngestRunLog::default());
        let result = recover_interrupted_runs(log.as_ref(), now).await;

        assert_eq!(
            (result, log.calls.lock().await.clone()),
            (
                Ok(14),
                vec![
                    ("fred_ingest".to_string(), now - Duration::hours(2)),
                    (
                        "fred_release_dates_ingest".to_string(),
                        now - Duration::hours(2),
                    ),
                    (
                        "e_stat_calendar_ingest".to_string(),
                        now - Duration::hours(2),
                    ),
                    ("short_ratio_ingest".to_string(), now - Duration::hours(2)),
                    (
                        "short_sale_report_ingest".to_string(),
                        now - Duration::hours(2)
                    ),
                    ("margin_ingest".to_string(), now - Duration::hours(2)),
                    ("prediction_grading".to_string(), now - Duration::hours(12)),
                    ("daily_bars_ingest".to_string(), now - Duration::hours(2)),
                    (
                        "earnings_schedule_ingest".to_string(),
                        now - Duration::hours(2)
                    ),
                    (
                        "financial_summary_ingest".to_string(),
                        now - Duration::hours(2)
                    ),
                    ("news_aggregation".to_string(), now - Duration::hours(2)),
                    ("valuation_ingest".to_string(), now - Duration::hours(2)),
                    ("equity_master_ingest".to_string(), now - Duration::hours(2)),
                    (
                        "shareholding_structure_ingest".to_string(),
                        now - Duration::hours(2)
                    ),
                ],
            ),
        );
    }
}
