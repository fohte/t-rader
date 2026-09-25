use chrono::NaiveDate;
use rust_decimal::Decimal;

/// 業種別空売り比率 1 件
///
/// `sector33_code` は 33 業種コードをそのまま保持する。既存の `sector` テーブルとの
/// 対応付けは行わず、読む側に委ねる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortRatio {
    /// 対象日
    pub date: NaiveDate,
    /// 33 業種コード
    pub sector33_code: String,
    /// 実注文売買代金 (円、売買が無い日は None)
    pub sell_excluding_short_value: Option<Decimal>,
    /// 価格規制ありの空売り売買代金 (売買が無い日は None)
    pub short_with_restriction_value: Option<Decimal>,
    /// 価格規制なしの空売り売買代金 (売買が無い日は None)
    pub short_without_restriction_value: Option<Decimal>,
}
