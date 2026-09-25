use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 信用取引週末残高 1 銘柄分
///
/// 2026-09-28 の新仕様切替 (2026-09-25 申込分以降) で追加される金額 (Val 系) は、
/// それ以前の日付では None になる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarginInterestRecord {
    pub date: NaiveDate,
    pub code: String,
    /// 銘柄区分 (1: 信用銘柄, 2: 貸借銘柄, 3: その他)
    pub iss_type: i16,
    pub shrt_vol: i64,
    pub long_vol: i64,
    pub shrt_neg_vol: i64,
    pub long_neg_vol: i64,
    pub shrt_std_vol: i64,
    pub long_std_vol: i64,
    pub shrt_val: Option<i64>,
    pub long_val: Option<i64>,
    pub shrt_neg_val: Option<i64>,
    pub long_neg_val: Option<i64>,
    pub shrt_std_val: Option<i64>,
    pub long_std_val: Option<i64>,
}

/// PubReason (日々公表信用取引残高の公表理由フラグ)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PubReason {
    pub restricted: bool,
    pub daily_publication: bool,
    pub monitoring: bool,
    pub restricted_by_jsf: bool,
    pub precaution_by_jsf: bool,
    pub unclear_or_sec_on_alert: bool,
}

/// 日々公表信用取引残高 1 銘柄分
///
/// 取引所が日々公表銘柄に指定した銘柄のみが対象。載っていないことは残高ゼロを意味しない。
#[derive(Debug, Clone, PartialEq)]
pub struct MarginAlertRecord {
    pub pub_date: NaiveDate,
    pub code: String,
    pub app_date: NaiveDate,
    pub pub_reason: PubReason,
    pub shrt_out: i64,
    pub long_out: i64,
    pub shrt_out_chg: Option<i64>,
    pub long_out_chg: Option<i64>,
    pub shrt_out_ratio: Option<Decimal>,
    pub long_out_ratio: Option<Decimal>,
    pub sl_ratio: Option<Decimal>,
    pub shrt_neg_out: i64,
    pub shrt_std_out: i64,
    pub long_neg_out: i64,
    pub long_std_out: i64,
    pub tse_mrgn_reg_cls: String,
}
