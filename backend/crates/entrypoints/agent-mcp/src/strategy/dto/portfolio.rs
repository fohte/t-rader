use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 銘柄ごとの未決済ポジションと損益 (FIFO ベース)
#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct PortfolioPositionDto {
    pub symbol: String,
    /// 保有数量 (買い残 - 売り残)
    pub qty: f64,
    /// 平均取得単価 (FIFO ベース)
    pub avg_cost: f64,
    /// 取得簿価 (qty * avg_cost)
    pub cost_basis: f64,
    /// 実現損益累計
    pub realized_pnl: f64,
    /// 直近終値。取得できなかった場合は null
    pub current_price: Option<f64>,
    /// 保有時価 (qty * current_price)。current_price が null の場合は null
    pub market_value: Option<f64>,
    /// 含み損益 (market_value - cost_basis)。current_price が null の場合は null
    pub unrealized_pnl: Option<f64>,
}

/// 口座全体、または単一戦略のポジション集計
#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct PortfolioScopeDto {
    pub trade_count: i64,
    /// 全銘柄合計の実現損益
    pub realized_pnl: f64,
    /// 保有時価合計 (current_price が取れたポジションのみの合計)
    pub market_value: f64,
    pub positions: Vec<PortfolioPositionDto>,
}

/// 戦略単位のポジション集計 + 投資可能額
#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct StrategyPortfolioScopeDto {
    pub trade_count: i64,
    pub realized_pnl: f64,
    pub market_value: f64,
    pub positions: Vec<PortfolioPositionDto>,
    /// 戦略に割り当てられた投資可能額の現在値。記録が無ければ null
    pub investable_amount: Option<f64>,
    /// 投資可能額 + 実現損益 - 取得原価。investable_amount が null の場合は null
    pub unused_investable_amount: Option<f64>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadPortfolioResult {
    /// 保有時価の評価に用いた対象営業日。current_price を持つ全ポジションに共通する日付
    /// (日付が食い違う銘柄は current_price が null になる)。1 銘柄も評価できなければ null
    pub priced_at: Option<NaiveDate>,
    /// 口座全体 (全戦略横断) の集計
    pub account: PortfolioScopeDto,
    /// 接続元戦略の集計
    pub strategy: StrategyPortfolioScopeDto,
}

/// 個々の約定 (account-wide、全戦略横断)
#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct TradeDto {
    pub trade_id: Uuid,
    pub strategy_id: Uuid,
    /// 約定日
    pub date: NaiveDate,
    pub symbol: String,
    /// "buy" | "sell"
    pub side: String,
    pub qty: f64,
    pub price: f64,
    /// 取引に紐付くノートと、紐付け時点で固定されたバージョン。
    pub notes: Vec<TradeNoteReferenceDto>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct TradeNoteReferenceDto {
    pub note_id: Uuid,
    pub note_version_id: Uuid,
}

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct ReadTradesParams {
    /// この銘柄コードに一致する取引のみ返す。省略時は全銘柄
    pub symbol: Option<String>,
    /// この約定日以降 (inclusive) の取引のみ返す。省略時は下限なし
    pub date_from: Option<NaiveDate>,
    pub limit: Option<u32>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadTradesResult {
    pub trades: Vec<TradeDto>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CheckBuyableQtyParams {
    /// 対象銘柄コード (例: "7203")
    pub symbol: String,
}

/// 制約単位の追加購入可能株数。
#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ConstraintResult {
    /// 上限が効いている。`max_additional_qty` は `lot_size` の倍数に切り捨て済み
    Limited { max_additional_qty: i64 },
    /// risk_policy でこの制約自体が未設定 (上限なし)
    Unlimited,
    /// 計算に必要な値が欠けており判定できない
    Unavailable { reason: String },
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct CheckBuyableQtyResult {
    pub symbol: String,
    /// 単元株数。各上限株数はこの倍数に切り捨てて返す
    pub lot_size: i64,
    /// 接続元戦略が現在保有する株数。保有していなければ 0
    pub current_qty: f64,
    /// 直近終値。取得できなかった場合は null (この場合、価格に依存する制約はすべて unavailable になる)
    pub current_price: Option<f64>,
    /// 価格取得を試みた銘柄 (口座全体の保有銘柄 + 対象銘柄) のうち、取得できたもので
    /// 最も新しい観測日。1 銘柄も取得できなければ null。`current_price` 自体の観測日とは
    /// 限らない
    pub priced_at: Option<NaiveDate>,
    /// `account_risk_policy.max_group_ratios` による制約
    pub max_qty_by_group_ratios: ConstraintResult,
    /// 戦略の未使用投資可能額による制約
    pub max_qty_by_cash: ConstraintResult,
    /// 上記制約のうち最も厳しいもの。いずれかが unavailable なら unavailable、
    /// 全て unlimited なら unlimited
    pub max_qty: ConstraintResult,
    /// `max_qty` が `Limited` のとき、根拠になった制約名
    /// (`group_ratios` / `cash`)。それ以外は null
    pub binding_constraint: Option<String>,
}
