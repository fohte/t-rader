#[cfg(test)]
mod tests {
    use serde_json::json;

    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::EntityTrait;

    use super::super::tests_common::{build_server, insert_strategy};
    use crate::mcp::strategy::test_api::ref_terms::{
        AddRefTermsParams, AddRefTermsResult, RemoveRefTermsParams,
    };
    use gateway_postgres::entities::ref_term;

    async fn seed_term(
        db: &impl sea_orm::ConnectionTrait,
        ref_kind: &str,
        ref_id: &str,
        term: &str,
    ) {
        ref_term::ActiveModel {
            ref_kind: Set(ref_kind.into()),
            ref_id: Set(ref_id.into()),
            term: Set(term.into()),
            origin: Set("human".into()),
            created_at: NotSet,
        }
        .insert(db)
        .await
        .expect("seed ref_term");
    }

    #[backend_test_macros::database_test]
    async fn add_ref_terms_inserts_new_terms(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let result = server
            .add_ref_terms(
                strategy_id,
                AddRefTermsParams {
                    ref_kind: "stock".into(),
                    ref_id: "7203".into(),
                    terms: vec!["トヨタ".into(), "Toyota".into()],
                },
            )
            .await
            .expect("add_ref_terms");

        assert_eq!(
            result,
            AddRefTermsResult {
                added: vec!["トヨタ".to_string(), "Toyota".to_string()],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn add_ref_terms_is_idempotent_and_skips_blank_terms(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db.clone());
        seed_term(&db, "stock", "7203", "トヨタ").await;

        let result = server
            .add_ref_terms(
                strategy_id,
                AddRefTermsParams {
                    ref_kind: "stock".into(),
                    ref_id: "7203".into(),
                    terms: vec!["トヨタ".into(), "  ".into(), "Toyota".into()],
                },
            )
            .await
            .expect("add_ref_terms");

        assert_eq!(
            result,
            AddRefTermsResult {
                added: vec!["Toyota".to_string()],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn add_ref_terms_rejects_invalid_ref_kind(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let err = server
            .add_ref_terms(
                strategy_id,
                AddRefTermsParams {
                    ref_kind: "bogus".into(),
                    ref_id: "7203".into(),
                    terms: vec!["トヨタ".into()],
                },
            )
            .await
            .expect_err("invalid kind");
        assert_eq!(
            error_shape(err),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "invalid ref_kind: bogus".to_string(),
                None,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn add_ref_terms_rejects_empty_ref_id(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let err = server
            .add_ref_terms(
                strategy_id,
                AddRefTermsParams {
                    ref_kind: "stock".into(),
                    ref_id: "  ".into(),
                    terms: vec!["トヨタ".into()],
                },
            )
            .await
            .expect_err("empty ref_id");
        assert_eq!(
            error_shape(err),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "ref_id must not be empty".to_string(),
                None,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn ref_term_operations_reject_invalid_group_ref_ids(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);
        let add_result = server
            .add_ref_terms(
                strategy_id,
                AddRefTermsParams {
                    ref_kind: "group".into(),
                    ref_id: "demo-group".into(),
                    terms: vec!["Sample Group".into()],
                },
            )
            .await
            .map(|_| ())
            .map_err(error_shape);
        let remove_result = server
            .remove_ref_terms(
                strategy_id,
                RemoveRefTermsParams {
                    ref_kind: "group".into(),
                    ref_id: "demo-group".into(),
                    terms: vec!["Sample Group".into()],
                },
            )
            .await
            .map(|_| ())
            .map_err(error_shape);

        assert_eq!(
            [add_result, remove_result],
            [
                Err((
                    rmcp::model::ErrorCode::INVALID_PARAMS,
                    "invalid group ref_id: demo-group".to_string(),
                    None,
                )),
                Err((
                    rmcp::model::ErrorCode::INVALID_PARAMS,
                    "invalid group ref_id: demo-group".to_string(),
                    None,
                )),
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn remove_ref_terms_deletes_only_matching_terms(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db.clone());
        seed_term(&db, "stock", "7203", "トヨタ").await;
        seed_term(&db, "stock", "7203", "Toyota").await;
        seed_term(&db, "stock", "9984", "トヨタ").await;

        let result = server
            .remove_ref_terms(
                strategy_id,
                RemoveRefTermsParams {
                    ref_kind: "stock".into(),
                    ref_id: "7203".into(),
                    terms: vec!["トヨタ".into(), "存在しない".into()],
                },
            )
            .await
            .expect("remove_ref_terms");

        let remaining = ref_term::Entity::find()
            .all(&db)
            .await
            .expect("list remaining terms");
        assert_eq!(
            (result.as_json().clone(), remaining.len()),
            (json!({"removed": ["トヨタ"]}), 2,),
        );
    }

    #[backend_test_macros::database_test]
    async fn remove_ref_terms_rejects_invalid_ref_kind(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let err = server
            .remove_ref_terms(
                strategy_id,
                RemoveRefTermsParams {
                    ref_kind: "bogus".into(),
                    ref_id: "7203".into(),
                    terms: vec!["トヨタ".into()],
                },
            )
            .await
            .expect_err("invalid kind");
        assert_eq!(
            error_shape(err),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "invalid ref_kind: bogus".to_string(),
                None,
            ),
        );
    }

    fn error_shape(
        error: rmcp::ErrorData,
    ) -> (rmcp::model::ErrorCode, String, Option<serde_json::Value>) {
        (error.code, error.message.to_string(), error.data)
    }
}
