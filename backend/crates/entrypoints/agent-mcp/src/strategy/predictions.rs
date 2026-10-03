//! 予測 (prediction) の記録 / 読み取り tool。
//!
//! 記録後の確率・期限・対象の書き換えは採点を無意味にするため、更新・削除の tool は
//! 意図的に用意しない。

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
