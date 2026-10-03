use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Utc};
use core_application::{ingest_run_log::IngestRunLog, persistence::PersistenceError};
use sea_orm::ActiveValue::{Set, Unchanged};
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::Value;
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::ingest_run;
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresIngestRunLog {
    db: DatabaseHandle,
}

impl PostgresIngestRunLog {
    pub fn new(db: impl Into<DatabaseHandle>) -> Self {
        Self { db: db.into() }
    }
}

#[async_trait]
impl IngestRunLog for PostgresIngestRunLog {
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
        run_id: Uuid,
        result: Result<Value, String>,
    ) -> Result<(), PersistenceError> {
        let (status, stats, error) = match result {
            Ok(stats) => ("succeeded", Some(stats), None),
            Err(error) => ("failed", None, Some(error)),
        };
        ingest_run::Entity::update(ingest_run::ActiveModel {
            id: Unchanged(run_id),
            finished_at: Set(Some(Utc::now().fixed_offset())),
            status: Set(status.to_string()),
            stats: Set(stats),
            error: Set(error),
            ..Default::default()
        })
        .exec(&self.db)
        .await
        .map(|_| ())
        .map_err(persistence_error)
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
    use chrono::{Duration, Timelike, Utc};
    use core_application::ingest_run_log::IngestRunLog;
    use sea_orm::ActiveValue::Set;
    use sea_orm::{ConnectionTrait, DatabaseBackend, EntityTrait, Statement};
    use serde_json::json;

    use super::PostgresIngestRunLog;
    use crate::DatabaseHandle;
    use crate::entities::ingest_run;

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
        let log = PostgresIngestRunLog::new(db.clone());
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

        assert_eq!(
            (
                normalize_timestamps(run),
                started_at_matches_transaction_time,
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
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn finish_stores_stats_for_a_successful_run(db: DatabaseHandle) {
        let log = PostgresIngestRunLog::new(db.clone());
        let run_id = log.start("sample_ingest").await.expect("start run");
        let started_at = row(&db, run_id).await.started_at;
        let stats = json!({ "rows": 3 });
        log.finish(run_id, Ok(stats.clone()))
            .await
            .expect("finish successful run");

        let run = row(&db, run_id).await;

        assert_eq!(
            normalize_finished_at(run),
            ingest_run::Model {
                id: run_id,
                job: "sample_ingest".to_string(),
                started_at,
                finished_at: Some(epoch()),
                status: "succeeded".to_string(),
                stats: Some(stats),
                error: None,
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn finish_stores_the_error_for_a_failed_run(db: DatabaseHandle) {
        let log = PostgresIngestRunLog::new(db.clone());
        let run_id = log.start("sample_ingest").await.expect("start run");
        let started_at = row(&db, run_id).await.started_at;
        log.finish(run_id, Err("operation failed".to_string()))
            .await
            .expect("finish failed run");

        let run = row(&db, run_id).await;

        assert_eq!(
            normalize_finished_at(run),
            ingest_run::Model {
                id: run_id,
                job: "sample_ingest".to_string(),
                started_at,
                finished_at: Some(epoch()),
                status: "failed".to_string(),
                stats: None,
                error: Some("operation failed".to_string()),
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn fail_interrupted_before_updates_only_expired_running_rows_for_the_job(
        db: DatabaseHandle,
    ) {
        let log = PostgresIngestRunLog::new(db.clone());
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

        assert_eq!(
            (
                updated,
                normalize_finished_at(expired),
                normalize_finished_at(recent),
                normalize_finished_at(other_job),
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
            ),
        );
    }
}
