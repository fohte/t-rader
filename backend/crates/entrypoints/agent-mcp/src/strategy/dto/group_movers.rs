use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListGroupMoversParams {
    pub axis_key: String,
    /// 開始日 (inclusive)。
    pub from: NaiveDate,
    /// 終了日 (inclusive)。
    pub to: NaiveDate,
    /// 構成銘柄ごとの期間平均売買代金の下限。省略時は下限なし。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_avg_turnover: Option<f64>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct GroupMoverStockDto {
    pub stock_id: String,
    pub name: String,
    /// 小数比率。0.05 は 5% を表す。
    pub change_rate: f64,
    /// 期間内の 1 営業日あたり平均売買代金。
    pub avg_turnover: f64,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct GroupMoverDto {
    pub group_key: String,
    pub name: String,
    /// 価格比較と最低売買代金の条件を満たす構成銘柄数。
    pub constituent_count: i64,
    /// 構成銘柄の騰落率の中央値。対象銘柄がない場合は null。
    pub median_change_rate: Option<f64>,
    /// 期間平均売買代金を重みにした騰落率。売買代金の合計が 0 の場合は null。
    pub turnover_weighted_change_rate: Option<f64>,
    /// 構成銘柄の期間平均売買代金の合計。
    pub total_avg_turnover: f64,
    /// 期間平均売買代金が大きい上位 5 銘柄。
    pub top_stocks: Vec<GroupMoverStockDto>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ListGroupMoversResult {
    pub axis_key: String,
    pub groups: Vec<GroupMoverDto>,
}
