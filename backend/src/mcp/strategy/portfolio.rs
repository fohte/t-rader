//! 口座全体 (全戦略横断) と接続元戦略、両方のポートフォリオ集計の inner method 実装。
//!
//! 戦略は「お金の区分」であり分析は口座全体を見る、という設計判断から account 側の集計は
//! 全戦略横断の trade を対象にする。そのうえで接続元 strategy_id 自身のスライスも
//! 追加で返す。両スコープとも保有銘柄の直近終値で時価評価する。

use std::collections::{BTreeSet, HashMap};

use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::PositionSummary;
use crate::services::investable_amount;
use crate::services::market_price::fetch_latest_prices;
use crate::services::trades::fetch_summary;

use super::dto::{
    PortfolioPositionDto, PortfolioScopeDto, ReadPortfolioResult, StrategyPortfolioScopeDto,
};
use super::{StrategyServer, db_error, decimal_to_f64, internal_error};

impl StrategyServer {
    pub(crate) async fn read_portfolio_inner(
        &self,
        strategy_id: Uuid,
    ) -> Result<ReadPortfolioResult, McpError> {
        let account_summary = fetch_summary(&self.db, None).await.map_err(db_error)?;
        let strategy_summary = fetch_summary(&self.db, Some(strategy_id))
            .await
            .map_err(db_error)?;

        let mut symbols: BTreeSet<String> = BTreeSet::new();
        symbols.extend(account_summary.positions.iter().map(|p| p.symbol.clone()));
        symbols.extend(strategy_summary.positions.iter().map(|p| p.symbol.clone()));
        let symbols: Vec<String> = symbols.into_iter().collect();

        let prices = fetch_latest_prices(&self.db, self.data_provider.as_deref(), &symbols).await;

        let account_positions = to_position_dtos(account_summary.positions, &prices.prices);
        let account = PortfolioScopeDto {
            trade_count: account_summary.trade_count,
            realized_pnl: decimal_to_f64(account_summary.realized_pnl),
            market_value: sum_market_value(&account_positions),
            positions: account_positions,
        };

        let strategy_cost_basis: Decimal = strategy_summary
            .positions
            .iter()
            .map(|p| p.cost_basis)
            .sum();
        let strategy_realized_pnl = strategy_summary.realized_pnl;
        let strategy_positions = to_position_dtos(strategy_summary.positions, &prices.prices);

        let investable_amount_row = investable_amount::find_current(&self.db, strategy_id)
            .await
            .map_err(app_error_to_mcp)?;
        let unused_investable_amount = investable_amount_row.as_ref().map(|row| {
            decimal_to_f64(row.amount_jpy + strategy_realized_pnl - strategy_cost_basis)
        });

        let strategy = StrategyPortfolioScopeDto {
            trade_count: strategy_summary.trade_count,
            realized_pnl: decimal_to_f64(strategy_realized_pnl),
            market_value: sum_market_value(&strategy_positions),
            positions: strategy_positions,
            investable_amount: investable_amount_row.map(|row| decimal_to_f64(row.amount_jpy)),
            unused_investable_amount,
        };

        Ok(ReadPortfolioResult {
            priced_at: prices.priced_at,
            account,
            strategy,
        })
    }
}

fn to_position_dtos(
    positions: Vec<PositionSummary>,
    prices: &HashMap<String, Decimal>,
) -> Vec<PortfolioPositionDto> {
    positions
        .into_iter()
        .map(|p| to_position_dto(p, prices))
        .collect()
}

fn to_position_dto(p: PositionSummary, prices: &HashMap<String, Decimal>) -> PortfolioPositionDto {
    let current_price = prices.get(&p.symbol).copied();
    let market_value = current_price.map(|price| p.qty * price);
    let unrealized_pnl = market_value.map(|mv| mv - p.cost_basis);
    PortfolioPositionDto {
        symbol: p.symbol,
        qty: decimal_to_f64(p.qty),
        avg_cost: decimal_to_f64(p.avg_cost),
        cost_basis: decimal_to_f64(p.cost_basis),
        realized_pnl: decimal_to_f64(p.realized_pnl),
        current_price: current_price.map(decimal_to_f64),
        market_value: market_value.map(decimal_to_f64),
        unrealized_pnl: unrealized_pnl.map(decimal_to_f64),
    }
}

