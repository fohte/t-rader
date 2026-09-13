use serde::Deserialize;

/// `data` + `pagination_key` を持つ一覧系レスポンスが実装するトレイト。
/// `fetch_all_pages` が `into_parts` でアイテム列と次ページキーを取り出す。
pub(crate) trait Paginated {
    type Item;

    fn into_parts(self) -> (Vec<Self::Item>, Option<String>);
}

/// J-Quants API V2 日足レスポンス (`GET /v2/equities/bars/daily`)
#[derive(Debug, Deserialize)]
pub(crate) struct DailyBarsResponse {
    pub data: Vec<DailyBar>,
    pub pagination_key: Option<String>,
}

impl Paginated for DailyBarsResponse {
    type Item = DailyBar;

    fn into_parts(self) -> (Vec<DailyBar>, Option<String>) {
        (self.data, self.pagination_key)
    }
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

/// J-Quants API V2 財務情報レスポンス (`GET /v2/fins/summary`)
///
/// フィールド数が多く記載欄も可変 (未記載の数値項目も空文字で返る等) なため、要素は
/// 個別の構造体にせず生の JSON のまま保持する。
#[derive(Debug, Deserialize)]
pub(crate) struct FinSummaryResponse {
    pub data: Vec<serde_json::Value>,
    pub pagination_key: Option<String>,
}

impl Paginated for FinSummaryResponse {
    type Item = serde_json::Value;

    fn into_parts(self) -> (Vec<serde_json::Value>, Option<String>) {
        (self.data, self.pagination_key)
    }
}

/// EDINET 由来のデータ (大量保有報告書 / 政策保有株式 / 大株主状況) の一覧レスポンス。
/// 書類ごとの内部構造はエンドポイントごとに異なるため、要素は serde_json::Value のまま保持する。
#[derive(Debug, Deserialize)]
pub(crate) struct EdinetDocumentsResponse {
    pub data: Vec<serde_json::Value>,
    pub pagination_key: Option<String>,
}

impl Paginated for EdinetDocumentsResponse {
    type Item = serde_json::Value;

    fn into_parts(self) -> (Vec<Self::Item>, Option<String>) {
        (self.data, self.pagination_key)
    }
}

/// J-Quants API V2 エラーレスポンス
#[derive(Debug, Deserialize)]
pub(crate) struct ErrorResponse {
    pub message: String,
}
