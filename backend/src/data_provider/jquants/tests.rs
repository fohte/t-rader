use chrono::NaiveDate;
use rstest::rstest;
use rust_decimal::Decimal;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::data_provider::jquants::mock::{JQuantsMockServer, MockBar};
use crate::data_provider::{DataProvider, DataProviderError, DateRange};

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap_or_default()
}

fn dec(v: f64) -> Decimal {
    Decimal::try_from(v).unwrap_or_default()
}

fn default_range() -> DateRange {
    DateRange {
        from: date(2025, 1, 6),
        to: date(2025, 1, 10),
    }
}

fn sample_bar(date_str: &str, close: f64) -> MockBar {
    MockBar {
        date: date_str.to_string(),
        code: "86970".to_string(),
        adj_open: Some(100.0),
        adj_high: Some(110.0),
        adj_low: Some(95.0),
        adj_close: Some(close),
        adj_volume: Some(1000.0),
    }
}

// === fetch_daily_bars ===

mod fetch_daily_bars {
    use super::*;

    #[rstest]
    #[tokio::test]
    async fn test_parses_single_bar() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        // API クエリには引数の instrument_id (4 桁) を送り、
        // レスポンスの Code は 5 桁で返る
        mock.daily_bars()
            .code("8697")
            .bars(vec![sample_bar("2025-01-06", 105.0)])
            .ok()
            .await;

        let client = mock.client()?;
        let bars = client.fetch_daily_bars("8697", &default_range()).await?;

        assert_eq!(bars.len(), 1);
        // レスポンスの Code (5 桁 "86970") ではなく引数の instrument_id (4 桁 "8697") が使われること
        assert_eq!(bars[0].instrument_id, "8697");
        assert_eq!(bars[0].open, dec(100.0));
        assert_eq!(bars[0].high, dec(110.0));
        assert_eq!(bars[0].low, dec(95.0));
        assert_eq!(bars[0].close, dec(105.0));
        assert_eq!(bars[0].volume, 1000);
        Ok(())
    }

    #[rstest]
    #[case::all_null(MockBar {
        date: "2025-01-07".to_string(),
        code: "86970".to_string(),
        adj_open: None,
        adj_high: None,
        adj_low: None,
        adj_close: None,
        adj_volume: None,
    })]
    #[case::partial_null(MockBar {
        date: "2025-01-07".to_string(),
        code: "86970".to_string(),
        adj_open: None,
        adj_high: Some(110.0),
        adj_low: Some(95.0),
        adj_close: Some(100.0),
        adj_volume: Some(1000.0),
    })]
    #[tokio::test]
    async fn test_skips_bars_with_null_prices(
        #[case] null_bar: MockBar,
    ) -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        mock.daily_bars()
            .code("8697")
            .bars(vec![sample_bar("2025-01-06", 105.0), null_bar])
            .ok()
            .await;

        let client = mock.client()?;
        let bars = client.fetch_daily_bars("8697", &default_range()).await?;

        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].close, dec(105.0));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_returns_empty_vec_when_no_data() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        mock.daily_bars().code("8697").bars(vec![]).ok().await;

        let client = mock.client()?;
        let bars = client.fetch_daily_bars("8697", &default_range()).await?;

        assert!(bars.is_empty());
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_bars_sorted_by_timestamp() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        mock.daily_bars()
            .code("8697")
            .bars(vec![
                sample_bar("2025-01-08", 103.0),
                sample_bar("2025-01-06", 100.0),
                sample_bar("2025-01-07", 102.0),
            ])
            .ok()
            .await;

        let client = mock.client()?;
        let bars = client.fetch_daily_bars("8697", &default_range()).await?;

        assert_eq!(bars.len(), 3);
        for pair in bars.windows(2) {
            assert!(pair[0].timestamp <= pair[1].timestamp);
        }
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_pagination_fetches_all_pages() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;

        // 1 ページ目: pagination_key を含むレスポンス (1 回のみマッチ)
        mock.daily_bars()
            .code("8697")
            .bars(vec![sample_bar("2025-01-06", 100.0)])
            .pagination_key("page2")
            .up_to_n_times(1)
            .ok()
            .await;

        // 2 ページ目: pagination_key なし (最終ページ)
        mock.daily_bars()
            .code("8697")
            .bars(vec![sample_bar("2025-01-07", 102.0)])
            .with_pagination_key_param("page2")
            .ok()
            .await;

        let client = mock.client()?;
        let bars = client.fetch_daily_bars("8697", &default_range()).await?;

        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].close, dec(100.0));
        assert_eq!(bars[1].close, dec(102.0));
        Ok(())
    }
}

