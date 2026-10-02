use chrono::NaiveDate;
use core_domain::business_day::latest_business_day;

use crate::ingest_status::IngestStatus;
use crate::ingest_status::{INGEST_JOBS, IngestJobStatus, SharedIngestStatusRepository};
use crate::persistence::PersistenceError;

pub struct IngestStatusUseCase {
    repository: SharedIngestStatusRepository,
}

impl IngestStatusUseCase {
    pub fn new(repository: SharedIngestStatusRepository) -> Self {
        Self { repository }
    }

    pub async fn get(&self, as_of_date: NaiveDate) -> Result<IngestStatus, PersistenceError> {
        let data = self.repository.read().await?;
        let expected_data_date = latest_business_day(as_of_date);

        let jobs = INGEST_JOBS
            .iter()
            .map(|definition| {
                let history = data
                    .run_histories
                    .iter()
                    .find(|history| history.job == definition.identifier);
                let latest_data_date = data
                    .latest_data_dates
                    .iter()
                    .find(|date| date.job == definition.identifier)
                    .and_then(|date| date.latest_data_date);
                let worker_jobs = data
                    .worker_jobs
                    .iter()
                    .filter(|job| job.job == definition.identifier)
                    .cloned()
                    .collect();

                IngestJobStatus {
                    job: definition.identifier.to_string(),
                    latest_run: history.map(|history| history.latest_run.clone()),
                    last_succeeded_at: history.and_then(|history| history.last_succeeded_at),
                    latest_data_date,
                    expected_data_date: definition.has_data_date.then_some(expected_data_date),
                    worker_jobs,
                }
            })
            .collect();

        Ok(IngestStatus { jobs })
    }
}
