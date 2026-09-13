use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::models::PubReason;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadMarginParams {
    /// 対象銘柄コード (4桁、例: "7203")
    pub symbol: String,
    /// 取得開始日 (YYYY-MM-DD, inclusive)。省略時は下限なし
    pub from: Option<NaiveDate>,
    /// 取得終了日 (YYYY-MM-DD, inclusive)。省略時は上限なし
    pub to: Option<NaiveDate>,
    /// interest / alerts それぞれに独立に適用される件数上限
    pub limit: Option<u32>,
}

/// 信用取引週末残高 (2026-09-28 以降の切替後は日次の信用取引残高) 1 行分。
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct MarginInterestDto {
    pub date: NaiveDate,
    /// J-Quants の5桁コード。同一銘柄でも普通株/優先株など株式の種類ごとに別コードで並び得る
    pub code: String,
    /// 銘柄区分 (1: 信用銘柄, 2: 貸借銘柄, 3: その他)
    pub iss_type: i16,
    pub shrt_vol: i64,
    pub long_vol: i64,
    pub shrt_neg_vol: i64,
    pub long_neg_vol: i64,
    pub shrt_std_vol: i64,
    pub long_std_vol: i64,
    /// 2026-09-25 申込分より前は金額データ自体が存在しないため null
    pub shrt_val: Option<i64>,
    pub long_val: Option<i64>,
    pub shrt_neg_val: Option<i64>,
    pub long_neg_val: Option<i64>,
    pub shrt_std_val: Option<i64>,
    pub long_std_val: Option<i64>,
}

/// 日々公表信用取引残高 1 行分。取引所が日々公表銘柄に指定した銘柄のみが対象であり、
/// この一覧に載っていないことは残高ゼロを意味しない。
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct MarginAlertDto {
    /// 申込日。同一 app_date に訂正が複数あれば公表日 (pub_date) が最新の 1 件のみ返る
    pub app_date: NaiveDate,
    /// この行の公表日
    pub pub_date: NaiveDate,
    pub code: String,
    pub pub_reason: PubReason,
    pub shrt_out: i64,
    pub long_out: i64,
    /// 前日に公表されていなければ null
    pub shrt_out_chg: Option<i64>,
    pub long_out_chg: Option<i64>,
    /// ETF 等では null
    pub shrt_out_ratio: Option<f64>,
    pub long_out_ratio: Option<f64>,
    pub sl_ratio: Option<f64>,
    pub shrt_neg_out: i64,
    pub shrt_std_out: i64,
    pub long_neg_out: i64,
    pub long_std_out: i64,
    /// 規制区分 (文字列。J-Quants 側の分類をそのまま保持)
    pub tse_mrgn_reg_cls: String,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadMarginResult {
    /// 信用取引週末残高 (日次切替後は信用取引残高)。新しい順
    pub interest: Vec<MarginInterestDto>,
    /// 日々公表信用取引残高。新しい順
    pub alerts: Vec<MarginAlertDto>,
}
