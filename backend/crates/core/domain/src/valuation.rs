use chrono::NaiveDate;
use rust_decimal::Decimal;

/// 1 銘柄・1 日分のバリュエーション指標。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Valuation {
    pub code: String,
    pub date: NaiveDate,
    pub eps: Option<Decimal>,
    pub fwd_eps: Option<Decimal>,
    pub bps: Option<Decimal>,
    pub roe: Option<Decimal>,
    pub fwd_roe: Option<Decimal>,
    pub per: Option<Decimal>,
    pub fwd_per: Option<Decimal>,
    pub pbr: Option<Decimal>,
    pub mkt_cap: Option<Decimal>,
}
