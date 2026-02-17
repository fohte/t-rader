use serde::Deserialize;

/// J-Quants API V2 日足レスポンス (`GET /v2/equities/bars/daily`)
#[derive(Debug, Deserialize)]
pub(crate) struct DailyBarsResponse {
    pub data: Vec<DailyBar>,
    pub pagination_key: Option<String>,
}

/// J-Quants API V2 日足データ 1 レコード
///
/// 調整後価格 (AdjO 等) を使用する。未調整価格やセッション別データは無視する。
#[derive(Debug, Deserialize)]
pub(crate) struct DailyBar {
    #[serde(rename = "Date")]
    pub date: String,
    #[serde(rename = "Code")]
    pub code: String,
    #[serde(rename = "AdjO")]
    pub adj_open: Option<f64>,
    #[serde(rename = "AdjH")]
    pub adj_high: Option<f64>,
    #[serde(rename = "AdjL")]
    pub adj_low: Option<f64>,
    #[serde(rename = "AdjC")]
    pub adj_close: Option<f64>,
    #[serde(rename = "AdjVo")]
    pub adj_volume: Option<f64>,
}

/// J-Quants API V2 銘柄マスタレスポンス (`GET /v2/equities/master`)
#[derive(Debug, Deserialize)]
pub(crate) struct EquitiesMasterResponse {
    pub data: Vec<EquityMaster>,
}

/// J-Quants API V2 銘柄マスタ 1 レコード
///
/// J-Quants は東証上場銘柄のみを提供するため、`MktNm` は構造体には含めない。
#[derive(Debug, Deserialize)]
pub(crate) struct EquityMaster {
    #[serde(rename = "Code")]
    pub code: String,
    #[serde(rename = "CoName")]
    pub company_name: String,
    #[serde(rename = "S33Nm")]
    pub sector_name: Option<String>,
}

/// J-Quants API V2 エラーレスポンス
#[derive(Debug, Deserialize)]
pub(crate) struct ErrorResponse {
    pub message: String,
}
