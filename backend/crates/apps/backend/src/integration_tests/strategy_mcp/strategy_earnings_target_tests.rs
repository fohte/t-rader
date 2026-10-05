#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;

    use super::super::test_api::earnings_targets::EarningsTargetParams;
    use super::super::test_server::{StrategyServer, ToolOutput};
    use super::super::tests_common::{build_server, insert_strategy, ts_sentinel};
    use crate::testing::{insert_test_group, insert_test_stock};

    async fn setup(
        db: gateway_postgres::DatabaseHandle,
    ) -> (gateway_postgres::DatabaseHandle, StrategyServer, Uuid) {
        insert_test_stock(&db, "SAMPLE001", "Sample issuer").await;
        insert_test_group(&db, "axis-demo", "group-demo", "Sample group").await;
        let strategy_id = insert_strategy(&db, "sample strategy").await;
        let server = build_server(db.clone());
        (db, server, strategy_id)
    }

    fn target(ref_kind: &str, ref_id: &str) -> EarningsTargetParams {
        EarningsTargetParams {
            ref_kind: ref_kind.into(),
            ref_id: ref_id.into(),
        }
    }

    fn normalize_targets(
        output: ToolOutput<super::super::test_api::earnings_targets::ListEarningsTargetsResult>,
    ) -> serde_json::Value {
        output
            .normalize_json(|value| {
                for target in value["targets"]
                    .as_array_mut()
                    .expect("targets are an array")
                {
                    target["created_at"] = json!(ts_sentinel());
                }
            })
            .as_json()
            .clone()
    }

    #[backend_test_macros::database_test]
    async fn adding_stock_and_group_targets_is_idempotent(db: gateway_postgres::DatabaseHandle) {
        let (_, server, strategy_id) = setup(db).await;
        let added_stock = server
            .add_earnings_target(strategy_id, target("stock", "SAMPLE001"))
            .await
            .expect("add stock target");
        let added_group = server
            .add_earnings_target(strategy_id, target("group", "axis-demo/group-demo"))
            .await
            .expect("add group target");
        let duplicate_stock = server
            .add_earnings_target(strategy_id, target("stock", "SAMPLE001"))
            .await
            .expect("repeat stock target");

        assert_eq!(
            (
                added_stock.as_json().clone(),
                added_group.as_json().clone(),
                duplicate_stock.as_json().clone(),
            ),
            (
                json!({ "changed": true }),
                json!({ "changed": true }),
                json!({ "changed": false }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn listing_targets_returns_sorted_stock_and_group_targets(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (_, server, strategy_id) = setup(db).await;
        server
            .add_earnings_target(strategy_id, target("stock", "SAMPLE001"))
            .await
            .expect("add stock target");
        server
            .add_earnings_target(strategy_id, target("group", "axis-demo/group-demo"))
            .await
            .expect("add group target");
        let listed = server
            .list_earnings_targets(strategy_id)
            .await
            .expect("list targets");

        assert_eq!(
            normalize_targets(listed),
            json!({
                "targets": [
                    {
                        "ref_kind": "group",
                        "ref_id": "axis-demo/group-demo",
                        "created_at": ts_sentinel(),
                    },
                    {
                        "ref_kind": "stock",
                        "ref_id": "SAMPLE001",
                        "created_at": ts_sentinel(),
                    },
                ],
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn adding_indicator_and_missing_stock_targets_returns_invalid_params(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (_, server, strategy_id) = setup(db).await;
        let invalid_kind = server
            .add_earnings_target(strategy_id, target("indicator", "sample-indicator"))
            .await
            .expect_err("indicator targets are rejected");
        let invalid_reference = server
            .add_earnings_target(strategy_id, target("stock", "MISSING001"))
            .await
            .expect_err("unknown stock targets are rejected");

        assert_eq!(
            [invalid_kind, invalid_reference],
            [
                rmcp::ErrorData::invalid_params("ref_kind must be stock or group", None),
                rmcp::ErrorData::invalid_params("stock reference not found: MISSING001", None,),
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn removing_targets_is_idempotent(db: gateway_postgres::DatabaseHandle) {
        let (_, server, strategy_id) = setup(db).await;
        let group_target = target("group", "axis-demo/group-demo");
        server
            .add_earnings_target(strategy_id, group_target.clone())
            .await
            .expect("add group target");
        let removed = server
            .remove_earnings_target(strategy_id, group_target.clone())
            .await
            .expect("remove group target");
        let repeated_remove = server
            .remove_earnings_target(strategy_id, group_target)
            .await
            .expect("repeat group target removal");

        assert_eq!(
            (removed.as_json().clone(), repeated_remove.as_json().clone()),
            (json!({ "changed": true }), json!({ "changed": false })),
        );
    }
}
