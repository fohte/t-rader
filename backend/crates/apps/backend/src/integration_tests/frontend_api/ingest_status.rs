#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset, NaiveDate};
    use core_application::indicator_observation::{
        IndicatorObservationMetadata, IndicatorObservationRepository,
    };
    use core_domain::IndicatorObservation;
    use rust_decimal::Decimal;
    use sea_orm::ActiveValue::Set;
    use sea_orm::EntityTrait;
    use serde_json::{Value, json};
    use uuid::Uuid;

    use crate::testing::create_test_server_with_db;
    use gateway_postgres::PostgresIndicatorObservationRepository;
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
        }
        response
    }

    async fn seed_indicator_observations(db: &gateway_postgres::DatabaseHandle) {
        let repository = PostgresIndicatorObservationRepository::new(db.clone());
        for (indicator_id, name, observations) in [
            (
                "KOSPI",
                "韓国総合株価指数",
                vec![
                    IndicatorObservation {
                        date: date(2030, 6, 6),
                        value: Decimal::from(100),
                    },
                    IndicatorObservation {
                        date: date(2030, 6, 7),
                        value: Decimal::from(101),
                    },
                ],
            ),
            (
                "TEST_OTHER",
                "架空指標",
                vec![IndicatorObservation {
                    date: date(2030, 6, 9),
                    value: Decimal::from(200),
                }],
            ),
        ] {
            repository
                .ensure_indicator(IndicatorObservationMetadata {
                    indicator_id: indicator_id.to_string(),
                    name: name.to_string(),
                    kind: "index".to_string(),
                })
                .await
                .expect("seed indicator");
            repository
                .upsert_observations(indicator_id, observations)
                .await
                .expect("seed observations");
        }
    }

    #[backend_test_macros::database_test]
    async fn get_returns_latest_run_history_and_data_dates(db: gateway_postgres::DatabaseHandle) {
        let (_, server) = create_test_server_with_db(db.clone()).await;
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
        seed_indicator_observations(&db).await;
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
                            "expected_data_date": "<expected-data-date>"
                        },
                        {
                            "job": "earnings_schedule_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": "2030-06-04",
                            "expected_data_date": "<expected-data-date>"
                        },
                        {
                            "job": "financial_summary_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": "<expected-data-date>"
                        },
                        {
                            "job": "valuation_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": "<expected-data-date>"
                        },
                        {
                            "job": "equity_master_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": null
                        },
                        {
                            "job": "us_stock_master_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": null
                        },
                        {
                            "job": "shareholding_structure_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": "<expected-data-date>"
                        },
                        {
                            "job": "fred_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": "<expected-data-date>"
                        },
                        {
                            "job": "eoddata_kospi_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": "2030-06-07",
                            "expected_data_date": "<expected-data-date>"
                        },
                        {
                            "job": "twse_index_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": null
                        },
                        {
                            "job": "fred_release_dates_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": null
                        },
                        {
                            "job": "e_stat_calendar_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": null
                        },
                        {
                            "job": "alpha_vantage_calendar_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": null
                        },
                        {
                            "job": "boj_calendar_event_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": null
                        },
                        {
                            "job": "ecb_calendar_event_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": null
                        },
                        {
                            "job": "fed_calendar_event_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": null
                        },
                        {
                            "job": "short_ratio_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": "<expected-data-date>"
                        },
                        {
                            "job": "short_sale_report_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": "<expected-data-date>"
                        },
                        {
                            "job": "margin_ingest",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": "<expected-data-date>"
                        },
                        {
                            "job": "news_aggregation",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": "<expected-data-date>"
                        },
                        {
                            "job": "news_content_fetch",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": null
                        },
                        {
                            "job": "prediction_grading",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": null
                        },
                        {
                            "job": "paper_order_filling",
                            "last_run": null,
                            "last_succeeded_at": null,
                            "latest_data_date": null,
                            "expected_data_date": null
                        }
                    ]
                }),
            ),
        );
    }
}
