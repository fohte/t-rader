use chrono::NaiveDate;
use rstest::rstest;
use rust_decimal::Decimal;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::data_provider::jquants::mock::{JQuantsMockServer, MockBar};
use crate::data_provider::{DailyBarSource, DailyBarSourceError, DataProviderError, DateRange};

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
    async fn test_parses_single_bar() -> Result<(), DailyBarSourceError> {
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
    ) -> Result<(), DailyBarSourceError> {
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
    async fn test_returns_empty_vec_when_no_data() -> Result<(), DailyBarSourceError> {
        let mock = JQuantsMockServer::start().await;
        mock.daily_bars().code("8697").bars(vec![]).ok().await;

        let client = mock.client()?;
        let bars = client.fetch_daily_bars("8697", &default_range()).await?;

        assert!(bars.is_empty());
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_bars_sorted_by_timestamp() -> Result<(), DailyBarSourceError> {
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
    async fn test_pagination_fetches_all_pages() -> Result<(), DailyBarSourceError> {
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

    #[rstest]
    #[tokio::test]
    async fn test_returns_bad_request_as_source_error() -> Result<(), DailyBarSourceError> {
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

        assert_eq!(
            client.fetch_daily_bars("8697", &default_range()).await,
            Err(DailyBarSourceError::Failed(
                "api error (status 400): Bad Request".to_string(),
            )),
        );
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
            .product_category(Some("011"))
            .ok()
            .await;

        let client = mock.client()?;
        let instrument = client.fetch_instrument("72030").await?;

        assert_eq!(instrument.id, "72030");
        assert_eq!(instrument.name, "トヨタ自動車");
        assert_eq!(instrument.sector, Some("輸送用機器".to_string()));
        assert_eq!(instrument.product_category, Some("011".to_string()));
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
            .product_category(None)
            .ok()
            .await;

        let client = mock.client()?;
        let instrument = client.fetch_instrument("86970").await?;

        assert!(instrument.sector.is_none());
        assert!(instrument.product_category.is_none());
        Ok(())
    }
}

// === エラーハンドリング ===

mod error_handling {
    use super::*;
    use crate::models::JQuantsPlan;

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
    async fn test_429_waits_for_cooldown_then_succeeds() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;

        // 最初の 2 回は 429 を返し、3 回目 (2 回の cooldown 明け後) で成功する
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
    async fn test_429_exhausts_cooldown_retries() -> Result<(), DataProviderError> {
        let mock = JQuantsMockServer::start().await;
        mock.error().rate_limited("/equities/master").await;

        let client = mock.client()?;
        let result = client.fetch_instrument("86970").await;

        assert!(matches!(result, Err(DataProviderError::RateLimited { .. })));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_client_fails_immediately_when_window_limit_is_exceeded() {
        let mock = JQuantsMockServer::start().await;
        mock.instrument().code("00001").ok().await;

        let client = mock.client_with_plan(JQuantsPlan::Free).expect("client");
        let max_requests = client.current_rate_limit();
        for _ in 0..max_requests {
            client
                .fetch_instrument("00001")
                .await
                .expect("request within the limit should succeed");
        }

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            client.fetch_instrument("00001"),
        )
        .await
        .expect("test client should fail fast instead of waiting for the window");

        assert_eq!(
            result.map(|_| ()),
            Err(DataProviderError::RateLimitWindowFull { max_requests }),
        );
    }
}

// === fetch_fin_summary_by_date ===

mod fetch_fin_summary_by_date {
    use super::*;
    use crate::data_provider::jquants::FIN_SUMMARY_RATE_LIMIT_PER_MINUTE;
    use crate::data_provider::jquants::JQuantsClient;
    use crate::data_provider::jquants::apply_safety_margin;
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
    /// (Standard=120, Premium=500) がそれより高くても 60 に抑えられる必要がある。
    /// 両者とも安全マージン (半分) を適用した値同士の min になる
    #[rstest]
    #[case::free(JQuantsPlan::Free, 2)]
    #[case::light(JQuantsPlan::Light, 30)]
    #[case::standard(JQuantsPlan::Standard, 30)]
    #[case::premium(JQuantsPlan::Premium, 30)]
    fn test_rate_limit_never_exceeds_endpoint_specific_cap(
        #[case] plan: JQuantsPlan,
        #[case] expected: usize,
    ) {
        let client = JQuantsClient::new("test-api-key".to_string(), plan).expect("client");

        let capped = client
            .current_rate_limit()
            .min(apply_safety_margin(FIN_SUMMARY_RATE_LIMIT_PER_MINUTE));

        assert_eq!(capped, expected);
    }
}

// === レートリミッター ===

mod rate_limiter {
    use super::super::RATE_LIMIT_COOLDOWN;
    use super::super::rate_limiter::{RATE_LIMIT_WINDOW, RateLimiter};
    use crate::data_provider::DataProviderError;
    use rstest::rstest;

    const TEST_LIMIT: usize = 3;

    async fn fill_window(limiter: &RateLimiter, limit: usize) {
        for _ in 0..limit {
            limiter
                .acquire(limit)
                .await
                .expect("request within the limit should be allowed");
        }
    }

    #[rstest]
    #[case::base_limit(TEST_LIMIT)]
    #[case::higher_limit(TEST_LIMIT * 2)]
    #[tokio::test]
    async fn test_allows_requests_within_limit(#[case] limit: usize) {
        let limiter = RateLimiter::new(RATE_LIMIT_COOLDOWN);

        // 上限以内のリクエストは即座に通過する
        fill_window(&limiter, limit).await;
    }

    #[rstest]
    #[tokio::test(start_paused = true)]
    async fn test_blocks_when_limit_exceeded() {
        let limiter = RateLimiter::new(RATE_LIMIT_COOLDOWN);

        // 上限まで消費
        fill_window(&limiter, TEST_LIMIT).await;

        // 次の acquire は待機するはず
        let acquire_future = limiter.acquire(TEST_LIMIT);
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(100), acquire_future).await;
        assert!(result.is_err(), "上限超過時に acquire がブロックされるべき");

        // ウィンドウを経過させると通過する
        tokio::time::advance(RATE_LIMIT_WINDOW).await;
        let acquire_future = limiter.acquire(TEST_LIMIT);
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(100), acquire_future).await;
        assert_eq!(result.expect("window should expire"), Ok(()),);
    }

    #[rstest]
    #[tokio::test(start_paused = true)]
    async fn test_cooldown_blocks_all_acquires_until_elapsed() {
        let limiter = RateLimiter::new(RATE_LIMIT_COOLDOWN);

        // ウィンドウの空きがあっても、cooldown 中は acquire がブロックされる
        limiter.note_rate_limited().await;
        let acquire_future = limiter.acquire(TEST_LIMIT);
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(100), acquire_future).await;
        assert!(
            result.is_err(),
            "cooldown 中は acquire がブロックされるべき"
        );

        // cooldown を経過させると通過する
        tokio::time::advance(RATE_LIMIT_COOLDOWN).await;
        let acquire_future = limiter.acquire(TEST_LIMIT);
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(100), acquire_future).await;
        assert_eq!(result.expect("cooldown should expire"), Ok(()),);
    }

    #[rstest]
    #[tokio::test(start_paused = true)]
    async fn test_fails_immediately_when_limit_exceeded_with_fail_fast_behavior() {
        let limiter = RateLimiter::new_fail_fast(RATE_LIMIT_COOLDOWN);

        fill_window(&limiter, TEST_LIMIT).await;

        assert_eq!(
            limiter.acquire(TEST_LIMIT).await,
            Err(DataProviderError::RateLimitWindowFull {
                max_requests: TEST_LIMIT,
            }),
        );
    }
}

