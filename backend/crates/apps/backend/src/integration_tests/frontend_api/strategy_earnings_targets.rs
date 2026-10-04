#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use gateway_postgres::entities::strategy_earnings_target;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
    use serde_json::{Value, json};
    use uuid::Uuid;

    use super::super::create_strategy;
    use crate::testing::{create_test_server_with_db, insert_test_group, insert_test_stock};

    const UNKNOWN_STRATEGY_ID: &str = "00000000-0000-0000-0000-000000000000";

    async fn setup(
        db: gateway_postgres::DatabaseHandle,
    ) -> (
        gateway_postgres::DatabaseHandle,
        axum_test::TestServer,
        String,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        insert_test_stock(&db, "SAMPLE001", "Sample issuer").await;
        insert_test_group(&db, "axis-demo", "group-demo", "Sample group").await;
        let strategy_id = create_strategy(&server, "sample strategy").await;
        (db, server, strategy_id)
    }

    fn response_output(response: &axum_test::TestResponse) -> (StatusCode, Option<Value>) {
        let body = if response.as_bytes().is_empty() {
            None
        } else {
            Some(response.json::<Value>())
        };
        (response.status_code(), body)
    }

    fn normalize_target_timestamps(mut value: Value) -> Value {
        if let Some(rows) = value.as_array_mut() {
            for row in rows {
                row["created_at"] = json!("<created_at>");
            }
        }
        value
    }

    #[backend_test_macros::database_test]
    async fn adding_targets_is_idempotent_and_strategy_scoped(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server, strategy_id) = setup(db).await;
        let other_strategy_id = create_strategy(&server, "other strategy").await;
        let target = json!({ "ref_kind": "stock", "ref_id": "SAMPLE001" });
        let add = server
            .post(&format!("/api/strategies/{strategy_id}/earnings-targets"))
            .json(&target)
            .await;
        let duplicate = server
            .post(&format!("/api/strategies/{strategy_id}/earnings-targets"))
            .json(&target)
            .await;
        let add_to_other_strategy = server
            .post(&format!(
                "/api/strategies/{other_strategy_id}/earnings-targets"
            ))
            .json(&target)
            .await;
        let rows = strategy_earnings_target::Entity::find()
            .order_by_asc(strategy_earnings_target::Column::StrategyId)
            .all(&db)
            .await
            .expect("query stored targets")
            .into_iter()
            .map(|row| (row.strategy_id, row.ref_kind, row.ref_id))
            .collect::<Vec<_>>();

        assert_eq!(
            (
                response_output(&add),
                response_output(&duplicate),
                response_output(&add_to_other_strategy),
                rows,
            ),
            (
                (StatusCode::OK, Some(json!({ "changed": true }))),
                (StatusCode::OK, Some(json!({ "changed": false }))),
                (StatusCode::OK, Some(json!({ "changed": true }))),
                vec![
                    (
                        Uuid::parse_str(&strategy_id).expect("valid strategy ID"),
                        "stock".into(),
                        "SAMPLE001".into(),
                    ),
                    (
                        Uuid::parse_str(&other_strategy_id).expect("valid strategy ID"),
                        "stock".into(),
                        "SAMPLE001".into(),
                    ),
                ],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn listing_targets_returns_sorted_stock_and_group_targets(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (_, server, strategy_id) = setup(db).await;
        for target in [
            json!({ "ref_kind": "stock", "ref_id": "SAMPLE001" }),
            json!({ "ref_kind": "group", "ref_id": "axis-demo/group-demo" }),
        ] {
            server
                .post(&format!("/api/strategies/{strategy_id}/earnings-targets"))
                .json(&target)
                .await;
        }
        let listed = server
            .get(&format!("/api/strategies/{strategy_id}/earnings-targets"))
            .await;
        let actual = normalize_target_timestamps(listed.json::<Value>());

        assert_eq!(
            (listed.status_code(), actual),
            (
                StatusCode::OK,
                json!([
                    {
                        "ref_kind": "group",
                        "ref_id": "axis-demo/group-demo",
                        "created_at": "<created_at>",
                    },
                    {
                        "ref_kind": "stock",
                        "ref_id": "SAMPLE001",
                        "created_at": "<created_at>",
                    },
                ]),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn adding_indicator_and_missing_stock_targets_returns_bad_request(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (_, server, strategy_id) = setup(db).await;
        let invalid_targets = [
            json!({ "ref_kind": "indicator", "ref_id": "sample-indicator" }),
            json!({ "ref_kind": "stock", "ref_id": "MISSING001" }),
        ];
        let mut responses = Vec::new();
        for target in invalid_targets {
            let response = server
                .post(&format!("/api/strategies/{strategy_id}/earnings-targets"))
                .json(&target)
                .await;
            responses.push(response_output(&response));
        }

        assert_eq!(
            responses,
            vec![
                (
                    StatusCode::BAD_REQUEST,
                    Some(json!({ "error": "ref_kind must be stock or group" })),
                ),
                (
                    StatusCode::BAD_REQUEST,
                    Some(json!({ "error": "stock reference not found: MISSING001" })),
                ),
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn removing_group_targets_is_idempotent(db: gateway_postgres::DatabaseHandle) {
        let (_, server, strategy_id) = setup(db).await;
        let target_url = format!(
            "/api/strategies/{strategy_id}/earnings-targets?ref_kind=group&ref_id=axis-demo%2Fgroup-demo"
        );
        server
            .post(&format!("/api/strategies/{strategy_id}/earnings-targets"))
            .json(&json!({ "ref_kind": "group", "ref_id": "axis-demo/group-demo" }))
            .await;
        let removed = server.delete(&target_url).await;
        let repeated_remove = server.delete(&target_url).await;

        assert_eq!(
            (response_output(&removed), response_output(&repeated_remove),),
            (
                (StatusCode::OK, Some(json!({ "changed": true }))),
                (StatusCode::OK, Some(json!({ "changed": false }))),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn deleting_strategy_cascades_to_its_targets(db: gateway_postgres::DatabaseHandle) {
        let (db, server, strategy_id) = setup(db).await;
        server
            .post(&format!("/api/strategies/{strategy_id}/earnings-targets"))
            .json(&json!({ "ref_kind": "stock", "ref_id": "SAMPLE001" }))
            .await;
        let deleted = server
            .delete(&format!("/api/strategies/{strategy_id}"))
            .await;
        let rows = strategy_earnings_target::Entity::find()
            .filter(
                strategy_earnings_target::Column::StrategyId
                    .eq(Uuid::parse_str(&strategy_id).expect("valid strategy ID")),
            )
            .all(&db)
            .await
            .expect("query deleted strategy targets");

        assert_eq!(
            (response_output(&deleted), rows.len()),
            ((StatusCode::NO_CONTENT, None), 0)
        );
    }

    #[backend_test_macros::database_test]
    async fn all_endpoints_return_not_found_for_unknown_strategy(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (_, server, _) = setup(db).await;
        let base_url = format!("/api/strategies/{UNKNOWN_STRATEGY_ID}/earnings-targets");
        let get = server.get(&base_url).await;
        let post = server
            .post(&base_url)
            .json(&json!({ "ref_kind": "stock", "ref_id": "SAMPLE001" }))
            .await;
        let delete = server
            .delete(&format!("{base_url}?ref_kind=stock&ref_id=SAMPLE001"))
            .await;

        assert_eq!(
            [
                response_output(&get),
                response_output(&post),
                response_output(&delete)
            ],
            [
                (
                    StatusCode::NOT_FOUND,
                    Some(json!({ "error": format!("strategy {UNKNOWN_STRATEGY_ID} not found") })),
                ),
                (
                    StatusCode::NOT_FOUND,
                    Some(json!({ "error": format!("strategy {UNKNOWN_STRATEGY_ID} not found") })),
                ),
                (
                    StatusCode::NOT_FOUND,
                    Some(json!({ "error": format!("strategy {UNKNOWN_STRATEGY_ID} not found") })),
                ),
            ],
        );
    }
}
