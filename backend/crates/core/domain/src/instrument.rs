use serde::{Deserialize, Serialize};

/// 株式市場の識別子
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Market {
    /// 東京証券取引所
    #[serde(rename = "TSE")]
    Tse,
}

impl std::fmt::Display for Market {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Market::Tse => write!(f, "TSE"),
        }
    }
}

/// 銘柄情報
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instrument {
    /// 銘柄コード
    pub id: String,
    /// 銘柄名
    pub name: String,
    /// 上場市場
    pub market: Market,
    /// 業種 (セクター)
    pub sector: Option<String>,
    /// 商品区分コード。データソースの生コードをそのまま保持する
    pub product_category: Option<String>,
}
