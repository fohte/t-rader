#[cfg(test)]
mod tests {
    use super::super::assert_response_eq;
    use axum::http::StatusCode;
    use chrono::{NaiveDate, TimeZone, Utc};
    use rust_decimal::Decimal;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use serde_json::{Value, json};
    use uuid::Uuid;

    use crate::testing::{create_test_server_with_db, insert_test_strategy};
    use gateway_postgres::entities::{bars, change_history, instruments, trade};

    fn normalize_trade(mut value: Value) -> Value {
        for key in ["id", "strategy_id", "created_at", "updated_at"] {
            if let Some(field) = value.get_mut(key) {
                *field = json!("<dyn>");
            }
        }
        value
    }

    struct SeedTrade {
        strategy_id: Uuid,
        trade_id: Uuid,
        side: &'static str,
        qty: i64,
        price: i64,
        fee: i64,
        day: u32,
    }

    async fn seed_trade(db: &impl sea_orm::ConnectionTrait, seed: SeedTrade) {
        seed_trade_for_symbol(db, "FICTIONAL-SYMBOL", seed).await;
    }

    async fn seed_trade_for_symbol(
        db: &impl sea_orm::ConnectionTrait,
        symbol: &str,
        seed: SeedTrade,
    ) {
        trade::ActiveModel {
            id: Set(seed.trade_id),
            strategy_id: Set(seed.strategy_id),
            symbol: Set(symbol.into()),
            side: Set(seed.side.into()),
            qty: Set(Decimal::from(seed.qty)),
            price: Set(Decimal::from(seed.price)),
            fee: Set(Decimal::from(seed.fee)),
            date: Set(chrono::NaiveDate::from_ymd_opt(2026, 6, seed.day).expect("valid date")),
            source: Set("manual".into()),
            note: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert trade");
    }

    async fn seed_daily_bar(
        db: &impl sea_orm::ConnectionTrait,
        symbol: &str,
        day: u32,
        close: i64,
    ) {
        instruments::Entity::insert(instruments::ActiveModel {
            id: Set(symbol.to_string()),
            name: Set("Fictional Instrument".into()),
            market: Set("TSE".into()),
            sector: Set(None),
        })
        .on_conflict(
            OnConflict::column(instruments::Column::Id)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(db)
        .await
        .expect("insert instrument");

        let timestamp = NaiveDate::from_ymd_opt(2026, 6, day)
            .and_then(|date| date.and_hms_opt(0, 0, 0))
            .map(|datetime| Utc.from_utc_datetime(&datetime).fixed_offset())
            .expect("valid daily bar timestamp");
        let close = Decimal::from(close);
        bars::Entity::insert(bars::ActiveModel {
            instrument_id: Set(symbol.to_string()),
            timeframe: Set("1d".into()),
            timestamp: Set(timestamp),
            open: Set(close),
            high: Set(close),
            low: Set(close),
            close: Set(close),
            volume: Set(100),
            adjustment_factor: Set(Decimal::ONE),
        })
        .exec_without_returning(db)
        .await
        .expect("insert daily bar");
    }

    async fn history_snapshot(
        db: &impl sea_orm::ConnectionTrait,
        trade_id: Uuid,
    ) -> (String, Uuid, String, String, String, Value) {
        let history = change_history::Entity::find()
            .filter(change_history::Column::TargetId.eq(trade_id))
            .one(db)
            .await
            .expect("query change history")
            .expect("change history exists");
        (
            history.target_kind,
            history.target_id,
            history.actor_kind,
            history.actor_label,
            history.op,
            history.diff_json,
        )
    }

    #[backend_test_macros::database_test]
    async fn list_returns_flattened_trade_with_note_count(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "fictional-strategy").await;
        let trade_id = Uuid::new_v4();
        trade::ActiveModel {
            id: Set(trade_id),
            strategy_id: Set(strategy_id),
            symbol: Set("fictional-symbol-123".into()),
            side: Set("buy".into()),
            qty: Set(Decimal::from(1200)),
            price: Set(Decimal::from(275)),
            fee: Set(Decimal::from(1)),
            date: Set(chrono::NaiveDate::from_ymd_opt(2026, 2, 3).expect("valid date")),
            source: Set("manual".into()),
            note: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(&db)
        .await
        .expect("insert trade");

        let response = server
            .get(&format!("/api/trades?strategy_id={strategy_id}"))
            .await;
        let actual = response
            .json::<Vec<Value>>()
            .into_iter()
            .map(normalize_trade)
            .collect::<Vec<_>>();
        assert_eq!(
            (response.status_code(), actual),
            (
                StatusCode::OK,
                vec![json!({
                    "id": "<dyn>",
                    "strategy_id": "<dyn>",
                    "symbol": "fictional-symbol-123",
                    "side": "buy",
                    "qty": 1200,
                    "price": 275,
                    "fee": 1,
                    "date": "2026-02-03",
                    "source": "manual",
                    "note": null,
                    "created_at": "<dyn>",
                    "updated_at": "<dyn>",
                    "note_count": 0,
                })],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn detail_returns_fifo_matches_and_remaining_quantity_across_strategies(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let first_strategy_id = insert_test_strategy(&db, "fictional-strategy-a").await;
        let second_strategy_id = insert_test_strategy(&db, "fictional-strategy-b").await;
        let first_buy_id = Uuid::new_v4();
        let second_buy_id = Uuid::new_v4();
        let sell_id = Uuid::new_v4();
        for (strategy_id, trade_id, side, qty, price, fee, day) in [
            (first_strategy_id, first_buy_id, "buy", 10, 100, 0, 1),
            (second_strategy_id, second_buy_id, "buy", 20, 120, 0, 2),
            (first_strategy_id, sell_id, "sell", 15, 130, 20, 3),
        ] {
            seed_trade(
                &db,
                SeedTrade {
                    strategy_id,
                    trade_id,
                    side,
                    qty,
                    price,
                    fee,
                    day,
                },
            )
            .await;
        }

        let sell_response = server.get(&format!("/api/trades/{sell_id}")).await;
        let buy_response = server.get(&format!("/api/trades/{second_buy_id}")).await;
        let closed_buy_response = server.get(&format!("/api/trades/{first_buy_id}")).await;
        assert_eq!(
            (
                sell_response.status_code(),
                normalize_trade(sell_response.json::<Value>()),
                buy_response.status_code(),
                normalize_trade(buy_response.json::<Value>()),
                closed_buy_response.status_code(),
                normalize_trade(closed_buy_response.json::<Value>()),
            ),
            (
                StatusCode::OK,
                json!({
                    "id": "<dyn>",
                    "strategy_id": "<dyn>",
                    "symbol": "FICTIONAL-SYMBOL",
                    "side": "sell",
                    "qty": 15,
                    "price": 130,
                    "fee": 20,
                    "date": "2026-06-03",
                    "source": "manual",
                    "note": null,
                    "created_at": "<dyn>",
                    "updated_at": "<dyn>",
                    "realized_pnl": 330,
                    "matched_buys": [
                        { "date": "2026-06-01", "qty": 10, "price": 100 },
                        { "date": "2026-06-02", "qty": 5, "price": 120 },
                    ],
                }),
                StatusCode::OK,
                json!({
                    "id": "<dyn>",
                    "strategy_id": "<dyn>",
                    "symbol": "FICTIONAL-SYMBOL",
                    "side": "buy",
                    "qty": 20,
                    "price": 120,
                    "fee": 0,
                    "date": "2026-06-02",
                    "source": "manual",
                    "note": null,
                    "created_at": "<dyn>",
                    "updated_at": "<dyn>",
                    "remaining_qty": 15,
                }),
                StatusCode::OK,
                json!({
                    "id": "<dyn>",
                    "strategy_id": "<dyn>",
                    "symbol": "FICTIONAL-SYMBOL",
                    "side": "buy",
                    "qty": 10,
                    "price": 100,
                    "fee": 0,
                    "date": "2026-06-01",
                    "source": "manual",
                    "note": null,
                    "created_at": "<dyn>",
                    "updated_at": "<dyn>",
                    "remaining_qty": 0,
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn detail_reports_sell_without_prior_buys(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "fictional-strategy").await;
        let sell_id = Uuid::new_v4();
        seed_trade(
            &db,
            SeedTrade {
                strategy_id,
                trade_id: sell_id,
                side: "sell",
                qty: 3,
                price: 140,
                fee: 10,
                day: 1,
            },
        )
        .await;

        let response = server.get(&format!("/api/trades/{sell_id}")).await;
        assert_eq!(
            (
                response.status_code(),
                normalize_trade(response.json::<Value>()),
            ),
            (
                StatusCode::OK,
                json!({
                    "id": "<dyn>",
                    "strategy_id": "<dyn>",
                    "symbol": "FICTIONAL-SYMBOL",
                    "side": "sell",
                    "qty": 3,
                    "price": 140,
                    "fee": 10,
                    "date": "2026-06-01",
                    "source": "manual",
                    "note": null,
                    "created_at": "<dyn>",
                    "updated_at": "<dyn>",
                    "realized_pnl": 410,
                    "matched_buys": [],
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_persists_trade_and_change_history(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "fictional-strategy").await;
        let response = server
            .post("/api/trades")
            .json(&json!({
                    "strategy_id": strategy_id,
                    "symbol": "  FICTIONAL-SYMBOL  ",
                    "side": "buy",
                    "qty": 3,
                    "price": 25,
                    "date": "2026-06-01",
                    "source": "manual",
            }))
            .await;
        let response_status = response.status_code();
        let response_body = response.json::<Value>();
        let trade_id = Uuid::parse_str(response_body["id"].as_str().expect("trade id"))
            .expect("valid trade id");
        let actual = normalize_trade(response_body);
        let history = history_snapshot(&db, trade_id).await;

        assert_eq!(
            (response_status, actual, history),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<dyn>",
                    "strategy_id": "<dyn>",
                    "symbol": "FICTIONAL-SYMBOL",
                    "side": "buy",
                    "qty": 3,
                    "price": 25,
                    "fee": 0,
                    "date": "2026-06-01",
                    "source": "manual",
                    "note": null,
                    "created_at": "<dyn>",
                    "updated_at": "<dyn>",
                }),
                (
                    "trade".to_string(),
                    trade_id,
                    "human".to_string(),
                    "user".to_string(),
                    "create".to_string(),
                    json!({
                        "strategy_id": strategy_id,
                        "symbol": "FICTIONAL-SYMBOL",
                        "side": "buy",
                        "qty": 3,
                        "price": 25,
                    }),
                ),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_records_the_changed_fields(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "fictional-strategy").await;
        let trade_id = Uuid::new_v4();
        seed_trade(
            &db,
            SeedTrade {
                strategy_id,
                trade_id,
                side: "buy",
                qty: 3,
                price: 25,
                fee: 0,
                day: 1,
            },
        )
        .await;

        let response = server
            .patch(&format!("/api/trades/{trade_id}"))
            .json(&json!({ "side": "sell", "qty": 5 }))
            .await;
        let response_status = response.status_code();
        let actual = normalize_trade(response.json::<Value>());
        let history = history_snapshot(&db, trade_id).await;

        assert_eq!(
            (response_status, actual, history),
            (
                StatusCode::OK,
                json!({
                    "id": "<dyn>",
                    "strategy_id": "<dyn>",
                    "symbol": "FICTIONAL-SYMBOL",
                    "side": "sell",
                    "qty": 5,
                    "price": 25,
                    "fee": 0,
                    "date": "2026-06-01",
                    "source": "manual",
                    "note": null,
                    "created_at": "<dyn>",
                    "updated_at": "<dyn>",
                }),
                (
                    "trade".to_string(),
                    trade_id,
                    "human".to_string(),
                    "user".to_string(),
                    "update".to_string(),
                    json!({
                        "side": { "from": "buy", "to": "sell" },
                        "qty": { "from": 3, "to": 5 },
                    }),
                ),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn summary_uses_fifo_cost_and_realized_profit(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "fictional-strategy").await;
        seed_trade(
            &db,
            SeedTrade {
                strategy_id,
                trade_id: Uuid::new_v4(),
                side: "buy",
                qty: 100,
                price: 100,
                fee: 10,
                day: 1,
            },
        )
        .await;
        seed_trade(
            &db,
            SeedTrade {
                strategy_id,
                trade_id: Uuid::new_v4(),
                side: "buy",
                qty: 100,
                price: 120,
                fee: 0,
                day: 2,
            },
        )
        .await;
        seed_trade(
            &db,
            SeedTrade {
                strategy_id,
                trade_id: Uuid::new_v4(),
                side: "sell",
                qty: 150,
                price: 130,
                fee: 20,
                day: 3,
            },
        )
        .await;
        seed_daily_bar(&db, "FICTIONAL-SYMBOL", 3, 140).await;

        let response = server
            .get(&format!("/api/trades/summary?strategy_id={strategy_id}"))
            .await;

        assert_eq!(
            (response.status_code(), response.json::<Value>()),
            (
                StatusCode::OK,
                json!({
                    "strategy_id": strategy_id,
                    "trade_count": 3,
                    "realized_pnl": 3470,
                    "price_observed_at": "2026-06-03",
                    "positions": [{
                        "symbol": "FICTIONAL-SYMBOL",
                        "qty": 50,
                        "avg_cost": 120,
                        "cost_basis": 6000,
                        "realized_pnl": 3470,
                        "current_price": 140,
                        "market_value": 7000,
                        "unrealized_pnl": 1000,
                    }],
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn summary_uses_only_positions_at_the_common_latest_bar_date(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "fictional-strategy").await;
        seed_trade(
            &db,
            SeedTrade {
                strategy_id,
                trade_id: Uuid::new_v4(),
                side: "buy",
                qty: 2,
                price: 50,
                fee: 0,
                day: 1,
            },
        )
        .await;
        seed_trade_for_symbol(
            &db,
            "FICTIONAL-BETA",
            SeedTrade {
                strategy_id,
                trade_id: Uuid::new_v4(),
                side: "buy",
                qty: 3,
                price: 20,
                fee: 0,
                day: 1,
            },
        )
        .await;
        seed_daily_bar(&db, "FICTIONAL-SYMBOL", 3, 70).await;
        seed_daily_bar(&db, "FICTIONAL-BETA", 2, 30).await;

        let response = server
            .get(&format!("/api/trades/summary?strategy_id={strategy_id}"))
            .await;

        assert_eq!(
            (response.status_code(), response.json::<Value>()),
            (
                StatusCode::OK,
                json!({
                    "strategy_id": strategy_id,
                    "trade_count": 2,
                    "realized_pnl": 0,
                    "price_observed_at": "2026-06-03",
                    "positions": [{
                        "symbol": "FICTIONAL-BETA",
                        "qty": 3,
                        "avg_cost": 20,
                        "cost_basis": 60,
                        "realized_pnl": 0,
                        "current_price": null,
                        "market_value": null,
                        "unrealized_pnl": null,
                    }, {
                        "symbol": "FICTIONAL-SYMBOL",
                        "qty": 2,
                        "avg_cost": 50,
                        "cost_basis": 100,
                        "realized_pnl": 0,
                        "current_price": 70,
                        "market_value": 140,
                        "unrealized_pnl": 40,
                    }],
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn summary_returns_missing_market_values_when_daily_bar_is_unavailable(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "fictional-strategy").await;
        seed_trade(
            &db,
            SeedTrade {
                strategy_id,
                trade_id: Uuid::new_v4(),
                side: "buy",
                qty: 4,
                price: 50,
                fee: 0,
                day: 1,
            },
        )
        .await;

        let response = server
            .get(&format!("/api/trades/summary?strategy_id={strategy_id}"))
            .await;
        let body = response.json::<Value>();
        let instrument_exists = instruments::Entity::find_by_id("FICTIONAL-SYMBOL")
            .one(&db)
            .await
            .expect("query instrument")
            .is_some();

        assert_eq!(
            (response.status_code(), body, instrument_exists),
            (
                StatusCode::OK,
                json!({
                    "strategy_id": strategy_id,
                    "trade_count": 1,
                    "realized_pnl": 0,
                    "price_observed_at": null,
                    "positions": [{
                        "symbol": "FICTIONAL-SYMBOL",
                        "qty": 4,
                        "avg_cost": 50,
                        "cost_basis": 200,
                        "realized_pnl": 0,
                        "current_price": null,
                        "market_value": null,
                        "unrealized_pnl": null,
                    }],
                }),
                false,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_removes_trade_and_records_change_history(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "fictional-strategy").await;
        let trade_id = Uuid::new_v4();
        seed_trade(
            &db,
            SeedTrade {
                strategy_id,
                trade_id,
                side: "buy",
                qty: 3,
                price: 25,
                fee: 0,
                day: 1,
            },
        )
        .await;

        let response = server.delete(&format!("/api/trades/{trade_id}")).await;
        assert_response_eq(&response, StatusCode::NO_CONTENT, None);
        let trade_exists = trade::Entity::find_by_id(trade_id)
            .one(&db)
            .await
            .expect("query trade")
            .is_some();
        let history = history_snapshot(&db, trade_id).await;

        assert_eq!(
            (trade_exists, history),
            (
                false,
                (
                    "trade".to_string(),
                    trade_id,
                    "human".to_string(),
                    "user".to_string(),
                    "delete".to_string(),
                    json!({}),
                ),
            ),
        );
    }
}
