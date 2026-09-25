use chrono::NaiveDate;
use rust_decimal::Decimal;

/// 経済指標の観測値
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndicatorObservation {
    pub date: NaiveDate,
    pub value: Decimal,
}
