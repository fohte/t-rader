use serde_json::{Value, json};

use super::tests_common::{build_server, insert_strategy};

#[backend_test_macros::database_test]
async fn japanese_stock_only_tools_reject_foreign_ids_through_tool_dispatch(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = insert_strategy(&db, "sample-strategy").await;
    let server = build_server(db);
    let invalid_stock_id = "KR:QZ9012";
    let errors = vec![
        server
            .invoke::<_, Value>(
                "read_fin_summary",
                strategy_id,
                json!({"symbol": invalid_stock_id}),
                None,
                None,
            )
            .await
            .expect_err("foreign stock IDs are unsupported"),
        server
            .invoke::<_, Value>(
                "read_margin",
                strategy_id,
                json!({"symbol": invalid_stock_id}),
                None,
                None,
            )
            .await
            .expect_err("foreign stock IDs are unsupported"),
        server
            .invoke::<_, Value>(
                "read_valuation",
                strategy_id,
                json!({
                    "symbol": invalid_stock_id,
                    "from": "2026-01-01",
                    "to": "2026-01-02",
                }),
                None,
                None,
            )
            .await
            .expect_err("foreign stock IDs are unsupported"),
        server
            .invoke::<_, Value>(
                "read_shareholding_structure",
                strategy_id,
                json!({"symbol": invalid_stock_id}),
                None,
                None,
            )
            .await
            .expect_err("foreign stock IDs are unsupported"),
        server
            .invoke::<_, Value>(
                "read_short_sale_reports",
                strategy_id,
                json!({"symbol": invalid_stock_id}),
                None,
                None,
            )
            .await
            .expect_err("foreign stock IDs are unsupported"),
        server
            .invoke::<_, Value>(
                "check_buyable_qty",
                strategy_id,
                json!({"symbol": invalid_stock_id}),
                None,
                None,
            )
            .await
            .expect_err("foreign stock IDs are unsupported"),
        server
            .invoke::<_, Value>(
                "record_prediction",
                strategy_id,
                json!({
                    "target_stock_id": invalid_stock_id,
                    "benchmark_stock_id": "US:QZ-7",
                    "direction": "outperform",
                    "probability": 0.7,
                    "base_date": "2026-01-01",
                    "due_date": "2026-02-01",
                }),
                None,
                None,
            )
            .await
            .expect_err("foreign stock IDs are unsupported"),
    ];

    assert_eq!(
        errors,
        vec![
            rmcp::ErrorData::invalid_params(
                "only Japanese stocks are supported by this tool",
                None,
            ),
            rmcp::ErrorData::invalid_params(
                "only Japanese stocks are supported by this tool",
                None,
            ),
            rmcp::ErrorData::invalid_params(
                "only Japanese stocks are supported by this tool",
                None,
            ),
            rmcp::ErrorData::invalid_params(
                "only Japanese stocks are supported by this tool",
                None,
            ),
            rmcp::ErrorData::invalid_params(
                "only Japanese stocks are supported by this tool",
                None,
            ),
            rmcp::ErrorData::invalid_params(
                "only Japanese stocks are supported by this tool",
                None,
            ),
            rmcp::ErrorData::invalid_params("predictions only support Japanese stocks", None,),
        ],
    );
}
