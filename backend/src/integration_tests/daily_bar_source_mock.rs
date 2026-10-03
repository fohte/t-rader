//! 共有 `MockProvider` が `DailyBarSource` の検索範囲と銘柄選択を満たすことを検証する。

use chrono::{NaiveDate, TimeZone, Utc};
use core_application::daily_bar_source::{DailyBarSource, DailyBarSourceError, DateRange};
use core_domain::bar::{Bar, Timeframe};
use core_domain::instrument::{Instrument, Market};
use rstest::{fixture, rstest};
use rust_decimal::Decimal;

use crate::testing::MockProvider;

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap_or_default()
}

fn make_bar(instrument_id: &str, day: NaiveDate, close: i64) -> Bar {
    let timestamp = Utc.from_utc_datetime(&day.and_hms_opt(0, 0, 0).unwrap_or_default());
    Bar {
        instrument_id: instrument_id.to_string(),
        timeframe: Timeframe::Daily,
        timestamp,
        open: Decimal::new(close, 0),
        high: Decimal::new(close + 10, 0),
        low: Decimal::new(close - 10, 0),
        close: Decimal::new(close, 0),
        volume: 1000,
    }
}

fn sample_instrument(id: &str) -> Instrument {
    Instrument {
        id: id.to_string(),
        name: format!("Sample Instrument {id}"),
        market: Market::Tse,
        sector: Some("Sample Sector".to_string()),
        product_category: Some("000".to_string()),
    }
}

#[fixture]
fn provider() -> MockProvider {
    MockProvider::new()
        .with_instruments(vec![
            sample_instrument("sample-stock-a"),
            sample_instrument("sample-stock-b"),
        ])
        .with_bars(vec![
            make_bar("sample-stock-a", date(2025, 1, 6), 100),
            make_bar("sample-stock-a", date(2025, 1, 7), 105),
            make_bar("sample-stock-a", date(2025, 1, 8), 103),
            make_bar("sample-stock-b", date(2025, 1, 6), 200),
        ])
}

#[rstest]
#[case::full_range(
    date(2025, 1, 6),
    date(2025, 1, 8),
    vec![(date(2025, 1, 6), 100), (date(2025, 1, 7), 105), (date(2025, 1, 8), 103)],
)]
#[case::partial_range(
    date(2025, 1, 6),
    date(2025, 1, 7),
    vec![(date(2025, 1, 6), 100), (date(2025, 1, 7), 105)],
)]
#[case::single_day(date(2025, 1, 7), date(2025, 1, 7), vec![(date(2025, 1, 7), 105)])]
#[case::no_data_in_range(date(2025, 2, 1), date(2025, 2, 28), vec![])]
#[tokio::test]
async fn fetch_daily_bars_returns_matching_rows(
    provider: MockProvider,
    #[case] from: NaiveDate,
    #[case] to: NaiveDate,
    #[case] expected_rows: Vec<(NaiveDate, i64)>,
) -> Result<(), DailyBarSourceError> {
    let range = DateRange { from, to };
    let actual = provider.fetch_daily_bars("sample-stock-a", &range).await?;
    let expected = expected_rows
        .into_iter()
        .map(|(day, close)| make_bar("sample-stock-a", day, close))
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
    Ok(())
}

#[rstest]
#[tokio::test]
async fn fetch_daily_bars_returns_rows_in_timestamp_order(
    provider: MockProvider,
) -> Result<(), DailyBarSourceError> {
    let range = DateRange {
        from: date(2025, 1, 6),
        to: date(2025, 1, 8),
    };
    let actual = provider.fetch_daily_bars("sample-stock-a", &range).await?;

    assert_eq!(
        actual,
        vec![
            make_bar("sample-stock-a", date(2025, 1, 6), 100),
            make_bar("sample-stock-a", date(2025, 1, 7), 105),
            make_bar("sample-stock-a", date(2025, 1, 8), 103),
        ],
    );
    Ok(())
}

#[rstest]
#[tokio::test]
async fn fetch_daily_bars_filters_by_instrument(
    provider: MockProvider,
) -> Result<(), DailyBarSourceError> {
    let range = DateRange {
        from: date(2025, 1, 6),
        to: date(2025, 1, 8),
    };
    let actual = (
        provider.fetch_daily_bars("sample-stock-a", &range).await?,
        provider.fetch_daily_bars("sample-stock-b", &range).await?,
    );

    assert_eq!(
        actual,
        (
            vec![
                make_bar("sample-stock-a", date(2025, 1, 6), 100),
                make_bar("sample-stock-a", date(2025, 1, 7), 105),
                make_bar("sample-stock-a", date(2025, 1, 8), 103),
            ],
            vec![make_bar("sample-stock-b", date(2025, 1, 6), 200)],
        ),
    );
    Ok(())
}

#[rstest]
#[tokio::test]
async fn fetch_daily_bars_returns_not_found_for_unknown_instrument(provider: MockProvider) {
    let range = DateRange {
        from: date(2025, 1, 6),
        to: date(2025, 1, 8),
    };
    let actual = provider.fetch_daily_bars("unknown-stock", &range).await;

    assert_eq!(
        actual,
        Err(DailyBarSourceError::NotFound(
            "instrument 'unknown-stock' not found".to_string()
        )),
    );
}
