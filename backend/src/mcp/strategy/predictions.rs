//! 予測 (prediction) の記録 / 読み取り tool。
//!
//! 記録後の確率・期限・対象の書き換えは採点を無意味にするため、更新・削除の tool は
//! 意図的に用意しない。読み取りは自戦略の予測に限る。

use core_application::persistence::PersistenceError;
use core_application::prediction::{
    PredictionListQuery, PredictionUseCaseError, RecordPredictionCommand,
};
use core_application::strategy_scope::StrategyScope;
use core_application::unit_of_work::UnitOfWorkError;
use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;

use super::dto::{
    ListPredictionsParams, ListPredictionsResult, PredictionDto, RecordPredictionParams,
    RecordPredictionResult,
};
use super::{StrategyServer, clamp_limit, decimal_to_f64, internal_error, invalid_params};

fn prediction_error_to_mcp(error: PredictionUseCaseError) -> McpError {
    match error {
        PredictionUseCaseError::Validation(message) => invalid_params(message),
        PredictionUseCaseError::NoteNotFound(_) => {
            McpError::resource_not_found("note not found", None)
        }
        PredictionUseCaseError::Forbidden(note_id) => invalid_params(format!(
            "forbidden: note {note_id} belongs to another strategy"
        )),
        PredictionUseCaseError::Repository(
            core_application::prediction::PredictionRepositoryError::Database(error),
        ) => prediction_database_error(error),
        PredictionUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | PredictionUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error)) => {
            prediction_database_error(error)
        }
        PredictionUseCaseError::Repository(
            core_application::prediction::PredictionRepositoryError::InvalidTransaction,
        )
        | PredictionUseCaseError::UnitOfWork(UnitOfWorkError::InvalidTransaction) => {
            internal_error("prediction transaction has an unexpected type")
        }
    }
}

fn prediction_database_error(error: PersistenceError) -> McpError {
    tracing::error!(error = %error, "strategy mcp prediction operation failed");
    internal_error(format!("database error: {error}"))
}

fn f64_to_decimal(v: f64) -> Result<Decimal, McpError> {
    Decimal::try_from(v).map_err(|err| invalid_params(format!("invalid decimal value: {err}")))
}

fn prediction_to_dto(m: core_application::prediction::Prediction) -> PredictionDto {
    PredictionDto {
        prediction_id: m.id,
        strategy_id: m.strategy_id,
        note_id: m.note_id,
        target_stock_id: m.target_stock_id,
        benchmark_stock_id: m.benchmark_stock_id,
        direction: m.direction,
        probability: decimal_to_f64(m.probability),
        base_date: m.base_date,
        due_date: m.due_date,
        created_at: m.created_at,
    }
}

