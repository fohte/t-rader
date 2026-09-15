//! 予測 (prediction) の値域チェックを集約する。MCP tool で使う。

use rust_decimal::Decimal;

use crate::error::AppError;

pub const DIRECTIONS: [&str; 2] = ["outperform", "underperform"];

/// `probability_steps()` の f64 版。DB 保存値の検証は Decimal 表現で厳密に行うが、
/// 採点結果の集計 (`mcp::strategy::prediction_stats`) は f64 で扱うため別表現を持つ。
pub const PROBABILITY_STEPS_F64: [f64; 8] = [0.55, 0.60, 0.65, 0.70, 0.75, 0.80, 0.85, 0.90];

fn probability_steps() -> [Decimal; 8] {
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
