use chrono::{NaiveDate, TimeZone, Utc};
use rstest::rstest;
use rust_decimal::Decimal;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::data_provider::ibkr::mock::{IbkrMockServer, MockHistoryBar};
use crate::data_provider::{DataProvider, DataProviderError, DataProviderKind, DateRange};
use crate::models::bar::{Bar, Timeframe};
use crate::models::instrument::Market;

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap_or_default()
}

fn dec(v: f64) -> Decimal {
    Decimal::try_from(v).unwrap_or_default()
}

/// 指定日 (UTC) の 00:00:00 を epoch ms に変換する
fn day_ms(year: i32, month: u32, day: u32) -> i64 {
    Utc.from_utc_datetime(
        &NaiveDate::from_ymd_opt(year, month, day)
            .unwrap_or_default()
            .and_hms_opt(0, 0, 0)
            .unwrap_or_default(),
    )
    .timestamp_millis()
}

fn default_range() -> DateRange {
    DateRange {
        from: date(2025, 1, 6),
        to: date(2025, 1, 10),
    }
}

// === fetch_instrument ===

mod fetch_instrument {
    use super::*;

    #[rstest]
    #[tokio::test]
    async fn test_resolves_via_tsej_contract() -> Result<(), DataProviderError> {
        let mock = IbkrMockServer::start().await;
        mock.stocks()
            .symbol("7203")
            .name(Some("TOYOTA MOTOR CORP"))
            .contracts(vec![("TSEJ", 12345)])
            .ok()
            .await;

        let client = mock.client()?;
        let instrument = client.fetch_instrument("7203").await?;

        assert_eq!(
            instrument,
            crate::models::instrument::Instrument {
                id: "7203".to_string(),
                name: "TOYOTA MOTOR CORP".to_string(),
                market: Market::Tse,
                sector: None,
            }
        );
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_returns_not_found_when_symbol_missing() {
        let mock = IbkrMockServer::start().await;
        mock.stocks().symbol("9999").not_found().await;

        let client = mock.client().unwrap();
        let result = client.fetch_instrument("9999").await;
        assert!(matches!(result, Err(DataProviderError::NotFound(_))));
    }

    #[rstest]
    #[tokio::test]
    async fn test_returns_not_found_when_exchange_mismatch() {
        let mock = IbkrMockServer::start().await;
        // 取引所が TSEJ 以外しかない場合は NotFound
        mock.stocks()
            .symbol("7203")
            .contracts(vec![("NASDAQ", 99)])
            .ok()
            .await;

        let client = mock.client().unwrap();
        let result = client.fetch_instrument("7203").await;
        assert!(matches!(result, Err(DataProviderError::NotFound(_))));
    }
}

// === fetch_daily_bars ===

mod fetch_daily_bars {
    use super::*;

