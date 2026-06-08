use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::IbkrClient;
use crate::data_provider::DataProviderError;

/// IBKR Client Portal API のテスト用モックサーバー
pub(crate) struct IbkrMockServer {
    server: MockServer,
}

impl IbkrMockServer {
    pub async fn start() -> Self {
        Self {
            server: MockServer::start().await,
        }
    }

    pub fn client(&self) -> Result<IbkrClient, DataProviderError> {
        IbkrClient::with_base_url(&self.server.uri())
    }

    pub fn stocks(&self) -> MockStocksBuilder<'_> {
        MockStocksBuilder {
            server: &self.server,
            symbol: "7203",
            name: Some("TOYOTA MOTOR CORP"),
            contracts: vec![("TSEJ", 12345)],
        }
    }

    pub fn history(&self) -> MockHistoryBuilder<'_> {
        MockHistoryBuilder {
            server: &self.server,
            conid: 12345,
            bars: Vec::new(),
        }
    }

    pub fn server_ref(&self) -> &MockServer {
        &self.server
    }
}

pub(crate) struct MockStocksBuilder<'a> {
    server: &'a MockServer,
    symbol: &'a str,
    name: Option<&'a str>,
    /// (exchange, conid) のリスト
    contracts: Vec<(&'a str, i64)>,
}

impl<'a> MockStocksBuilder<'a> {
    pub fn symbol(mut self, symbol: &'a str) -> Self {
        self.symbol = symbol;
        self
    }

    pub fn name(mut self, name: Option<&'a str>) -> Self {
        self.name = name;
        self
    }

    pub fn contracts(mut self, contracts: Vec<(&'a str, i64)>) -> Self {
        self.contracts = contracts;
        self
    }

    pub async fn ok(self) {
        let contracts: Vec<serde_json::Value> = self
            .contracts
            .iter()
            .map(|(exchange, conid)| {
                json!({
                    "conid": conid,
                    "exchange": exchange,
                })
            })
            .collect();

        let body = json!({
            self.symbol: [
                {
                    "name": self.name,
                    "contracts": contracts,
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/trsrv/stocks"))
            .and(query_param("symbols", self.symbol))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(self.server)
            .await;
    }

    pub async fn not_found(self) {
        Mock::given(method("GET"))
            .and(path("/trsrv/stocks"))
            .and(query_param("symbols", self.symbol))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(self.server)
            .await;
    }
}

pub(crate) struct MockHistoryBar {
    /// UNIX epoch milliseconds
    pub t: i64,
    pub o: f64,
    pub h: f64,
    pub l: f64,
    pub c: f64,
    pub v: f64,
}

pub(crate) struct MockHistoryBuilder<'a> {
    server: &'a MockServer,
    conid: i64,
    bars: Vec<MockHistoryBar>,
}

impl<'a> MockHistoryBuilder<'a> {
    pub fn conid(mut self, conid: i64) -> Self {
        self.conid = conid;
        self
    }

    pub fn bars(mut self, bars: Vec<MockHistoryBar>) -> Self {
        self.bars = bars;
        self
    }

    pub async fn ok(self) {
        let data: Vec<serde_json::Value> = self
            .bars
            .iter()
            .map(|b| {
                json!({
                    "t": b.t,
                    "o": b.o,
                    "h": b.h,
                    "l": b.l,
                    "c": b.c,
                    "v": b.v,
                })
            })
            .collect();

        Mock::given(method("GET"))
            .and(path("/iserver/marketdata/history"))
            .and(query_param("conid", self.conid.to_string().as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": data,
            })))
            .mount(self.server)
            .await;
    }
}
