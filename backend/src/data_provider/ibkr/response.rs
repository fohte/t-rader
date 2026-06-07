use serde::Deserialize;

/// `/v1/api/trsrv/stocks` レスポンス
///
/// シンボル文字列をキーにした連想配列で、各シンボルに該当する企業 (issuer) のリストを返す。
/// 同一シンボルが複数取引所に存在しうるため、`Issuer.contracts` に取引所別の contract が並ぶ。
pub(crate) type StocksResponse = std::collections::HashMap<String, Vec<Issuer>>;

#[derive(Debug, Deserialize)]
pub(crate) struct Issuer {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub contracts: Vec<StockContract>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StockContract {
    pub conid: i64,
    pub exchange: String,
}

/// `/v1/api/iserver/marketdata/history` レスポンス
#[derive(Debug, Deserialize)]
pub(crate) struct HistoryResponse {
    #[serde(default)]
    pub data: Vec<HistoryBar>,
}

/// 1 本のローソク足。`t` は UNIX epoch ミリ秒。
#[derive(Debug, Deserialize)]
pub(crate) struct HistoryBar {
    pub t: i64,
    pub o: f64,
    pub h: f64,
    pub l: f64,
    pub c: f64,
    #[serde(default)]
    pub v: f64,
}

/// IBKR Client Portal API のエラーレスポンス。
///
/// `error` または `message` のいずれかが返ることがある。
#[derive(Debug, Deserialize)]
pub(crate) struct ErrorResponse {
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

impl ErrorResponse {
    pub fn into_message(self) -> String {
        self.error
            .or(self.message)
            .unwrap_or_else(|| "unknown error".to_string())
    }
}