    #[rstest]
    #[tokio::test]
    async fn test_parses_bars_in_range() -> Result<(), DataProviderError> {
        let mock = IbkrMockServer::start().await;
        mock.stocks()
            .symbol("7203")
            .contracts(vec![("TSEJ", 12345)])
            .ok()
            .await;
        mock.history()
            .conid(12345)
            .bars(vec![
                MockHistoryBar {
                    t: day_ms(2025, 1, 6),
                    o: 100.0,
                    h: 110.0,
                    l: 95.0,
                    c: 105.0,
                    v: 1000.0,
                },
                MockHistoryBar {
                    t: day_ms(2025, 1, 7),
                    o: 105.0,
                    h: 115.0,
                    l: 100.0,
                    c: 112.0,
                    v: 1500.0,
                },
            ])
            .ok()
            .await;

        let client = mock.client()?;
        let bars = client.fetch_daily_bars("7203", &default_range()).await?;

        let expected = vec![
            Bar {
                instrument_id: "7203".to_string(),
                timeframe: Timeframe::Daily,
                timestamp: Utc
                    .from_utc_datetime(&date(2025, 1, 6).and_hms_opt(0, 0, 0).unwrap_or_default()),
                open: dec(100.0),
                high: dec(110.0),
                low: dec(95.0),
                close: dec(105.0),
                volume: 1000,
            },
            Bar {
                instrument_id: "7203".to_string(),
                timeframe: Timeframe::Daily,
                timestamp: Utc
                    .from_utc_datetime(&date(2025, 1, 7).and_hms_opt(0, 0, 0).unwrap_or_default()),
                open: dec(105.0),
                high: dec(115.0),
                low: dec(100.0),
                close: dec(112.0),
                volume: 1500,
            },
        ];
        assert_eq!(bars, expected);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_filters_bars_outside_range() -> Result<(), DataProviderError> {
        let mock = IbkrMockServer::start().await;
        mock.stocks()
            .symbol("7203")
            .contracts(vec![("TSEJ", 12345)])
            .ok()
            .await;
        // IBKR は period 指定により範囲外のバーも返すことがあるため、範囲外は捨てられること
        mock.history()
            .conid(12345)
            .bars(vec![
                MockHistoryBar {
                    t: day_ms(2025, 1, 1),
                    o: 1.0,
                    h: 1.0,
                    l: 1.0,
                    c: 1.0,
                    v: 1.0,
                },
                MockHistoryBar {
                    t: day_ms(2025, 1, 6),
                    o: 100.0,
                    h: 110.0,
                    l: 95.0,
                    c: 105.0,
                    v: 1000.0,
                },
                MockHistoryBar {
                    t: day_ms(2025, 1, 20),
                    o: 2.0,
                    h: 2.0,
                    l: 2.0,
                    c: 2.0,
                    v: 2.0,
                },
            ])
            .ok()
            .await;

        let client = mock.client()?;
        let bars = client.fetch_daily_bars("7203", &default_range()).await?;

        let expected = vec![Bar {
            instrument_id: "7203".to_string(),
            timeframe: Timeframe::Daily,
            timestamp: Utc
                .from_utc_datetime(&date(2025, 1, 6).and_hms_opt(0, 0, 0).unwrap_or_default()),
            open: dec(100.0),
            high: dec(110.0),
            low: dec(95.0),
            close: dec(105.0),
            volume: 1000,
        }];
        assert_eq!(bars, expected);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_returns_empty_when_history_empty() -> Result<(), DataProviderError> {
        let mock = IbkrMockServer::start().await;
        mock.stocks()
            .symbol("7203")
            .contracts(vec![("TSEJ", 12345)])
            .ok()
            .await;
        mock.history().conid(12345).bars(vec![]).ok().await;

        let client = mock.client()?;
        let bars = client.fetch_daily_bars("7203", &default_range()).await?;
        assert_eq!(bars, vec![]);
        Ok(())
    }
}

// === エラー処理とリトライ ===

mod error_handling {
    use super::*;

    #[rstest]
    #[tokio::test]
    async fn test_retries_on_5xx_then_succeeds() -> Result<(), DataProviderError> {
        let mock = IbkrMockServer::start().await;

        // 最初の 2 回は 500、その後成功するパターン
        Mock::given(method("GET"))
            .and(path("/trsrv/stocks"))
            .respond_with(ResponseTemplate::new(500).set_body_json(json!({"error": "boom"})))
            .up_to_n_times(2)
            .mount(mock.server_ref())
            .await;

        mock.stocks()
            .symbol("7203")
            .contracts(vec![("TSEJ", 1)])
            .ok()
            .await;

        let client = mock.client()?;
        let instrument = client.fetch_instrument("7203").await?;
        assert_eq!(instrument.id, "7203");
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_returns_api_error_on_4xx() {
        let mock = IbkrMockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/trsrv/stocks"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({"error": "unauth"})))
            .mount(mock.server_ref())
            .await;

        let client = mock.client().unwrap();
        let result = client.fetch_instrument("7203").await;
        assert!(matches!(
            result,
            Err(DataProviderError::Api { status: 401, .. })
        ));
    }
}

// === DataProviderKind ===

mod data_provider_kind {
    use super::*;

    #[rstest]
    #[tokio::test]
    async fn test_delegates_fetch_instrument_to_ibkr() -> Result<(), DataProviderError> {
        let mock = IbkrMockServer::start().await;
        mock.stocks()
            .symbol("7203")
            .name(Some("TOYOTA MOTOR CORP"))
            .contracts(vec![("TSEJ", 12345)])
            .ok()
            .await;

        let client = mock.client()?;
        let kind = DataProviderKind::Ibkr(client);
        let instrument = kind.fetch_instrument("7203").await?;
        assert_eq!(
            instrument,
            crate::models::instrument::Instrument {
                id: "7203".to_string(),
                name: "TOYOTA MOTOR CORP".to_string(),
                market: Market::Tse,
                sector: None,
            }
        );
        Ok(())
    }
}

// === 実 IBKR 接続テスト (手元実行用、CI ではスキップ) ===

#[tokio::test]
#[ignore = "実 IBKR Client Portal Gateway への接続が必要。IBKR_BASE_URL と IBKR_SESSION_TOKEN を設定の上、手動で `cargo test -- --ignored` を実行する"]
async fn ibkr_live_smoke() {
    let base = std::env::var("IBKR_BASE_URL").expect("IBKR_BASE_URL");
    let token = std::env::var("IBKR_SESSION_TOKEN").ok();
    let client = crate::data_provider::ibkr::IbkrClient::new(Some(base), token, None).unwrap();
    let instrument = client.fetch_instrument("7203").await.unwrap();
    assert_eq!(instrument.id, "7203");
}
