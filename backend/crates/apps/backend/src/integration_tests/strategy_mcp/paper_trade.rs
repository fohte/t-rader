#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::paper_trade::NewPaperAccount;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use serde_json::{Value, json};
    use uuid::Uuid;

    use gateway_postgres::entities::strategy_task;

    use super::super::tests_common::{current_note_version_id, insert_strategy, ts_sentinel};

    async fn create_account(
        db: &gateway_postgres::DatabaseHandle,
        strategy_id: Uuid,
        name: &str,
        purpose: &str,
    ) -> Uuid {
        crate::testing::agent_config::create(db, purpose.to_string())
            .await
            .expect("create test agent config");
        crate::services::use_cases::build_use_cases(db.clone())
            .paper_trade()
            .create_account(NewPaperAccount {
                name: name.to_string(),
                strategy_id,
                purpose: purpose.to_string(),
                initial_cash_jpy: rust_decimal::Decimal::from(100_000),
                benchmark_stock_id: None,
                started_on: NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid date"),
            })
            .await
            .expect("create paper account")
            .id
    }

    async fn set_task_execution_id(
        db: &gateway_postgres::DatabaseHandle,
        strategy_id: Uuid,
        purpose: &str,
    ) -> String {
        let task_id = crate::testing::insert_test_strategy_task(
            db,
            strategy_id,
            "record a paper order",
            Some(purpose),
            ts_sentinel(),
        )
        .await;
        let a2a_task_id = format!("paper-task-{}", Uuid::new_v4());
        strategy_task::ActiveModel {
            task_id: Set(task_id),
            a2a_task_id: Set(Some(a2a_task_id.clone())),
            ..Default::default()
        }
        .update(db)
        .await
        .expect("set task execution id");
        a2a_task_id
    }

    fn execution_id(a2a_task_id: &str) -> String {
        format!("{a2a_task_id}:{}", Uuid::new_v4())
    }

    fn normalize_ordered_at(value: &mut Value) {
        value["ordered_at"] = serde_json::to_value(ts_sentinel()).expect("serialize timestamp");
    }

    fn normalize_uuid_field(value: &mut Value, field: &str, expected: &Value, normalized: &str) {
        value[field] = if value[field] == *expected {
            json!(normalized)
        } else {
            json!("unexpected-id")
        };
    }

    fn normalize_portfolio(
        value: &mut Value,
        expected_account_id: &Value,
        expected_strategy_id: &Value,
        expected_note_version_id: &Value,
        expected_order_id: &Value,
    ) {
        normalize_uuid_field(
            value,
            "account_id",
            expected_account_id,
            "normalized-account-id",
        );
        normalize_uuid_field(
            value,
            "strategy_id",
            expected_strategy_id,
            "normalized-strategy-id",
        );
        let timestamp = serde_json::to_value(ts_sentinel()).expect("serialize timestamp");
        for order in value["orders"].as_array_mut().expect("orders are an array") {
            order["ordered_at"] = timestamp.clone();
            normalize_uuid_field(order, "order_id", expected_order_id, "normalized-order-id");
            normalize_uuid_field(
                order,
                "note_version_id",
                expected_note_version_id,
                "normalized-note-version-id",
            );
        }
    }

    #[backend_test_macros::database_test]
    async fn place_paper_order_uses_execution_task_purpose_to_select_account(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "paper strategy").await;
        let first_account_id =
            create_account(&db, strategy_id, "Sample account A", "purpose-a").await;
        let second_account_id =
            create_account(&db, strategy_id, "Sample account B", "purpose-b").await;
        let first_task_id = set_task_execution_id(&db, strategy_id, "purpose-a").await;
        let second_task_id = set_task_execution_id(&db, strategy_id, "purpose-b").await;
        crate::testing::insert_test_stock(&db, "TST1", "Sample stock").await;
        let note_id =
            crate::testing::insert_test_note(&db, "Sample rationale", "Order basis").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let server = super::super::tests_common::build_server(db);

        let placed = server
            .invoke::<_, Value>(
                "place_paper_order",
                strategy_id,
                json!({
                    "stock_id": "TST1",
                    "side": "buy",
                    "qty": 100,
                    "note_version_id": note_version_id,
                }),
                Some(execution_id(&first_task_id)),
                None,
            )
            .await
            .expect("place paper order");
        let actual_order_id = Uuid::parse_str(
            placed.as_json()["order_id"]
                .as_str()
                .expect("order ID is a string"),
        )
        .expect("order ID is a UUID");
        let actual_order_id = serde_json::to_value(actual_order_id).expect("serialize order ID");
        let expected_note_version_id =
            serde_json::to_value(note_version_id).expect("serialize note version ID");
        let placed = placed.normalize_json(|value| {
            normalize_ordered_at(value);
            normalize_uuid_field(value, "order_id", &actual_order_id, "normalized-order-id");
            normalize_uuid_field(
                value,
                "note_version_id",
                &expected_note_version_id,
                "normalized-note-version-id",
            );
        });
        let target_portfolio = server
            .invoke::<_, Value>(
                "read_paper_portfolio",
                strategy_id,
                json!({}),
                Some(execution_id(&first_task_id)),
                None,
            )
            .await
            .expect("read target paper portfolio")
            .normalize_json(|value| {
                normalize_portfolio(
                    value,
                    &serde_json::to_value(first_account_id).expect("serialize account ID"),
                    &serde_json::to_value(strategy_id).expect("serialize strategy ID"),
                    &expected_note_version_id,
                    &actual_order_id,
                );
            });
        let other_portfolio = server
            .invoke::<_, Value>(
                "read_paper_portfolio",
                strategy_id,
                json!({}),
                Some(execution_id(&second_task_id)),
                None,
            )
            .await
            .expect("read other paper portfolio")
            .normalize_json(|value| {
                normalize_portfolio(
                    value,
                    &serde_json::to_value(second_account_id).expect("serialize account ID"),
                    &serde_json::to_value(strategy_id).expect("serialize strategy ID"),
                    &expected_note_version_id,
                    &actual_order_id,
                );
            });

        let ordered_at = serde_json::to_value(ts_sentinel()).expect("serialize timestamp");
        assert_eq!(
            (
                placed.as_json().clone(),
                target_portfolio.as_json().clone(),
                other_portfolio.as_json().clone(),
            ),
            (
                json!({
                    "order_id": "normalized-order-id",
                    "stock_id": "TST1",
                    "side": "buy",
                    "qty": 100,
                    "note_version_id": "normalized-note-version-id",
                    "ordered_at": ordered_at.clone(),
                }),
                json!({
                    "account_id": "normalized-account-id",
                    "account_name": "Sample account A",
                    "strategy_id": "normalized-strategy-id",
                    "purpose": "purpose-a",
                    "started_on": "2025-01-01",
                    "as_of": "2025-01-01",
                    "initial_cash_jpy": 100_000.0,
                    "cash_jpy": 100_000.0,
                    "positions": [],
                    "orders": [{
                        "order_id": "normalized-order-id",
                        "stock_id": "TST1",
                        "side": "buy",
                        "qty": 100,
                        "note_version_id": "normalized-note-version-id",
                        "ordered_at": ordered_at,
                        "outcome": null,
                        "fill_date": null,
                        "fill_price_jpy": null,
                        "reject_reason": null,
                    }],
                }),
                json!({
                    "account_id": "normalized-account-id",
                    "account_name": "Sample account B",
                    "strategy_id": "normalized-strategy-id",
                    "purpose": "purpose-b",
                    "started_on": "2025-01-01",
                    "as_of": "2025-01-01",
                    "initial_cash_jpy": 100_000.0,
                    "cash_jpy": 100_000.0,
                    "positions": [],
                    "orders": [],
                }),
            ),
        );
    }
}
