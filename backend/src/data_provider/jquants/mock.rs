use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::JQuantsClient;
use crate::data_provider::DataProviderError;

/// J-Quants API のテスト用モックサーバー
///
/// wiremock の MockServer をラップし、Builder pattern で
/// J-Quants API のレスポンスをセットアップする。
pub(crate) struct JQuantsMockServer {
    server: MockServer,
}

impl JQuantsMockServer {
    pub async fn start() -> Self {
        Self {
            server: MockServer::start().await,
        }
    }

    /// このモックサーバーに接続する JQuantsClient を返す
    pub fn client(&self) -> Result<JQuantsClient, DataProviderError> {
        JQuantsClient::with_base_url(&self.server.uri(), "test-api-key")
    }

    pub fn daily_bars(&self) -> MockDailyBarsBuilder<'_> {
        MockDailyBarsBuilder {
            server: &self.server,
            code: "86970",
            bars: Vec::new(),
            pagination_key: None,
            with_pagination_key_param: None,
            max_times: None,
        }
    }

    pub fn instrument(&self) -> MockInstrumentBuilder<'_> {
        MockInstrumentBuilder {
            server: &self.server,
            code: "86970",
            company_name: "テスト株式会社",
            market_name: "プライム",
            sector_name: Some("情報通信"),
        }
    }

    pub fn error(&self) -> MockErrorBuilder<'_> {
        MockErrorBuilder {
            server: &self.server,
        }
    }

    /// テストで直接 wiremock の Mock を登録する際に使用する
    pub fn server_ref(&self) -> &MockServer {
        &self.server
    }
}

/// テスト用の日足データ
pub(crate) struct MockBar {
    pub date: &'static str,
    pub code: &'static str,
    pub adj_open: Option<f64>,
    pub adj_high: Option<f64>,
    pub adj_low: Option<f64>,
    pub adj_close: Option<f64>,
    pub adj_volume: Option<f64>,
}

pub(crate) struct MockDailyBarsBuilder<'a> {
    server: &'a MockServer,
    code: &'a str,
    bars: Vec<MockBar>,
    pagination_key: Option<&'a str>,
    /// このパラメータが指定されたリクエストにのみマッチさせる
    with_pagination_key_param: Option<&'a str>,
    /// レスポンスを返す回数の上限 (ページネーションテスト時に使用)
    max_times: Option<u64>,
}

impl<'a> MockDailyBarsBuilder<'a> {
    pub fn code(mut self, code: &'a str) -> Self {
        self.code = code;
        self
    }

    pub fn bars(mut self, bars: Vec<MockBar>) -> Self {
        self.bars = bars;
        self
    }

    /// レスポンスに含める pagination_key (次ページがある場合)
    pub fn pagination_key(mut self, key: &'a str) -> Self {
        self.pagination_key = Some(key);
        self
    }

    /// pagination_key クエリパラメータを持つリクエストにマッチさせる
    pub fn with_pagination_key_param(mut self, key: &'a str) -> Self {
        self.with_pagination_key_param = Some(key);
        self
    }

    /// この mock がレスポンスを返す回数の上限
    pub fn up_to_n_times(mut self, n: u64) -> Self {
        self.max_times = Some(n);
        self
    }

    pub async fn ok(self) {
        let data: Vec<serde_json::Value> = self
            .bars
            .iter()
            .map(|b| {
                json!({
                    "Date": b.date,
                    "Code": b.code,
                    "AdjO": b.adj_open,
                    "AdjH": b.adj_high,
                    "AdjL": b.adj_low,
                    "AdjC": b.adj_close,
                    "AdjVo": b.adj_volume,
                })
            })
            .collect();

        let mut mock = Mock::given(method("GET"))
            .and(path("/equities/bars/daily"))
            .and(query_param("code", self.code))
            .and(header("x-api-key", "test-api-key"));

        if let Some(key) = self.with_pagination_key_param {
            mock = mock.and(query_param("pagination_key", key));
        }

        let mut mock = mock.respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": data,
            "pagination_key": self.pagination_key,
        })));

        if let Some(n) = self.max_times {
            mock = mock.up_to_n_times(n);
        }

        mock.mount(self.server).await;
    }
}

pub(crate) struct MockInstrumentBuilder<'a> {
    server: &'a MockServer,
    code: &'a str,
    company_name: &'a str,
    market_name: &'a str,
    sector_name: Option<&'a str>,
}

impl<'a> MockInstrumentBuilder<'a> {
    pub fn code(mut self, code: &'a str) -> Self {
        self.code = code;
        self
    }

    pub fn company_name(mut self, name: &'a str) -> Self {
        self.company_name = name;
        self
    }

    pub fn sector_name(mut self, sector: Option<&'a str>) -> Self {
        self.sector_name = sector;
        self
    }

    pub async fn ok(self) {
        Mock::given(method("GET"))
            .and(path("/equities/master"))
            .and(query_param("code", self.code))
            .and(header("x-api-key", "test-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{
                    "Code": self.code,
                    "CoName": self.company_name,
                    "MktNm": self.market_name,
                    "S33Nm": self.sector_name,
                }],
            })))
            .mount(self.server)
            .await;
    }

    pub async fn not_found(self) {
        Mock::given(method("GET"))
            .and(path("/equities/master"))
            .and(query_param("code", self.code))
            .and(header("x-api-key", "test-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [],
            })))
            .mount(self.server)
            .await;
    }
}

pub(crate) struct MockErrorBuilder<'a> {
    server: &'a MockServer,
}

impl<'a> MockErrorBuilder<'a> {
    pub async fn rate_limited(self, endpoint_path: &str) {
        Mock::given(method("GET"))
            .and(path(endpoint_path))
            .respond_with(ResponseTemplate::new(429).set_body_json(json!({
                "message": "Too Many Requests",
            })))
            .mount(self.server)
            .await;
    }

    pub async fn forbidden(self, endpoint_path: &str) {
        Mock::given(method("GET"))
            .and(path(endpoint_path))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({
                "message": "Forbidden",
            })))
            .mount(self.server)
            .await;
    }
}
