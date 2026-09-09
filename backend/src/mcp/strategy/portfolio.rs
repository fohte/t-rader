//! 口座全体 (全戦略横断) のポートフォリオ取得の inner method 実装。
//!
//! 戦略は「お金の区分」であり分析は口座全体を見る、という設計判断から、
//! ここでは接続元の strategy_id によるフィルタを行わず全 trade を対象にする。

use rmcp::ErrorData as McpError;

use crate::services::trades::fetch_summary;

use super::dto::{PortfolioPositionDto, ReadPortfolioResult};
use super::{StrategyServer, db_error, decimal_to_f64};

impl StrategyServer {
    pub(crate) async fn read_portfolio_inner(&self) -> Result<ReadPortfolioResult, McpError> {
        let summary = fetch_summary(&self.db, None).await.map_err(db_error)?;
        Ok(ReadPortfolioResult {
            trade_count: summary.trade_count,
            realized_pnl: decimal_to_f64(summary.realized_pnl),
            positions: summary
                .positions
                .into_iter()
                .map(|p| PortfolioPositionDto {
                    symbol: p.symbol,
                    qty: decimal_to_f64(p.qty),
                    avg_cost: decimal_to_f64(p.avg_cost),
                    cost_basis: decimal_to_f64(p.cost_basis),
                    realized_pnl: decimal_to_f64(p.realized_pnl),
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{ActiveModelTrait, DatabaseConnection};
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::trade;
    use crate::testing::create_test_db;

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
    async fn read_portfolio_aggregates_trades_across_strategies(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        seed_trade(&db, strategy_a, "7203", "buy", 100, 1000).await;
        seed_trade(&db, strategy_b, "6758", "buy", 50, 2000).await;
        let server = build_server(db);

        let result = server.read_portfolio_inner().await.expect("read_portfolio");

        let positions: Vec<(String, f64, f64)> = result
            .positions
            .iter()
            .map(|p| (p.symbol.clone(), p.qty, p.avg_cost))
            .collect();
        assert_eq!(
            (result.trade_count, result.realized_pnl, positions),
            (
                2,
                0.0,
                vec![
                    ("6758".to_string(), 50.0, 2000.0),
                    ("7203".to_string(), 100.0, 1000.0),
                ],
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn read_portfolio_returns_empty_when_no_trades(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db);

        let result = server.read_portfolio_inner().await.expect("read_portfolio");

        assert_eq!(
            (result.trade_count, result.realized_pnl, result.positions),
            (0, 0.0, vec![]),
        );
    }
}
