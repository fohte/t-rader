use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use serde_json::Value;
use uuid::Uuid;

use crate::event::{NewEvent, SharedEventRepository};
use crate::event_dispatch_job::EventDispatchJobUseCases;
use crate::persistence::PersistenceError;
use crate::unit_of_work::{SharedUnitOfWork, UnitOfWorkTransaction};

#[async_trait]
pub trait IngestRunRepository: Send + Sync {
    async fn start(&self, job: &str) -> Result<Uuid, PersistenceError>;

    async fn finish(
        &self,
        transaction: &UnitOfWorkTransaction,
        run_id: Uuid,
        status: &str,
        stats: Option<Value>,
        error: Option<String>,
    ) -> Result<String, PersistenceError>;

    async fn fail_interrupted_before(
        &self,
        job: &str,
        started_before: DateTime<FixedOffset>,
    ) -> Result<u64, PersistenceError>;
}

pub type SharedIngestRunRepository = std::sync::Arc<dyn IngestRunRepository + Send + Sync>;

#[async_trait]
pub trait IngestRunLog: Send + Sync {
    async fn start(&self, job: &str) -> Result<Uuid, PersistenceError>;

    async fn finish(
        &self,
        run_id: Uuid,
        result: Result<Value, String>,
    ) -> Result<(), PersistenceError>;

    async fn finish_with_stats_and_error(
        &self,
        run_id: Uuid,
        stats: Value,
        error: String,
    ) -> Result<(), PersistenceError> {
        self.finish(run_id, Err(format!("{error}; stats={stats}")))
            .await
    }

    async fn fail_interrupted_before(
        &self,
        job: &str,
        started_before: DateTime<FixedOffset>,
    ) -> Result<u64, PersistenceError>;
}

pub type SharedIngestRunLog = std::sync::Arc<dyn IngestRunLog + Send + Sync>;

#[derive(Clone)]
pub struct IngestRunLogUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedIngestRunRepository,
    event_repository: SharedEventRepository,
    event_dispatch_job: EventDispatchJobUseCases,
}

impl IngestRunLogUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedIngestRunRepository,
        event_repository: SharedEventRepository,
        event_dispatch_job: EventDispatchJobUseCases,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            event_repository,
            event_dispatch_job,
        }
    }

    async fn finish_with_event(
        &self,
        run_id: Uuid,
        event_type: &str,
        status: &str,
        stats: Option<Value>,
        error: Option<String>,
    ) -> Result<(), PersistenceError> {
        let transaction = self
            .unit_of_work
            .begin()
            .await
            .map_err(|error| PersistenceError::Database(error.to_string()))?;
        let job = self
            .repository
            .finish(&transaction, run_id, status, stats.clone(), error.clone())
            .await?;
        self.event_repository
            .create(
                &transaction,
                NewEvent {
                    event_type: event_type.to_owned(),
                    payload: serde_json::json!({
                        "run_id": run_id,
                        "job": job,
                        "stats": stats,
                        "error": error,
                    }),
                    chain_depth: Some(0),
                },
            )
            .await
            .map_err(|error| PersistenceError::Database(error.to_string()))?;
        self.unit_of_work
            .commit(transaction)
            .await
            .map_err(|error| PersistenceError::Database(error.to_string()))?;

        if let Err(error) = self.event_dispatch_job.enqueue_dispatch().await {
            tracing::warn!(
                %run_id,
                %error,
                "failed to enqueue event dispatch; periodic dispatch will retry"
            );
        }

        Ok(())
    }
}

#[async_trait]
impl IngestRunLog for IngestRunLogUseCases {
    async fn start(&self, job: &str) -> Result<Uuid, PersistenceError> {
        self.repository.start(job).await
    }

    async fn finish(
        &self,
        run_id: Uuid,
        result: Result<Value, String>,
    ) -> Result<(), PersistenceError> {
        match result {
            Ok(stats) => {
                self.finish_with_event(
                    run_id,
                    "ingest_run.succeeded",
                    "succeeded",
                    Some(stats),
                    None,
                )
                .await
            }
            Err(error) => {
                self.finish_with_event(run_id, "ingest_run.failed", "failed", None, Some(error))
                    .await
            }
        }
    }

    async fn finish_with_stats_and_error(
        &self,
        run_id: Uuid,
        stats: Value,
        error: String,
    ) -> Result<(), PersistenceError> {
        self.finish_with_event(
            run_id,
            "ingest_run.failed",
            "failed",
            Some(stats),
            Some(error),
        )
        .await
    }

    async fn fail_interrupted_before(
        &self,
        job: &str,
        started_before: DateTime<FixedOffset>,
    ) -> Result<u64, PersistenceError> {
        self.repository
            .fail_interrupted_before(job, started_before)
            .await
    }
}