mod configured_plan {
    use super::super::JQuantsClient;
    use super::date;
    use crate::data_provider::DateRange;
    use crate::models::jquants_plan::JQuantsPlan;
    use rstest::rstest;

    #[rstest]
    #[case::free(JQuantsPlan::Free, 2)]
    #[case::light(JQuantsPlan::Light, 30)]
    #[case::standard(JQuantsPlan::Standard, 60)]
    #[case::premium(JQuantsPlan::Premium, 250)]
    fn required_plan_controls_fetchable_range_and_rate_limit(
        #[case] plan: JQuantsPlan,
        #[case] max_requests: usize,
    ) {
        let today = date(2025, 1, 10);
        let client = JQuantsClient::with_base_url("http://localhost", "key", plan).expect("client");

        assert_eq!(
            (client.plan_date_range(today), client.current_rate_limit(),),
            (
                DateRange {
                    from: plan.range(today).0,
                    to: plan.range(today).1
                },
                max_requests
            ),
        );
    }

    #[test]
    fn premium_daily_bars_range_starts_at_the_first_available_date() {
        let today = date(2025, 1, 10);
        let client = JQuantsClient::with_base_url("http://localhost", "key", JQuantsPlan::Premium)
            .expect("client");

        assert_eq!(
            client.daily_bars_date_range(today),
            DateRange {
                from: date(2008, 5, 7),
                to: JQuantsPlan::Premium.range(today).1,
            },
        );
    }
}

mod daily_bar_source {
    use super::*;
    use std::sync::Arc;

    use crate::data_provider::SharedDailyBarSource;

    #[rstest]
    #[tokio::test]
    async fn test_fetches_daily_bars_through_shared_source() {
        let mock = JQuantsMockServer::start().await;
        mock.daily_bars()
            .code("8697")
            .bars(vec![sample_bar("2025-01-06", 105.0)])
            .ok()
            .await;

        let client = Arc::new(mock.client().expect("client"));
        let source: SharedDailyBarSource = client;
        let bars = source
            .fetch_daily_bars("8697", &default_range())
            .await
            .expect("fetch daily bars");

        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].close, dec(105.0));
    }
}
