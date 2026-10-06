use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, NaiveDate};
use core_application::ingest_status::{
    DAILY_BARS_INGEST_JOB, EARNINGS_SCHEDULE_INGEST_JOB, EODDATA_KOSPI_INGEST_JOB,
    FINANCIAL_SUMMARY_INGEST_JOB, FRED_INGEST_JOB, INGEST_JOBS, IngestRun, IngestRunHistory,
    IngestStatusData, IngestStatusJobDate, IngestStatusRepository, IngestWorkerJob,
    MARGIN_INGEST_JOB, NEWS_AGGREGATION_JOB, SHAREHOLDING_STRUCTURE_INGEST_JOB,
    SHORT_RATIO_INGEST_JOB, SHORT_SALE_REPORT_INGEST_JOB, VALUATION_INGEST_JOB,
};
use core_application::persistence::PersistenceError;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement, Value};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::persistence::persistence_error;

const INGEST_RUNS_SQL: &str = "SELECT DISTINCT ON (job) \
    job, id, started_at, finished_at, status, stats, error, \
    MAX(finished_at) FILTER (WHERE status = 'succeeded') \
        OVER (PARTITION BY job) AS last_succeeded_at \
FROM public.ingest_run \
ORDER BY job, started_at DESC, id DESC";

const LATEST_DATA_DATE_QUERIES: &[(&str, &str)] = &[
    (
        DAILY_BARS_INGEST_JOB,
        "SELECT MAX(date) FROM public.jquants_daily_bars_ingested_date",
    ),
    (
        EARNINGS_SCHEDULE_INGEST_JOB,
        "SELECT MAX(date) FROM public.earnings_schedule_ingested_date",
    ),
    (
        FINANCIAL_SUMMARY_INGEST_JOB,
        "SELECT MAX(disclosure_date) FROM public.financial_summary",
    ),
    (
        VALUATION_INGEST_JOB,
        "SELECT MAX(date) FROM public.valuation_ingested_date",
    ),
    (
        SHAREHOLDING_STRUCTURE_INGEST_JOB,
        "SELECT GREATEST( \
            (SELECT MAX(submitted_on) FROM public.large_volume_shareholding_documents), \
            (SELECT MAX(submitted_on) FROM public.major_shareholder_documents), \
            (SELECT MAX(submitted_on) FROM public.cross_shareholding_documents) \
        )",
    ),
    (
        FRED_INGEST_JOB,
        "SELECT MAX(date) FROM public.indicator_observation \
         WHERE indicator_id IN ('USDJPY', 'VIX', 'US10Y', 'NIKKEI225', 'SP500', 'NASDAQ', 'SOX')",
    ),
    (
        EODDATA_KOSPI_INGEST_JOB,
        "SELECT MAX(date) FROM public.indicator_observation WHERE indicator_id = 'KOSPI'",
    ),
    (
        SHORT_RATIO_INGEST_JOB,
        "SELECT MAX(date) FROM public.short_ratio",
    ),
    (
        SHORT_SALE_REPORT_INGEST_JOB,
        "SELECT MAX(disc_date) FROM public.short_sale_report",
    ),
    (
        MARGIN_INGEST_JOB,
        "SELECT GREATEST( \
            (SELECT MAX(date) FROM public.margin_interest), \
            (SELECT MAX(pub_date) FROM public.margin_alert) \
        )",
    ),
    (
        NEWS_AGGREGATION_JOB,
        "SELECT MAX((published_at AT TIME ZONE 'Asia/Tokyo')::date) FROM public.news_item",
    ),
];

#[derive(Clone)]
pub struct PostgresIngestStatusRepository {
    db: DatabaseHandle,
}

impl PostgresIngestStatusRepository {
    pub fn new(db: impl Into<DatabaseHandle>) -> Self {
        Self { db: db.into() }
    }
}

#[async_trait]
impl IngestStatusRepository for PostgresIngestStatusRepository {
    async fn read(&self) -> Result<IngestStatusData, PersistenceError> {
        let run_histories = read_run_histories(&self.db).await?;
        let latest_data_dates = read_latest_data_dates(&self.db).await?;
        let worker_jobs = read_worker_jobs(&self.db).await?;

        Ok(IngestStatusData {
            run_histories,
            latest_data_dates,
            worker_jobs,
        })
    }
}

