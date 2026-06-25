use std::str::FromStr;

use chrono::{TimeZone, Utc};
use indoc::indoc;
use rstest::{fixture, rstest};
use rust_decimal::Decimal;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::data_provider::DataProviderError;
use crate::data_provider::macro_data::stooq::StooqClient;
use crate::data_provider::macro_data::{
    MacroCache, MacroCacheSnapshot, MacroDataProvider, MacroTick,
};

fn utc_at(year: i32, mon: u32, day: u32, h: u32, m: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(year, mon, day, h, m, 0)
        .single()
        .expect("valid time")
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("valid decimal")
}

#[fixture]
fn sample_ticks() -> Vec<MacroTick> {
    vec![MacroTick {
        symbol: "日経225".into(),
        value: "38420.00".into(),
        pct: dec("1.11"),
        fetched_at: utc_at(2026, 6, 25, 6, 0),
    }]
}

#[rstest]
#[tokio::test]
async fn macro_cache_record_success_clears_stale(sample_ticks: Vec<MacroTick>) {
    let cache = MacroCache::new();
    cache.record_failure(utc_at(2026, 6, 25, 5, 0)).await;
    cache.record_success(sample_ticks.clone()).await;

    assert_eq!(
        cache.snapshot().await,
        MacroCacheSnapshot {
            ticks: sample_ticks,
            stale_since: None,
        }
    );
}

#[rstest]
#[tokio::test]
async fn macro_cache_record_failure_preserves_first_timestamp(sample_ticks: Vec<MacroTick>) {
    let cache = MacroCache::new();
    cache.record_success(sample_ticks.clone()).await;
    cache.record_failure(utc_at(2026, 6, 25, 5, 0)).await;
    cache.record_failure(utc_at(2026, 6, 25, 5, 30)).await;

    assert_eq!(
        cache.snapshot().await,
        MacroCacheSnapshot {
            ticks: sample_ticks,
            stale_since: Some(utc_at(2026, 6, 25, 5, 0)),
        }
    );
}

#[rstest]
#[tokio::test]
async fn macro_cache_empty_snapshot() {
    let cache = MacroCache::new();
    assert_eq!(
        cache.snapshot().await,
        MacroCacheSnapshot {
            ticks: vec![],
            stale_since: None,
        }
    );
}

#[rstest]
#[tokio::test]
async fn stooq_client_fetches_and_parses_csv() -> Result<(), DataProviderError> {
    let server = MockServer::start().await;
    let csv = indoc! {"
        Symbol,Date,Time,Close,Change
        ^NKX,2026-06-25,15:00:00,38420.0,1.11
    "};
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(csv))
        .mount(&server)
        .await;

    let client = StooqClient::with_base_url(&format!("{}/q/l/", server.uri()))?;
    let ticks = client.fetch_macro_ticks().await?;
    // fetched_at は呼び出し時の `Utc::now()` で動的なので、固定値に正規化してから等値比較する
    let normalized: Vec<MacroTick> = ticks
        .into_iter()
        .map(|t| MacroTick {
            fetched_at: utc_at(2026, 6, 25, 6, 0),
            ..t
        })
        .collect();

    assert_eq!(
        normalized,
        vec![MacroTick {
            symbol: "日経225".into(),
            value: "38420.00".into(),
            pct: dec("1.11"),
            fetched_at: utc_at(2026, 6, 25, 6, 0),
        }],
    );
    Ok(())
}

#[rstest]
#[tokio::test]
async fn stooq_client_returns_api_error_on_5xx() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let client = StooqClient::with_base_url(&format!("{}/q/l/", server.uri())).expect("ok");

    let result = client.fetch_macro_ticks().await;
    let expected: Result<Vec<MacroTick>, DataProviderError> = Err(DataProviderError::Api {
        status: 503,
        message: "stooq returned status 503".into(),
    });
    assert_eq!(format!("{result:?}"), format!("{expected:?}"));
}
