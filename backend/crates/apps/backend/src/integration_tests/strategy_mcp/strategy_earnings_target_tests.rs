#[cfg(test)]
mod tests {
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use serde_json::json;

    use super::super::test_api::earnings_targets::EarningsTargetParams;
    use super::super::test_server::StrategyServer;
    use super::super::tests_common::{build_server, insert_strategy, ts_sentinel};
    use crate::testing::{insert_test_group, insert_test_stock};

    #[backend_test_macros::database_test]
    async fn mcp_tools_add_list_and_remove_stock_and_group_targets(
        db: gateway_postgres::DatabaseHandle,
    ) {
        insert_test_stock(&db, "SAMPLE001", "Sample issuer").await;
        insert_test_group(&db, "axis-demo", "group-demo", "Sample group").await;
        let strategy_id = insert_strategy(&db, "sample strategy").await;
        let server: StrategyServer = build_server(db.clone());

        let stock_target = EarningsTargetParams {
            ref_kind: "stock".into(),
            ref_id: "SAMPLE001".into(),
        };
        let group_target = EarningsTargetParams {
            ref_kind: "group".into(),
            ref_id: "axis-demo/group-demo".into(),
        };
        let added_stock = server
            .add_earnings_target(strategy_id, stock_target.clone())
            .await
            .expect("add stock target");
        let added_group = server
            .add_earnings_target(strategy_id, group_target.clone())
            .await
            .expect("add group target");
        let duplicate_stock = server
            .add_earnings_target(strategy_id, stock_target.clone())
            .await
            .expect("repeat stock target");
        let invalid_kind = server
            .add_earnings_target(
                strategy_id,
                EarningsTargetParams {
                    ref_kind: "indicator".into(),
                    ref_id: "sample-indicator".into(),
                },
            )
            .await
            .expect_err("indicator targets are rejected");
        let invalid_reference = server
            .add_earnings_target(
                strategy_id,
                EarningsTargetParams {
                    ref_kind: "stock".into(),
                    ref_id: "MISSING001".into(),
                },
            )
            .await
            .expect_err("unknown stock targets are rejected");
        let listed = server
            .list_earnings_targets(strategy_id)
            .await
            .expect("list targets")
            .normalize_json(|value| {
                for target in value["targets"]
                    .as_array_mut()
                    .expect("targets are an array")
                {
                    target["created_at"] = json!(ts_sentinel());
                }
            });
        let removed_group = server
            .remove_earnings_target(strategy_id, group_target.clone())
            .await
            .expect("remove group target");
        let repeated_remove_group = server
            .remove_earnings_target(strategy_id, group_target)
            .await
            .expect("repeat group target removal");
        let remaining = server
            .list_earnings_targets(strategy_id)
            .await
            .expect("list remaining targets")
            .normalize_json(|value| {
                for target in value["targets"]
                    .as_array_mut()
                    .expect("targets are an array")
                {
                    target["created_at"] = json!(ts_sentinel());
                }
            });
        let rows = gateway_postgres::entities::strategy_earnings_target::Entity::find()
            .filter(
                gateway_postgres::entities::strategy_earnings_target::Column::StrategyId
                    .eq(strategy_id),
            )
            .all(&db)
            .await
            .expect("query persisted targets");
        let persisted = rows
            .into_iter()
            .map(|row| (row.ref_kind, row.ref_id))
            .collect::<Vec<_>>();

        assert_eq!(
            (
                added_stock.as_json().clone(),
                added_group.as_json().clone(),
                duplicate_stock.as_json().clone(),
                invalid_kind,
                invalid_reference,
                listed.as_json().clone(),
                removed_group.as_json().clone(),
                repeated_remove_group.as_json().clone(),
                remaining.as_json().clone(),
                persisted,
            ),
            (
                json!({ "changed": true }),
                json!({ "changed": true }),
                json!({ "changed": false }),
                rmcp::ErrorData::invalid_params("ref_kind must be stock or group", None),
                rmcp::ErrorData::invalid_params("stock reference not found: MISSING001", None),
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
                json!({ "changed": true }),
                json!({ "changed": false }),
                json!({
                    "targets": [{
                        "ref_kind": "stock",
                        "ref_id": "SAMPLE001",
                        "created_at": ts_sentinel(),
                    }],
                }),
                vec![("stock".into(), "SAMPLE001".into())],
            ),
        );
    }
}