mod fetch_daily_bars_by_date {
    use super::*;

    #[rstest]
    #[tokio::test]
    async fn test_normalizes_ordinary_stock_code_to_4_digits() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        mock.daily_bars_by_date()
            .date("2025-01-06")
            .bars(vec![MockBar {
                code: "86970".to_string(),
                ..sample_bar("2025-01-06", 105.0)
            }])
            .ok()
            .await;

        let client = mock.client()?;
        let bars = client.fetch_daily_bars_by_date(date(2025, 1, 6)).await?;

        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].instrument_id, "8697");
        assert_eq!(bars[0].close, dec(105.0));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_keeps_non_ordinary_stock_code_as_is() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        mock.daily_bars_by_date()
            .date("2025-01-06")
            .bars(vec![MockBar {
                date: "2025-01-06".to_string(),
                code: "86971".to_string(),
                adj_open: Some(100.0),
                adj_high: Some(110.0),
                adj_low: Some(95.0),
                adj_close: Some(105.0),
                adj_volume: Some(1000.0),
            }])
            .ok()
            .await;

        let client = mock.client()?;
        let bars = client.fetch_daily_bars_by_date(date(2025, 1, 6)).await?;

        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].instrument_id, "86971");
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_returns_multiple_instruments_for_the_date() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        mock.daily_bars_by_date()
            .date("2025-01-06")
            .bars(vec![
                MockBar {
                    code: "72030".to_string(),
                    ..sample_bar("2025-01-06", 100.0)
                },
                MockBar {
                    code: "67580".to_string(),
                    ..sample_bar("2025-01-06", 200.0)
                },
            ])
            .ok()
            .await;

        let client = mock.client()?;
        let mut bars = client.fetch_daily_bars_by_date(date(2025, 1, 6)).await?;
        bars.sort_by(|a, b| a.instrument_id.cmp(&b.instrument_id));

        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].instrument_id, "6758");
        assert_eq!(bars[1].instrument_id, "7203");
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_skips_bars_with_null_prices() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        mock.daily_bars_by_date()
            .date("2025-01-06")
            .bars(vec![MockBar {
                date: "2025-01-06".to_string(),
                code: "72030".to_string(),
                adj_open: None,
                adj_high: None,
                adj_low: None,
                adj_close: None,
                adj_volume: None,
            }])
            .ok()
            .await;

        let client = mock.client()?;
        let bars = client.fetch_daily_bars_by_date(date(2025, 1, 6)).await?;

        assert!(bars.is_empty());
        Ok(())
    }
}

// === fetch_edinet_documents ===

mod fetch_edinet_documents {
    use super::*;

    #[rstest]
    #[tokio::test]
    async fn test_parses_single_document() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        let doc = json!({
            "DocId": "S100ABCD",
            "Code": "72030",
            "EdinetCode": "E00001",
            "SubDate": "2025-01-06",
        });
        mock.edinet_documents("/edinet/large-volume-shareholders")
            .docs(vec![doc.clone()])
            .ok()
            .await;

        let client = mock.client()?;
        let docs = client
            .fetch_edinet_documents("/edinet/large-volume-shareholders", date(2025, 1, 6))
            .await?;

        assert_eq!(docs, vec![doc]);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_returns_empty_vec_when_no_data() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        mock.edinet_documents("/edinet/cross-shareholdings")
            .docs(vec![])
            .ok()
            .await;

        let client = mock.client()?;
        let docs = client
            .fetch_edinet_documents("/edinet/cross-shareholdings", date(2025, 1, 6))
            .await?;

        assert!(docs.is_empty());
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_pagination_fetches_all_pages() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        let doc1 = json!({
            "DocId": "S100ABCD",
            "Code": "72030",
            "EdinetCode": "E00001",
            "SubDate": "2025-01-06",
        });
        let doc2 = json!({
            "DocId": "S100EFGH",
            "Code": "67580",
            "EdinetCode": "E00002",
            "SubDate": "2025-01-06",
        });

        // 1 ページ目: pagination_key を含むレスポンス (1 回のみマッチ)
        mock.edinet_documents("/edinet/major-shareholders")
            .docs(vec![doc1.clone()])
            .pagination_key("page2")
            .up_to_n_times(1)
            .ok()
            .await;

