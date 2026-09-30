//! 採点は終値と予測方向から決定的に計算し、LLM の判断に依存させない。

use chrono::NaiveDate;

use core_domain::business_day::latest_business_day;

use super::error::PredictionUseCaseError;
use super::types::{GradingStats, NewPredictionGrade, Prediction};
use super::use_cases::PredictionUseCases;

impl PredictionUseCases {
    /// 個人利用の規模を前提に、全戦略の期限到来済み予測をまとめて取得して採点する。
    pub(super) async fn grade_due_inner(
        &self,
        today: NaiveDate,
    ) -> Result<GradingStats, PredictionUseCaseError> {
        let predictions = self.repository.list_ungraded_due(today).await?;
        let mut stats = GradingStats::default();
        for prediction in predictions {
            match self.try_grade(&prediction).await {
                Ok(Some(grade)) => match self.repository.insert_grade(grade).await {
                    Ok(()) => stats.graded += 1,
                    Err(error) => tracing::warn!(
                        error = %error,
                        prediction_id = %prediction.id,
                        "予測の採点結果を保存できなかったため次回の poll で再試行します",
                    ),
                },
                Ok(None) => {}
                Err(error) => tracing::warn!(
                    error = %error,
                    prediction_id = %prediction.id,
                    "予測を採点できなかったため次回の poll で再試行します",
                ),
            }
        }
        Ok(stats)
    }

    async fn try_grade(
        &self,
        prediction: &Prediction,
    ) -> Result<Option<NewPredictionGrade>, PredictionUseCaseError> {
        let due_business_day = latest_business_day(prediction.due_date);
        let Some(target_due) = self
            .repository
            .find_latest_daily_bar_on_or_before(&prediction.target_stock_id, prediction.due_date)
            .await?
        else {
            tracing::debug!(
                prediction_id = %prediction.id,
                "期限日以前の対象銘柄の日足が無いため採点を保留します",
            );
            return Ok(None);
        };
        let Some(benchmark_due) = self
            .repository
            .find_latest_daily_bar_on_or_before(&prediction.benchmark_stock_id, prediction.due_date)
            .await?
        else {
            tracing::debug!(
                prediction_id = %prediction.id,
                "期限日以前の比較銘柄の日足が無いため採点を保留します",
            );
            return Ok(None);
        };
        if target_due.timestamp.date_naive() != due_business_day
            || benchmark_due.timestamp.date_naive() != due_business_day
        {
            tracing::debug!(
                prediction_id = %prediction.id,
                "期限日の確定日足が無いため採点を保留します",
            );
            return Ok(None);
        }

        let Some(target_base) = self
            .repository
            .find_latest_daily_bar_on_or_before(&prediction.target_stock_id, prediction.base_date)
            .await?
        else {
            tracing::debug!(
                prediction_id = %prediction.id,
                "基準日以前の対象銘柄の日足が無いため採点を保留します",
            );
            return Ok(None);
        };
        let Some(benchmark_base) = self
            .repository
            .find_latest_daily_bar_on_or_before(
                &prediction.benchmark_stock_id,
                prediction.base_date,
            )
            .await?
        else {
            tracing::debug!(
                prediction_id = %prediction.id,
                "基準日以前の比較銘柄の日足が無いため採点を保留します",
            );
            return Ok(None);
        };

        let Some(outcome) = super::types::grading_outcome(
            &target_base,
            &target_due,
            &benchmark_base,
            &benchmark_due,
            &prediction.direction,
        ) else {
            tracing::debug!(
                prediction_id = %prediction.id,
                "基準日終値が 0 のためリターンを計算できず採点を保留します",
            );
            return Ok(None);
        };

        Ok(Some(NewPredictionGrade {
            prediction_id: prediction.id,
            target_base_close: outcome.target_base_close,
            target_due_close: outcome.target_due_close,
            benchmark_base_close: outcome.benchmark_base_close,
            benchmark_due_close: outcome.benchmark_due_close,
            target_return: outcome.target_return,
            benchmark_return: outcome.benchmark_return,
            correct: outcome.correct,
        }))
    }
}
