use serde_json::json;
use wiremock::matchers::{header, method, path, query_param, query_param_is_missing};
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

    pub fn daily_bars_by_date(&self) -> MockDailyBarsByDateBuilder<'_> {
        MockDailyBarsByDateBuilder {
            server: &self.server,
            date: "2025-01-06",
            bars: Vec::new(),
        }
    }

    pub fn fin_summary(&self) -> MockFinSummaryBuilder<'_> {
        MockFinSummaryBuilder {
            server: &self.server,
            date: "2025-01-06",
            items: Vec::new(),
        }
    }

    pub fn earnings_date(&self) -> MockEarningsDateBuilder<'_> {
        MockEarningsDateBuilder {
            server: &self.server,
            date: "2025-01-06",
            items: Vec::new(),
        }
    }

    pub fn instrument(&self) -> MockInstrumentBuilder<'_> {
        MockInstrumentBuilder {
            server: &self.server,
            code: "86970",
            company_name: "テスト株式会社",
            market_name: "プライム",
            sector_name: Some("情報通信"),
            product_category: Some("011"),
        }
    }

    pub fn equities_master(&self) -> MockEquitiesMasterBuilder<'_> {
        MockEquitiesMasterBuilder {
            server: &self.server,
            entries: Vec::new(),
        }
    }

    pub fn error(&self) -> MockErrorBuilder<'_> {
        MockErrorBuilder {
            server: &self.server,
        }
    }

    pub fn short_sale_report(&self) -> MockShortSaleReportBuilder<'_> {
        MockShortSaleReportBuilder {
            server: &self.server,
            disc_date: "",
            reports: Vec::new(),
        }
    }

    pub fn short_ratio(&self) -> MockShortRatioBuilder<'_> {
        MockShortRatioBuilder {
            server: &self.server,
            date: "",
            ratios: Vec::new(),
        }
    }

    pub fn margin_interest(&self) -> MockMarginInterestBuilder<'_> {
        MockMarginInterestBuilder {
            server: &self.server,
            date: "2024-01-01",
            rows: Vec::new(),
        }
    }

    pub fn margin_alert(&self) -> MockMarginAlertBuilder<'_> {
        MockMarginAlertBuilder {
            server: &self.server,
            date: "2024-01-01",
            rows: Vec::new(),
        }
    }

    pub fn edinet_documents(&self, path: &'static str) -> MockEdinetDocumentsBuilder<'_> {
        MockEdinetDocumentsBuilder {
            server: &self.server,
            path,
            date: "20250106",
            docs: Vec::new(),
            pagination_key: None,
            with_pagination_key_param: None,
            max_times: None,
        }
    }

    /// テストで直接 wiremock の Mock を登録する際に使用する
    pub fn server_ref(&self) -> &MockServer {
        &self.server
    }
}

/// テスト用の日足データ
pub(crate) struct MockBar {
    pub date: String,
    pub code: String,
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

pub(crate) struct MockDailyBarsByDateBuilder<'a> {
    server: &'a MockServer,
    date: &'a str,
    bars: Vec<MockBar>,
}

impl<'a> MockDailyBarsByDateBuilder<'a> {
    pub fn date(mut self, date: &'a str) -> Self {
        self.date = date;
        self
    }

    pub fn bars(mut self, bars: Vec<MockBar>) -> Self {
        self.bars = bars;
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

        Mock::given(method("GET"))
            .and(path("/equities/bars/daily"))
            .and(query_param("date", self.date))
            .and(header("x-api-key", "test-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": data,
                "pagination_key": Option::<&str>::None,
            })))
            .mount(self.server)
            .await;
    }
}

pub(crate) struct MockFinSummaryBuilder<'a> {
    server: &'a MockServer,
    date: &'a str,
    items: Vec<serde_json::Value>,
}

impl<'a> MockFinSummaryBuilder<'a> {
    pub fn date(mut self, date: &'a str) -> Self {
        self.date = date;
        self
    }

    pub fn items(mut self, items: Vec<serde_json::Value>) -> Self {
        self.items = items;
        self
    }