        // 2 ページ目: pagination_key なし (最終ページ)
        mock.edinet_documents("/edinet/major-shareholders")
            .docs(vec![doc2.clone()])
            .with_pagination_key_param("page2")
            .ok()
            .await;

        let client = mock.client()?;
        let docs = client
            .fetch_edinet_documents("/edinet/major-shareholders", date(2025, 1, 6))
            .await?;

        assert_eq!(docs, vec![doc1, doc2]);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_sends_date_param_in_yyyymmdd_format() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        let doc = json!({
            "DocId": "S100ABCD",
            "Code": "72030",
            "EdinetCode": "E00001",
            "SubDate": "2025-01-06",
        });
        mock.edinet_documents("/edinet/large-volume-shareholders")
            .date("20250106")
            .docs(vec![doc.clone()])
            .ok()
            .await;

        let client = mock.client()?;
        let docs = client
            .fetch_edinet_documents("/edinet/large-volume-shareholders", date(2025, 1, 6))
            .await?;

        assert_eq!(docs, vec![doc]);
        Ok(())
    }
}

// === fetch_instrument ===

mod fetch_instrument {
    use super::*;

    #[rstest]
    #[tokio::test]
    async fn test_parses_instrument() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        mock.instrument()
            .code("72030")
            .company_name("トヨタ自動車")
            .sector_name(Some("輸送用機器"))
            .ok()
            .await;

        let client = mock.client()?;
        let instrument = client.fetch_instrument("72030").await?;

        assert_eq!(instrument.id, "72030");
        assert_eq!(instrument.name, "トヨタ自動車");
        assert_eq!(instrument.sector, Some("輸送用機器".to_string()));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_not_found_when_empty_response() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        mock.instrument().code("99999").not_found().await;

        let client = mock.client()?;
        let result = client.fetch_instrument("99999").await;

        assert!(matches!(result, Err(DataProviderError::NotFound(_))));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_sector_can_be_null() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        mock.instrument()
            .code("86970")
            .company_name("日本取引所グループ")
            .sector_name(None)
            .ok()
            .await;

        let client = mock.client()?;
        let instrument = client.fetch_instrument("86970").await?;

        assert!(instrument.sector.is_none());
        Ok(())
    }
}

// === エラーハンドリング ===

mod error_handling {
    use super::*;

    #[rstest]
    #[tokio::test]
    async fn test_403_returns_api_error() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        mock.error().forbidden("/equities/master").await;

        let client = mock.client()?;
        let result = client.fetch_instrument("86970").await;

        assert!(matches!(
            result,
            Err(DataProviderError::Api { status: 403, .. })
        ));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_429_retries_then_succeeds() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;

        // 最初の 2 回は 429 を返し、3 回目で成功する
        Mock::given(method("GET"))
            .and(path("/equities/master"))
            .and(query_param("code", "86970"))
            .respond_with(ResponseTemplate::new(429).set_body_json(json!({
                "message": "Too Many Requests",
            })))
            .up_to_n_times(2)
            .mount(mock.server_ref())
            .await;

        Mock::given(method("GET"))
            .and(path("/equities/master"))
            .and(query_param("code", "86970"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{
                    "Code": "86970",
                    "CoName": "日本取引所グループ",
                    "MktNm": "プライム",
                    "S33Nm": "その他金融業",
                }],
            })))
            .mount(mock.server_ref())
            .await;

        let client = mock.client()?;
        let instrument = client.fetch_instrument("86970").await?;

        assert_eq!(instrument.id, "86970");
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_429_exhausts_retries() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        mock.error().rate_limited("/equities/master").await;

        let client = mock.client()?;
        let result = client.fetch_instrument("86970").await;

        assert!(matches!(result, Err(DataProviderError::RateLimited { .. })));
        Ok(())
    }
}

// === fetch_fin_summary_by_date ===

mod fetch_fin_summary_by_date {
    use super::*;
    use crate::data_provider::jquants::FIN_SUMMARY_RATE_LIMIT_PER_MINUTE;
    use crate::data_provider::jquants::JQuantsClient;
    use crate::models::jquants_plan::JQuantsPlan;

    #[rstest]
    #[tokio::test]
    async fn test_returns_raw_items_unchanged() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        let item = json!({
            "DiscDate": "2025-01-06",
            "Code": "72030",
            "DiscNo": "1",
            "Sales": "1000000",
            "OdP": "",
        });
        mock.fin_summary()
            .date("2025-01-06")
            .items(vec![item.clone()])
            .ok()
            .await;

