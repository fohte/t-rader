use chrono::{NaiveDate, TimeZone, Utc};
use rust_decimal::Decimal;

use crate::data_provider::{DataProvider, DataProviderError, DateRange};
use crate::models::bar::{Bar, Timeframe};
use crate::models::instrument::{Instrument, Market};

/// テスト用のモックデータプロバイダー
///
/// 事前に登録されたデータを返す。登録されていない銘柄には NotFound を返す。
struct MockDataProvider {
    bars: Vec<Bar>,
    instruments: Vec<Instrument>,
}

impl MockDataProvider {
    /// 空のモックプロバイダーを作成する
    fn new() -> Self {
        Self {
            bars: Vec::new(),
            instruments: Vec::new(),
        }
    }

    /// バーデータを登録する (ビルダーパターン)
    fn with_bars(mut self, bars: Vec<Bar>) -> Self {
        self.bars = bars;
        self
    }

    /// 銘柄情報を登録する (ビルダーパターン)
    fn with_instruments(mut self, instruments: Vec<Instrument>) -> Self {
        self.instruments = instruments;
        self
    }
}

impl DataProvider for MockDataProvider {
    async fn fetch_daily_bars(
        &self,
        instrument_id: &str,
        range: &DateRange,
    ) -> Result<Vec<Bar>, DataProviderError> {
        let instrument_exists = self.instruments.iter().any(|i| i.id == instrument_id);

        if !instrument_exists {
            return Err(DataProviderError::NotFound(format!(
                "instrument '{instrument_id}' not found"
            )));
        }

        let from_dt = Utc.from_utc_datetime(&range.from.and_hms_opt(0, 0, 0).unwrap_or_default());
        // to は inclusive なので、翌日の 00:00:00 を排他的上限として使う
        let to_exclusive = range.to.succ_opt().unwrap_or(range.to);
        let to_dt = Utc.from_utc_datetime(&to_exclusive.and_hms_opt(0, 0, 0).unwrap_or_default());

        let mut bars: Vec<Bar> = self
            .bars
            .iter()
            .filter(|b| {
                b.instrument_id == instrument_id && b.timestamp >= from_dt && b.timestamp < to_dt
            })
            .cloned()
            .collect();

        bars.sort_by_key(|b| b.timestamp);

        Ok(bars)
    }

    async fn fetch_instrument(&self, instrument_id: &str) -> Result<Instrument, DataProviderError> {
        self.instruments
            .iter()
            .find(|i| i.id == instrument_id)
            .cloned()
            .ok_or_else(|| {
                DataProviderError::NotFound(format!("instrument '{instrument_id}' not found"))
            })
    }
}

/// テスト用ヘルパー: NaiveDate を簡潔に作成する
fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap_or_default()
}

/// テスト用ヘルパー: 指定日の Bar を生成する
fn make_bar(instrument_id: &str, d: NaiveDate, close: i64) -> Bar {
    let timestamp = Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap_or_default());
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

/// テスト用ヘルパー: サンプル銘柄情報を作成する
fn sample_instrument(id: &str) -> Instrument {
    Instrument {
        id: id.to_string(),
        name: format!("Test Instrument {id}"),
        market: Market::Tse,
        sector: Some("Technology".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn provider() -> MockDataProvider {
        MockDataProvider::new()
            .with_instruments(vec![sample_instrument("86970"), sample_instrument("72030")])
            .with_bars(vec![
                make_bar("86970", date(2025, 1, 6), 100),
                make_bar("86970", date(2025, 1, 7), 105),
                make_bar("86970", date(2025, 1, 8), 103),
                make_bar("72030", date(2025, 1, 6), 200),
            ])
    }

    #[rstest]
    #[case::full_range(date(2025, 1, 6), date(2025, 1, 8), 3)]
    #[case::partial_range(date(2025, 1, 6), date(2025, 1, 7), 2)]
    #[case::single_day(date(2025, 1, 7), date(2025, 1, 7), 1)]
    #[case::no_data_in_range(date(2025, 2, 1), date(2025, 2, 28), 0)]
    #[tokio::test]
    async fn test_fetch_daily_bars_returns_correct_count(
        provider: MockDataProvider,
        #[case] from: NaiveDate,
        #[case] to: NaiveDate,
        #[case] expected_count: usize,
    ) -> Result<(), DataProviderError> {
        let range = DateRange { from, to };
        let bars = provider.fetch_daily_bars("86970", &range).await?;
        assert_eq!(bars.len(), expected_count);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_fetch_daily_bars_sorted_by_timestamp(
        provider: MockDataProvider,
    ) -> Result<(), DataProviderError> {
        let range = DateRange {
            from: date(2025, 1, 6),
            to: date(2025, 1, 8),
        };
        let bars = provider.fetch_daily_bars("86970", &range).await?;

        for pair in bars.windows(2) {
            assert!(pair[0].timestamp <= pair[1].timestamp);
        }
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_fetch_daily_bars_filters_by_instrument(
        provider: MockDataProvider,
    ) -> Result<(), DataProviderError> {
        let range = DateRange {
            from: date(2025, 1, 6),
            to: date(2025, 1, 8),
        };

        let bars_86970 = provider.fetch_daily_bars("86970", &range).await?;
        let bars_72030 = provider.fetch_daily_bars("72030", &range).await?;

        assert_eq!(bars_86970.len(), 3);
        assert_eq!(bars_72030.len(), 1);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_fetch_daily_bars_unknown_instrument_returns_not_found(
        provider: MockDataProvider,
    ) {
        let range = DateRange {
            from: date(2025, 1, 6),
            to: date(2025, 1, 8),
        };
        let result = provider.fetch_daily_bars("99999", &range).await;
        assert!(matches!(result, Err(DataProviderError::NotFound(_))));
    }

    #[rstest]
    #[tokio::test]
    async fn test_fetch_instrument_returns_matching_data(
        provider: MockDataProvider,
    ) -> Result<(), DataProviderError> {
        let instrument = provider.fetch_instrument("86970").await?;
        assert_eq!(instrument.id, "86970");
        assert_eq!(instrument.market, Market::Tse);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_fetch_instrument_unknown_returns_not_found(provider: MockDataProvider) {
        let result = provider.fetch_instrument("99999").await;
        assert!(matches!(result, Err(DataProviderError::NotFound(_))));
    }
}