    pub async fn ok(self) {
        Mock::given(method("GET"))
            .and(path("/fins/summary"))
            .and(query_param("date", self.date))
            .and(header("x-api-key", "test-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": self.items,
                "pagination_key": Option::<&str>::None,
            })))
            .mount(self.server)
            .await;
    }
}

pub(crate) struct MockEarningsDateBuilder<'a> {
    server: &'a MockServer,
    date: &'a str,
    items: Vec<serde_json::Value>,
}

impl<'a> MockEarningsDateBuilder<'a> {
    pub fn date(mut self, date: &'a str) -> Self {
        self.date = date;
        self
    }

    pub fn items(mut self, items: Vec<serde_json::Value>) -> Self {
        self.items = items;
        self
    }

    pub async fn ok(self) {
        Mock::given(method("GET"))
            .and(path("/fins/earnings-date"))
            .and(query_param("date", self.date))
            .and(header("x-api-key", "test-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": self.items,
                "pagination_key": Option::<&str>::None,
            })))
            .mount(self.server)
            .await;
    }
}

pub(crate) struct MockEdinetDocumentsBuilder<'a> {
    server: &'a MockServer,
    path: &'a str,
    date: &'a str,
    docs: Vec<serde_json::Value>,
    pagination_key: Option<&'a str>,
    /// このパラメータが指定されたリクエストにのみマッチさせる
    with_pagination_key_param: Option<&'a str>,
    /// レスポンスを返す回数の上限 (ページネーションテスト時に使用)
    max_times: Option<u64>,
}

impl<'a> MockEdinetDocumentsBuilder<'a> {
    pub fn date(mut self, date: &'a str) -> Self {
        self.date = date;
        self
    }

    pub fn docs(mut self, docs: Vec<serde_json::Value>) -> Self {
        self.docs = docs;
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
        let mut mock = Mock::given(method("GET"))
            .and(path(self.path))
            .and(query_param("date", self.date))
            .and(header("x-api-key", "test-api-key"));

        if let Some(key) = self.with_pagination_key_param {
            mock = mock.and(query_param("pagination_key", key));
        }

        let mut mock = mock.respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": self.docs,
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
    product_category: Option<&'a str>,
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

    pub fn product_category(mut self, product_category: Option<&'a str>) -> Self {
        self.product_category = product_category;
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
                    "ProdCat": self.product_category,
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

/// テスト用の全銘柄マスタ 1 行
pub(crate) struct MockEquitiesMasterEntry {
    pub code: &'static str,
    pub company_name: &'static str,
    pub market_name: Option<&'static str>,
    pub sector_name: Option<&'static str>,
    pub product_category: Option<&'static str>,
}

pub(crate) struct MockEquitiesMasterBuilder<'a> {
    server: &'a MockServer,
    entries: Vec<MockEquitiesMasterEntry>,
}

impl<'a> MockEquitiesMasterBuilder<'a> {
    pub fn entries(mut self, entries: Vec<MockEquitiesMasterEntry>) -> Self {
        self.entries = entries;
        self
    }

    /// `code` クエリパラメータを付けない (全銘柄取得) リクエストにのみマッチする
    pub async fn ok(self) {
        let data: Vec<serde_json::Value> = self
            .entries
            .iter()
            .map(|e| {
                json!({
                    "Code": e.code,
                    "CoName": e.company_name,
                    "MktNm": e.market_name,
                    "S33Nm": e.sector_name,
                    "ProdCat": e.product_category,
                })
            })
            .collect();

        Mock::given(method("GET"))
            .and(path("/equities/master"))
            .and(query_param_is_missing("code"))
            .and(header("x-api-key", "test-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": data,
            })))
            .mount(self.server)
            .await;
    }
}

/// テスト用の空売り残高報告レコード
pub(crate) struct MockShortSaleReport {
    pub code: &'static str,
    pub ss_name: &'static str,
    pub short_position_ratio: f64,
    pub prev_report_date: &'static str,
}

pub(crate) struct MockShortSaleReportBuilder<'a> {
    server: &'a MockServer,
    disc_date: &'a str,
    reports: Vec<MockShortSaleReport>,
}