        let client = mock.client()?;
        let items = client.fetch_fin_summary_by_date(date(2025, 1, 6)).await?;

        assert_eq!(items, vec![item]);
        Ok(())
    }

    /// `/fins/summary` は契約プランと別枠で 60 req/分の上限があるため、契約プランの上限
    /// (Standard=120, Premium=500) がそれより高くても 60 に抑えられる必要がある
    #[rstest]
    #[case::free(JQuantsPlan::Free, 5)]
    #[case::light(JQuantsPlan::Light, 60)]
    #[case::standard(JQuantsPlan::Standard, 60)]
    #[case::premium(JQuantsPlan::Premium, 60)]
    fn test_rate_limit_never_exceeds_endpoint_specific_cap(
        #[case] plan: JQuantsPlan,
        #[case] expected: usize,
    ) {
        let client = JQuantsClient::new("test-api-key".to_string()).expect("client");
        client.set_manual_plan(Some(plan));

        let capped = client
            .current_rate_limit()
            .min(FIN_SUMMARY_RATE_LIMIT_PER_MINUTE);

        assert_eq!(capped, expected);
    }
}

// === レートリミッター ===

mod rate_limiter {
    use super::super::{RATE_LIMIT_MAX_REQUESTS, RATE_LIMIT_WINDOW, RateLimiter};
    use rstest::rstest;

    #[rstest]
    #[case::default_limit(RATE_LIMIT_MAX_REQUESTS)]
    #[case::higher_limit(RATE_LIMIT_MAX_REQUESTS * 2)]
    #[tokio::test]
    async fn test_allows_requests_within_limit(#[case] limit: usize) {
        let limiter = RateLimiter::new();

        // 上限以内のリクエストは即座に通過する
        for _ in 0..limit {
            limiter.acquire(limit).await;
        }
    }

    #[rstest]
    #[tokio::test(start_paused = true)]
    async fn test_blocks_when_limit_exceeded() {
        let limiter = RateLimiter::new();

        // 上限まで消費
        for _ in 0..RATE_LIMIT_MAX_REQUESTS {
            limiter.acquire(RATE_LIMIT_MAX_REQUESTS).await;
        }

        // 次の acquire は待機するはず
        let acquire_future = limiter.acquire(RATE_LIMIT_MAX_REQUESTS);
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(100), acquire_future).await;
        assert!(result.is_err(), "上限超過時に acquire がブロックされるべき");

        // ウィンドウを経過させると通過する
        tokio::time::advance(RATE_LIMIT_WINDOW).await;
        let acquire_future = limiter.acquire(RATE_LIMIT_MAX_REQUESTS);
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(100), acquire_future).await;
        assert!(result.is_ok(), "ウィンドウ経過後に acquire が通過するべき");
    }
}

// === 契約範囲の自己検出 (400 エラーメッセージからの検出) ===

mod subscription_range_detection {
    use super::*;

    #[rstest]
    #[tokio::test]
    async fn test_detects_range_from_400_and_retries_successfully() -> Result<(), DataProviderError>
    {
        let mock = JQuantsMockServer::start().await;

        // 検出後の再取得リクエスト。先に mount することで、from/to が一致するリクエストは
        // こちらが優先される (wiremock は同一 priority ならマウント順を優先する)
        Mock::given(method("GET"))
            .and(path("/equities/bars/daily"))
            .and(query_param("code", "8697"))
            .and(query_param("from", "20200401"))
            .and(query_param("to", "20220401"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{
                    "Date": "2025-01-06",
                    "Code": "86970",
                    "AdjO": 100.0,
                    "AdjH": 110.0,
                    "AdjL": 95.0,
                    "AdjC": 105.0,
                    "AdjVo": 1000.0,
                }],
                "pagination_key": null,
            })))
            .mount(mock.server_ref())
            .await;

        // 契約範囲外を指定した初回リクエストへの応答。このメッセージから契約範囲を検出する
        mock.error()
            .subscription_range("/equities/bars/daily", "2020-04-01", "2022-04-01")
            .await;

        let client = mock.client()?;
        let bars = client.fetch_daily_bars("8697", &default_range()).await?;

        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].close, dec(105.0));
        assert_eq!(
            client.known_fetchable_range(),
            Some((date(2020, 4, 1), date(2022, 4, 1)))
        );
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_unparsable_400_message_is_returned_as_is() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/equities/bars/daily"))
            .and(query_param("code", "8697"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "message": "Bad Request",
            })))
            .mount(mock.server_ref())
            .await;

        let client = mock.client()?;
        let result = client.fetch_daily_bars("8697", &default_range()).await;

        assert!(matches!(
            result,
            Err(DataProviderError::Api { status: 400, .. })
        ));
        assert_eq!(client.known_fetchable_range(), None);
        Ok(())
    }
}

