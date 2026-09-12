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

fn sample_bar(date_str: &'static str, close: f64) -> MockBar {
    MockBar {
        date: date_str,
        code: "86970",
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
        date: "2025-01-07",
        code: "86970",
        adj_open: None,
        adj_high: None,
        adj_low: None,
        adj_close: None,
        adj_volume: None,
    })]
    #[case::partial_null(MockBar {
        date: "2025-01-07",
        code: "86970",
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

// === レートリミッター ===

mod rate_limiter {
    use super::super::{RATE_LIMIT_MAX_REQUESTS, RATE_LIMIT_WINDOW, RateLimiter};
    use rstest::rstest;

    #[rstest]
    #[tokio::test]
    async fn test_allows_requests_within_limit() {
        let limiter = RateLimiter::new();

        // 上限以内のリクエストは即座に通過する
        for _ in 0..RATE_LIMIT_MAX_REQUESTS {
            limiter.acquire().await;
        }
    }

    #[rstest]
    #[tokio::test(start_paused = true)]
    async fn test_blocks_when_limit_exceeded() {
        let limiter = RateLimiter::new();

        // 上限まで消費
        for _ in 0..RATE_LIMIT_MAX_REQUESTS {
            limiter.acquire().await;
        }

        // 次の acquire は待機するはず
        let acquire_future = limiter.acquire();
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(100), acquire_future).await;
        assert!(result.is_err(), "上限超過時に acquire がブロックされるべき");

        // ウィンドウを経過させると通過する
        tokio::time::advance(RATE_LIMIT_WINDOW).await;
        let acquire_future = limiter.acquire();
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(100), acquire_future).await;
        assert!(result.is_ok(), "ウィンドウ経過後に acquire が通過するべき");
    }
}

// === 契約範囲の自己学習 (400 エラーメッセージからの学習) ===

mod subscription_range_learning {
    use super::*;

    #[rstest]
    #[tokio::test]
    async fn test_learns_range_from_400_and_retries_successfully() -> Result<(), DataProviderError>
    {
        let mock = JQuantsMockServer::start().await;

        // 学習後の再取得リクエスト。先に mount することで、from/to が一致するリクエストは
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

        // 契約範囲外を指定した初回リクエストへの応答。このメッセージから契約範囲を学習する
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
