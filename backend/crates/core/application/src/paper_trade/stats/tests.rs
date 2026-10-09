#![allow(
    clippy::expect_used,
    reason = "ユースケースが成功しなければテストを失敗させるため"
)]

use std::sync::Arc;

use rust_decimal::Decimal;

use crate::{
    bars::FakeBarsRepository,
    paper_trade::{
        PaperOrderSide, PaperTradeAccountStats,
        test_support::{account, bar, date, filled, order, use_cases},
    },
};

#[tokio::test]
async fn stats_uses_fifo_pairs_benchmark_open_prices_and_latest_close_values() {
    let first_account = account(101, "Demo account", 100_000, date(1), Some("DEMO-B1"));
    let second_account = account(102, "Comparison account", 50_000, date(1), Some("DEMO-B1"));
    let first_buy = order(
        1,
        first_account.id,
        "DEMO-A1",
        PaperOrderSide::Buy,
        100,
        date(1),
    );
    let second_buy = order(
        2,
        first_account.id,
        "DEMO-A1",
        PaperOrderSide::Buy,
        100,
        date(2),
    );
    let third_buy = order(
        3,
        first_account.id,
        "DEMO-A1",
        PaperOrderSide::Buy,
        100,
        date(4),
    );
    let first_sell = order(
        4,
        first_account.id,
        "DEMO-A1",
        PaperOrderSide::Sell,
        200,
        date(3),
    );
    let second_sell = order(
        5,
        first_account.id,
        "DEMO-A1",
        PaperOrderSide::Sell,
        50,
        date(5),
    );
    let orders = vec![
        filled(first_buy, date(2), 100),
        filled(second_buy, date(3), 200),
        filled(third_buy, date(5), 200),
        filled(first_sell, date(4), 200),
        filled(second_sell, date(6), 250),
    ];
    let bars = Arc::new(FakeBarsRepository::new());
    bars.seed_bars(vec![
        bar("DEMO-A1", 6, 250, 300),
        bar("DEMO-B1", 1, 100, 100),
        bar("DEMO-B1", 2, 100, 100),
        bar("DEMO-B1", 3, 120, 120),
        bar("DEMO-B1", 4, 150, 150),
        bar("DEMO-B1", 5, 100, 150),
        bar("DEMO-B1", 6, 200, 200),
    ])
    .await;
    let use_cases = use_cases(
        vec![second_account.clone(), first_account.clone()],
        orders,
        bars,
    );

    let actual = use_cases.stats().await;

    assert_eq!(
        actual.expect("account statistics are available"),
        vec![
            PaperTradeAccountStats {
                account: first_account,
                as_of: date(6),
                total_assets_jpy: Decimal::from(117_500),
                return_since_start: Decimal::new(175, 3),
                benchmark_return: Some(Decimal::ONE),
                closed_trade_count: 2,
                win_rate: Some(Decimal::new(5, 1)),
                average_win_excess_return: Some(Decimal::new(125, 3)),
                average_loss_excess_return: Some(Decimal::new(-75, 2)),
                unrealized_pnl_jpy: Decimal::from(5_000),
            },
            PaperTradeAccountStats {
                account: second_account,
                as_of: date(6),
                total_assets_jpy: Decimal::from(50_000),
                return_since_start: Decimal::ZERO,
                benchmark_return: Some(Decimal::ONE),
                closed_trade_count: 0,
                win_rate: None,
                average_win_excess_return: None,
                average_loss_excess_return: None,
                unrealized_pnl_jpy: Decimal::ZERO,
            },
        ],
    );
}