// === 手動設定プラン (`set_manual_plan`) の優先順位 ===

mod manual_plan_priority {
    use rstest::{fixture, rstest};

    use crate::data_provider::DataProvider;
    use crate::models::jquants_plan::JQuantsPlan;

    use super::super::JQuantsClient;
    use super::date;

    #[fixture]
    fn client() -> JQuantsClient {
        JQuantsClient::with_base_url("http://localhost", "key").expect("client")
    }

    #[rstest]
    fn falls_back_to_detected_range_when_manual_plan_is_unset(client: JQuantsClient) {
        client.set_detected_range((date(2020, 4, 1), date(2022, 4, 1)));

        assert_eq!(
            client.known_fetchable_range(),
            Some((date(2020, 4, 1), date(2022, 4, 1)))
        );
    }

    #[rstest]
    fn manual_plan_overrides_already_detected_range(client: JQuantsClient) {
        client.set_detected_range((date(2020, 4, 1), date(2022, 4, 1)));
        client.set_manual_plan(Some(JQuantsPlan::Standard));

        let today = chrono::Utc::now().date_naive();
        assert_eq!(
            client.known_fetchable_range(),
            Some(JQuantsPlan::Standard.range(today))
        );
    }

    #[rstest]
    fn clearing_manual_plan_restores_detected_range(client: JQuantsClient) {
        client.set_detected_range((date(2020, 4, 1), date(2022, 4, 1)));
        client.set_manual_plan(Some(JQuantsPlan::Standard));
        client.set_manual_plan(None);

        assert_eq!(
            client.known_fetchable_range(),
            Some((date(2020, 4, 1), date(2022, 4, 1)))
        );
    }

    #[rstest]
    fn current_rate_limit_falls_back_to_free_plan_when_manual_plan_is_unset(client: JQuantsClient) {
        assert_eq!(
            client.current_rate_limit(),
            JQuantsPlan::Free.rate_limit_per_minute()
        );
    }

    #[rstest]
    fn current_rate_limit_follows_manual_plan(client: JQuantsClient) {
        client.set_manual_plan(Some(JQuantsPlan::Standard));

        assert_eq!(
            client.current_rate_limit(),
            JQuantsPlan::Standard.rate_limit_per_minute()
        );
    }
}

// === 検出範囲からの初回プラン推定・永続化 (`persist_inferred_range_if_needed`) ===

mod persist_inferred_range_if_needed {
    use chrono::Duration;
    use sqlx::PgPool;

    use crate::data_provider::DataProvider;
    use crate::models::{JQuantsPlan, JQuantsPlanSettingData, parse_plan_setting};
    use crate::services::jquants_plan_setting;
    use crate::testing::create_test_db;

    use super::super::JQuantsClient;
    use super::date;

    // rstest の #[fixture] は #[sqlx::test] と組み合わせられないため、プレーンな
    // ヘルパー関数として抽出する
    fn test_client() -> JQuantsClient {
        JQuantsClient::with_base_url("http://localhost", "key").expect("client")
    }

    #[sqlx::test(migrations = false)]
    async fn does_nothing_when_manual_plan_already_set(pool: PgPool) {
        let db = create_test_db(pool).await;
        let client = test_client();
        client.set_detected_range((date(2020, 4, 1), date(2022, 4, 1)));
        client.set_manual_plan(Some(JQuantsPlan::Premium));

        client
            .persist_inferred_range_if_needed(&db)
            .await
            .expect("persist");

        let current = jquants_plan_setting::find_current(&db)
            .await
            .expect("query");
        assert_eq!(current, None, "手動設定がある間は DB に書き込まない");
        assert_eq!(client.manual_plan(), Some(JQuantsPlan::Premium));
    }

    #[sqlx::test(migrations = false)]
    async fn does_nothing_when_no_range_detected(pool: PgPool) {
        let db = create_test_db(pool).await;
        let client = test_client();

        client
            .persist_inferred_range_if_needed(&db)
            .await
            .expect("persist");

        let current = jquants_plan_setting::find_current(&db)
            .await
            .expect("query");
        assert_eq!(current, None);
        assert_eq!(client.manual_plan(), None);
    }