async fn read_run_histories(
    db: &DatabaseHandle,
) -> Result<Vec<IngestRunHistory>, PersistenceError> {
    db.query_all_raw(Statement::from_string(
        DatabaseBackend::Postgres,
        INGEST_RUNS_SQL,
    ))
    .await
    .map_err(persistence_error)?
    .into_iter()
    .map(|row| {
        Ok(IngestRunHistory {
            job: row.try_get("", "job").map_err(persistence_error)?,
            latest_run: IngestRun {
                id: row.try_get::<Uuid>("", "id").map_err(persistence_error)?,
                started_at: row
                    .try_get::<DateTime<FixedOffset>>("", "started_at")
                    .map_err(persistence_error)?,
                finished_at: row
                    .try_get::<Option<DateTime<FixedOffset>>>("", "finished_at")
                    .map_err(persistence_error)?,
                status: row.try_get("", "status").map_err(persistence_error)?,
                stats: row
                    .try_get::<Option<JsonValue>>("", "stats")
                    .map_err(persistence_error)?,
                error: row
                    .try_get::<Option<String>>("", "error")
                    .map_err(persistence_error)?,
            },
            last_succeeded_at: row
                .try_get::<Option<DateTime<FixedOffset>>>("", "last_succeeded_at")
                .map_err(persistence_error)?,
        })
    })
    .collect()
}

async fn read_latest_data_dates(
    db: &DatabaseHandle,
) -> Result<Vec<IngestStatusJobDate>, PersistenceError> {
    db.query_all_raw(latest_data_date_statement())
        .await
        .map_err(persistence_error)?
        .into_iter()
        .map(|row| {
            Ok(IngestStatusJobDate {
                job: row.try_get("", "job").map_err(persistence_error)?,
                latest_data_date: row
                    .try_get::<Option<NaiveDate>>("", "latest_data_date")
                    .map_err(persistence_error)?,
            })
        })
        .collect()
}

fn latest_data_date_statement() -> Statement {
    let sql = LATEST_DATA_DATE_QUERIES
        .iter()
        .enumerate()
        .map(|(index, (_, query))| {
            format!(
                "SELECT ${}::text AS job, ({query}) AS latest_data_date",
                index + 1
            )
        })
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    let values = LATEST_DATA_DATE_QUERIES
        .iter()
        .map(|(job, _)| (*job).to_owned().into())
        .collect::<Vec<Value>>();

    Statement::from_sql_and_values(DatabaseBackend::Postgres, sql, values)
}

async fn read_worker_jobs(db: &DatabaseHandle) -> Result<Vec<IngestWorkerJob>, PersistenceError> {
    let placeholders = (1..=INGEST_JOBS.len())
        .map(|index| format!("${index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT id, task_identifier, \
            CASE \
                WHEN locked_at IS NOT NULL THEN 'running' \
                WHEN attempts >= max_attempts THEN 'failed' \
                ELSE 'waiting' \
            END AS state, \
            queue_name, run_at, attempts::int AS attempts, \
            max_attempts::int AS max_attempts, last_error \
         FROM graphile_worker.jobs \
         WHERE task_identifier IN ({placeholders}) \
         ORDER BY task_identifier, run_at, id"
    );
    let values: Vec<Value> = INGEST_JOBS
        .iter()
        .map(|definition| definition.identifier.to_owned().into())
        .collect();
    let statement = Statement::from_sql_and_values(DatabaseBackend::Postgres, sql, values);

    db.query_all_raw(statement)
        .await
        .map_err(persistence_error)?
        .into_iter()
        .map(|row| {
            Ok(IngestWorkerJob {
                id: row.try_get("", "id").map_err(persistence_error)?,
                job: row
                    .try_get("", "task_identifier")
                    .map_err(persistence_error)?,
                state: row.try_get("", "state").map_err(persistence_error)?,
                queue_name: row
                    .try_get::<Option<String>>("", "queue_name")
                    .map_err(persistence_error)?,
                run_at: row
                    .try_get::<DateTime<FixedOffset>>("", "run_at")
                    .map_err(persistence_error)?,
                attempts: row.try_get("", "attempts").map_err(persistence_error)?,
                max_attempts: row.try_get("", "max_attempts").map_err(persistence_error)?,
                last_error: row
                    .try_get::<Option<String>>("", "last_error")
                    .map_err(persistence_error)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use core_application::ingest_status::INGEST_JOBS;

    use super::LATEST_DATA_DATE_QUERIES;

    #[test]
    fn data_date_queries_cover_every_job_with_a_data_date() {
        let jobs_with_data_dates = INGEST_JOBS
            .iter()
            .filter(|definition| definition.has_data_date)
            .map(|definition| definition.identifier)
            .collect::<BTreeSet<_>>();
        let queried_jobs = LATEST_DATA_DATE_QUERIES
            .iter()
            .map(|(job, _)| *job)
            .collect::<BTreeSet<_>>();

        assert_eq!(queried_jobs, jobs_with_data_dates);
    }
}
