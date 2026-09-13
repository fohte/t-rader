//! 戦略実行 MCP の `read_trades` tool。
//!
//! `read_portfolio` の account スコープ (全戦略横断) と同じ規則で対象を決める
//! (`super` の doc comment にある例外参照)。戦略境界の検査は行わず、
//! `TradeDto::strategy_id` でどの戦略の約定かを判別できるようにする。

use rmcp::ErrorData as McpError;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::entities::trade;

use super::dto::{ReadTradesParams, ReadTradesResult, TradeDto};
use super::{StrategyServer, clamp_limit, db_error, decimal_to_f64};

fn trade_to_dto(m: trade::Model) -> TradeDto {
    TradeDto {
        trade_id: m.id,
        strategy_id: m.strategy_id,
        date: m.date,
        symbol: m.symbol,
        side: m.side,
        qty: decimal_to_f64(m.qty),
        price: decimal_to_f64(m.price),
    }
}

impl StrategyServer {
    pub(crate) async fn read_trades_inner(
        &self,
        _session_strategy_id: Uuid,
        params: ReadTradesParams,
    ) -> Result<ReadTradesResult, McpError> {
        let mut query = trade::Entity::find();
        if let Some(symbol) = params
            .symbol
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            query = query.filter(trade::Column::Symbol.eq(symbol));
        }
        if let Some(date_from) = params.date_from {
            query = query.filter(trade::Column::Date.gte(date_from));
        }
        let rows = query
            .order_by_desc(trade::Column::Date)
            .order_by_desc(trade::Column::CreatedAt)
            .limit(clamp_limit(params.limit))
            .all(&self.db)
            .await
            .map_err(db_error)?;
        Ok(ReadTradesResult {
            trades: rows.into_iter().map(trade_to_dto).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use rust_decimal::Decimal;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::DatabaseConnection;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::trade;
    use crate::testing::create_test_db;

    use super::super::dto::{ReadTradesParams, ReadTradesResult, TradeDto};
    use super::super::tests_common::{build_server, insert_strategy};

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    async fn seed_trade(
        db: &DatabaseConnection,
        strategy_id: Uuid,
        symbol: &str,
        side: &str,
        qty: i64,
        price: i64,
        date: NaiveDate,
    ) -> Uuid {
        let id = Uuid::new_v4();
        trade::ActiveModel {
            id: Set(id),
            strategy_id: Set(strategy_id),
            symbol: Set(symbol.to_string()),
            side: Set(side.to_string()),
            qty: Set(Decimal::from(qty)),
            price: Set(Decimal::from(price)),
            fee: Set(Decimal::ZERO),
            date: Set(date),
            source: Set("manual".into()),
            note: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("seed trade");
        id
    }

    #[sqlx::test(migrations = false)]
    async fn read_trades_returns_full_shape_across_strategies(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let trade_a = seed_trade(&db, strategy_a, "7203", "buy", 100, 1000, ymd(2026, 6, 1)).await;
        let trade_b = seed_trade(&db, strategy_b, "6758", "sell", 50, 2000, ymd(2026, 6, 2)).await;
        let server = build_server(db);

        let result = server
            .read_trades_inner(strategy_a, ReadTradesParams::default())
            .await
            .expect("read_trades");

        assert_eq!(
            result,
            ReadTradesResult {
                trades: vec![
                    TradeDto {
                        trade_id: trade_b,
                        strategy_id: strategy_b,
                        date: ymd(2026, 6, 2),
                        symbol: "6758".into(),
                        side: "sell".into(),
                        qty: 50.0,
                        price: 2000.0,
                    },
                    TradeDto {
                        trade_id: trade_a,
                        strategy_id: strategy_a,
                        date: ymd(2026, 6, 1),
                        symbol: "7203".into(),
                        side: "buy".into(),
                        qty: 100.0,
                        price: 1000.0,
                    },
                ],
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn read_trades_filters_by_symbol(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        seed_trade(&db, strategy_id, "7203", "buy", 100, 1000, ymd(2026, 6, 1)).await;
        seed_trade(&db, strategy_id, "6758", "buy", 50, 2000, ymd(2026, 6, 1)).await;
        let server = build_server(db);

        let result = server
            .read_trades_inner(
                strategy_id,
                ReadTradesParams {
                    symbol: Some("7203".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("read_trades");

        let symbols: Vec<&str> = result.trades.iter().map(|t| t.symbol.as_str()).collect();
        assert_eq!(symbols, vec!["7203"]);
    }

    #[sqlx::test(migrations = false)]
    async fn read_trades_filters_by_date_from_inclusive(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        seed_trade(&db, strategy_id, "7203", "buy", 100, 1000, ymd(2026, 6, 1)).await;
        seed_trade(&db, strategy_id, "7203", "buy", 100, 1000, ymd(2026, 6, 5)).await;
        seed_trade(&db, strategy_id, "7203", "buy", 100, 1000, ymd(2026, 6, 10)).await;
        let server = build_server(db);

        let result = server
            .read_trades_inner(
                strategy_id,
                ReadTradesParams {
                    date_from: Some(ymd(2026, 6, 5)),
                    ..Default::default()
                },
            )
            .await
            .expect("read_trades");

        let dates: Vec<NaiveDate> = result.trades.iter().map(|t| t.date).collect();
        assert_eq!(dates, vec![ymd(2026, 6, 10), ymd(2026, 6, 5)]);
    }

    #[sqlx::test(migrations = false)]
    async fn read_trades_respects_limit(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        for day in 1..=3 {
            seed_trade(
                &db,
                strategy_id,
                "7203",
                "buy",
                100,
                1000,
                ymd(2026, 6, day),
            )
            .await;
        }
        let server = build_server(db);

        let result = server
            .read_trades_inner(
                strategy_id,
                ReadTradesParams {
                    limit: Some(1),
                    ..Default::default()
                },
            )
            .await
            .expect("read_trades");

        let dates: Vec<NaiveDate> = result.trades.iter().map(|t| t.date).collect();
        assert_eq!(dates, vec![ymd(2026, 6, 3)]);
    }
}