    #[sqlx::test(migrations = false)]
    async fn infers_and_persists_plan_from_detected_range_once(pool: PgPool) {
        let db = create_test_db(pool).await;
        let client = test_client();
        // Standard の提供期間 (3650 日) ちょうどの範囲を検出させる
        let from = date(2010, 1, 1);
        let to = from + Duration::days(3650);
        client.set_detected_range((from, to));

        client
            .persist_inferred_range_if_needed(&db)
            .await
            .expect("persist");

        assert_eq!(client.manual_plan(), Some(JQuantsPlan::Standard));

        let saved = jquants_plan_setting::find_current(&db)
            .await
            .expect("query")
            .expect("row exists");
        let data = parse_plan_setting::<JQuantsPlanSettingData>(saved.plan_setting).expect("parse");
        assert_eq!(data.plan, Some(JQuantsPlan::Standard));
    }

    #[sqlx::test(migrations = false)]
    async fn does_not_overwrite_when_already_persisted_in_db(pool: PgPool) {
        let db = create_test_db(pool).await;
        let client = test_client();
        let from = date(2010, 1, 1);
        let to = from + Duration::days(3650);
        client.set_detected_range((from, to));
        client
            .persist_inferred_range_if_needed(&db)
            .await
            .expect("first persist");

        // 別プロセス/リクエストが先に推定・永続化した状態を模すため、in-memory の
        // manual_plan だけをクリアし、別範囲を検出させる
        client.set_manual_plan(None);
        let other_from = date(2005, 1, 1);
        let other_to = other_from + Duration::days(730);
        client.set_detected_range((other_from, other_to));

        client
            .persist_inferred_range_if_needed(&db)
            .await
            .expect("second persist");

        let saved = jquants_plan_setting::find_current(&db)
            .await
            .expect("query")
            .expect("row exists");
        let data = parse_plan_setting::<JQuantsPlanSettingData>(saved.plan_setting).expect("parse");
        assert_eq!(
            data.plan,
            Some(JQuantsPlan::Standard),
            "初回の推定結果が保持され、2 回目の検出では上書きされないこと"
        );
    }
}

// === 検出済み契約範囲の TTL (`effective_range`) ===

mod detected_range_ttl {
    use chrono::Duration;
    use rstest::rstest;

    use super::super::{DETECTED_RANGE_TTL_DAYS, DetectedRange, effective_range};
    use super::date;

    #[rstest]
    #[case::same_day_still_valid(0, Some((date(2020, 1, 1), date(2020, 4, 1))))]
    #[case::at_ttl_boundary_expired(DETECTED_RANGE_TTL_DAYS, None)]
    #[case::well_past_ttl_expired(DETECTED_RANGE_TTL_DAYS + 4, None)]
    fn effective_range_expires_after_ttl(
        #[case] days_elapsed: i64,
        #[case] expected: Option<(chrono::NaiveDate, chrono::NaiveDate)>,
    ) {
        let detected_at = date(2020, 4, 1);
        let detected = DetectedRange {
            from: date(2020, 1, 1),
            to: date(2020, 4, 1),
            detected_at,
        };

        let today = detected_at + Duration::days(days_elapsed);
        assert_eq!(effective_range(Some(&detected), today), expected);
    }

    #[test]
    fn effective_range_returns_none_when_nothing_detected_yet() {
        assert_eq!(effective_range(None, date(2020, 4, 1)), None);
    }
}

// === DataProviderKind ===

mod data_provider_kind {
    use super::*;
    use crate::data_provider::DataProviderKind;

    #[rstest]
    #[tokio::test]
    async fn test_delegates_fetch_instrument_to_jquants() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        mock.instrument()
            .code("72030")
            .company_name("トヨタ自動車")
            .sector_name(Some("輸送用機器"))
            .ok()
            .await;

        let client = mock.client()?;
        let kind = DataProviderKind::JQuants(client);
        let instrument = kind.fetch_instrument("72030").await?;

        assert_eq!(instrument.id, "72030");
        assert_eq!(instrument.name, "トヨタ自動車");
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_delegates_fetch_daily_bars_to_jquants() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        mock.daily_bars()
            .code("8697")
            .bars(vec![sample_bar("2025-01-06", 105.0)])
            .ok()
            .await;

        let client = mock.client()?;
        let kind = DataProviderKind::JQuants(client);
        let bars = kind.fetch_daily_bars("8697", &default_range()).await?;

        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].close, dec(105.0));
        Ok(())
    }
}
