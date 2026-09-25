use chrono::NaiveDate;
use rust_decimal::Decimal;

/// 空売り残高報告 1 件
///
/// 同一日・同一銘柄に報告者ごとの行が並ぶため、報告者を識別するコードが無い J-Quants の
/// レスポンス上では `(disc_date, calc_date, code, ss_name, ss_addr, dic_name, dic_addr,
/// fund_name)` が実質的に取れる最良の自然キーになる。同一 disc_date に複数 calc_date の
/// 報告が公表されることがあるため calc_date も主キーに含める。
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
