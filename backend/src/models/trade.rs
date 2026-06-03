use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateTradeRequest {
    pub strategy_id: Uuid,
    #[schema(min_length = 1)]
    pub symbol: String,
    /// "buy" | "sell"
    pub side: String,
    #[schema(value_type = f64)]
    pub qty: Decimal,
    #[schema(value_type = f64)]
    pub price: Decimal,
    #[serde(default)]
    #[schema(value_type = Option<f64>)]
    pub fee: Option<Decimal>,
    pub date: NaiveDate,
    /// "manual" | "csv" | "api"
    pub source: String,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateTradeRequest {
    pub symbol: Option<String>,
    pub side: Option<String>,
    #[schema(value_type = Option<f64>)]
    pub qty: Option<Decimal>,
    #[schema(value_type = Option<f64>)]
    pub price: Option<Decimal>,
    #[schema(value_type = Option<f64>)]
    pub fee: Option<Decimal>,
    pub date: Option<NaiveDate>,
    pub source: Option<String>,
    pub note: Option<String>,
}

/// 銘柄ごとの未決済ポジションと損益
#[derive(Debug, Serialize, ToSchema)]
pub struct PositionSummary {
    pub symbol: String,
    /// 保有数量 (買い残 - 売り残)
    #[schema(value_type = f64)]
    pub qty: Decimal,
    /// 平均取得単価 (FIFO ベース)
    #[schema(value_type = f64)]
    pub avg_cost: Decimal,
    /// 取得簿価 (qty * avg_cost)
    #[schema(value_type = f64)]
    pub cost_basis: Decimal,
    /// 実現損益累計
    #[schema(value_type = f64)]
    pub realized_pnl: Decimal,
}

/// 戦略単位もしくはポートフォリオ全体の損益サマリ
#[derive(Debug, Serialize, ToSchema)]
pub struct PerformanceSummary {
    pub strategy_id: Option<Uuid>,
    pub trade_count: i64,
    #[schema(value_type = f64)]
    pub realized_pnl: Decimal,
    pub positions: Vec<PositionSummary>,
}
