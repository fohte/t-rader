//! 予測 (prediction) の値域チェックを集約する。MCP tool で使う。

use rust_decimal::Decimal;

use crate::error::AppError;

pub const DIRECTIONS: [&str; 2] = ["outperform", "underperform"];

/// 予測登録時に受け付ける確率の固定刻み。`mcp::strategy::prediction_stats` の
/// バケット集計でも同じ刻みを使うため `pub(crate)` にしている。
pub(crate) fn probability_steps() -> [Decimal; 8] {
    [55, 60, 65, 70, 75, 80, 85, 90].map(|n| Decimal::new(n, 2))
}

pub fn ensure_direction(value: &str) -> Result<(), AppError> {
    if DIRECTIONS.contains(&value) {
        Ok(())
    } else {
        Err(AppError::Validation(format!("invalid direction: {value}")))
    }
}

pub fn ensure_probability(value: Decimal) -> Result<(), AppError> {
    if probability_steps().contains(&value) {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "invalid probability: {value}"
        )))
    }
}
