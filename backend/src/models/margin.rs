use chrono::NaiveDate;
use rust_decimal::Decimal;
use sea_orm::Set;

use crate::entities::{margin_alert, margin_interest};

/// 信用取引週末残高 1 銘柄分 (margin_interest テーブルに対応)
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

impl From<MarginInterestRecord> for margin_interest::ActiveModel {
    fn from(r: MarginInterestRecord) -> Self {
        margin_interest::ActiveModel {
            date: Set(r.date),
            code: Set(r.code),
            iss_type: Set(r.iss_type),
            shrt_vol: Set(r.shrt_vol),
            long_vol: Set(r.long_vol),
            shrt_neg_vol: Set(r.shrt_neg_vol),
            long_neg_vol: Set(r.long_neg_vol),
            shrt_std_vol: Set(r.shrt_std_vol),
            long_std_vol: Set(r.long_std_vol),
            shrt_val: Set(r.shrt_val),
            long_val: Set(r.long_val),
            shrt_neg_val: Set(r.shrt_neg_val),
            long_neg_val: Set(r.long_neg_val),
            shrt_std_val: Set(r.shrt_std_val),
            long_std_val: Set(r.long_std_val),
        }
    }
}

/// PubReason (日々公表信用取引残高の公表理由フラグ)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PubReason {
    pub restricted: bool,
    pub daily_publication: bool,
    pub monitoring: bool,
    pub restricted_by_jsf: bool,
    pub precaution_by_jsf: bool,
    pub unclear_or_sec_on_alert: bool,
}

/// 日々公表信用取引残高 1 銘柄分 (margin_alert テーブルに対応)
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

impl From<MarginAlertRecord> for margin_alert::ActiveModel {
    fn from(r: MarginAlertRecord) -> Self {
        margin_alert::ActiveModel {
            pub_date: Set(r.pub_date),
            code: Set(r.code),
            app_date: Set(r.app_date),
            pub_reason: Set(serde_json::json!({
                "restricted": r.pub_reason.restricted,
                "daily_publication": r.pub_reason.daily_publication,
                "monitoring": r.pub_reason.monitoring,
                "restricted_by_jsf": r.pub_reason.restricted_by_jsf,
                "precaution_by_jsf": r.pub_reason.precaution_by_jsf,
                "unclear_or_sec_on_alert": r.pub_reason.unclear_or_sec_on_alert,
            })),
            shrt_out: Set(r.shrt_out),
            long_out: Set(r.long_out),
            shrt_out_chg: Set(r.shrt_out_chg),
            long_out_chg: Set(r.long_out_chg),
            shrt_out_ratio: Set(r.shrt_out_ratio),
            long_out_ratio: Set(r.long_out_ratio),
            sl_ratio: Set(r.sl_ratio),
            shrt_neg_out: Set(r.shrt_neg_out),
            shrt_std_out: Set(r.shrt_std_out),
            long_neg_out: Set(r.long_neg_out),
            long_std_out: Set(r.long_std_out),
            tse_mrgn_reg_cls: Set(r.tse_mrgn_reg_cls),
        }
    }
}