fn sum_market_value(positions: &[PortfolioPositionDto]) -> f64 {
    positions.iter().filter_map(|p| p.market_value).sum()
}

/// `investable_amount::find_current` が返す `AppError` の MCP エラー変換。
fn app_error_to_mcp(err: AppError) -> McpError {
    match err {
        AppError::Database(e) => db_error(e),
        other => internal_error(format!("investable amount error: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{Duration, TimeZone, Utc};
    use rust_decimal::Decimal;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{ActiveModelTrait, DatabaseConnection};
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::data_provider::DataProviderKind;
    use crate::data_provider::ibkr::mock::{IbkrMockServer, MockHistoryBar};
    use crate::entities::trade;
    use crate::services::investable_amount;
    use crate::testing::create_test_db;

    use super::super::StrategyServer;
    use super::super::dto::{
        PortfolioPositionDto, PortfolioScopeDto, ReadPortfolioResult, StrategyPortfolioScopeDto,
    };
    use super::super::tests_common::{build_server, insert_strategy};

    async fn seed_trade(
        db: &DatabaseConnection,
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

    #[sqlx::test(migrations = false)]
    async fn read_portfolio_returns_account_and_strategy_scopes(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        seed_trade(&db, strategy_a, "7203", "buy", 100, 1000).await;
        seed_trade(&db, strategy_b, "6758", "buy", 50, 2000).await;
        let server = build_server(db);

        let result = server
            .read_portfolio_inner(strategy_a)
            .await
            .expect("read_portfolio");

        assert_eq!(
            result,
            ReadPortfolioResult {
                priced_at: None,
                account: PortfolioScopeDto {
                    trade_count: 2,
                    realized_pnl: 0.0,
                    market_value: 0.0,
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
                    market_value: 0.0,
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

    #[sqlx::test(migrations = false)]
    async fn read_portfolio_returns_empty_scopes_when_no_trades(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        let server = build_server(db);

        let result = server
            .read_portfolio_inner(strategy_id)
            .await
            .expect("read_portfolio");

        assert_eq!(
            result,
            ReadPortfolioResult {
                priced_at: None,
                account: PortfolioScopeDto {
                    trade_count: 0,
                    realized_pnl: 0.0,
                    market_value: 0.0,
                    positions: vec![],
                },
                strategy: StrategyPortfolioScopeDto {
                    trade_count: 0,
                    realized_pnl: 0.0,
                    market_value: 0.0,
                    positions: vec![],
                    investable_amount: None,
                    unused_investable_amount: None,
                },
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn read_portfolio_backfills_prices_and_computes_investable_amount(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        seed_trade(&db, strategy_id, "7203", "buy", 100, 1000).await;

        investable_amount::record(
            &db,
            strategy_id,
            Decimal::from(500_000),
            Utc::now().fixed_offset() - Duration::days(1),
        )
        .await
        .expect("record investable amount");

        // J-Quants Free プラン相当の取得可能範囲 (12 週間前) 内に収まる日付の bar を用意する
        let bar_date = Utc::now().date_naive() - Duration::weeks(12) - Duration::days(1);
        let bar_millis = Utc
            .from_utc_datetime(&bar_date.and_hms_opt(0, 0, 0).expect("time"))
            .timestamp_millis();

        let ibkr = IbkrMockServer::start().await;
        ibkr.stocks().ok().await;
        ibkr.history()
            .bars(vec![MockHistoryBar {
                t: bar_millis,
                o: 1150.0,
                h: 1250.0,
                l: 1100.0,
                c: 1200.0,
                v: 10_000.0,
            }])
            .ok()
            .await;
        let client = ibkr.client().expect("client");
        let provider = Arc::new(DataProviderKind::Ibkr(client));

        let server = StrategyServer::new(db, Some(provider));

        let result = server
            .read_portfolio_inner(strategy_id)
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
