use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Utc};
use core_application::{ingest_run_log::IngestRunLog, persistence::PersistenceError};
use sea_orm::ActiveValue::{NotSet, Set};
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
            id: Set(run_id),
            finished_at: Set(Some(Utc::now().fixed_offset())),
            status: Set(status.to_string()),
            stats: Set(stats),
            error: Set(error),
            job: NotSet,
            started_at: NotSet,
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
    use chrono::{Duration, Utc};
    use core_application::ingest_run_log::IngestRunLog;
    use sea_orm::ActiveValue::Set;
    use sea_orm::EntityTrait;
    use serde_json::json;

    use super::PostgresIngestRunLog;
    use crate::DatabaseHandle;
    use crate::entities::ingest_run;

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
        let before = (Utc::now() - Duration::seconds(2)).fixed_offset();
        let run_id = log.start("sample_ingest").await.expect("start run");
        let after = (Utc::now() + Duration::seconds(2)).fixed_offset();

        let run = row(&db, run_id).await;

        assert_eq!(
            (
                run.id,
                run.job,
                run.started_at >= before && run.started_at <= after,
                run.finished_at.is_some(),
                run.status,
                run.stats,
                run.error,
            ),
            (
                run_id,
                "sample_ingest".to_string(),
                true,
                false,
                "running".to_string(),
                None,
                None,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn finish_stores_stats_for_a_successful_run(db: DatabaseHandle) {
        let log = PostgresIngestRunLog::new(db.clone());
        let run_id = log.start("sample_ingest").await.expect("start run");
        let stats = json!({ "rows": 3 });
        log.finish(run_id, Ok(stats.clone()))
            .await
            .expect("finish successful run");

        let run = row(&db, run_id).await;

        assert_eq!(
            (
                run.id,
                run.job,
                run.status,
                run.stats,
                run.error,
                run.finished_at.is_some()
            ),
            (
                run_id,
                "sample_ingest".to_string(),
                "succeeded".to_string(),
                Some(stats),
                None,
                true,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn finish_stores_the_error_for_a_failed_run(db: DatabaseHandle) {
        let log = PostgresIngestRunLog::new(db.clone());
        let run_id = log.start("sample_ingest").await.expect("start run");
        log.finish(run_id, Err("operation failed".to_string()))
            .await
            .expect("finish failed run");

        let run = row(&db, run_id).await;

        assert_eq!(
            (
                run.id,
                run.job,
                run.status,
                run.stats,
                run.error,
                run.finished_at.is_some()
            ),
            (
                run_id,
                "sample_ingest".to_string(),
                "failed".to_string(),
                None,
                Some("operation failed".to_string()),
                true,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn fail_interrupted_before_updates_only_expired_running_rows_for_the_job(
        db: DatabaseHandle,
    ) {
        let log = PostgresIngestRunLog::new(db.clone());
        let cutoff = (Utc::now() - Duration::hours(2)).fixed_offset();
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
                (
                    expired.status,
                    expired.finished_at.is_some(),
                    expired.stats,
                    expired.error
                ),
                (
                    recent.status,
                    recent.finished_at.is_some(),
                    recent.stats,
                    recent.error
                ),
                (
                    other_job.status,
                    other_job.finished_at.is_some(),
                    other_job.stats,
                    other_job.error,
                ),
            ),
            (
                1,
                (
                    "failed".to_string(),
                    true,
                    None,
                    Some("interrupted".to_string()),
                ),
                ("running".to_string(), false, None, None),
                ("running".to_string(), false, None, None),
            ),
        );
    }
}
