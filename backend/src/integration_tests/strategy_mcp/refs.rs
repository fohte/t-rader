#[cfg(test)]
mod tests {
    use serde_json::json;

    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};

    use super::super::tests_common::{build_server, insert_strategy};
    use crate::integration_tests::strategy_mcp::test_api::refs::{
        RefDto, SearchRefsParams, SearchRefsResult,
    };
    use gateway_postgres::entities::{indicator, ref_term, stock};

    async fn seed_ref_term(
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

    async fn seed_stock(db: &impl sea_orm::ConnectionTrait, id: &str, name: &str) {
        seed_stock_with_product_category(db, id, name, None).await;
    }

    async fn seed_stock_with_product_category(
        db: &impl sea_orm::ConnectionTrait,
        id: &str,
        name: &str,
        product_category: Option<&str>,
    ) {
        stock::ActiveModel {
            id: Set(id.into()),
            name: Set(name.into()),
            market: Set(None),
            product_category: Set(product_category.map(str::to_string)),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("seed stock");
    }

    async fn seed_indicator(db: &impl sea_orm::ConnectionTrait, id: &str, name: &str) {
        indicator::ActiveModel {
            id: Set(id.into()),
            name: Set(name.into()),
            kind: Set("fx".into()),
        }
        .insert(db)
        .await
        .expect("seed indicator");
    }

    #[backend_test_macros::database_test]
    async fn search_refs_matches_across_all_kinds_ordered_by_name(
        db: gateway_postgres::DatabaseHandle,
    ) {
        seed_indicator(&db, "IND1", "Alpha Indicator").await;
        let group_id =
            crate::testing::insert_test_group(&db, "demo-axis", "demo-group", "Alpha Group").await;
        seed_stock(&db, "STK1", "Alpha Stock").await;
        seed_stock(&db, "STK2", "Beta Stock").await;
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let result = server
            .search_refs(
                strategy_id,
                SearchRefsParams {
                    query: "Alpha".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![
                    RefDto {
                        ref_kind: "group".into(),
                        ref_id: group_id,
                        name: "Alpha Group".into(),
                        product_category: None,
                    },
                    RefDto {
                        ref_kind: "indicator".into(),
                        ref_id: "IND1".into(),
                        name: "Alpha Indicator".into(),
                        product_category: None,
                    },
                    RefDto {
                        ref_kind: "stock".into(),
                        ref_id: "STK1".into(),
                        name: "Alpha Stock".into(),
                        product_category: None,
                    },
                ],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_matches_by_id_substring(db: gateway_postgres::DatabaseHandle) {
        seed_stock(&db, "DEMO-STOCK", "Something").await;
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let result = server
            .search_refs(
                strategy_id,
                SearchRefsParams {
                    query: "DEMO".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![RefDto {
                    ref_kind: "stock".into(),
                    ref_id: "DEMO-STOCK".into(),
                    name: "Something".into(),
                    product_category: None,
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_is_case_insensitive(db: gateway_postgres::DatabaseHandle) {
        let group_id =
            crate::testing::insert_test_group(&db, "demo-axis", "sample-group", "Sample Group")
                .await;
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let result = server
            .search_refs(
                strategy_id,
                SearchRefsParams {
                    query: "sample".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![RefDto {
                    ref_kind: "group".into(),
                    ref_id: group_id,
                    name: "Sample Group".into(),
                    product_category: None,
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_does_not_treat_underscore_as_single_char_wildcard(
        db: gateway_postgres::DatabaseHandle,
    ) {
        // "_" は ILIKE の単一文字ワイルドカードなので、素通しすると "AXB" が
        // "A_B" にマッチしてしまう。sanitize_like で除去され、マッチしないことを確認する。
        crate::testing::insert_test_group(&db, "demo-axis", "sample-group", "AXB").await;
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let result = server
            .search_refs(
                strategy_id,
                SearchRefsParams {
                    query: "A_B".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(result, SearchRefsResult { refs: vec![] });
    }

    #[backend_test_macros::database_test]
    async fn search_refs_respects_limit_after_ordering(db: gateway_postgres::DatabaseHandle) {
        crate::testing::insert_test_group(&db, "demo-axis-a", "sample-group", "Match A").await;
        crate::testing::insert_test_group(&db, "demo-axis-b", "sample-group", "Match B").await;
        crate::testing::insert_test_group(&db, "demo-axis-c", "sample-group", "Match C").await;
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let result = server
            .search_refs(
                strategy_id,
                SearchRefsParams {
                    query: "Match".into(),
                    limit: Some(2),
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![
                    RefDto {
                        ref_kind: "group".into(),
                        ref_id: "demo-axis-a/sample-group".into(),
                        name: "Match A".into(),
                        product_category: None,
                    },
                    RefDto {
                        ref_kind: "group".into(),
                        ref_id: "demo-axis-b/sample-group".into(),
                        name: "Match B".into(),
                        product_category: None,
                    },
                ],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_includes_stock_product_category(db: gateway_postgres::DatabaseHandle) {
        seed_stock_with_product_category(&db, "ETF1", "Alpha ETF", Some("014")).await;
        seed_stock(&db, "STK1", "Alpha Stock").await;
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let result = server
            .search_refs(
                strategy_id,
                SearchRefsParams {
                    query: "Alpha".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![
                    RefDto {
                        ref_kind: "stock".into(),
                        ref_id: "ETF1".into(),
                        name: "Alpha ETF".into(),
                        product_category: Some("014".into()),
                    },
                    RefDto {
                        ref_kind: "stock".into(),
                        ref_id: "STK1".into(),
                        name: "Alpha Stock".into(),
                        product_category: None,
                    },
                ],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_rejects_empty_query(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let err = server
            .search_refs(
                strategy_id,
                SearchRefsParams {
                    query: "   ".into(),
                    limit: None,
                },
            )
            .await
            .expect_err("empty query");
        assert_eq!(
            error_shape(err),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "query must not be empty".to_string(),
                None,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_matches_full_width_query_against_half_width_name(
        db: gateway_postgres::DatabaseHandle,
    ) {
        seed_stock(&db, "DEMO-STOCK", "Sample Motors").await;
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let result = server
            .search_refs(
                strategy_id,
                SearchRefsParams {
                    query: "Ｓａｍｐｌｅ".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![RefDto {
                    ref_kind: "stock".into(),
                    ref_id: "DEMO-STOCK".into(),
                    name: "Sample Motors".into(),
                    product_category: None,
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_matches_full_width_query_against_half_width_id(
        db: gateway_postgres::DatabaseHandle,
    ) {
        seed_indicator(&db, "DEMO-INDEX", "Sample Indicator").await;
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let result = server
            .search_refs(
                strategy_id,
                SearchRefsParams {
                    query: "ＤＥＭＯ－ＩＮＤＥＸ".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![RefDto {
                    ref_kind: "indicator".into(),
                    ref_id: "DEMO-INDEX".into(),
                    name: "Sample Indicator".into(),
                    product_category: None,
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_does_not_treat_full_width_underscore_as_wildcard(
        db: gateway_postgres::DatabaseHandle,
    ) {
        crate::testing::insert_test_group(&db, "demo-axis", "sample-group", "AXB").await;
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let result = server
            .search_refs(
                strategy_id,
                SearchRefsParams {
                    query: "Ａ＿Ｂ".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(result, SearchRefsResult { refs: vec![] });
    }

    #[backend_test_macros::database_test]
    async fn search_refs_matches_ref_term_alias(db: gateway_postgres::DatabaseHandle) {
        seed_stock(&db, "DEMO-STOCK", "Sample Motors").await;
        seed_ref_term(
            &db,
            "stock",
            "DEMO-STOCK",
            "Ｓａｍｐｌｅ Ｍｏｔｏｒｓ Ｇｒｏｕｐ",
        )
        .await;
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let result = server
            .search_refs(
                strategy_id,
                SearchRefsParams {
                    query: "motors group".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![RefDto {
                    ref_kind: "stock".into(),
                    ref_id: "DEMO-STOCK".into(),
                    name: "Sample Motors".into(),
                    product_category: None,
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_matches_group_id_and_alias(db: gateway_postgres::DatabaseHandle) {
        let group_id =
            crate::testing::insert_test_group(&db, "demo-axis", "demo-group", "Sample Group").await;
        seed_ref_term(&db, "group", &group_id, "demo-alias").await;
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let by_id = server
            .search_refs(
                strategy_id,
                SearchRefsParams {
                    query: "demo-axis/demo-group".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs by group id");
        let by_alias = server
            .search_refs(
                strategy_id,
                SearchRefsParams {
                    query: "demo-alias".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs by group alias");

        assert_eq!(
            (by_id.as_json().clone(), by_alias.as_json().clone()),
            (
                json!({
                    "refs": [{
                        "ref_kind": "group",
                        "ref_id": group_id.to_string(),
                        "name": "Sample Group",
                        "product_category": null,
                    }],
                }),
                json!({
                    "refs": [{
                        "ref_kind": "group",
                        "ref_id": group_id.to_string(),
                        "name": "Sample Group",
                        "product_category": null,
                    }],
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_returns_one_row_when_both_name_and_alias_match(
        db: gateway_postgres::DatabaseHandle,
    ) {
        seed_stock(&db, "DEMO-STOCK", "Sample Motors").await;
        seed_ref_term(&db, "stock", "DEMO-STOCK", "Sample Auto").await;
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let result = server
            .search_refs(
                strategy_id,
                SearchRefsParams {
                    query: "Sample".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![RefDto {
                    ref_kind: "stock".into(),
                    ref_id: "DEMO-STOCK".into(),
                    name: "Sample Motors".into(),
                    product_category: None,
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_ignores_dangling_alias_not_in_master(
        db: gateway_postgres::DatabaseHandle,
    ) {
        seed_ref_term(&db, "stock", "DEMO-MISSING", "Ghost Co").await;
        let strategy_id = insert_strategy(&db, "test").await;
        let server = build_server(db);

        let result = server
            .search_refs(
                strategy_id,
                SearchRefsParams {
                    query: "Ghost".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(result, SearchRefsResult { refs: vec![] });
    }

    fn error_shape(
        error: rmcp::ErrorData,
    ) -> (rmcp::model::ErrorCode, String, Option<serde_json::Value>) {
        (error.code, error.message.to_string(), error.data)
    }
}
