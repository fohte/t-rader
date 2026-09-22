use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadValuationParams {
    /// 対象銘柄コード (4 桁)
    pub symbol: String,
    /// 取得開始日 (YYYY-MM-DD, inclusive)
    pub from: NaiveDate,
    /// 取得終了日 (YYYY-MM-DD, inclusive)
    pub to: NaiveDate,
}

/// J-Quants の日次バリュエーション指標 1 件。
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ValuationDto {
    /// 指標の対象日
    pub date: NaiveDate,
    /// 1 株当たり利益 (実績・円)
    pub eps: Option<f64>,
    /// 1 株当たり利益 (予想・円)
    pub fwd_eps: Option<f64>,
    /// 1 株当たり純資産 (円)
    pub bps: Option<f64>,
    /// 自己資本利益率 (実績・小数比率。パーセント値ではない)
    pub roe: Option<f64>,
    /// 自己資本利益率 (予想・小数比率。パーセント値ではない)
    pub fwd_roe: Option<f64>,
    /// 株価収益率 (実績・倍)
    pub per: Option<f64>,
    /// 株価収益率 (予想・倍)
    pub fwd_per: Option<f64>,
    /// 株価純資産倍率 (倍)
    pub pbr: Option<f64>,
    /// 時価総額 (百万円)
    pub mkt_cap: Option<f64>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadValuationResult {
    pub symbol: String,
    /// 日付降順。J-Quants が算出できない指標は null。
    pub items: Vec<ValuationDto>,
}