impl<'a> MockShortSaleReportBuilder<'a> {
    pub fn disc_date(mut self, disc_date: &'a str) -> Self {
        self.disc_date = disc_date;
        self
    }

    pub fn reports(mut self, reports: Vec<MockShortSaleReport>) -> Self {
        self.reports = reports;
        self
    }

    pub async fn ok(self) {
        let disc_date = self.disc_date;
        let data: Vec<serde_json::Value> = self
            .reports
            .iter()
            .map(|r| {
                json!({
                    "DiscDate": disc_date,
                    "CalcDate": disc_date,
                    "Code": r.code,
                    "SSName": r.ss_name,
                    "SSAddr": "テスト住所",
                    "DICName": "テスト委託者",
                    "DICAddr": "テスト住所",
                    "FundName": "",
                    "ShrtPosToSO": r.short_position_ratio,
                    "ShrtPosShares": 1000,
                    "ShrtPosUnits": 10,
                    "PrevRptDate": r.prev_report_date,
                    "PrevRptRatio": null,
                    "Notes": "",
                })
            })
            .collect();

        Mock::given(method("GET"))
            .and(path("/markets/short-sale-report"))
            .and(query_param("disc_date", disc_date))
            .and(header("x-api-key", "test-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": data,
                "pagination_key": null,
            })))
            .mount(self.server)
            .await;
    }
}

/// テスト用の業種別空売り比率レコード
pub(crate) struct MockShortRatio {
    pub sector33_code: &'static str,
    pub sell_excluding_short_value: Option<f64>,
}

pub(crate) struct MockShortRatioBuilder<'a> {
    server: &'a MockServer,
    date: &'a str,
    ratios: Vec<MockShortRatio>,
}

impl<'a> MockShortRatioBuilder<'a> {
    pub fn date(mut self, date: &'a str) -> Self {
        self.date = date;
        self
    }

    pub fn ratios(mut self, ratios: Vec<MockShortRatio>) -> Self {
        self.ratios = ratios;
        self
    }

    pub async fn ok(self) {
        let date = self.date;
        let data: Vec<serde_json::Value> = self
            .ratios
            .iter()
            .map(|r| {
                json!({
                    "Date": date,
                    "S33": r.sector33_code,
                    "SellExShortVa": r.sell_excluding_short_value,
                    "ShrtWithResVa": r.sell_excluding_short_value,
                    "ShrtNoResVa": r.sell_excluding_short_value,
                })
            })
            .collect();

        Mock::given(method("GET"))
            .and(path("/markets/short-ratio"))
            .and(query_param("date", date))
            .and(header("x-api-key", "test-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": data,
                "pagination_key": null,
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

    /// 契約範囲外の日付を指定したときに J-Quants API が返す 400 エラー
    pub async fn subscription_range(self, endpoint_path: &str, from: &str, to: &str) {
        Mock::given(method("GET"))
            .and(path(endpoint_path))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "message": format!(
                    "Your subscription covers the following dates: {from} ~ {to}.\nIf you want more data, please check other plans:https://jpx-jquants.com/#dataset"
                ),
            })))
            .mount(self.server)
            .await;
    }
}

/// テスト用の信用取引週末残高 1 行
pub(crate) struct MockMarginInterestRow {
    pub date: &'static str,
    pub code: &'static str,
    pub iss_type: &'static str,
    pub shrt_vol: f64,
    pub long_vol: f64,
    pub shrt_neg_vol: f64,
    pub long_neg_vol: f64,
    pub shrt_std_vol: f64,
    pub long_std_vol: f64,
    pub shrt_val: Option<f64>,
    pub long_val: Option<f64>,
    pub shrt_neg_val: Option<f64>,
    pub long_neg_val: Option<f64>,
    pub shrt_std_val: Option<f64>,
    pub long_std_val: Option<f64>,
}

pub(crate) struct MockMarginInterestBuilder<'a> {
    server: &'a MockServer,
    date: &'a str,
    rows: Vec<MockMarginInterestRow>,
}

impl<'a> MockMarginInterestBuilder<'a> {
    pub fn date(mut self, date: &'a str) -> Self {
        self.date = date;
        self
    }

