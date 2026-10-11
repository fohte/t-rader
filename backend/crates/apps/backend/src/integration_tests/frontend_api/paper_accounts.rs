#[cfg(test)]
mod tests {
    use super::super::{assert_response_eq, create_agent_config, create_strategy};
    use crate::testing::{
        create_test_server, find_current_note_version, insert_test_note, insert_test_stock,
    };
    use axum::http::StatusCode;
    use chrono::{DateTime, NaiveDate};
    use gateway_postgres::entities::{bars, instruments, paper_order, paper_order_result};
    use rust_decimal::Decimal;
    use sea_orm::{ActiveValue::Set, EntityTrait};
    use serde_json::{Value, json};
    use uuid::Uuid;

    async fn server_with_account_references(
        db: gateway_postgres::DatabaseHandle,
    ) -> (axum_test::TestServer, String) {
        insert_test_stock(&db, "DEMO-BENCHMARK", "Demo benchmark").await;
        let server = create_test_server(db).await;
        let strategy_id = create_strategy(&server, "sample-strategy").await;
        create_agent_config(&server, "example-purpose").await;
        (server, strategy_id)
    }

    fn account_request(
        strategy_id: &str,
        purpose: &str,
        name: &str,
        benchmark_stock_id: Option<&str>,
    ) -> Value {
        json!({
            "strategy_id": strategy_id,
            "purpose": purpose,
            "name": name,
            "initial_cash_jpy": 1500000,
            "benchmark_stock_id": benchmark_stock_id,
            "started_on": "2026-01-02",
        })
    }

    fn normalize_account(mut value: Value) -> Value {
        if let Some(accounts) = value.as_array_mut() {
            for account in accounts {
                if let Some(id) = account.get_mut("id") {
                    *id = json!("<id>");
                }
            }
        } else if let Some(id) = value.get_mut("id") {
            *id = json!("<id>");
        }
        value
    }

    async fn create_account(
        server: &axum_test::TestServer,
        request: Value,
    ) -> axum_test::TestResponse {
        server.post("/api/paper-accounts").json(&request).await
    }

