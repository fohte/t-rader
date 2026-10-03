#[cfg(test)]
mod tests {
    use crate::testing::insert_test_stock;
    use chrono::NaiveDate;

    use super::super::ToolOutput;
    use super::super::dto::{ListPredictionsParams, PredictionDto, RecordPredictionParams};
    use super::super::tests_common::{build_server, insert_strategy, seed_note, ts_sentinel};

    fn normalize_prediction(mut p: PredictionDto) -> PredictionDto {
        p.prediction_id = uuid::Uuid::nil();
        p.created_at = ts_sentinel();
        p
    }

    fn normalize_record_result(
        result: ToolOutput<super::super::dto::RecordPredictionResult>,
    ) -> ToolOutput<super::super::dto::RecordPredictionResult> {
        result.normalize_json(|value| {
            value["prediction"]["prediction_id"] = serde_json::json!(uuid::Uuid::nil());
            value["prediction"]["created_at"] =
                serde_json::to_value(ts_sentinel()).expect("serialize timestamp");
        })
    }

    fn normalize_list_result(
        result: ToolOutput<super::super::dto::ListPredictionsResult>,
    ) -> ToolOutput<super::super::dto::ListPredictionsResult> {
        result.normalize_json(|value| {
            for prediction in value["predictions"]
                .as_array_mut()
                .expect("predictions are an array")
            {
                prediction["prediction_id"] = serde_json::json!(uuid::Uuid::nil());
                prediction["created_at"] =
                    serde_json::to_value(ts_sentinel()).expect("serialize timestamp");
            }
        })
    }

    fn base_params(target: &str, benchmark: &str) -> RecordPredictionParams {
        RecordPredictionParams {
            note_id: None,
            target_stock_id: target.into(),
            benchmark_stock_id: benchmark.into(),
            direction: "outperform".into(),
            probability: 0.65,
            base_date: NaiveDate::from_ymd_opt(2026, 6, 1).expect("date"),
            due_date: NaiveDate::from_ymd_opt(2026, 7, 1).expect("date"),
        }
    }

    #[backend_test_macros::database_test]
    async fn record_prediction_creates_prediction_with_given_values(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "a").await;
        insert_test_stock(&db, "TGT1", "Target").await;
        insert_test_stock(&db, "BM1", "Benchmark").await;
        let server = build_server(db);

        let result = server
            .record_prediction(strategy_id, base_params("TGT1", "BM1"))
            .await
            .expect("record_prediction");

        assert_eq!(
            normalize_record_result(result),
            super::super::dto::RecordPredictionResult {
                prediction: PredictionDto {
                    prediction_id: uuid::Uuid::nil(),
                    strategy_id,
                    note_id: None,
                    target_stock_id: "TGT1".into(),
                    benchmark_stock_id: "BM1".into(),
                    direction: "outperform".into(),
                    probability: 0.65,
                    base_date: NaiveDate::from_ymd_opt(2026, 6, 1).expect("date"),
                    due_date: NaiveDate::from_ymd_opt(2026, 7, 1).expect("date"),
                    created_at: ts_sentinel(),
                },
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn record_prediction_rejects_same_target_and_benchmark(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "a").await;
        insert_test_stock(&db, "TGT1", "Target").await;
        let server = build_server(db);

        let err = server
            .record_prediction(strategy_id, base_params("TGT1", "TGT1"))
            .await
            .expect_err("same target and benchmark expected to be rejected");
        assert_eq!(
            err,
            rmcp::ErrorData::invalid_params(
                "target_stock_id and benchmark_stock_id must differ",
                None,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn record_prediction_rejects_invalid_direction(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "a").await;
        insert_test_stock(&db, "TGT1", "Target").await;
        insert_test_stock(&db, "BM1", "Benchmark").await;
        let server = build_server(db);

        let mut params = base_params("TGT1", "BM1");
        params.direction = "bogus".into();
        let err = server
            .record_prediction(strategy_id, params)
            .await
            .expect_err("invalid direction expected to be rejected");
        assert_eq!(
            err,
            rmcp::ErrorData::invalid_params("invalid direction: bogus", None),
        );
    }

    #[backend_test_macros::database_test]
    async fn record_prediction_rejects_invalid_probability(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "a").await;
        insert_test_stock(&db, "TGT1", "Target").await;
        insert_test_stock(&db, "BM1", "Benchmark").await;
        let server = build_server(db);

        let mut params = base_params("TGT1", "BM1");
        params.probability = 0.5;
        let err = server
            .record_prediction(strategy_id, params)
            .await
            .expect_err("invalid probability expected to be rejected");
        assert_eq!(
            err,
            rmcp::ErrorData::invalid_params("invalid probability: 0.5", None),
        );
    }

    #[backend_test_macros::database_test]
    async fn record_prediction_rejects_due_date_not_after_base_date(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "a").await;
        insert_test_stock(&db, "TGT1", "Target").await;
        insert_test_stock(&db, "BM1", "Benchmark").await;
        let server = build_server(db);

        let mut params = base_params("TGT1", "BM1");
        params.due_date = params.base_date;
        let err = server
            .record_prediction(strategy_id, params)
            .await
            .expect_err("due_date not after base_date expected to be rejected");
        assert_eq!(
            err,
            rmcp::ErrorData::invalid_params("due_date must be after base_date", None),
        );
    }

    #[backend_test_macros::database_test]
    async fn record_prediction_rejects_missing_target_stock(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "a").await;
        insert_test_stock(&db, "BM1", "Benchmark").await;
        let server = build_server(db);

        let err = server
            .record_prediction(strategy_id, base_params("MISSING", "BM1"))
            .await
            .expect_err("missing target stock expected to be rejected");
        assert_eq!(
            err,
            rmcp::ErrorData::invalid_params("stock MISSING not found", None),
        );
    }

