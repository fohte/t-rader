//! 自戦略の採点済み予測を Brier score と確率刻みごとの的中率で集計する読み取り専用 tool。

use core_application::prediction::PredictionUseCaseError;
use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;

use super::StrategyServer;
use super::dto::{PredictionProbabilityBucketDto, ReadPredictionStatsResult};

impl StrategyServer {
    pub(crate) async fn read_prediction_stats_inner(
        &self,
        scope: impl Into<StrategyScope>,
    ) -> Result<ReadPredictionStatsResult, McpError> {
        let stats = self
            .use_cases
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
    match error {
        PredictionUseCaseError::Repository(
            core_application::prediction::PredictionRepositoryError::Database(error),
        ) => {
            tracing::error!(error = %error, "strategy mcp db error");
            super::internal_error(format!("database error: {error}"))
        }
        other => {
            tracing::error!(error = %other, "strategy mcp prediction stats failed");
            super::internal_error(format!("database error: {other}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use uuid::Uuid;

    use crate::testing::insert_test_stock;
    use gateway_postgres::entities::{prediction, prediction_grade};

    use super::super::dto::{PredictionProbabilityBucketDto, ReadPredictionStatsResult};
    use super::super::tests_common::{build_server, insert_strategy};

    /// prediction を 1 件 seed する (MCP tool を経由せず ActiveModel で直接 insert)。
    async fn seed_prediction(
        db: &impl sea_orm::ConnectionTrait,
        strategy_id: Uuid,
        probability: f64,
    ) -> Uuid {
        let id = Uuid::new_v4();
        prediction::ActiveModel {
            prediction_id: Set(id),
            strategy_id: Set(strategy_id),
            note_id: Set(None),
            target_stock_id: Set("TGT1".into()),
            benchmark_stock_id: Set("BM1".into()),
            direction: Set("outperform".into()),
            probability: Set(rust_decimal::Decimal::try_from(probability).expect("decimal")),
            base_date: Set(NaiveDate::from_ymd_opt(2026, 6, 1).expect("date")),
            due_date: Set(NaiveDate::from_ymd_opt(2026, 7, 1).expect("date")),
            created_at: NotSet,
        }
        .insert(db)
        .await
        .expect("seed prediction");
        id
    }

    /// 指定 prediction の採点結果を seed する。
    async fn seed_grade(db: &impl sea_orm::ConnectionTrait, prediction_id: Uuid, correct: bool) {
        prediction_grade::ActiveModel {
            prediction_id: Set(prediction_id),
            target_base_close: Set(rust_decimal::Decimal::new(1000, 0)),
            target_due_close: Set(rust_decimal::Decimal::new(1100, 0)),
            benchmark_base_close: Set(rust_decimal::Decimal::new(2000, 0)),
            benchmark_due_close: Set(rust_decimal::Decimal::new(2050, 0)),
            target_return: Set(rust_decimal::Decimal::new(10, 2)),
            benchmark_return: Set(rust_decimal::Decimal::new(25, 3)),
            correct: Set(correct),
            graded_at: NotSet,
        }
        .insert(db)
        .await
        .expect("seed prediction grade");
    }

    fn empty_buckets() -> Vec<PredictionProbabilityBucketDto> {
        [0.55, 0.60, 0.65, 0.70, 0.75, 0.80, 0.85, 0.90]
            .into_iter()
            .map(|probability| PredictionProbabilityBucketDto {
                probability,
                count: 0,
                hit_rate: None,
            })
            .collect()
    }

    #[backend_test_macros::database_test]
    async fn read_prediction_stats_returns_empty_when_no_graded_predictions(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "a").await;
        let server = build_server(db);

        let result = server
            .read_prediction_stats_inner(strategy_id)
            .await
            .expect("read_prediction_stats");

        assert_eq!(
            result,
            ReadPredictionStatsResult {
                graded_count: 0,
                brier_score: None,
                buckets: empty_buckets(),
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn read_prediction_stats_excludes_ungraded_predictions(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "a").await;
        insert_test_stock(&db, "TGT1", "Target").await;
        insert_test_stock(&db, "BM1", "Benchmark").await;
        seed_prediction(&db, strategy_id, 0.65).await;
        let server = build_server(db);

        let result = server
            .read_prediction_stats_inner(strategy_id)
            .await
            .expect("read_prediction_stats");

        assert_eq!(
            result,
            ReadPredictionStatsResult {
                graded_count: 0,
                brier_score: None,
                buckets: empty_buckets(),
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn read_prediction_stats_excludes_other_strategy_predictions(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        insert_test_stock(&db, "TGT1", "Target").await;
        insert_test_stock(&db, "BM1", "Benchmark").await;
        let other_prediction = seed_prediction(&db, strategy_b, 0.65).await;
        seed_grade(&db, other_prediction, true).await;
        let server = build_server(db);

        let result = server
            .read_prediction_stats_inner(strategy_a)
            .await
            .expect("read_prediction_stats");

        assert_eq!(
            result,
            ReadPredictionStatsResult {
                graded_count: 0,
                brier_score: None,
                buckets: empty_buckets(),
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn read_prediction_stats_aggregates_buckets_and_brier_score(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "a").await;
        insert_test_stock(&db, "TGT1", "Target").await;
        insert_test_stock(&db, "BM1", "Benchmark").await;

        // 0.65 の刻みに 2 件 (的中 1 件, 非的中 1 件)、0.75 の刻みに 1 件 (的中)。
        let p1 = seed_prediction(&db, strategy_id, 0.65).await;
        seed_grade(&db, p1, true).await;
        let p2 = seed_prediction(&db, strategy_id, 0.65).await;
        seed_grade(&db, p2, false).await;
        let p3 = seed_prediction(&db, strategy_id, 0.75).await;
        seed_grade(&db, p3, true).await;

        let server = build_server(db);

        let result = server
            .read_prediction_stats_inner(strategy_id)
            .await
            .expect("read_prediction_stats");

        // brier = ((0.65-1)^2 + (0.65-0)^2 + (0.75-1)^2) / 3
        //       = (0.1225 + 0.4225 + 0.0625) / 3 = 0.6075 / 3 = 0.2025
        let mut expected_buckets = empty_buckets();
        expected_buckets[2] = PredictionProbabilityBucketDto {
            probability: 0.65,
            count: 2,
            hit_rate: Some(0.5),
        };
        expected_buckets[4] = PredictionProbabilityBucketDto {
            probability: 0.75,
            count: 1,
            hit_rate: Some(1.0),
        };

        assert_eq!(
            result,
            ReadPredictionStatsResult {
                graded_count: 3,
                brier_score: Some(0.2025),
                buckets: expected_buckets,
            },
        );
    }
}
