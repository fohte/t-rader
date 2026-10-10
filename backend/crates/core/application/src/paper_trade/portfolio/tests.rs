#![allow(
    clippy::expect_used,
    reason = "ユースケースが成功しなければテストを失敗させるため"
)]

use std::sync::Arc;

use rust_decimal::Decimal;
use uuid::Uuid;

use crate::{
    bars::FakeBarsRepository,
    paper_trade::{
        PaperOrderSide, PaperOrderWithResult, PaperTradePortfolio, PaperTradePosition,
        test_support::{account, at, bar, date, filled, order, pending, use_cases},
    },
};

#[tokio::test]
async fn portfolio_replays_fills_and_keeps_pending_orders_in_history() {
    let account = account(11, "Demo portfolio", 20_000, date(1), None);
    let buy = order(1, account.id, "DEMO-A1", PaperOrderSide::Buy, 100, date(1));
    let sell = order(2, account.id, "DEMO-A1", PaperOrderSide::Sell, 50, date(2));
    let pending_buy = order(3, account.id, "DEMO-A1", PaperOrderSide::Buy, 100, date(3));
    let orders = vec![
        filled(buy.clone(), date(2), 100),
        filled(sell.clone(), date(3), 120),
        pending(pending_buy.clone()),
    ];
    let bars = Arc::new(FakeBarsRepository::new());
    bars.seed_bars(vec![bar("DEMO-A1", 4, 125, 130)]).await;
    let use_cases = use_cases(vec![account.clone()], orders.clone(), bars);

    let actual = use_cases.portfolio(account.id).await;

    assert_eq!(
        actual.expect("portfolio is available"),
        PaperTradePortfolio {
            account,
            as_of: date(4),
            cash_jpy: Decimal::from(16_000),
            positions: vec![PaperTradePosition {
                stock_id: "DEMO-A1".to_string(),
                qty: 50,
                avg_cost_jpy: Decimal::from(100),
                current_price_jpy: Decimal::from(130),
                market_value_jpy: Decimal::from(6_500),
                unrealized_pnl_jpy: Decimal::from(1_500),
            }],
            orders: vec![
                PaperOrderWithResult {
                    order: buy.clone(),
                    result: Some(crate::paper_trade::PaperOrderResult::Filled {
                        order_id: buy.id,
                        fill_date: date(2),
                        fill_price: Decimal::from(100),
                        decided_at: at(date(2)),
                    }),
                },
                PaperOrderWithResult {
                    order: sell.clone(),
                    result: Some(crate::paper_trade::PaperOrderResult::Filled {
                        order_id: sell.id,
                        fill_date: date(3),
                        fill_price: Decimal::from(120),
                        decided_at: at(date(3)),
                    }),
                },
                PaperOrderWithResult {
                    order: pending_buy,
                    result: None,
                },
            ],
        },
    );
}

#[tokio::test]
async fn portfolio_keeps_split_adjusted_cost_basis_for_the_remaining_shares() {
    let account = account(12, "Demo portfolio", 100_000, date(1), None);
    let buy = order(1, account.id, "DEMO-A1", PaperOrderSide::Buy, 100, date(1));
    let sell = order(2, account.id, "DEMO-A1", PaperOrderSide::Sell, 100, date(3));
    let orders = vec![
        filled(buy.clone(), date(2), 1_000),
        filled(sell.clone(), date(4), 600),
    ];
    let mut split_bar = bar("DEMO-A1", 3, 500, 500);
    split_bar.adjustment_factor = Decimal::new(5, 1);
    let bars = Arc::new(FakeBarsRepository::new());
    bars.seed_bars(vec![split_bar, bar("DEMO-A1", 5, 600, 600)])
        .await;
    let use_cases = use_cases(vec![account.clone()], orders, bars);

    let actual = use_cases.portfolio(account.id).await;

    assert_eq!(
        actual.expect("portfolio is available"),
        PaperTradePortfolio {
            account,
            as_of: date(5),
            cash_jpy: Decimal::from(60_000),
            positions: vec![PaperTradePosition {
                stock_id: "DEMO-A1".to_string(),
                qty: 100,
                avg_cost_jpy: Decimal::from(500),
                current_price_jpy: Decimal::from(600),
                market_value_jpy: Decimal::from(60_000),
                unrealized_pnl_jpy: Decimal::from(10_000),
            }],
            orders: vec![
                PaperOrderWithResult {
                    order: buy.clone(),
                    result: Some(crate::paper_trade::PaperOrderResult::Filled {
                        order_id: buy.id,
                        fill_date: date(2),
                        fill_price: Decimal::from(1_000),
                        decided_at: at(date(2)),
                    }),
                },
                PaperOrderWithResult {
                    order: sell.clone(),
                    result: Some(crate::paper_trade::PaperOrderResult::Filled {
                        order_id: sell.id,
                        fill_date: date(4),
                        fill_price: Decimal::from(600),
                        decided_at: at(date(4)),
                    }),
                },
            ],
        },
    );
}

#[tokio::test]
async fn account_lookup_uses_both_strategy_and_purpose() {
    let account = account(12, "Demo account", 20_000, date(1), None);
    let use_cases = use_cases(
        vec![account.clone()],
        Vec::new(),
        Arc::new(FakeBarsRepository::new()),
    );

    let matching = use_cases
        .account_for_strategy_purpose(account.strategy_id, &account.purpose)
        .await;
    let other_purpose = use_cases
        .account_for_strategy_purpose(account.strategy_id, "demo-purpose-other")
        .await;
    let other_strategy = use_cases
        .account_for_strategy_purpose(Uuid::from_u128(999), &account.purpose)
        .await;

    assert_eq!(
        (
            matching.expect("lookup succeeds"),
            other_purpose.expect("lookup succeeds"),
            other_strategy.expect("lookup succeeds"),
        ),
        (Some(account), None, None),
    );
}
