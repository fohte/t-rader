use std::sync::Arc;

use chrono::{NaiveDate, NaiveTime, TimeZone, Utc};
use core_domain::bar::{Bar, Timeframe};
use rstest::rstest;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::bars::{DailyBarAdjustmentFactor, FakeBarsRepository, SharedBarsRepository};
use crate::strategy::{FakeStrategyRepository, InvestableAmount, SharedStrategyRepository};
use crate::trade::{FakeTradeRepository, SharedTradeRepository, Trade};

use super::{PortfolioHistoryPoint, PortfolioHistoryQuery, PortfolioHistoryUseCases};

struct Fixture {
    use_cases: PortfolioHistoryUseCases,
    trades: Arc<FakeTradeRepository>,
    bars: Arc<FakeBarsRepository>,
    strategies: Arc<FakeStrategyRepository>,
}

fn fixture() -> Fixture {
    let trades = Arc::new(FakeTradeRepository::new());
    let bars = Arc::new(FakeBarsRepository::new());
    let strategies = Arc::new(FakeStrategyRepository::new());
    let trade_repository: SharedTradeRepository = trades.clone();
    let bars_repository: SharedBarsRepository = bars.clone();
    let strategy_repository: SharedStrategyRepository = strategies.clone();

    Fixture {
        use_cases: PortfolioHistoryUseCases::new(
            trade_repository,
            bars_repository,
            strategy_repository,
        ),
        trades,
        bars,
        strategies,
    }
}

fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, day).unwrap_or_default()
}

fn trade(day: u32, side: &str, qty: i64, price: i64) -> Trade {
    let timestamp = date(day).and_time(NaiveTime::MIN).and_utc().fixed_offset();
    Trade {
        id: Uuid::from_u128(u128::from(day)),
        strategy_id: Uuid::from_u128(10),
        symbol: "DEMO-STOCK".into(),
        side: side.into(),
        qty: Decimal::from(qty),
        price: Decimal::from(price),
        fee: Decimal::ZERO,
        date: date(day),
        source: "manual".into(),
        note: None,
        created_at: timestamp,
        updated_at: timestamp,
    }
}

fn bar(day: u32, close: i64, adjustment_factor: Decimal) -> Bar {
    let timestamp = Utc.from_utc_datetime(&date(day).and_time(NaiveTime::MIN));
    Bar {
        instrument_id: "DEMO-STOCK".into(),
        timeframe: Timeframe::Daily,
        timestamp,
        open: Decimal::from(close),
        high: Decimal::from(close),
        low: Decimal::from(close),
        close: Decimal::from(close),
        volume: 100,
        adjustment_factor,
    }
}

fn investable_amount(day: u32, amount_jpy: i64) -> InvestableAmount {
    investable_amount_for(day, amount_jpy, Uuid::from_u128(10))
}

fn investable_amount_for(day: u32, amount_jpy: i64, strategy_id: Uuid) -> InvestableAmount {
    let effective_at = date(day).and_time(NaiveTime::MIN).and_utc().fixed_offset();
    InvestableAmount {
        id: Uuid::from_u128(u128::from(day) + 100),
        strategy_id,
        amount_jpy: Decimal::from(amount_jpy),
        effective_at,
        created_at: effective_at,
    }
}

fn point(day: u32, total: i64, stock: i64, cash: i64, capital: i64) -> PortfolioHistoryPoint {
    PortfolioHistoryPoint {
        date: date(day),
        total_assets: Decimal::from(total),
        stock_market_value: Decimal::from(stock),
        cash: Decimal::from(cash),
        investable_amount: Decimal::from(capital),
    }
}

#[tokio::test]
async fn split_adjusted_prices_keep_assets_level_across_a_split() {
    let fixture = fixture();
    fixture.trades.insert_trade(trade(10, "buy", 1, 100)).await;
    fixture
        .bars
        .seed_bars(vec![
            bar(10, 50, Decimal::ONE),
            bar(12, 50, Decimal::new(5, 1)),
        ])
        .await;
    fixture
        .bars
        .seed_daily_adjustment_factors(
            "DEMO-STOCK",
            vec![DailyBarAdjustmentFactor {
                date: date(12),
                factor: Decimal::new(5, 1),
            }],
        )
        .await;
    fixture
        .strategies
        .insert_investable_amount(investable_amount(1, 1_000))
        .await;

    let actual = fixture
        .use_cases
        .history(PortfolioHistoryQuery {
            from: date(10),
            to: date(12),
        })
        .await
        .map_err(|error| error.to_string());

    assert_eq!(
        actual,
        Ok(vec![
            point(10, 1_000, 100, 900, 1_000),
            point(11, 1_000, 100, 900, 1_000),
            point(12, 1_000, 100, 900, 1_000),
        ])
    );
}

