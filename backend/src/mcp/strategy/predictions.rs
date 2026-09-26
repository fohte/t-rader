//! 予測 (prediction) の記録 / 読み取り tool。
//!
//! 記録後の確率・期限・対象の書き換えは採点を無意味にするため、更新・削除の tool は
//! 意図的に用意しない。読み取りは自戦略の予測に限る。

use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::entities::{prediction, stock};
use crate::services::predictions::{ensure_direction, ensure_probability};

use super::dto::{
    ListPredictionsParams, ListPredictionsResult, PredictionDto, RecordPredictionParams,
    RecordPredictionResult,
};
use super::{
    StrategyServer, clamp_limit, db_error, decimal_to_f64, ensure_strategy_exists,
    fetch_note_owned_by, invalid_params,
};

fn validation_to_mcp(err: crate::error::AppError) -> McpError {
    match err {
        crate::error::AppError::Validation(msg) => invalid_params(msg),
        other => invalid_params(format!("validation failed: {other}")),
    }
}

fn f64_to_decimal(v: f64) -> Result<Decimal, McpError> {
    Decimal::try_from(v).map_err(|err| invalid_params(format!("invalid decimal value: {err}")))
}

fn prediction_to_dto(m: prediction::Model) -> PredictionDto {
    PredictionDto {
        prediction_id: m.prediction_id,
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

async fn ensure_stock_exists(db: &impl sea_orm::ConnectionTrait, id: &str) -> Result<(), McpError> {
    let exists = stock::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(db_error)?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(invalid_params(format!("stock {id} not found")))
    }
}

impl StrategyServer {
    pub(crate) async fn record_prediction_inner(
        &self,
        session_strategy_id: Uuid,
        params: RecordPredictionParams,
    ) -> Result<RecordPredictionResult, McpError> {
        let target_stock_id = params.target_stock_id.trim().to_string();
        let benchmark_stock_id = params.benchmark_stock_id.trim().to_string();
        if target_stock_id.is_empty() {
            return Err(invalid_params("target_stock_id must not be empty"));
        }
        if benchmark_stock_id.is_empty() {
            return Err(invalid_params("benchmark_stock_id must not be empty"));
        }
        if target_stock_id == benchmark_stock_id {
            return Err(invalid_params(
                "target_stock_id and benchmark_stock_id must differ",
            ));
        }
        let direction = params.direction.trim();
        ensure_direction(direction).map_err(validation_to_mcp)?;
        let probability = f64_to_decimal(params.probability)?;
        ensure_probability(probability).map_err(validation_to_mcp)?;
        if params.due_date <= params.base_date {
            return Err(invalid_params("due_date must be after base_date"));
        }

        ensure_strategy_exists(&self.db, session_strategy_id).await?;
        if let Some(note_id) = params.note_id {
            fetch_note_owned_by(&self.db, note_id, session_strategy_id).await?;
        }
        ensure_stock_exists(&self.db, &target_stock_id).await?;
        ensure_stock_exists(&self.db, &benchmark_stock_id).await?;

        let model = prediction::ActiveModel {
            prediction_id: Set(Uuid::new_v4()),
            strategy_id: Set(session_strategy_id),
            note_id: Set(params.note_id),
            target_stock_id: Set(target_stock_id),
            benchmark_stock_id: Set(benchmark_stock_id),
            direction: Set(direction.to_string()),
            probability: Set(probability),
            base_date: Set(params.base_date),
            due_date: Set(params.due_date),
            created_at: NotSet,
        };
        let created = prediction::Entity::insert(model)
            .exec_with_returning(&self.db)
            .await
            .map_err(db_error)?;
        Ok(RecordPredictionResult {
            prediction: prediction_to_dto(created),
        })
    }

    pub(crate) async fn list_predictions_inner(
        &self,
        session_strategy_id: Uuid,
        params: ListPredictionsParams,
    ) -> Result<ListPredictionsResult, McpError> {
        let mut query = prediction::Entity::find()
            .filter(prediction::Column::StrategyId.eq(session_strategy_id));
        if let Some(due_after) = params.due_after {
            query = query.filter(prediction::Column::DueDate.gte(due_after));
        }
        if let Some(due_before) = params.due_before {
            query = query.filter(prediction::Column::DueDate.lte(due_before));
        }
        let rows = query
            .order_by_desc(prediction::Column::CreatedAt)
            .limit(clamp_limit(params.limit))
            .all(&self.db)
            .await
            .map_err(db_error)?;
        Ok(ListPredictionsResult {
            predictions: rows.into_iter().map(prediction_to_dto).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use sqlx::PgPool;

    use crate::testing::{create_test_db, insert_test_stock};

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
    async fn record_prediction_creates_prediction_with_given_values(pool: PgPool) {
        let db = create_test_db(pool).await;
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
    async fn record_prediction_rejects_same_target_and_benchmark(pool: PgPool) {
        let db = create_test_db(pool).await;
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
    async fn record_prediction_rejects_invalid_direction(pool: PgPool) {
        let db = create_test_db(pool).await;
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
    async fn record_prediction_rejects_invalid_probability(pool: PgPool) {
        let db = create_test_db(pool).await;
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
    async fn record_prediction_rejects_due_date_not_after_base_date(pool: PgPool) {
        let db = create_test_db(pool).await;
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
    async fn record_prediction_rejects_missing_target_stock(pool: PgPool) {
        let db = create_test_db(pool).await;
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
    async fn record_prediction_rejects_missing_benchmark_stock(pool: PgPool) {
        let db = create_test_db(pool).await;
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
    async fn record_prediction_rejects_cross_strategy_linked_note(pool: PgPool) {
        let db = create_test_db(pool).await;
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
    async fn list_predictions_returns_only_own_strategy_predictions(pool: PgPool) {
        let db = create_test_db(pool).await;
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
    async fn list_predictions_filters_by_due_date_range(pool: PgPool) {
        let db = create_test_db(pool).await;
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