    #[backend_test_macros::database_test]
    async fn create_returns_the_paper_account(db: gateway_postgres::DatabaseHandle) {
        let (server, strategy_id) = server_with_account_references(db).await;
        let response = create_account(
            &server,
            account_request(
                &strategy_id,
                "example-purpose",
                "sample-account",
                Some("DEMO-BENCHMARK"),
            ),
        )
        .await;
        let body = normalize_account(response.json());

        assert_eq!(
            (response.status_code(), body),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "name": "sample-account",
                    "strategy_id": strategy_id,
                    "purpose": "example-purpose",
                    "initial_cash_jpy": 1500000,
                    "benchmark_stock_id": "DEMO-BENCHMARK",
                    "started_on": "2026-01-02",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_returns_created_paper_accounts(db: gateway_postgres::DatabaseHandle) {
        let (server, strategy_id) = server_with_account_references(db).await;
        let _ = create_account(
            &server,
            account_request(
                &strategy_id,
                "example-purpose",
                "sample-account",
                Some("DEMO-BENCHMARK"),
            ),
        )
        .await;

        let response = server.get("/api/paper-accounts").await;
        let body = normalize_account(response.json());

        assert_eq!(
            (response.status_code(), body),
            (
                StatusCode::OK,
                json!([{
                    "id": "<id>",
                    "name": "sample-account",
                    "strategy_id": strategy_id,
                    "purpose": "example-purpose",
                    "initial_cash_jpy": 1500000,
                    "benchmark_stock_id": "DEMO-BENCHMARK",
                    "started_on": "2026-01-02",
                }]),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_returns_empty_without_paper_accounts(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;

        let response = server.get("/api/paper-accounts").await;

        assert_response_eq(&response, StatusCode::OK, Some(json!([])));
    }

    #[backend_test_macros::database_test]
    async fn stats_returns_performance_fields_for_each_account(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (server, strategy_id) = server_with_account_references(db).await;
        let account = create_account(
            &server,
            account_request(&strategy_id, "example-purpose", "sample-account", None),
        )
        .await;
        let account_id = account.json::<Value>()["id"]
            .as_str()
            .expect("account id")
            .to_string();
        account.assert_status(StatusCode::CREATED);

        let response = server.get("/api/paper-accounts/stats").await;

        assert_response_eq(
            &response,
            StatusCode::OK,
            Some(json!([{
                "account_id": account_id,
                "account_name": "sample-account",
                "strategy_id": strategy_id,
                "purpose": "example-purpose",
                "started_on": "2026-01-02",
                "initial_cash_jpy": 1500000,
                "benchmark_stock_id": null,
                "as_of": "2026-01-02",
                "total_assets_jpy": 1500000,
                "return_since_start": 0,
                "benchmark_return": null,
                "closed_trade_count": 0,
                "win_rate": null,
                "average_win_excess_return": null,
                "average_loss_excess_return": null,
                "unrealized_pnl_jpy": 0,
            }])),
        );
    }

    #[backend_test_macros::database_test]
    async fn portfolio_returns_valued_positions_and_note_references_for_orders(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (server, strategy_id) = server_with_account_references(db.clone()).await;
        let account = create_account(
            &server,
            account_request(&strategy_id, "example-purpose", "sample-account", None),
        )
        .await;
        let account_id =
            Uuid::parse_str(account.json::<Value>()["id"].as_str().expect("account id"))
                .expect("account id is a UUID");
        account.assert_status(StatusCode::CREATED);

        let note_id = insert_test_note(&db, "sample decision", "sample rationale").await;
        let note_version_id = find_current_note_version(&db, note_id)
            .await
            .expect("find note version")
            .expect("note has a current version")
            .id;
        let second_note_id =
            insert_test_note(&db, "sample decision two", "sample rationale two").await;
        let second_note_version_id = find_current_note_version(&db, second_note_id)
            .await
            .expect("find second note version")
            .expect("second note has a current version")
            .id;
        let stock_id = "DEMO-STOCK";
        insert_test_stock(&db, stock_id, "Demo stock").await;
        instruments::Entity::insert(instruments::ActiveModel {
            id: Set(stock_id.into()),
            name: Set("Demo stock".into()),
            market: Set("TSE".into()),
            sector: Set(None),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert instrument");

        let order_id = Uuid::new_v4();
        let ordered_at = DateTime::parse_from_rfc3339("2026-01-05T10:00:00+09:00")
            .expect("valid order timestamp");
        paper_order::Entity::insert(paper_order::ActiveModel {
            id: Set(order_id),
            account_id: Set(account_id),
            stock_id: Set(stock_id.to_string()),
            side: Set("buy".into()),
            qty: Set(100),
            note_version_id: Set(note_version_id),
            ordered_at: Set(ordered_at),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert paper order");
        paper_order_result::Entity::insert(paper_order_result::ActiveModel {
            order_id: Set(order_id),
            outcome: Set("filled".into()),
            fill_date: Set(Some(
                NaiveDate::from_ymd_opt(2026, 1, 6).expect("valid fill date"),
            )),
            fill_price: Set(Some(Decimal::from(1000))),
            reject_reason: Set(None),
            decided_at: Set(DateTime::parse_from_rfc3339("2026-01-06T18:00:00+09:00")
                .expect("valid decision timestamp")),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert paper order result");
        let rejected_order_id = Uuid::new_v4();
        let rejected_ordered_at = DateTime::parse_from_rfc3339("2026-01-07T10:00:00+09:00")
            .expect("valid rejected order timestamp");
        paper_order::Entity::insert(paper_order::ActiveModel {
            id: Set(rejected_order_id),
            account_id: Set(account_id),
            stock_id: Set(stock_id.to_string()),
            side: Set("buy".into()),
            qty: Set(500),
            note_version_id: Set(second_note_version_id),
            ordered_at: Set(rejected_ordered_at),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert rejected paper order");
        paper_order_result::Entity::insert(paper_order_result::ActiveModel {
            order_id: Set(rejected_order_id),
            outcome: Set("rejected".into()),
            fill_date: Set(None),
            fill_price: Set(None),
            reject_reason: Set(Some("insufficient_cash".into())),
            decided_at: Set(DateTime::parse_from_rfc3339("2026-01-07T18:00:00+09:00")
                .expect("valid rejected decision timestamp")),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert rejected paper order result");
        let pending_order_id = Uuid::new_v4();
        let pending_ordered_at = DateTime::parse_from_rfc3339("2026-01-08T10:00:00+09:00")
            .expect("valid pending order timestamp");
        paper_order::Entity::insert(paper_order::ActiveModel {
            id: Set(pending_order_id),
            account_id: Set(account_id),
            stock_id: Set(stock_id.to_string()),
            side: Set("sell".into()),
            qty: Set(100),
            note_version_id: Set(note_version_id),
            ordered_at: Set(pending_ordered_at),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert pending paper order");
        bars::Entity::insert(bars::ActiveModel {
            instrument_id: Set(stock_id.to_string()),
            timeframe: Set("1d".into()),
            timestamp: Set(DateTime::parse_from_rfc3339("2026-01-07T00:00:00Z")
                .expect("valid daily bar timestamp")),
            open: Set(Decimal::from(1190)),
            high: Set(Decimal::from(1210)),
            low: Set(Decimal::from(1180)),
            close: Set(Decimal::from(1200)),
            volume: Set(1000),
            adjustment_factor: Set(Decimal::ONE),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert daily bar");

        let response = server
            .get(&format!("/api/paper-accounts/{account_id}/portfolio"))
            .await;

        assert_response_eq(
            &response,
            StatusCode::OK,
            Some(json!({
                "account_id": account_id,
                "account_name": "sample-account",
                "strategy_id": strategy_id,
                "purpose": "example-purpose",
                "started_on": "2026-01-02",
                "as_of": "2026-01-07",
                "initial_cash_jpy": 1500000,
                "cash_jpy": 1400000,
                "positions": [{
                    "stock_id": stock_id,
                    "qty": 100,
                    "avg_cost_jpy": 1000,
                    "current_price_jpy": 1200,
                    "market_value_jpy": 120000,
                    "unrealized_pnl_jpy": 20000,
                }],
                "orders": [{
                    "order_id": order_id,
                    "stock_id": stock_id,
                    "side": "buy",
                    "qty": 100,
                    "note_id": note_id,
                    "note_version_id": note_version_id,
                    "ordered_at": "2026-01-05T01:00:00Z",
                    "outcome": "filled",
                    "fill_date": "2026-01-06",
                    "fill_price_jpy": 1000,
                    "reject_reason": null,
                }, {
                    "order_id": rejected_order_id,
                    "stock_id": stock_id,
                    "side": "buy",
                    "qty": 500,
                    "note_id": second_note_id,
                    "note_version_id": second_note_version_id,
                    "ordered_at": "2026-01-07T01:00:00Z",
                    "outcome": "rejected",
                    "fill_date": null,
                    "fill_price_jpy": null,
                    "reject_reason": "insufficient_cash",
                }, {
                    "order_id": pending_order_id,
                    "stock_id": stock_id,
                    "side": "sell",
                    "qty": 100,
                    "note_id": note_id,
                    "note_version_id": note_version_id,
                    "ordered_at": "2026-01-08T01:00:00Z",
                    "outcome": null,
                    "fill_date": null,
                    "fill_price_jpy": null,
                    "reject_reason": null,
                }],
            })),
        );
    }

    #[backend_test_macros::database_test]
    async fn portfolio_returns_empty_positions_and_orders_for_a_new_account(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (server, strategy_id) = server_with_account_references(db).await;
        let account = create_account(
            &server,
            account_request(&strategy_id, "example-purpose", "sample-account", None),
        )
        .await;
        let account_id =
            Uuid::parse_str(account.json::<Value>()["id"].as_str().expect("account id"))
                .expect("account id is a UUID");
        account.assert_status(StatusCode::CREATED);

        let response = server
            .get(&format!("/api/paper-accounts/{account_id}/portfolio"))
            .await;

        assert_response_eq(
            &response,
            StatusCode::OK,
            Some(json!({
                "account_id": account_id,
                "account_name": "sample-account",
                "strategy_id": strategy_id,
                "purpose": "example-purpose",
                "started_on": "2026-01-02",
                "as_of": "2026-01-02",
                "initial_cash_jpy": 1500000,
                "cash_jpy": 1500000,
                "positions": [],
                "orders": [],
            })),
        );
    }

    #[backend_test_macros::database_test]
    async fn portfolio_returns_not_found_for_an_unknown_account(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let account_id = Uuid::new_v4();

        let response = server
            .get(&format!("/api/paper-accounts/{account_id}/portfolio"))
            .await;

        assert_response_eq(
            &response,
            StatusCode::NOT_FOUND,
            Some(json!({
                "error": format!("paper account {account_id} not found"),
            })),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_rejects_missing_strategy_purpose_and_benchmark(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (server, strategy_id) = server_with_account_references(db).await;
        let missing_strategy_id = Uuid::new_v4().to_string();
        let requests = [
            account_request(
                &missing_strategy_id,
                "example-purpose",
                "missing-strategy-account",
                None,
            ),
            account_request(
                &strategy_id,
                "missing-purpose",
                "missing-purpose-account",
                None,
            ),
            account_request(
                &strategy_id,
                "example-purpose",
                "missing-benchmark-account",
                Some("DEMO-MISSING"),
            ),
        ];

        let mut actual = Vec::new();
        for request in requests {
            let response = create_account(&server, request).await;
            actual.push((response.status_code(), response.json::<Value>()));
        }

        assert_eq!(
            actual,
            vec![
                (
                    StatusCode::BAD_REQUEST,
                    json!({ "error": "referenced resource does not exist" }),
                );
                3
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn create_rejects_zero_initial_cash(db: gateway_postgres::DatabaseHandle) {
        let (server, strategy_id) = server_with_account_references(db).await;
        let mut request =
            account_request(&strategy_id, "example-purpose", "zero-cash-account", None);
        request["initial_cash_jpy"] = json!(0);

        let response = create_account(&server, request).await;

        assert_response_eq(
            &response,
            StatusCode::BAD_REQUEST,
            Some(json!({ "error": "initial_cash_jpy must be greater than zero" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_rejects_a_duplicate_strategy_purpose_pair(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (server, strategy_id) = server_with_account_references(db).await;
        let first = create_account(
            &server,
            account_request(&strategy_id, "example-purpose", "first-account", None),
        )
        .await;
        first.assert_status(StatusCode::CREATED);

        let duplicate = create_account(
            &server,
            account_request(&strategy_id, "example-purpose", "second-account", None),
        )
        .await;

        assert_response_eq(
            &duplicate,
            StatusCode::CONFLICT,
            Some(json!({ "error": "resource already exists" })),
        );
    }
}
