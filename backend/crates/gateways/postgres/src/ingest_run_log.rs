use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Utc};
use core_application::ingest_run_log::IngestRunRepository;
use core_application::persistence::PersistenceError;
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{Set, Unchanged};
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::Value;
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::ingest_run;
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref;

#[derive(Clone)]
pub struct PostgresIngestRunRepository {
    db: DatabaseHandle,
}

impl PostgresIngestRunRepository {
    pub fn new(db: impl Into<DatabaseHandle>) -> Self {
        Self { db: db.into() }
    }
}

#[async_trait]
impl IngestRunRepository for PostgresIngestRunRepository {
    async fn start(&self, job: &str) -> Result<Uuid, PersistenceError> {
        ingest_run::Entity::insert(ingest_run::ActiveModel {
            job: Set(job.to_string()),
            ..Default::default()
        })
        .exec_with_returning(&self.db)
        .await
        .map(|run| run.id)
        .map_err(persistence_error)
    }

    async fn finish(
        &self,
        transaction: &UnitOfWorkTransaction,
        run_id: Uuid,
        status: &str,
        stats: Option<Value>,
        error: Option<String>,
    ) -> Result<String, PersistenceError> {
        let connection = transaction_ref(transaction).ok_or_else(|| {
            PersistenceError::Database("transaction has an unexpected type".to_owned())
        })?;
        let run = ingest_run::Entity::find_by_id(run_id)
            .one(connection)
            .await
            .map_err(persistence_error)?
            .ok_or_else(|| {
                PersistenceError::RecordNotUpdated(format!("ingest run {run_id} was not found"))
            })?;
        ingest_run::Entity::update(ingest_run::ActiveModel {
            id: Unchanged(run_id),
            finished_at: Set(Some(Utc::now().fixed_offset())),
            status: Set(status.to_owned()),
            stats: Set(stats.clone()),
            error: Set(error.clone()),
            ..Default::default()
        })
        .exec(connection)
        .await
        .map_err(persistence_error)?;
        Ok(run.job)
    }

    async fn fail_interrupted_before(
        &self,
        job: &str,
        started_before: DateTime<FixedOffset>,
    ) -> Result<u64, PersistenceError> {
        ingest_run::Entity::update_many()
            .col_expr(ingest_run::Column::FinishedAt, Expr::current_timestamp())
            .col_expr(ingest_run::Column::Status, Expr::value("failed"))
            .col_expr(ingest_run::Column::Error, Expr::value("interrupted"))
            .filter(ingest_run::Column::Job.eq(job))
            .filter(ingest_run::Column::Status.eq("running"))
            .filter(ingest_run::Column::StartedAt.lt(started_before))
            .exec(&self.db)
            .await
            .map(|result| result.rows_affected)
            .map_err(persistence_error)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };

    use async_trait::async_trait;
    use chrono::{Duration, Timelike, Utc};
    use core_application::event_dispatch_job::{
        EventDispatchJobQueue, EventDispatchJobQueueError, EventDispatchJobUseCases,
    };
    use core_application::ingest_run_log::{IngestRunLog, IngestRunLogUseCases};
    use core_application::unit_of_work::{
        SharedUnitOfWork, UnitOfWork, UnitOfWorkError, UnitOfWorkTransaction,
    };
    use sea_orm::ActiveValue::Set;
    use sea_orm::{ConnectionTrait, DatabaseBackend, EntityTrait, PaginatorTrait, Statement};
    use serde_json::json;

