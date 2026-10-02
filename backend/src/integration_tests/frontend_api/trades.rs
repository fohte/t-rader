#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use rust_decimal::Decimal;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use serde_json::{Value, json};
    use uuid::Uuid;

    use crate::testing::{create_test_server_with_db, insert_test_strategy};
    use gateway_postgres::entities::{change_history, trade};

    fn normalize_trade(mut value: Value) -> Value {
        value["id"] = json!("<dyn>");
        value["strategy_id"] = json!("<dyn>");
        value["created_at"] = json!("<dyn>");
        value["updated_at"] = json!("<dyn>");
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
        trade::ActiveModel {
            id: Set(seed.trade_id),
            strategy_id: Set(seed.strategy_id),
            symbol: Set("FICTIONAL-SYMBOL".into()),
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
        response.assert_status_ok();
        let actual = response
            .json::<Vec<Value>>()
            .into_iter()
            .map(normalize_trade)
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
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

        let response = server
            .get(&format!("/api/trades/summary?strategy_id={strategy_id}"))
            .await;

        assert_eq!(
            response.json::<Value>(),
            json!({
                "strategy_id": strategy_id,
                "trade_count": 3,
                "realized_pnl": 3470,
                "positions": [{
                    "symbol": "FICTIONAL-SYMBOL",
                    "qty": 50,
                    "avg_cost": 120,
                    "cost_basis": 6000,
                    "realized_pnl": 3470,
                }],
            }),
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
        let response_status = response.status_code();
        let trade_exists = trade::Entity::find_by_id(trade_id)
            .one(&db)
            .await
            .expect("query trade")
            .is_some();
        let history = history_snapshot(&db, trade_id).await;

        assert_eq!(
            (response_status, trade_exists, history),
            (
                StatusCode::NO_CONTENT,
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
