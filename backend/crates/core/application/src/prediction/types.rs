use chrono::{DateTime, FixedOffset, NaiveDate};
use core_domain::bar::Bar;
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prediction {
    pub id: Uuid,
    pub strategy_id: Uuid,
    pub note_id: Option<Uuid>,
    pub target_stock_id: String,
    pub benchmark_stock_id: String,
    pub direction: String,
    pub probability: Decimal,
    pub base_date: NaiveDate,
    pub due_date: NaiveDate,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPrediction {
    pub id: Uuid,
    pub strategy_id: Uuid,
    pub note_id: Option<Uuid>,
    pub target_stock_id: String,
    pub benchmark_stock_id: String,
    pub direction: String,
    pub probability: Decimal,
    pub base_date: NaiveDate,
    pub due_date: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteOwner {
    pub strategy_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PredictionListQuery {
    pub due_after: Option<NaiveDate>,
    pub due_before: Option<NaiveDate>,
    pub limit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PredictionGrade {
    pub prediction_id: Uuid,
    pub target_base_close: Decimal,
    pub target_due_close: Decimal,
    pub benchmark_base_close: Decimal,
    pub benchmark_due_close: Decimal,
    pub target_return: Decimal,
    pub benchmark_return: Decimal,
    pub correct: bool,
    pub graded_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPredictionGrade {
    pub prediction_id: Uuid,
    pub target_base_close: Decimal,
    pub target_due_close: Decimal,
    pub benchmark_base_close: Decimal,
    pub benchmark_due_close: Decimal,
    pub target_return: Decimal,
    pub benchmark_return: Decimal,
    pub correct: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradedPrediction {
    pub prediction: Prediction,
    pub grade: PredictionGrade,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PredictionProbabilityBucket {
    pub probability: f64,
    pub count: u32,
    pub hit_rate: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PredictionStats {
    pub graded_count: u32,
    pub brier_score: Option<f64>,
    pub buckets: Vec<PredictionProbabilityBucket>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GradingStats {
    pub graded: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordPredictionCommand {
    pub note_id: Option<Uuid>,
    pub target_stock_id: String,
    pub benchmark_stock_id: String,
    pub direction: String,
    pub probability: Decimal,
    pub base_date: NaiveDate,
    pub due_date: NaiveDate,
}

pub(super) struct GradingOutcome {
    pub target_base_close: Decimal,
    pub target_due_close: Decimal,
    pub benchmark_base_close: Decimal,
    pub benchmark_due_close: Decimal,
    pub target_return: Decimal,
    pub benchmark_return: Decimal,
    pub correct: bool,
}

pub(super) fn grading_outcome(
    target_base: &Bar,
    target_due: &Bar,
    benchmark_base: &Bar,
    benchmark_due: &Bar,
    direction: &str,
) -> Option<GradingOutcome> {
    let target_return = compute_return(target_base.close, target_due.close)?;
    let benchmark_return = compute_return(benchmark_base.close, benchmark_due.close)?;
    let correct = if direction == "outperform" {
        target_return > benchmark_return
    } else {
        target_return < benchmark_return
    };

    Some(GradingOutcome {
        target_base_close: target_base.close,
        target_due_close: target_due.close,
        benchmark_base_close: benchmark_base.close,
        benchmark_due_close: benchmark_due.close,
        target_return,
        benchmark_return,
        correct,
    })
}

pub(super) fn compute_return(base: Decimal, due: Decimal) -> Option<Decimal> {
    (due - base).checked_div(base)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use rust_decimal::Decimal;

    use super::compute_return;

    #[rstest]
    #[case::positive_return(Decimal::new(100, 0), Decimal::new(110, 0), Some(Decimal::new(10, 2)))]
    #[case::negative_return(Decimal::new(100, 0), Decimal::new(90, 0), Some(Decimal::new(-10, 2)))]
    #[case::zero_base_is_none(Decimal::new(0, 0), Decimal::new(90, 0), None)]
    fn compute_return_cases(
        #[case] base: Decimal,
        #[case] due: Decimal,
        #[case] expected: Option<Decimal>,
    ) {
        assert_eq!(compute_return(base, due), expected);
    }
}
