#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::paper_trade::NewPaperAccount;
    use core_domain::bar::{Bar, Timeframe};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sea_orm::EntityTrait;
    use serde_json::{Value, json};
    use uuid::Uuid;

    use gateway_postgres::entities::{instruments, paper_order, paper_order_result, strategy_task};
    use gateway_postgres::repositories::bars::upsert_bars;

    use super::super::StrategyServer;
    use super::super::tests_common::{current_note_version_id, insert_strategy, ts_sentinel};

    async fn create_account(
        db: &gateway_postgres::DatabaseHandle,
        strategy_id: Uuid,
        name: &str,
        purpose: &str,
        benchmark_stock_id: Option<&str>,
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
                benchmark_stock_id: benchmark_stock_id.map(str::to_string),
                started_on: NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid date"),
            })
            .await
            .expect("create paper account")
            .id
    }

    async fn set_task_execution_id(
        db: &gateway_postgres::DatabaseHandle,
        strategy_id: Uuid,
        purpose: Option<&str>,
    ) -> String {
        let task_id = crate::testing::insert_test_strategy_task(
            db,
            strategy_id,
            "paper trading task",
            purpose,
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

    async fn place_order(
        server: &StrategyServer,
        strategy_id: Uuid,
        task_id: &str,
        stock_id: &str,
        side: &str,
        note_version_id: Uuid,
    ) -> Uuid {
        let response = server
            .invoke::<_, Value>(
                "place_paper_order",
                strategy_id,
                json!({
                    "stock_id": stock_id,
                    "side": side,
                    "qty": 100,
                    "note_version_id": note_version_id,
                }),
                Some(execution_id(task_id)),
                None,
            )
            .await
            .expect("place paper order");
        Uuid::parse_str(
            response.as_json()["order_id"]
                .as_str()
                .expect("order ID is a string"),
        )
        .expect("order ID is a UUID")
    }

    fn date(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2025, 1, day).expect("valid test date")
    }

    fn timestamp(date: NaiveDate) -> chrono::DateTime<chrono::FixedOffset> {
        date.and_hms_opt(0, 0, 0)
            .expect("valid test time")
            .and_utc()
            .fixed_offset()
    }

    async fn set_ordered_at(
        db: &impl sea_orm::ConnectionTrait,
        order_id: Uuid,
        ordered_on: NaiveDate,
    ) {
        paper_order::ActiveModel {
            id: Set(order_id),
            ordered_at: Set(timestamp(ordered_on)),
            ..Default::default()
        }
        .update(db)
        .await
        .expect("set order date");
    }

    async fn insert_filled_result(
        db: &impl sea_orm::ConnectionTrait,
        order_id: Uuid,
        fill_date: NaiveDate,
        fill_price: i64,
    ) {
        paper_order_result::ActiveModel {
            order_id: Set(order_id),
            outcome: Set("filled".to_string()),
            fill_date: Set(Some(fill_date)),
            fill_price: Set(Some(rust_decimal::Decimal::from(fill_price))),
            reject_reason: Set(None),
            decided_at: Set(timestamp(fill_date)),
        }
        .insert(db)
        .await
        .expect("insert filled result");
    }

    async fn insert_rejected_result(
        db: &impl sea_orm::ConnectionTrait,
        order_id: Uuid,
        rejected_on: NaiveDate,
    ) {
        paper_order_result::ActiveModel {
            order_id: Set(order_id),
            outcome: Set("rejected".to_string()),
            fill_date: Set(None),
            fill_price: Set(None),
            reject_reason: Set(Some("insufficient_shares".to_string())),
            decided_at: Set(timestamp(rejected_on)),
        }
        .insert(db)
        .await
        .expect("insert rejected result");
    }

    async fn seed_daily_bar(
        db: &impl sea_orm::ConnectionTrait,
        stock_id: &str,
        bar_date: NaiveDate,
        open: i64,
        close: i64,
    ) {
        instruments::Entity::insert(instruments::ActiveModel {
            id: Set(stock_id.to_string()),
            name: Set("Sample instrument".to_string()),
            market: Set("TSE".to_string()),
            sector: Set(None),
        })
        .exec(db)
        .await
        .expect("insert test instrument");
        upsert_bars(
            db,
            vec![Bar {
                instrument_id: stock_id.to_string(),
                timeframe: Timeframe::Daily,
                timestamp: bar_date
                    .and_hms_opt(0, 0, 0)
                    .expect("valid test time")
                    .and_utc(),
                open: rust_decimal::Decimal::from(open),
                high: rust_decimal::Decimal::from(open.max(close)),
                low: rust_decimal::Decimal::from(open.min(close)),
                close: rust_decimal::Decimal::from(close),
                volume: 100,
            }],
        )
        .await
        .expect("insert test bar");
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
        expected_order_ids: &[Value],
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
        for (index, order) in value["orders"]
            .as_array_mut()
            .expect("orders are an array")
            .iter_mut()
            .enumerate()
        {
            order["ordered_at"] = timestamp.clone();
            if let Some(expected_order_id) = expected_order_ids.get(index) {
                normalize_uuid_field(
                    order,
                    "order_id",
                    expected_order_id,
                    &format!("normalized-order-id-{index}"),
                );
            } else {
                order["order_id"] = json!("unexpected-id");
            }
            normalize_uuid_field(
                order,
                "note_version_id",
                expected_note_version_id,
                "normalized-note-version-id",
            );
        }
    }

    fn normalize_account_stats(
        value: &mut Value,
        expected_account_id: &Value,
        expected_strategy_id: &Value,
    ) {
        for account in value["accounts"]
            .as_array_mut()
            .expect("accounts are an array")
        {
            normalize_uuid_field(
                account,
                "account_id",
                expected_account_id,
                "normalized-account-id",
            );
            normalize_uuid_field(
                account,
                "strategy_id",
                expected_strategy_id,
                "normalized-strategy-id",
            );
        }
    }

    #[backend_test_macros::database_test]
    async fn place_paper_order_uses_execution_task_purpose_to_select_account(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "paper strategy").await;
        let first_account_id =
            create_account(&db, strategy_id, "Sample account A", "purpose-a", None).await;
        let second_account_id =
            create_account(&db, strategy_id, "Sample account B", "purpose-b", None).await;
        let first_task_id = set_task_execution_id(&db, strategy_id, Some("purpose-a")).await;
        let second_task_id = set_task_execution_id(&db, strategy_id, Some("purpose-b")).await;
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
                    std::slice::from_ref(&actual_order_id),
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
                    &[],
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
                        "order_id": "normalized-order-id-0",
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

    #[backend_test_macros::database_test]
    async fn read_paper_portfolio_maps_filled_and_rejected_results(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "paper strategy").await;
        let account_id =
            create_account(&db, strategy_id, "Sample account", "purpose-a", None).await;
        let task_id = set_task_execution_id(&db, strategy_id, Some("purpose-a")).await;
        crate::testing::insert_test_stock(&db, "TST2", "Sample stock").await;
        let note_id =
            crate::testing::insert_test_note(&db, "Sample rationale", "Order basis").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let server = super::super::tests_common::build_server(db.clone());
        let buy_order_id = place_order(
            &server,
            strategy_id,
            &task_id,
            "TST2",
            "buy",
            note_version_id,
        )
        .await;
        let rejected_order_id = place_order(
            &server,
            strategy_id,
            &task_id,
            "TST2",
            "sell",
            note_version_id,
        )
        .await;
        set_ordered_at(&db, buy_order_id, date(5)).await;
        set_ordered_at(&db, rejected_order_id, date(6)).await;
        insert_filled_result(&db, buy_order_id, date(6), 100).await;
        insert_rejected_result(&db, rejected_order_id, date(7)).await;
        seed_daily_bar(&db, "TST2", date(7), 110, 120).await;

        let expected_account_id = serde_json::to_value(account_id).expect("serialize account ID");
        let expected_strategy_id =
            serde_json::to_value(strategy_id).expect("serialize strategy ID");
        let expected_note_version_id =
            serde_json::to_value(note_version_id).expect("serialize note version ID");
        let expected_order_ids = vec![
            serde_json::to_value(buy_order_id).expect("serialize buy order ID"),
            serde_json::to_value(rejected_order_id).expect("serialize rejected order ID"),
        ];
        let portfolio = server
            .invoke::<_, Value>(
                "read_paper_portfolio",
                strategy_id,
                json!({}),
                Some(execution_id(&task_id)),
                None,
            )
            .await
            .expect("read paper portfolio")
            .normalize_json(|value| {
                normalize_portfolio(
                    value,
                    &expected_account_id,
                    &expected_strategy_id,
                    &expected_note_version_id,
                    &expected_order_ids,
                );
            });
        let ordered_at = serde_json::to_value(ts_sentinel()).expect("serialize timestamp");

        assert_eq!(
            portfolio.as_json(),
            &json!({
                "account_id": "normalized-account-id",
                "account_name": "Sample account",
                "strategy_id": "normalized-strategy-id",
                "purpose": "purpose-a",
                "started_on": "2025-01-01",
                "as_of": "2025-01-07",
                "initial_cash_jpy": 100_000.0,
                "cash_jpy": 90_000.0,
                "positions": [{
                    "stock_id": "TST2",
                    "qty": 100,
                    "avg_cost_jpy": 100.0,
                    "current_price_jpy": 120.0,
                    "market_value_jpy": 12_000.0,
                    "unrealized_pnl_jpy": 2_000.0,
                }],
                "orders": [
                    {
                        "order_id": "normalized-order-id-0",
                        "stock_id": "TST2",
                        "side": "buy",
                        "qty": 100,
                        "note_version_id": "normalized-note-version-id",
                        "ordered_at": ordered_at.clone(),
                        "outcome": "filled",
                        "fill_date": "2025-01-06",
                        "fill_price_jpy": 100.0,
                        "reject_reason": null,
                    },
                    {
                        "order_id": "normalized-order-id-1",
                        "stock_id": "TST2",
                        "side": "sell",
                        "qty": 100,
                        "note_version_id": "normalized-note-version-id",
                        "ordered_at": ordered_at,
                        "outcome": "rejected",
                        "fill_date": null,
                        "fill_price_jpy": null,
                        "reject_reason": "insufficient_shares",
                    },
                ],
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn read_paper_portfolio_reads_stats_account_without_task_purpose(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let source_strategy_id = insert_strategy(&db, "source strategy").await;
        let reader_strategy_id = insert_strategy(&db, "reader strategy").await;
        create_account(
            &db,
            source_strategy_id,
            "Sample source account",
            "purpose-a",
            None,
        )
        .await;
        let source_task_id =
            set_task_execution_id(&db, source_strategy_id, Some("purpose-a")).await;
        let reader_task_id = set_task_execution_id(&db, reader_strategy_id, None).await;
        crate::testing::insert_test_stock(&db, "TST6", "Sample stock").await;
        let note_id =
            crate::testing::insert_test_note(&db, "Sample rationale", "Order basis").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let server = super::super::tests_common::build_server(db.clone());
        let order_id = place_order(
            &server,
            source_strategy_id,
            &source_task_id,
            "TST6",
            "buy",
            note_version_id,
        )
        .await;
        set_ordered_at(&db, order_id, date(5)).await;
        insert_filled_result(&db, order_id, date(6), 100).await;
        seed_daily_bar(&db, "TST6", date(7), 110, 120).await;

        let stats = server
            .invoke::<_, Value>(
                "read_paper_stats",
                reader_strategy_id,
                json!({}),
                None,
                None,
            )
            .await
            .expect("read paper stats");
        let account_id = stats.as_json()["accounts"]
            .as_array()
            .expect("accounts are an array")
            .iter()
            .find(|account| account["account_name"] == "Sample source account")
            .expect("source account is listed in stats")["account_id"]
            .clone();
        let expected_account_id = account_id.clone();
        let expected_strategy_id =
            serde_json::to_value(source_strategy_id).expect("serialize strategy ID");
        let expected_note_version_id =
            serde_json::to_value(note_version_id).expect("serialize note version ID");
        let expected_order_ids = vec![serde_json::to_value(order_id).expect("serialize order ID")];
        let portfolio = server
            .invoke::<_, Value>(
                "read_paper_portfolio",
                reader_strategy_id,
                json!({ "account_id": account_id }),
                Some(execution_id(&reader_task_id)),
                None,
            )
            .await
            .expect("read paper account from a task without a purpose")
            .normalize_json(|value| {
                normalize_portfolio(
                    value,
                    &expected_account_id,
                    &expected_strategy_id,
                    &expected_note_version_id,
                    &expected_order_ids,
                );
            });
        let ordered_at = serde_json::to_value(ts_sentinel()).expect("serialize timestamp");

        assert_eq!(
            portfolio.as_json(),
            &json!({
                "account_id": "normalized-account-id",
                "account_name": "Sample source account",
                "strategy_id": "normalized-strategy-id",
                "purpose": "purpose-a",
                "started_on": "2025-01-01",
                "as_of": "2025-01-07",
                "initial_cash_jpy": 100_000.0,
                "cash_jpy": 90_000.0,
                "positions": [{
                    "stock_id": "TST6",
                    "qty": 100,
                    "avg_cost_jpy": 100.0,
                    "current_price_jpy": 120.0,
                    "market_value_jpy": 12_000.0,
                    "unrealized_pnl_jpy": 2_000.0,
                }],
                "orders": [{
                    "order_id": "normalized-order-id-0",
                    "stock_id": "TST6",
                    "side": "buy",
                    "qty": 100,
                    "note_version_id": "normalized-note-version-id",
                    "ordered_at": ordered_at,
                    "outcome": "filled",
                    "fill_date": "2025-01-06",
                    "fill_price_jpy": 100.0,
                    "reject_reason": null,
                }],
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn read_paper_portfolio_rejects_unknown_account_id(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "reader strategy").await;
        let server = super::super::tests_common::build_server(db);
        let error = server
            .invoke::<_, Value>(
                "read_paper_portfolio",
                strategy_id,
                json!({ "account_id": Uuid::new_v4() }),
                None,
                None,
            )
            .await
            .expect_err("unknown account ID is rejected");

        assert_eq!(
            error,
            rmcp::ErrorData::invalid_params("paper account does not exist", None),
        );
    }

    #[backend_test_macros::database_test]
    async fn read_paper_stats_maps_returns_and_missing_benchmark_data(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "paper strategy").await;
        crate::testing::insert_test_stock(&db, "TSTB", "Sample benchmark").await;
        let account_id = create_account(
            &db,
            strategy_id,
            "Sample account",
            "purpose-a",
            Some("TSTB"),
        )
        .await;
        let task_id = set_task_execution_id(&db, strategy_id, Some("purpose-a")).await;
        crate::testing::insert_test_stock(&db, "TST3", "Sample stock").await;
        let note_id =
            crate::testing::insert_test_note(&db, "Sample rationale", "Order basis").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let server = super::super::tests_common::build_server(db.clone());
        let buy_order_id = place_order(
            &server,
            strategy_id,
            &task_id,
            "TST3",
            "buy",
            note_version_id,
        )
        .await;
        let sell_order_id = place_order(
            &server,
            strategy_id,
            &task_id,
            "TST3",
            "sell",
            note_version_id,
        )
        .await;
        set_ordered_at(&db, buy_order_id, date(5)).await;
        set_ordered_at(&db, sell_order_id, date(7)).await;
        insert_filled_result(&db, buy_order_id, date(6), 100).await;
        insert_filled_result(&db, sell_order_id, date(8), 120).await;

        let expected_account_id = serde_json::to_value(account_id).expect("serialize account ID");
        let expected_strategy_id =
            serde_json::to_value(strategy_id).expect("serialize strategy ID");
        let stats = server
            .invoke::<_, Value>("read_paper_stats", strategy_id, json!({}), None, None)
            .await
            .expect("read paper stats")
            .normalize_json(|value| {
                normalize_account_stats(value, &expected_account_id, &expected_strategy_id);
            });

        assert_eq!(
            stats.as_json(),
            &json!({
                "accounts": [{
                    "account_id": "normalized-account-id",
                    "account_name": "Sample account",
                    "strategy_id": "normalized-strategy-id",
                    "purpose": "purpose-a",
                    "started_on": "2025-01-01",
                    "initial_cash_jpy": 100_000.0,
                    "benchmark_stock_id": "TSTB",
                    "as_of": "2025-01-01",
                    "total_assets_jpy": 102_000.0,
                    "return_since_start": 0.02,
                    "benchmark_return": null,
                    "closed_trade_count": 1,
                    "win_rate": null,
                    "average_win_excess_return": null,
                    "average_loss_excess_return": null,
                    "unrealized_pnl_jpy": 0.0,
                }],
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn place_paper_order_rejects_a_task_from_another_strategy(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let task_strategy_id = insert_strategy(&db, "task strategy").await;
        let request_strategy_id = insert_strategy(&db, "request strategy").await;
        create_account(
            &db,
            request_strategy_id,
            "Request account",
            "shared-purpose",
            None,
        )
        .await;
        let task_id = set_task_execution_id(&db, task_strategy_id, Some("shared-purpose")).await;
        let server = super::super::tests_common::build_server(db);

        let error = server
            .invoke::<_, Value>(
                "place_paper_order",
                request_strategy_id,
                json!({
                    "stock_id": "TST4",
                    "side": "buy",
                    "qty": 100,
                    "note_version_id": Uuid::nil(),
                }),
                Some(execution_id(&task_id)),
                None,
            )
            .await
            .expect_err("task from another strategy is rejected");

        assert_eq!(
            error,
            rmcp::ErrorData::invalid_params(
                "x-execution-id refers to a task from another strategy",
                None,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn place_paper_order_maps_unknown_references_to_invalid_params(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "paper strategy").await;
        create_account(&db, strategy_id, "Sample account", "purpose-a", None).await;
        let task_id = set_task_execution_id(&db, strategy_id, Some("purpose-a")).await;
        crate::testing::insert_test_stock(&db, "TST5", "Sample stock").await;
        let note_id =
            crate::testing::insert_test_note(&db, "Sample rationale", "Order basis").await;
        let note_version_id = current_note_version_id(&db, note_id).await;
        let server = super::super::tests_common::build_server(db);

        let errors = vec![
            server
                .invoke::<_, Value>(
                    "place_paper_order",
                    strategy_id,
                    json!({
                        "stock_id": "TST-MISSING",
                        "side": "buy",
                        "qty": 100,
                        "note_version_id": note_version_id,
                    }),
                    Some(execution_id(&task_id)),
                    None,
                )
                .await
                .expect_err("unknown stock is rejected"),
            server
                .invoke::<_, Value>(
                    "place_paper_order",
                    strategy_id,
                    json!({
                        "stock_id": "TST5",
                        "side": "buy",
                        "qty": 100,
                        "note_version_id": Uuid::nil(),
                    }),
                    Some(execution_id(&task_id)),
                    None,
                )
                .await
                .expect_err("unknown note version is rejected"),
        ];

        assert_eq!(
            errors,
            vec![
                rmcp::ErrorData::invalid_params("referenced resource does not exist", None),
                rmcp::ErrorData::invalid_params("referenced resource does not exist", None),
            ],
        );
    }
}
