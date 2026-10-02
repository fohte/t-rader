#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use rstest::{fixture, rstest};
    use sea_orm::ActiveValue::Set;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use serde_json::{Value, json};
    use uuid::Uuid;

    use gateway_postgres::entities::{group_axis, stock_group};

    use crate::testing::create_test_server;

    #[fixture]
    async fn group_axis_test_context() -> (axum_test::TestServer, gateway_postgres::DatabaseHandle)
    {
        let db = gateway_postgres::test_support::create_test_transaction().await;
        (create_test_server(db.clone()).await, db)
    }

    #[rstest]
    #[tokio::test]
    async fn create_returns_full_row(
        #[future] group_axis_test_context: (
            axum_test::TestServer,
            gateway_postgres::DatabaseHandle,
        ),
    ) {
        let (server, _) = group_axis_test_context.await;
        let created = server
            .post("/api/group-axes")
            .json(&json!({
                "key": "sample-axis",
                "name": "Sample Axis",
                "description": "A synthetic classification axis",
                "sync_source": "sample-source",
            }))
            .await;
        assert_eq!(
            (created.status_code(), created.json::<Value>()),
            (
                StatusCode::CREATED,
                json!({
                    "key": "sample-axis",
                    "name": "Sample Axis",
                    "description": "A synthetic classification axis",
                    "sync_source": "sample-source",
                }),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn get_returns_full_row(
        #[future] group_axis_test_context: (
            axum_test::TestServer,
            gateway_postgres::DatabaseHandle,
        ),
    ) {
        let (server, _) = group_axis_test_context.await;
        create_group_axis(&server).await;

        let fetched = server.get("/api/group-axes/sample-axis").await;
        assert_eq!(
            (fetched.status_code(), fetched.json::<Value>()),
            (
                StatusCode::OK,
                json!({
                    "key": "sample-axis",
                    "name": "Sample Axis",
                    "description": "A synthetic classification axis",
                    "sync_source": "sample-source",
                }),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn list_includes_created_axis(
        #[future] group_axis_test_context: (
            axum_test::TestServer,
            gateway_postgres::DatabaseHandle,
        ),
    ) {
        let (server, _) = group_axis_test_context.await;
        create_group_axis(&server).await;

        let listed = server.get("/api/group-axes").await;
        assert_eq!(
            (listed.status_code(), listed.json::<Value>()),
            (
                StatusCode::OK,
                json!([{
                    "key": "sample-axis",
                    "name": "Sample Axis",
                    "description": "A synthetic classification axis",
                    "sync_source": "sample-source",
                }]),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn update_changes_fields_and_clears_sync_source(
        #[future] group_axis_test_context: (
            axum_test::TestServer,
            gateway_postgres::DatabaseHandle,
        ),
    ) {
        let (server, _) = group_axis_test_context.await;
        create_group_axis(&server).await;

        let updated = server
            .patch("/api/group-axes/sample-axis")
            .json(&json!({ "name": "Updated Axis", "sync_source": null }))
            .await;
        assert_eq!(
            (updated.status_code(), updated.json::<Value>()),
            (
                StatusCode::OK,
                json!({
                    "key": "sample-axis",
                    "name": "Updated Axis",
                    "description": "A synthetic classification axis",
                    "sync_source": null,
                }),
            ),
        );
    }

    #[rstest]
    #[case::empty_key(
        "",
        "Sample Axis",
        "key must not be empty, contain '/', or have surrounding whitespace"
    )]
    #[case::key_with_path_separator(
        "sample/axis",
        "Sample Axis",
        "key must not be empty, contain '/', or have surrounding whitespace"
    )]
    #[case::key_with_surrounding_whitespace(
        " sample-axis ",
        "Sample Axis",
        "key must not be empty, contain '/', or have surrounding whitespace"
    )]
    #[case::empty_name("sample-axis", "   ", "name must not be empty")]
    #[tokio::test]
    async fn create_rejects_invalid_axis_definition(
        #[future] group_axis_test_context: (
            axum_test::TestServer,
            gateway_postgres::DatabaseHandle,
        ),
        #[case] key: &str,
        #[case] name: &str,
        #[case] expected_error: &str,
    ) {
        let (server, _) = group_axis_test_context.await;
        let response = server
            .post("/api/group-axes")
            .json(&json!({
                "key": key,
                "name": name,
                "description": "A synthetic classification axis",
            }))
            .await;

        assert_eq!(
            (response.status_code(), response.json::<Value>()),
            (StatusCode::BAD_REQUEST, json!({ "error": expected_error }),),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn deleting_group_axis_with_groups_returns_conflict(
        #[future] group_axis_test_context: (
            axum_test::TestServer,
            gateway_postgres::DatabaseHandle,
        ),
    ) {
        let (server, db) = group_axis_test_context.await;
        create_group_axis(&server).await;

        let axis = group_axis::Entity::find()
            .filter(group_axis::Column::Key.eq("sample-axis"))
            .one(&db)
            .await
            .expect("find group axis")
            .expect("group axis exists");
        stock_group::Entity::insert(stock_group::ActiveModel {
            id: Set(Uuid::new_v4()),
            axis_id: Set(axis.id),
            key: Set("sample-group".into()),
            name: Set("Sample Group".into()),
            description: Set(None),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert group");

        let deleted = server.delete("/api/group-axes/sample-axis").await;
        let deleted_output = (deleted.status_code(), deleted.json::<Value>());
        assert_eq!(
            deleted_output,
            (
                StatusCode::CONFLICT,
                json!({
                    "error": "group axis sample-axis cannot be deleted while it contains groups",
                }),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn deleting_group_axis_without_groups_succeeds(
        #[future] group_axis_test_context: (
            axum_test::TestServer,
            gateway_postgres::DatabaseHandle,
        ),
    ) {
        let (server, _) = group_axis_test_context.await;
        create_group_axis(&server).await;

        let deleted = server.delete("/api/group-axes/sample-axis").await;
        let deleted_output = (deleted.status_code(), deleted.text());
        let fetched = server.get("/api/group-axes/sample-axis").await;
        let fetched_output = (fetched.status_code(), fetched.json::<Value>());

        assert_eq!(
            (deleted_output, fetched_output),
            (
                (StatusCode::NO_CONTENT, String::new()),
                (
                    StatusCode::NOT_FOUND,
                    json!({ "error": "group axis sample-axis not found" }),
                ),
            ),
        );
    }

    async fn create_group_axis(server: &axum_test::TestServer) {
        server
            .post("/api/group-axes")
            .json(&json!({
                "key": "sample-axis",
                "name": "Sample Axis",
                "description": "A synthetic classification axis",
                "sync_source": "sample-source",
            }))
            .await
            .assert_status(StatusCode::CREATED);
    }
}