    pub fn rows(mut self, rows: Vec<MockMarginInterestRow>) -> Self {
        self.rows = rows;
        self
    }

    pub async fn ok(self) {
        let data: Vec<serde_json::Value> = self
            .rows
            .iter()
            .map(|r| {
                json!({
                    "Date": r.date,
                    "Code": r.code,
                    "IssType": r.iss_type,
                    "ShrtVol": r.shrt_vol,
                    "LongVol": r.long_vol,
                    "ShrtNegVol": r.shrt_neg_vol,
                    "LongNegVol": r.long_neg_vol,
                    "ShrtStdVol": r.shrt_std_vol,
                    "LongStdVol": r.long_std_vol,
                    "ShrtVal": r.shrt_val,
                    "LongVal": r.long_val,
                    "ShrtNegVal": r.shrt_neg_val,
                    "LongNegVal": r.long_neg_val,
                    "ShrtStdVal": r.shrt_std_val,
                    "LongStdVal": r.long_std_val,
                })
            })
            .collect();

        Mock::given(method("GET"))
            .and(path("/markets/margin-interest"))
            .and(query_param("date", self.date))
            .and(header("x-api-key", "test-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": data,
                "pagination_key": null,
            })))
            .mount(self.server)
            .await;
    }
}

/// テスト用の日々公表信用取引残高 1 行。文字列/数値どちらも入りうるフィールドは
/// serde_json::Value で渡すことで "-"/"*" のテストケースも表現できるようにする。
pub(crate) struct MockMarginAlertRow {
    pub pub_date: &'static str,
    pub code: &'static str,
    pub app_date: &'static str,
    pub shrt_out: f64,
    pub long_out: f64,
    pub shrt_out_chg: serde_json::Value,
    pub long_out_chg: serde_json::Value,
    pub shrt_out_ratio: serde_json::Value,
    pub long_out_ratio: serde_json::Value,
    pub sl_ratio: serde_json::Value,
    pub shrt_neg_out: f64,
    pub shrt_std_out: f64,
    pub long_neg_out: f64,
    pub long_std_out: f64,
    pub tse_mrgn_reg_cls: &'static str,
}

pub(crate) struct MockMarginAlertBuilder<'a> {
    server: &'a MockServer,
    date: &'a str,
    rows: Vec<MockMarginAlertRow>,
}

impl<'a> MockMarginAlertBuilder<'a> {
    pub fn date(mut self, date: &'a str) -> Self {
        self.date = date;
        self
    }

    pub fn rows(mut self, rows: Vec<MockMarginAlertRow>) -> Self {
        self.rows = rows;
        self
    }

    pub async fn ok(self) {
        let data: Vec<serde_json::Value> = self
            .rows
            .iter()
            .map(|r| {
                json!({
                    "PubDate": r.pub_date,
                    "Code": r.code,
                    "AppDate": r.app_date,
                    "PubReason": {
                        "Restricted": "0",
                        "DailyPublication": "1",
                        "Monitoring": "0",
                        "RestrictedByJSF": "0",
                        "PrecautionByJSF": "0",
                        "UnclearOrSecOnAlert": "0",
                    },
                    "ShrtOut": r.shrt_out,
                    "LongOut": r.long_out,
                    "ShrtOutChg": r.shrt_out_chg,
                    "LongOutChg": r.long_out_chg,
                    "ShrtOutRatio": r.shrt_out_ratio,
                    "LongOutRatio": r.long_out_ratio,
                    "SLRatio": r.sl_ratio,
                    "ShrtNegOut": r.shrt_neg_out,
                    "ShrtStdOut": r.shrt_std_out,
                    "LongNegOut": r.long_neg_out,
                    "LongStdOut": r.long_std_out,
                    "TSEMrgnRegCls": r.tse_mrgn_reg_cls,
                })
            })
            .collect();

        Mock::given(method("GET"))
            .and(path("/markets/margin-alert"))
            .and(query_param("date", self.date))
            .and(header("x-api-key", "test-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": data,
                "pagination_key": null,
            })))
            .mount(self.server)
            .await;
    }
}
