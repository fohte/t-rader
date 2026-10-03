//! 採点済み予測を Brier score と確率刻みごとの的中率で集計する読み取り専用 tool。

use core_application::prediction::{PredictionRepositoryError, PredictionUseCaseError};
use core_application::strategy_scope::StrategyScope;
use core_application::unit_of_work::UnitOfWorkError;
use rmcp::ErrorData as McpError;

use super::StrategyServer;
use super::dto::{PredictionProbabilityBucketDto, ReadPredictionStatsResult};

impl StrategyServer {
    pub(crate) async fn read_prediction_stats_inner(
        &self,
        scope: impl Into<StrategyScope>,
    ) -> Result<ReadPredictionStatsResult, McpError> {
        let stats = self
            .dependencies
            .predictions
            .stats(scope.into())
            .await
            .map_err(prediction_stats_error_to_mcp)?;

        Ok(ReadPredictionStatsResult {
            graded_count: stats.graded_count,
            brier_score: stats.brier_score,
            buckets: stats
                .buckets
                .into_iter()
                .map(|bucket| PredictionProbabilityBucketDto {
                    probability: bucket.probability,
                    count: bucket.count,
                    hit_rate: bucket.hit_rate,
                })
                .collect(),
        })
    }
}

fn prediction_stats_error_to_mcp(error: PredictionUseCaseError) -> McpError {
    tracing::error!(error = %error, "strategy mcp prediction stats failed");
    let message = match error {
        PredictionUseCaseError::Validation(message) => {
            format!("prediction stats validation failed: {message}")
        }
        PredictionUseCaseError::NoteNotFound(note_id) => format!("note {note_id} not found"),
        PredictionUseCaseError::Repository(PredictionRepositoryError::Database(error))
        | PredictionUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | PredictionUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error)) => {
            format!("database error: {error}")
        }
        PredictionUseCaseError::Repository(PredictionRepositoryError::InvalidTransaction)
        | PredictionUseCaseError::UnitOfWork(UnitOfWorkError::InvalidTransaction) => {
            "prediction transaction has an unexpected type".into()
        }
    };
    super::internal_error(message)
}
