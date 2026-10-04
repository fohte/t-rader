#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use gateway_postgres::entities::strategy_earnings_target;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use serde_json::{Value, json};
    use uuid::Uuid;

    use super::super::{assert_response_eq, create_strategy};
    use crate::testing::{create_test_server_with_db, insert_test_group, insert_test_stock};

    fn normalize_target_timestamps(mut value: Value) -> Value {
        if let Some(rows) = value.as_array_mut() {
            for row in rows {
                row["created_at"] = json!("<created_at>");
            }
        }
        value
    }

    #[backend_test_macros::database_test]
    async fn endpoints_manage_targets_validate_refs_and_cascade_on_strategy_delete(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        insert_test_stock(&db, "SAMPLE001", "Sample issuer").await;
        insert_test_group(&db, "axis-demo", "group-demo", "Sample group").await;
        let strategy_id = create_strategy(&server, "sample strategy").await;
        let other_strategy_id = create_strategy(&server, "other strategy").await;

        let stock_target = json!({ "ref_kind": "stock", "ref_id": "SAMPLE001" });
        let group_target = json!({ "ref_kind": "group", "ref_id": "axis-demo/group-demo" });
        let add_stock = server
            .post(&format!("/api/strategies/{strategy_id}/earnings-targets"))
            .json(&stock_target)
            .await;
        let add_group = server
            .post(&format!("/api/strategies/{strategy_id}/earnings-targets"))
            .json(&group_target)
            .await;
        let add_duplicate = server
            .post(&format!("/api/strategies/{strategy_id}/earnings-targets"))
            .json(&stock_target)
            .await;
        let add_to_other_strategy = server
            .post(&format!(
                "/api/strategies/{other_strategy_id}/earnings-targets"
            ))
            .json(&stock_target)
            .await;
        let list = server
            .get(&format!("/api/strategies/{strategy_id}/earnings-targets"))
            .await;
        let invalid_ref = server
            .post(&format!("/api/strategies/{strategy_id}/earnings-targets"))
            .json(&json!({ "ref_kind": "stock", "ref_id": "MISSING001" }))
            .await;
        let invalid_kind = server
            .post(&format!("/api/strategies/{strategy_id}/earnings-targets"))
            .json(&json!({ "ref_kind": "indicator", "ref_id": "sample-indicator" }))
            .await;
        let remove_group = server
            .delete(&format!(
                "/api/strategies/{strategy_id}/earnings-targets?ref_kind=group&ref_id=axis-demo%2Fgroup-demo"
            ))
            .await;
        let remove_missing_group = server
            .delete(&format!(
                "/api/strategies/{strategy_id}/earnings-targets?ref_kind=group&ref_id=axis-demo%2Fgroup-demo"
            ))
            .await;
        let deleted_strategy = server
            .delete(&format!("/api/strategies/{strategy_id}"))
            .await;
        let deleted_strategy_id = Uuid::parse_str(&strategy_id).expect("valid strategy ID");
        let deleted_rows = strategy_earnings_target::Entity::find()
            .filter(strategy_earnings_target::Column::StrategyId.eq(deleted_strategy_id))
            .all(&db)
            .await
            .expect("query deleted strategy targets");
        let remaining_rows = strategy_earnings_target::Entity::find()
            .filter(
                strategy_earnings_target::Column::StrategyId
                    .eq(Uuid::parse_str(&other_strategy_id).expect("valid strategy ID")),
            )
            .all(&db)
            .await
            .expect("query other strategy targets");

        let responses = [
            &add_stock,
            &add_group,
            &add_duplicate,
            &add_to_other_strategy,
            &list,
            &invalid_ref,
            &invalid_kind,
            &remove_group,
            &remove_missing_group,
            &deleted_strategy,
        ]
        .into_iter()
        .map(|response| {
            let body = if response.as_bytes().is_empty() {
                None
            } else {
                Some(response.json::<Value>())
            };
            (response.status_code(), body)
        })
        .collect::<Vec<_>>();
        let mut responses = responses;
        if let Some((_, body)) = responses.get_mut(4) {
            *body = body.take().map(normalize_target_timestamps);
        }
        let other_targets = remaining_rows
            .into_iter()
            .map(|row| (row.ref_kind, row.ref_id))
            .collect::<Vec<_>>();

        assert_eq!(
            (responses, deleted_rows.len(), other_targets),
            (
                vec![
                    (StatusCode::OK, Some(json!({ "changed": true }))),
                    (StatusCode::OK, Some(json!({ "changed": true }))),
                    (StatusCode::OK, Some(json!({ "changed": false }))),
                    (StatusCode::OK, Some(json!({ "changed": true }))),
                    (
                        StatusCode::OK,
                        Some(json!([
                            {"ref_kind": "group", "ref_id": "axis-demo/group-demo", "created_at": "<created_at>"},
                            {"ref_kind": "stock", "ref_id": "SAMPLE001", "created_at": "<created_at>"},
                        ])),
                    ),
                    (
                        StatusCode::BAD_REQUEST,
                        Some(json!({ "error": "stock reference not found: MISSING001" })),
                    ),
                    (
                        StatusCode::BAD_REQUEST,
                        Some(json!({ "error": "ref_kind must be stock or group" })),
                    ),
                    (StatusCode::OK, Some(json!({ "changed": true }))),
                    (StatusCode::OK, Some(json!({ "changed": false }))),
                    (StatusCode::NO_CONTENT, None),
                ],
                0,
                vec![("stock".into(), "SAMPLE001".into())],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_targets_returns_404_for_unknown_strategy(db: gateway_postgres::DatabaseHandle) {
        let server = crate::testing::create_test_server(db).await;
        let response = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000/earnings-targets")
            .await;

        assert_response_eq(
            &response,
            StatusCode::NOT_FOUND,
            Some(json!({
                "error": "strategy 00000000-0000-0000-0000-000000000000 not found"
            })),
        );
    }
}
