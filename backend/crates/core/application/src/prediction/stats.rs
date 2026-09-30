use rust_decimal::prelude::ToPrimitive;

use crate::strategy_scope::StrategyScope;

use super::error::PredictionUseCaseError;
use super::types::{PredictionProbabilityBucket, PredictionStats};
use super::use_cases::{PredictionUseCases, probability_steps};

const PROBABILITY_EPSILON: f64 = 1e-9;

impl PredictionUseCases {
    pub async fn stats(
        &self,
        scope: StrategyScope,
    ) -> Result<PredictionStats, PredictionUseCaseError> {
        let graded = self.repository.list_graded_by_strategy(scope.id()).await?;
        let graded_count = graded.len();
        let brier_score = if graded_count == 0 {
            None
        } else {
            let sum: f64 = graded
                .iter()
                .map(|item| {
                    let probability = decimal_to_f64(item.prediction.probability);
                    let outcome = if item.grade.correct { 1.0 } else { 0.0 };
                    (probability - outcome).powi(2)
                })
                .sum();
            Some(sum / graded_count as f64)
        };

        let buckets = probability_steps()
            .map(decimal_to_f64)
            .into_iter()
            .map(|step| {
                let correct: Vec<bool> = graded
                    .iter()
                    .filter(|item| {
                        (decimal_to_f64(item.prediction.probability) - step).abs()
                            < PROBABILITY_EPSILON
                    })
                    .map(|item| item.grade.correct)
                    .collect();
                let count = correct.len();
                let hit_rate = if count == 0 {
                    None
                } else {
                    Some(correct.iter().filter(|correct| **correct).count() as f64 / count as f64)
                };
                PredictionProbabilityBucket {
                    probability: step,
                    count: count as u32,
                    hit_rate,
                }
            })
            .collect();

        Ok(PredictionStats {
            graded_count: graded_count as u32,
            brier_score,
            buckets,
        })
    }
}

fn decimal_to_f64(value: rust_decimal::Decimal) -> f64 {
    value.to_f64().unwrap_or_else(|| {
        tracing::warn!(value = %value, "予測値を f64 に変換できないため 0.0 として扱います");
        0.0
    })
}