#[tokio::test]
async fn missing_daily_bars_carry_forward_the_previous_close() {
    let fixture = fixture();
    fixture.trades.insert_trade(trade(10, "buy", 2, 100)).await;
    fixture
        .bars
        .seed_bars(vec![bar(10, 110, Decimal::ONE), bar(12, 120, Decimal::ONE)])
        .await;
    fixture
        .strategies
        .insert_investable_amount(investable_amount(1, 1_000))
        .await;

    let actual = fixture
        .use_cases
        .history(PortfolioHistoryQuery {
            from: date(10),
            to: date(12),
        })
        .await
        .map_err(|error| error.to_string());

    assert_eq!(
        actual,
        Ok(vec![
            point(10, 1_020, 220, 800, 1_000),
            point(11, 1_020, 220, 800, 1_000),
            point(12, 1_040, 240, 800, 1_000),
        ])
    );
}

#[tokio::test]
async fn an_unpriced_position_uses_its_cost_basis_until_a_close_exists() {
    let fixture = fixture();
    fixture.trades.insert_trade(trade(10, "buy", 2, 100)).await;
    fixture
        .bars
        .seed_bars(vec![bar(12, 120, Decimal::ONE)])
        .await;
    fixture
        .strategies
        .insert_investable_amount(investable_amount(1, 1_000))
        .await;

    let actual = fixture
        .use_cases
        .history(PortfolioHistoryQuery {
            from: date(10),
            to: date(12),
        })
        .await
        .map_err(|error| error.to_string());

    assert_eq!(
        actual,
        Ok(vec![
            point(10, 1_000, 200, 800, 1_000),
            point(11, 1_000, 200, 800, 1_000),
            point(12, 1_040, 240, 800, 1_000),
        ])
    );
}

#[tokio::test]
async fn investable_amount_histories_sum_and_backfill_first_settings() {
    let fixture = fixture();
    fixture.trades.insert_trade(trade(10, "buy", 1, 80)).await;
    fixture
        .bars
        .seed_bars(vec![bar(10, 80, Decimal::ONE)])
        .await;
    fixture
        .strategies
        .insert_investable_amount(investable_amount(12, 500))
        .await;
    fixture
        .strategies
        .insert_investable_amount(investable_amount_for(11, 300, Uuid::from_u128(11)))
        .await;

    let actual = fixture
        .use_cases
        .history(PortfolioHistoryQuery {
            from: date(10),
            to: date(12),
        })
        .await
        .map_err(|error| error.to_string());

    assert_eq!(
        actual,
        Ok(vec![
            point(10, 800, 80, 720, 800),
            point(11, 800, 80, 720, 800),
            point(12, 800, 80, 720, 800),
        ])
    );
}

#[rstest]
#[case::reversed(date(12), date(10), "from must be on or before to")]
#[case::too_long(
    date(10),
    date(10) + chrono::Duration::days(3_660),
    "date range must not exceed 3660 days",
)]
#[tokio::test]
async fn history_rejects_invalid_date_ranges(
    #[case] from: NaiveDate,
    #[case] to: NaiveDate,
    #[case] expected_error: &str,
) {
    let fixture = fixture();

    let actual = fixture
        .use_cases
        .history(PortfolioHistoryQuery { from, to })
        .await
        .map_err(|error| error.to_string());

    assert_eq!(actual, Err(expected_error.into()),);
}

#[tokio::test]
async fn history_accepts_the_maximum_inclusive_date_range() {
    let fixture = fixture();
    let from = date(10);
    let to = from + chrono::Duration::days(3_659);

    let actual = fixture
        .use_cases
        .history(PortfolioHistoryQuery { from, to })
        .await
        .map_err(|error| error.to_string());
    let expected = (0..3_660)
        .map(|offset| super::PortfolioHistoryPoint {
            date: from + chrono::Duration::days(offset),
            total_assets: Decimal::ZERO,
            stock_market_value: Decimal::ZERO,
            cash: Decimal::ZERO,
            investable_amount: Decimal::ZERO,
        })
        .collect::<Vec<_>>();

    assert_eq!(actual, Ok(expected));
}
