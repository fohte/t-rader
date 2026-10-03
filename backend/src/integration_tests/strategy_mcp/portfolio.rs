#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{Duration, TimeZone, Utc};
    use rust_decimal::Decimal;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use uuid::Uuid;

    use crate::testing::MockProvider;
    use core_application::daily_bar_source::SharedDailyBarSource;
    use core_domain::bar::{Bar, Timeframe};
    use core_domain::instrument::{Instrument, Market};
    use gateway_postgres::entities::{strategy_investable_amount, trade};

    use super::super::dto::{
        PortfolioPositionDto, PortfolioScopeDto, ReadPortfolioResult, StrategyPortfolioScopeDto,
    };
    use super::super::tests_common::{build_server, insert_strategy};

    async fn seed_trade(
        db: &impl sea_orm::ConnectionTrait,
        strategy_id: Uuid,
        symbol: &str,
        side: &str,
        qty: i64,
        price: i64,
    ) {
        trade::ActiveModel {
            id: Set(Uuid::new_v4()),
            strategy_id: Set(strategy_id),
            symbol: Set(symbol.to_string()),
            side: Set(side.to_string()),
            qty: Set(Decimal::from(qty)),
            price: Set(Decimal::from(price)),
            fee: Set(Decimal::ZERO),
            date: Set(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date")),
            source: Set("manual".into()),
            note: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("seed trade");
    }

    #[backend_test_macros::database_test]
    async fn read_portfolio_returns_account_and_strategy_scopes(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        seed_trade(&db, strategy_a, "7203", "buy", 100, 1000).await;
        seed_trade(&db, strategy_b, "6758", "buy", 50, 2000).await;
        let server = build_server(db);

        let result = server
            .read_portfolio(strategy_a)
            .await
            .expect("read_portfolio");

        assert_eq!(
            result,
            ReadPortfolioResult {
                priced_at: None,
                account: PortfolioScopeDto {
                    trade_count: 2,
                    realized_pnl: 0.0,
                    market_value: -0.0,
                    positions: vec![
                        PortfolioPositionDto {
                            symbol: "6758".to_string(),
                            qty: 50.0,
                            avg_cost: 2000.0,
                            cost_basis: 100_000.0,
                            realized_pnl: 0.0,
                            current_price: None,
                            market_value: None,
                            unrealized_pnl: None,
                        },
                        PortfolioPositionDto {
                            symbol: "7203".to_string(),
                            qty: 100.0,
                            avg_cost: 1000.0,
                            cost_basis: 100_000.0,
                            realized_pnl: 0.0,
                            current_price: None,
                            market_value: None,
                            unrealized_pnl: None,
                        },
                    ],
                },
                strategy: StrategyPortfolioScopeDto {
                    trade_count: 1,
                    realized_pnl: 0.0,
                    market_value: -0.0,
                    positions: vec![PortfolioPositionDto {
                        symbol: "7203".to_string(),
                        qty: 100.0,
                        avg_cost: 1000.0,
                        cost_basis: 100_000.0,
                        realized_pnl: 0.0,
                        current_price: None,
                        market_value: None,
                        unrealized_pnl: None,
                    }],
                    investable_amount: None,
                    unused_investable_amount: None,
                },
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn read_portfolio_returns_empty_scopes_when_no_trades(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "a").await;
        let server = build_server(db);

        let result = server
            .read_portfolio(strategy_id)
            .await
            .expect("read_portfolio");

        assert_eq!(
            result,
            ReadPortfolioResult {
                priced_at: None,
                account: PortfolioScopeDto {
                    trade_count: 0,
                    realized_pnl: 0.0,
                    market_value: -0.0,
                    positions: vec![],
                },
                strategy: StrategyPortfolioScopeDto {
                    trade_count: 0,
                    realized_pnl: 0.0,
                    market_value: -0.0,
                    positions: vec![],
                    investable_amount: None,
                    unused_investable_amount: None,
                },
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn read_portfolio_backfills_prices_and_computes_investable_amount(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "a").await;
        seed_trade(&db, strategy_id, "7203", "buy", 100, 1000).await;

        strategy_investable_amount::ActiveModel {
            id: Set(Uuid::new_v4()),
            strategy_id: Set(strategy_id),
            amount_jpy: Set(Decimal::from(500_000)),
            effective_at: Set(Utc::now().fixed_offset() - Duration::days(1)),
            created_at: NotSet,
        }
        .insert(&db)
        .await
        .expect("insert investable amount");

        // 取得元が範囲を公開しない場合は today が上限になり、それより古い日付なら fresh 判定される
        let bar_date = Utc::now().date_naive() - Duration::weeks(12) - Duration::days(1);
        let provider: SharedDailyBarSource = Arc::new(
            MockProvider::new()
                .with_instruments(vec![Instrument {
                    id: "7203".to_string(),
                    name: "サンプル自動車".to_string(),
                    market: Market::Tse,
                    sector: None,
                    product_category: None,
                }])
                .with_bars(vec![Bar {
                    instrument_id: "7203".to_string(),
                    timeframe: Timeframe::Daily,
                    timestamp: Utc.from_utc_datetime(&bar_date.and_hms_opt(0, 0, 0).expect("time")),
                    open: Decimal::from(1150),
                    high: Decimal::from(1250),
                    low: Decimal::from(1100),
                    close: Decimal::from(1200),
                    volume: 10_000,
                }]),
        );

        let server = super::super::tests_common::build_server_with_source(db, Some(provider));

        let result = server
            .read_portfolio(strategy_id)
            .await
            .expect("read_portfolio");

        fn priced_position() -> PortfolioPositionDto {
            PortfolioPositionDto {
                symbol: "7203".to_string(),
                qty: 100.0,
                avg_cost: 1000.0,
                cost_basis: 100_000.0,
                realized_pnl: 0.0,
                current_price: Some(1200.0),
                market_value: Some(120_000.0),
                unrealized_pnl: Some(20_000.0),
            }
        }
        assert_eq!(
            result,
            ReadPortfolioResult {
                priced_at: Some(bar_date),
                account: PortfolioScopeDto {
                    trade_count: 1,
                    realized_pnl: 0.0,
                    market_value: 120_000.0,
                    positions: vec![priced_position()],
                },
                strategy: StrategyPortfolioScopeDto {
                    trade_count: 1,
                    realized_pnl: 0.0,
                    market_value: 120_000.0,
                    positions: vec![priced_position()],
                    investable_amount: Some(500_000.0),
                    unused_investable_amount: Some(400_000.0),
                },
            },
        );
    }
}
