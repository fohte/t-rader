use chrono::NaiveDate;
use rust_decimal::Decimal;
use sea_orm::Set;

use crate::entities::short_sale_report;

/// 空売り残高報告 1 件 (`short_sale_report` テーブルに対応)
///
/// 同一日・同一銘柄に報告者ごとの行が並ぶため、報告者を識別するコードが無い J-Quants の
/// レスポンス上では `(disc_date, code, ss_name, ss_addr, dic_name, dic_addr, fund_name)` が
/// 実質的に取れる最良の自然キーになる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortSaleReport {
    /// 公表日
    pub disc_date: NaiveDate,
    /// 計算日
    pub calc_date: NaiveDate,
    /// 銘柄コード
    pub code: String,
    /// 商号等
    pub ss_name: String,
    /// 住所
    pub ss_addr: String,
    /// 委託者
    pub dic_name: String,
    /// 委託者住所
    pub dic_addr: String,
    /// 信託財産名 (個人等は空文字)
    pub fund_name: String,
    /// 空売り残高割合
    pub short_position_ratio: Decimal,
    /// 空売り残高数量
    pub short_position_shares: i64,
    /// 空売り残高売買単位数
    pub short_position_units: i64,
    /// 直近計算年月日 (該当なしは None)
    pub prev_report_date: Option<NaiveDate>,
    /// 直近残高割合 (該当なしは None)
    pub prev_report_ratio: Option<Decimal>,
    /// 備考
    pub notes: String,
}

/// models::ShortSaleReport -> entities::short_sale_report::ActiveModel 変換 (upsert 用)
impl From<ShortSaleReport> for short_sale_report::ActiveModel {
    fn from(report: ShortSaleReport) -> Self {
        short_sale_report::ActiveModel {
            disc_date: Set(report.disc_date),
            calc_date: Set(report.calc_date),
            code: Set(report.code),
            ss_name: Set(report.ss_name),
            ss_addr: Set(report.ss_addr),
            dic_name: Set(report.dic_name),
            dic_addr: Set(report.dic_addr),
            fund_name: Set(report.fund_name),
            short_position_ratio: Set(report.short_position_ratio),
            short_position_shares: Set(report.short_position_shares),
            short_position_units: Set(report.short_position_units),
            prev_report_date: Set(report.prev_report_date),
            prev_report_ratio: Set(report.prev_report_ratio),
            notes: Set(report.notes),
        }
    }
}