    #[backend_test_macros::database_test]
    async fn record_prediction_rejects_missing_benchmark_stock(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "a").await;
        insert_test_stock(&db, "TGT1", "Target").await;
        let server = build_server(db);

        let err = server
            .record_prediction(strategy_id, base_params("TGT1", "MISSING"))
            .await
            .expect_err("missing benchmark stock expected to be rejected");
        assert_eq!(
            err,
            rmcp::ErrorData::invalid_params("stock MISSING not found", None),
        );
    }

    #[backend_test_macros::database_test]
    async fn record_prediction_links_note_without_strategy_ownership(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_a = insert_strategy(&db, "a").await;
        insert_test_stock(&db, "TGT1", "Target").await;
        insert_test_stock(&db, "BM1", "Benchmark").await;
        let foreign_note = seed_note(&db, "sample note").await;
        let server = build_server(db);

        let mut params = base_params("TGT1", "BM1");
        params.note_id = Some(foreign_note);
        let result = server
            .record_prediction(strategy_a, params)
            .await
            .expect("record prediction with note from another strategy");
        assert_eq!(
            normalize_record_result(result),
            super::super::dto::RecordPredictionResult {
                prediction: PredictionDto {
                    prediction_id: uuid::Uuid::nil(),
                    strategy_id: strategy_a,
                    note_id: Some(foreign_note),
                    target_stock_id: "TGT1".into(),
                    benchmark_stock_id: "BM1".into(),
                    direction: "outperform".into(),
                    probability: 0.65,
                    base_date: NaiveDate::from_ymd_opt(2026, 6, 1).expect("date"),
                    due_date: NaiveDate::from_ymd_opt(2026, 7, 1).expect("date"),
                    created_at: ts_sentinel(),
                },
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn list_predictions_returns_only_own_strategy_predictions(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        insert_test_stock(&db, "TGT1", "Target").await;
        insert_test_stock(&db, "BM1", "Benchmark").await;
        let server = build_server(db);

        let own = server
            .record_prediction(strategy_a, base_params("TGT1", "BM1"))
            .await
            .expect("record own prediction")
            .into_value()
            .prediction;
        server
            .record_prediction(strategy_b, base_params("TGT1", "BM1"))
            .await
            .expect("record other strategy prediction");

        let result = server
            .list_predictions(
                strategy_a,
                ListPredictionsParams {
                    limit: None,
                    due_after: None,
                    due_before: None,
                },
            )
            .await
            .expect("list_predictions");

        assert_eq!(
            normalize_list_result(result),
            super::super::dto::ListPredictionsResult {
                predictions: vec![normalize_prediction(own)],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn list_predictions_filters_by_due_date_range(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "a").await;
        insert_test_stock(&db, "TGT1", "Target").await;
        insert_test_stock(&db, "BM1", "Benchmark").await;
        let server = build_server(db);

        let mut early = base_params("TGT1", "BM1");
        early.due_date = NaiveDate::from_ymd_opt(2026, 6, 15).expect("date");
        let early = server
            .record_prediction(strategy_id, early)
            .await
            .expect("record early prediction")
            .into_value()
            .prediction;

        let mut late = base_params("TGT1", "BM1");
        late.due_date = NaiveDate::from_ymd_opt(2026, 8, 1).expect("date");
        server
            .record_prediction(strategy_id, late)
            .await
            .expect("record late prediction");

        let result = server
            .list_predictions(
                strategy_id,
                ListPredictionsParams {
                    limit: None,
                    due_after: Some(NaiveDate::from_ymd_opt(2026, 6, 10).expect("date")),
                    due_before: Some(NaiveDate::from_ymd_opt(2026, 6, 20).expect("date")),
                },
            )
            .await
            .expect("list_predictions");

        assert_eq!(
            normalize_list_result(result),
            super::super::dto::ListPredictionsResult {
                predictions: vec![normalize_prediction(early)],
            },
        );
    }
}
