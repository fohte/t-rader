#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset, NaiveDate};
    use sea_orm::ActiveValue::Set;
    use sea_orm::{ConnectionTrait, DatabaseBackend, EntityTrait, Statement};
    use serde_json::{Value, json};
    use uuid::Uuid;

    use crate::testing::create_test_server_with_graphile_worker;
    use gateway_postgres::entities::{
        earnings_schedule_ingested_date, ingest_run, jquants_daily_bars_ingested_date,
    };

    fn timestamp(value: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(value).expect("valid fixture timestamp")
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid fixture date")
    }

    fn normalize(mut response: Value) -> Value {
        for job in response["jobs"].as_array_mut().expect("jobs array") {
            if let Some(run) = job["last_run"].as_object_mut() {
                run.insert("id".into(), Value::String("<run-id>".into()));
                run.insert("started_at".into(), Value::String("<started-at>".into()));
                run.insert("finished_at".into(), Value::String("<finished-at>".into()));
            }
            if !job["expected_data_date"].is_null() {
                job["expected_data_date"] = Value::String("<expected-data-date>".into());
            }
            for worker_job in job["worker_jobs"]
                .as_array_mut()
                .expect("worker jobs array")
            {
                worker_job["id"] = json!(-1);
                worker_job["run_at"] = Value::String("<run-at>".into());
            }
        }
        response
    }

    async fn enqueue_job(
        db: &gateway_postgres::DatabaseHandle,
        run_at: DateTime<FixedOffset>,
    ) -> i64 {
        let rows = db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT id FROM graphile_worker.add_job(\
                    $1::text, '{}'::json, $2::text, $3::timestamptz, $4::int\
                )",
                [
                    "daily_bars_ingest".to_string().into(),
                    "sample-queue".to_string().into(),
                    run_at.into(),
                    3_i32.into(),
                ],
            ))
            .await
            .expect("enqueue graphile job");
        rows[0].try_get("", "id").expect("graphile job id")
    }

    async fn mark_job_failed(db: &gateway_postgres::DatabaseHandle, job_id: i64) {
        db.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE graphile_worker._private_jobs \
             SET attempts = max_attempts, last_error = 'sample queue failure' \
             WHERE id = $1",
            [job_id.into()],
        ))
        .await
        .expect("mark graphile job failed");
    }

    async fn mark_job_running(
        db: &gateway_postgres::DatabaseHandle,
        job_id: i64,
        locked_at: DateTime<FixedOffset>,
    ) {
        db.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE graphile_worker._private_jobs SET locked_at = $2 WHERE id = $1",
            [job_id.into(), locked_at.into()],
        ))
        .await
        .expect("mark graphile job running");
    }

    #[backend_test_macros::database_test]
    async fn get_returns_latest_run_data_date_and_graphile_queue_states(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server_with_graphile_worker(db.clone()).await;
        let earlier_id = Uuid::from_u128(1);
        let latest_id = Uuid::from_u128(2);
        let earlier_started_at = timestamp("2030-06-05T12:00:00Z");
        let earlier_finished_at = timestamp("2030-06-05T12:01:00Z");
        let latest_started_at = timestamp("2030-06-06T12:00:00Z");
        let latest_finished_at = timestamp("2030-06-06T12:01:00Z");

        ingest_run::Entity::insert_many([
            ingest_run::ActiveModel {
                id: Set(earlier_id),
                job: Set("daily_bars_ingest".into()),
                started_at: Set(earlier_started_at),
                finished_at: Set(Some(earlier_finished_at)),
                status: Set("succeeded".into()),
                stats: Set(Some(json!({"rows": 2}))),
                ..Default::default()
            },
            ingest_run::ActiveModel {
                id: Set(latest_id),
                job: Set("daily_bars_ingest".into()),
                started_at: Set(latest_started_at),
                finished_at: Set(Some(latest_finished_at)),
                status: Set("failed".into()),
                error: Set(Some("sample ingest failure".into())),
                ..Default::default()
            },
        ])
        .exec(&db)
        .await
        .expect("insert ingest run history");
        jquants_daily_bars_ingested_date::Entity::insert_many([
            jquants_daily_bars_ingested_date::ActiveModel {
                date: Set(date(2030, 6, 5)),
            },
            jquants_daily_bars_ingested_date::ActiveModel {
                date: Set(date(2030, 6, 6)),
            },
        ])
        .exec(&db)
        .await
        .expect("insert daily bar ingested dates");
        earnings_schedule_ingested_date::Entity::insert(
            earnings_schedule_ingested_date::ActiveModel {
                date: Set(date(2030, 6, 4)),
            },
        )
        .exec(&db)
        .await
        .expect("insert earnings schedule date");
        let _waiting_id = enqueue_job(&db, timestamp("2030-06-06T12:02:00Z")).await;
        let failed_id = enqueue_job(&db, timestamp("2030-06-06T12:03:00Z")).await;
        mark_job_failed(&db, failed_id).await;
        let running_id = enqueue_job(&db, timestamp("2030-06-06T12:04:00Z")).await;
        mark_job_running(&db, running_id, timestamp("2030-06-06T12:04:00Z")).await;

        let response = server.get("/api/ingest-status").await;
        let actual = (response.status_code(), normalize(response.json::<Value>()));
        let expected_last_succeeded_at =
            serde_json::to_value(earlier_finished_at).expect("serialize expected timestamp");

        assert_eq!(
            actual,
            (
                axum::http::StatusCode::OK,
                json!({
                    "jobs": [
                        {
                            "job": "daily_bars_ingest",
                            "last_run": {
                                "id": "<run-id>",
                                "started_at": "<started-at>",
                                "finished_at": "<finished-at>",
                                "status": "failed",
                                "stats": null,
                                "error": "sample ingest failure"
                            },
                            "last_succeeded_at": expected_last_succeeded_at,
                            "latest_data_date": "2030-06-06",
                            "expected_data_date": "<expected-data-date>",
                            "worker_jobs": [
                                {
                                    "id": -1,
                                    "task_identifier": "daily_bars_ingest",
                                    "state": "waiting",
                                    "queue_name": "sample-queue",
                                    "run_at": "<run-at>",
                                    "attempts": 0,
                                    "max_attempts": 3,
                                    "last_error": null
                                },
                                {
                                    "id": -1,
                                    "task_identifier": "daily_bars_ingest",
                                    "state": "failed",
                                    "queue_name": "sample-queue",
                                    "run_at": "<run-at>",
                                    "attempts": 3,
                                    "max_attempts": 3,
                                    "last_error": "sample queue failure"
                                },
                                {
                                    "id": -1,
                                    "task_identifier": "daily_bars_ingest",
                                    "state": "running",
                                    "queue_name": "sample-queue",
                                    "run_at": "<run-at>",
                                    "attempts": 0,
                                    "max_attempts": 3,
                                    "last_error": null
                                }
                            ]
                        },
                        {
                            "job": "earnings_schedule_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": "2030-06-04",
                            "expected_data_date": "<expected-data-date>",
                            "worker_jobs": []
                        },
                        {
                            "job": "financial_summary_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": "<expected-data-date>",
                            "worker_jobs": []
                        },
                        {
                            "job": "valuation_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": "<expected-data-date>",
                            "worker_jobs": []
                        },
                        {
                            "job": "equity_master_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": null,
                            "worker_jobs": []
                        },
                        {
                            "job": "shareholding_structure_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": "<expected-data-date>",
                            "worker_jobs": []
                        },
                        {
                            "job": "fred_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": "<expected-data-date>",
                            "worker_jobs": []
                        },
                        {
                            "job": "fred_release_dates_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": null,
                            "worker_jobs": []
                        },
                        {
                            "job": "alpha_vantage_calendar_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": null,
                            "worker_jobs": []
                        },
                        {
                            "job": "short_ratio_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": "<expected-data-date>",
                            "worker_jobs": []
                        },
                        {
                            "job": "short_sale_report_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": "<expected-data-date>",
                            "worker_jobs": []
                        },
                        {
                            "job": "margin_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": "<expected-data-date>",
                            "worker_jobs": []
                        },
                        {
                            "job": "news_aggregation",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": "<expected-data-date>",
                            "worker_jobs": []
                        },
                        {
                            "job": "news_content_fetch",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": null,
                            "worker_jobs": []
                        },
                        {
                            "job": "prediction_grading",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": null,
                            "worker_jobs": []
                        }
                    ]
                }),
            ),
        );
    }
}