impl StrategyServer {
    pub(crate) async fn record_prediction_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: RecordPredictionParams,
    ) -> Result<RecordPredictionResult, McpError> {
        let probability = f64_to_decimal(params.probability)?;
        let created = self
            .dependencies
            .predictions
            .record(
                scope.into(),
                RecordPredictionCommand {
                    note_id: params.note_id,
                    target_stock_id: params.target_stock_id,
                    benchmark_stock_id: params.benchmark_stock_id,
                    direction: params.direction,
                    probability,
                    base_date: params.base_date,
                    due_date: params.due_date,
                },
            )
            .await
            .map_err(prediction_error_to_mcp)?;
        Ok(RecordPredictionResult {
            prediction: prediction_to_dto(created),
        })
    }

    pub(crate) async fn list_predictions_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: ListPredictionsParams,
    ) -> Result<ListPredictionsResult, McpError> {
        let rows = self
            .dependencies
            .predictions
            .list_by_strategy(
                scope.into(),
                PredictionListQuery {
                    due_after: params.due_after,
                    due_before: params.due_before,
                    limit: clamp_limit(params.limit),
                },
            )
            .await
            .map_err(prediction_error_to_mcp)?;
        Ok(ListPredictionsResult {
            predictions: rows.into_iter().map(prediction_to_dto).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::insert_test_stock;
    use chrono::NaiveDate;

    use super::super::dto::{ListPredictionsParams, PredictionDto, RecordPredictionParams};
    use super::super::tests_common::{
        build_server, insert_strategy, seed_foreign_note, ts_sentinel,
    };

    fn normalize_prediction(mut p: PredictionDto) -> PredictionDto {
        p.created_at = ts_sentinel();
        p
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
            .record_prediction_inner(strategy_id, base_params("TGT1", "BM1"))
            .await
            .expect("record_prediction");

        let expected = PredictionDto {
            prediction_id: result.prediction.prediction_id,
            strategy_id,
            note_id: None,
            target_stock_id: "TGT1".into(),
            benchmark_stock_id: "BM1".into(),
            direction: "outperform".into(),
            probability: 0.65,
            base_date: NaiveDate::from_ymd_opt(2026, 6, 1).expect("date"),
            due_date: NaiveDate::from_ymd_opt(2026, 7, 1).expect("date"),
            created_at: ts_sentinel(),
        };
        assert_eq!(normalize_prediction(result.prediction), expected);
    }

    #[backend_test_macros::database_test]
    async fn record_prediction_rejects_same_target_and_benchmark(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "a").await;
        insert_test_stock(&db, "TGT1", "Target").await;
        let server = build_server(db);

        let err = server
            .record_prediction_inner(strategy_id, base_params("TGT1", "TGT1"))
            .await
            .expect_err("same target and benchmark expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
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
            .record_prediction_inner(strategy_id, params)
            .await
            .expect_err("invalid direction expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
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
            .record_prediction_inner(strategy_id, params)
            .await
            .expect_err("invalid probability expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
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
            .record_prediction_inner(strategy_id, params)
            .await
            .expect_err("due_date not after base_date expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn record_prediction_rejects_missing_target_stock(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "a").await;
        insert_test_stock(&db, "BM1", "Benchmark").await;
        let server = build_server(db);

        let err = server
            .record_prediction_inner(strategy_id, base_params("MISSING", "BM1"))
            .await
            .expect_err("missing target stock expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn record_prediction_rejects_missing_benchmark_stock(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "a").await;
        insert_test_stock(&db, "TGT1", "Target").await;
        let server = build_server(db);

        let err = server
            .record_prediction_inner(strategy_id, base_params("TGT1", "MISSING"))
            .await
            .expect_err("missing benchmark stock expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn record_prediction_rejects_cross_strategy_linked_note(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        insert_test_stock(&db, "TGT1", "Target").await;
        insert_test_stock(&db, "BM1", "Benchmark").await;
        let foreign_note = seed_foreign_note(&db, strategy_b, "b's note").await;
        let server = build_server(db);

        let mut params = base_params("TGT1", "BM1");
        params.note_id = Some(foreign_note);
        let err = server
            .record_prediction_inner(strategy_a, params)
            .await
            .expect_err("cross-strategy linked note expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
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
            .record_prediction_inner(strategy_a, base_params("TGT1", "BM1"))
            .await
            .expect("record own prediction")
            .prediction;
        server
            .record_prediction_inner(strategy_b, base_params("TGT1", "BM1"))
            .await
            .expect("record other strategy prediction");

        let result = server
            .list_predictions_inner(
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
            result
                .predictions
                .into_iter()
                .map(normalize_prediction)
                .collect::<Vec<_>>(),
            vec![normalize_prediction(own)],
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
            .record_prediction_inner(strategy_id, early)
            .await
            .expect("record early prediction")
            .prediction;

        let mut late = base_params("TGT1", "BM1");
        late.due_date = NaiveDate::from_ymd_opt(2026, 8, 1).expect("date");
        server
            .record_prediction_inner(strategy_id, late)
            .await
            .expect("record late prediction");

        let result = server
            .list_predictions_inner(
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
            result
                .predictions
                .into_iter()
                .map(normalize_prediction)
                .collect::<Vec<_>>(),
            vec![normalize_prediction(early)],
        );
    }
}