    use super::PostgresIngestRunRepository;
    use crate::{
        DatabaseHandle, PostgresEventRepository, PostgresUnitOfWork,
        entities::{event, ingest_run},
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct DispatchObservation {
        committed: bool,
        event_count: u64,
    }

    #[derive(Clone)]
    struct ObservingUnitOfWork {
        inner: SharedUnitOfWork,
        commit_succeeded: Arc<AtomicBool>,
    }

    #[async_trait]
    impl UnitOfWork for ObservingUnitOfWork {
        async fn begin(&self) -> Result<UnitOfWorkTransaction, UnitOfWorkError> {
            self.inner.begin().await
        }

        async fn commit(&self, transaction: UnitOfWorkTransaction) -> Result<(), UnitOfWorkError> {
            self.inner.commit(transaction).await?;
            self.commit_succeeded.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    #[derive(Clone)]
    struct ObservingEventDispatchJobQueue {
        db: DatabaseHandle,
        commit_succeeded: Arc<AtomicBool>,
        observations: Arc<Mutex<Vec<DispatchObservation>>>,
        fail_enqueue: bool,
    }

    #[async_trait]
    impl EventDispatchJobQueue for ObservingEventDispatchJobQueue {
        async fn enqueue_dispatch(&self) -> Result<(), EventDispatchJobQueueError> {
            let event_count = event::Entity::find()
                .count(&self.db)
                .await
                .map_err(|error| EventDispatchJobQueueError::new(error.to_string()))?;
            self.observations
                .lock()
                .map_err(|error| EventDispatchJobQueueError::new(error.to_string()))?
                .push(DispatchObservation {
                    committed: self.commit_succeeded.load(Ordering::SeqCst),
                    event_count,
                });
            if self.fail_enqueue {
                return Err(EventDispatchJobQueueError::new(
                    "sample_enqueue_error".to_owned(),
                ));
            }
            Ok(())
        }
    }

    fn log(db: DatabaseHandle) -> (IngestRunLogUseCases, Arc<Mutex<Vec<DispatchObservation>>>) {
        log_with_dispatch_failure(db, false)
    }

    fn log_with_dispatch_failure(
        db: DatabaseHandle,
        fail_enqueue: bool,
    ) -> (IngestRunLogUseCases, Arc<Mutex<Vec<DispatchObservation>>>) {
        let commit_succeeded = Arc::new(AtomicBool::new(false));
        let observations = Arc::new(Mutex::new(Vec::new()));
        let unit_of_work: SharedUnitOfWork = Arc::new(ObservingUnitOfWork {
            inner: Arc::new(PostgresUnitOfWork::new(db.clone())),
            commit_succeeded: commit_succeeded.clone(),
        });
        let event_dispatch_job =
            EventDispatchJobUseCases::new(Arc::new(ObservingEventDispatchJobQueue {
                db: db.clone(),
                commit_succeeded,
                observations: observations.clone(),
                fail_enqueue,
            }));
        (
            IngestRunLogUseCases::new(
                unit_of_work,
                Arc::new(PostgresIngestRunRepository::new(db.clone())),
                Arc::new(PostgresEventRepository::new(db)),
                event_dispatch_job,
            ),
            observations,
        )
    }

    fn read_dispatch_observations(
        observations: &Arc<Mutex<Vec<DispatchObservation>>>,
    ) -> Vec<DispatchObservation> {
        observations
            .lock()
            .expect("read dispatch observations")
            .clone()
    }

    fn epoch() -> chrono::DateTime<chrono::FixedOffset> {
        chrono::DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z").expect("valid epoch timestamp")
    }

    fn normalize_finished_at(mut run: ingest_run::Model) -> ingest_run::Model {
        run.finished_at = run.finished_at.map(|_| epoch());
        run
    }

    fn normalize_timestamps(mut run: ingest_run::Model) -> ingest_run::Model {
        run.started_at = epoch();
        run.finished_at = run.finished_at.map(|_| epoch());
        run
    }

    fn normalize_events(mut events: Vec<event::Model>) -> Vec<event::Model> {
        for event in &mut events {
            event.event_id = uuid::Uuid::nil();
            event.occurred_at = epoch();
        }
        events
    }

    fn truncate_to_postgres_precision(
        timestamp: chrono::DateTime<chrono::FixedOffset>,
    ) -> chrono::DateTime<chrono::FixedOffset> {
        timestamp
            .with_nanosecond(timestamp.timestamp_subsec_micros() * 1_000)
            .expect("valid nanosecond")
    }

    async fn row(db: &DatabaseHandle, run_id: uuid::Uuid) -> ingest_run::Model {
        ingest_run::Entity::find_by_id(run_id)
            .one(db)
            .await
            .expect("read ingest run")
            .expect("ingest run exists")
    }

    async fn events(db: &DatabaseHandle) -> Vec<event::Model> {
        event::Entity::find().all(db).await.expect("read events")
    }

    async fn set_started_at(
        db: &DatabaseHandle,
        run_id: uuid::Uuid,
        started_at: chrono::DateTime<chrono::FixedOffset>,
    ) {
        ingest_run::Entity::update(ingest_run::ActiveModel {
            id: Set(run_id),
            started_at: Set(started_at),
            ..Default::default()
        })
        .exec(db)
        .await
        .expect("set ingest run start time");
    }

    #[backend_test_macros::database_test]
    async fn start_creates_a_running_row(db: DatabaseHandle) {
        let (log, dispatch_observations) = log(db.clone());
        let transaction_time = db
            .query_one_raw(Statement::from_string(
                DatabaseBackend::Postgres,
                "SELECT CURRENT_TIMESTAMP AS transaction_time",
            ))
            .await
            .expect("read transaction timestamp")
            .expect("transaction timestamp exists")
            .try_get::<chrono::DateTime<chrono::FixedOffset>>("", "transaction_time")
            .expect("decode transaction timestamp");
        let run_id = log.start("sample_ingest").await.expect("start run");

        let run = row(&db, run_id).await;
        let started_at_matches_transaction_time = run.started_at == transaction_time;
        let events = events(&db).await;

        assert_eq!(
            (
                normalize_timestamps(run),
                started_at_matches_transaction_time,
                normalize_events(events),
                read_dispatch_observations(&dispatch_observations),
            ),
            (
                ingest_run::Model {
                    id: run_id,
                    job: "sample_ingest".to_string(),
                    started_at: epoch(),
                    finished_at: None,
                    status: "running".to_string(),
                    stats: None,
                    error: None,
                },
                true,
                Vec::new(),
                Vec::new(),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn finish_stores_stats_for_a_successful_run(db: DatabaseHandle) {
        let (log, dispatch_observations) = log(db.clone());
        let run_id = log.start("sample_ingest").await.expect("start run");
        let started_at = row(&db, run_id).await.started_at;
        let stats = json!({ "rows": 3 });
        log.finish(run_id, Ok(stats.clone()))
            .await
            .expect("finish successful run");

        let run = row(&db, run_id).await;
        let events = events(&db).await;

        assert_eq!(
            (
                normalize_finished_at(run),
                normalize_events(events),
                read_dispatch_observations(&dispatch_observations),
            ),
            (
                ingest_run::Model {
                    id: run_id,
                    job: "sample_ingest".to_string(),
                    started_at,
                    finished_at: Some(epoch()),
                    status: "succeeded".to_string(),
                    stats: Some(stats.clone()),
                    error: None,
                },
                vec![event::Model {
                    event_id: uuid::Uuid::nil(),
                    event_type: "ingest_run.succeeded".to_owned(),
                    payload: json!({
                        "run_id": run_id.to_string(),
                        "job": "sample_ingest",
                        "stats": stats,
                        "error": null,
                    }),
                    chain_depth: 0,
                    occurred_at: epoch(),
                    dispatched_at: None,
                }],
                vec![DispatchObservation {
                    committed: true,
                    event_count: 1,
                }],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn finish_stores_the_error_for_a_failed_run(db: DatabaseHandle) {
        let (log, dispatch_observations) = log(db.clone());
        let run_id = log.start("sample_ingest").await.expect("start run");
        let started_at = row(&db, run_id).await.started_at;
        log.finish(run_id, Err("operation failed".to_string()))
            .await
            .expect("finish failed run");

        let run = row(&db, run_id).await;
        let events = events(&db).await;

        assert_eq!(
            (
                normalize_finished_at(run),
                normalize_events(events),
                read_dispatch_observations(&dispatch_observations),
            ),
            (
                ingest_run::Model {
                    id: run_id,
                    job: "sample_ingest".to_string(),
                    started_at,
                    finished_at: Some(epoch()),
                    status: "failed".to_string(),
                    stats: None,
                    error: Some("operation failed".to_string()),
                },
                vec![event::Model {
                    event_id: uuid::Uuid::nil(),
                    event_type: "ingest_run.failed".to_owned(),
                    payload: json!({
                        "run_id": run_id.to_string(),
                        "job": "sample_ingest",
                        "stats": null,
                        "error": "operation failed",
                    }),
                    chain_depth: 0,
                    occurred_at: epoch(),
                    dispatched_at: None,
                }],
                vec![DispatchObservation {
                    committed: true,
                    event_count: 1,
                }],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn finish_with_stats_stores_partial_stats_for_a_failed_run(db: DatabaseHandle) {
        let (log, dispatch_observations) = log(db.clone());
        let run_id = log
            .start("sample_ingest_with_stats")
            .await
            .expect("start run");
        let started_at = row(&db, run_id).await.started_at;
        let stats = json!({
            "rows_completed": 0,
            "rows_failed": 0,
            "retry_count": 1,
            "error_code": "sample_fetch_error",
        });
        log.finish_with_stats_and_error(run_id, stats.clone(), "sample_fetch_error".to_owned())
            .await
            .expect("finish failed run with stats");

        let run = row(&db, run_id).await;
        let events = events(&db).await;

        assert_eq!(
            (
                normalize_finished_at(run),
                normalize_events(events),
                read_dispatch_observations(&dispatch_observations),
            ),
            (
                ingest_run::Model {
                    id: run_id,
                    job: "sample_ingest_with_stats".to_owned(),
                    started_at,
                    finished_at: Some(epoch()),
                    status: "failed".to_owned(),
                    stats: Some(stats.clone()),
                    error: Some("sample_fetch_error".to_owned()),
                },
                vec![event::Model {
                    event_id: uuid::Uuid::nil(),
                    event_type: "ingest_run.failed".to_owned(),
                    payload: json!({
                        "run_id": run_id.to_string(),
                        "job": "sample_ingest_with_stats",
                        "stats": stats,
                        "error": "sample_fetch_error",
                    }),
                    chain_depth: 0,
                    occurred_at: epoch(),
                    dispatched_at: None,
                }],
                vec![DispatchObservation {
                    committed: true,
                    event_count: 1,
                }],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn finish_keeps_the_event_when_dispatch_enqueue_fails(db: DatabaseHandle) {
        let (log, dispatch_observations) = log_with_dispatch_failure(db.clone(), true);
        let run_id = log.start("sample_ingest").await.expect("start run");
        let started_at = row(&db, run_id).await.started_at;
        let stats = json!({ "rows": 3 });
        log.finish(run_id, Ok(stats.clone()))
            .await
            .expect("finish despite dispatch enqueue failure");

        let run = row(&db, run_id).await;
        let events = events(&db).await;

        assert_eq!(
            (
                normalize_finished_at(run),
                normalize_events(events),
                read_dispatch_observations(&dispatch_observations),
            ),
            (
                ingest_run::Model {
                    id: run_id,
                    job: "sample_ingest".to_string(),
                    started_at,
                    finished_at: Some(epoch()),
                    status: "succeeded".to_string(),
                    stats: Some(stats.clone()),
                    error: None,
                },
                vec![event::Model {
                    event_id: uuid::Uuid::nil(),
                    event_type: "ingest_run.succeeded".to_owned(),
                    payload: json!({
                        "run_id": run_id.to_string(),
                        "job": "sample_ingest",
                        "stats": stats,
                        "error": null,
                    }),
                    chain_depth: 0,
                    occurred_at: epoch(),
                    dispatched_at: None,
                }],
                vec![DispatchObservation {
                    committed: true,
                    event_count: 1,
                }],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn fail_interrupted_before_updates_only_expired_running_rows_for_the_job(
        db: DatabaseHandle,
    ) {
        let (log, dispatch_observations) = log(db.clone());
        let cutoff =
            truncate_to_postgres_precision((Utc::now() - Duration::hours(2)).fixed_offset());
        let expired_id = log.start("sample_ingest").await.expect("start expired run");
        let recent_id = log.start("sample_ingest").await.expect("start recent run");
        let other_job_id = log
            .start("other_ingest")
            .await
            .expect("start other job run");
        set_started_at(&db, expired_id, cutoff - Duration::minutes(1)).await;
        set_started_at(&db, recent_id, cutoff + Duration::minutes(1)).await;
        set_started_at(&db, other_job_id, cutoff - Duration::minutes(1)).await;

        let updated = log
            .fail_interrupted_before("sample_ingest", cutoff)
            .await
            .expect("recover expired run");
        let expired = row(&db, expired_id).await;
        let recent = row(&db, recent_id).await;
        let other_job = row(&db, other_job_id).await;
        let events = events(&db).await;

        assert_eq!(
            (
                updated,
                normalize_finished_at(expired),
                normalize_finished_at(recent),
                normalize_finished_at(other_job),
                normalize_events(events),
                read_dispatch_observations(&dispatch_observations),
            ),
            (
                1,
                ingest_run::Model {
                    id: expired_id,
                    job: "sample_ingest".to_string(),
                    started_at: cutoff - Duration::minutes(1),
                    finished_at: Some(epoch()),
                    status: "failed".to_string(),
                    stats: None,
                    error: Some("interrupted".to_string()),
                },
                ingest_run::Model {
                    id: recent_id,
                    job: "sample_ingest".to_string(),
                    started_at: cutoff + Duration::minutes(1),
                    finished_at: None,
                    status: "running".to_string(),
                    stats: None,
                    error: None,
                },
                ingest_run::Model {
                    id: other_job_id,
                    job: "other_ingest".to_string(),
                    started_at: cutoff - Duration::minutes(1),
                    finished_at: None,
                    status: "running".to_string(),
                    stats: None,
                    error: None,
                },
                Vec::new(),
                Vec::new(),
            ),
        );
    }
}
