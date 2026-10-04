#[cfg(test)]
mod tests {
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};
    use serde_json::json;
    use uuid::Uuid;

    use gateway_postgres::entities::{change_history, group_axis, instruments, stock};

    use rmcp::ErrorData as McpError;

    use super::super::stock_groups::{
        CreateStockGroupParams, ListStockGroupMembersParams, ListStockGroupMembersResult,
        StockGroupDto, StockGroupMemberChangeResult, StockGroupMemberParams,
        UpdateStockGroupParams,
    };
    use super::super::stock_registration::RegisterStockParams;
    use super::super::tests_common::{
        ChangeHistoryShape, build_server as build_composed_server, change_history_for,
        insert_strategy, ts_sentinel,
    };
    use super::super::{StrategyServer, ToolOutput};

    struct ScopedStrategyServer {
        server: StrategyServer,
        strategy_id: Uuid,
    }

    impl ScopedStrategyServer {
        async fn register_stock(
            &self,
            params: RegisterStockParams,
        ) -> Result<ToolOutput<super::super::stock_registration::RegisterStockResult>, McpError>
        {
            self.server.register_stock(self.strategy_id, params).await
        }

        async fn create_stock_group(
            &self,
            params: CreateStockGroupParams,
        ) -> Result<ToolOutput<StockGroupDto>, McpError> {
            self.server
                .create_stock_group(self.strategy_id, params)
                .await
        }

        async fn update_stock_group(
            &self,
            params: UpdateStockGroupParams,
        ) -> Result<ToolOutput<StockGroupDto>, McpError> {
            self.server
                .update_stock_group(self.strategy_id, params)
                .await
        }

        async fn add_stock_to_group(
            &self,
            params: StockGroupMemberParams,
        ) -> Result<ToolOutput<StockGroupMemberChangeResult>, McpError> {
            self.server
                .add_stock_to_group(self.strategy_id, params)
                .await
        }

        async fn remove_stock_from_group(
            &self,
            params: StockGroupMemberParams,
        ) -> Result<ToolOutput<StockGroupMemberChangeResult>, McpError> {
            self.server
                .remove_stock_from_group(self.strategy_id, params)
                .await
        }

        async fn list_stock_group_members(
            &self,
            params: ListStockGroupMembersParams,
        ) -> Result<ToolOutput<ListStockGroupMembersResult>, McpError> {
            self.server
                .list_stock_group_members(self.strategy_id, params)
                .await
        }
    }

    async fn build_server(db: gateway_postgres::DatabaseHandle) -> ScopedStrategyServer {
        let strategy_id = insert_strategy(&db, "scope").await;
        ScopedStrategyServer {
            server: build_composed_server(db),
            strategy_id,
        }
    }

    async fn insert_axis(db: &impl sea_orm::ConnectionTrait, sync_source: Option<&str>) {
        group_axis::ActiveModel {
            id: Set(Uuid::new_v4()),
            key: Set("sample-axis".into()),
            name: Set("Sample axis".into()),
            description: Set("Sample description".into()),
            sync_source: Set(sync_source.map(str::to_owned)),
        }
        .insert(db)
        .await
        .expect("insert group axis");
    }

    async fn insert_stock(db: &impl sea_orm::ConnectionTrait, stock_id: &str) {
        stock::ActiveModel {
            id: Set(stock_id.into()),
            name: Set("Sample stock".into()),
            market: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
            product_category: Set(None),
        }
        .insert(db)
        .await
        .expect("insert stock");
    }

    async fn create_group(
        server: &ScopedStrategyServer,
    ) -> Result<ToolOutput<StockGroupDto>, McpError> {
        server
            .create_stock_group(CreateStockGroupParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                name: "Sample group".into(),
                description: Some("Sample description".into()),
            })
            .await
    }

    fn normalize_group(group: ToolOutput<StockGroupDto>) -> ToolOutput<StockGroupDto> {
        group.normalize_json(|value| value["id"] = json!(Uuid::nil()))
    }

    async fn clear_change_history(db: &impl sea_orm::ConnectionTrait, target_id: Uuid) {
        change_history::Entity::delete_many()
            .filter(change_history::Column::TargetId.eq(target_id))
            .exec(db)
            .await
            .expect("clear change history");
    }

    fn history_shape(
        target_id: Uuid,
        op: &str,
        diff_json: serde_json::Value,
    ) -> ChangeHistoryShape {
        ChangeHistoryShape {
            id: Uuid::nil(),
            target_kind: "stock_group".into(),
            target_id,
            actor_kind: "llm".into(),
            actor_label: "analyst".into(),
            op: op.into(),
            diff_json,
            summary: None,
            created_at: ts_sentinel(),
        }
    }

    #[backend_test_macros::database_test]
    async fn create_stock_group_persists_and_audits(db: gateway_postgres::DatabaseHandle) {
        insert_axis(&db, None).await;
        let server = build_server(db.clone()).await;
        let created = create_group(&server).await.expect("create stock group");
        let history = change_history_for(&db, created.id).await;

        assert_eq!(
            (normalize_group(created.clone()).as_json().clone(), history),
            (
                json!({
                    "id": Uuid::nil(),
                    "axis_key": "sample-axis",
                    "group_key": "sample-group",
                    "name": "Sample group",
                    "description": "Sample description",
                }),
                vec![history_shape(
                    created.id,
                    "create",
                    json!({
                        "axis_key": "sample-axis",
                        "group_key": "sample-group",
                        "name": "Sample group",
                        "description": "Sample description",
                    }),
                )],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn register_foreign_stock_persists_instruments_audit_and_group_membership(
        db: gateway_postgres::DatabaseHandle,
    ) {
        insert_axis(&db, None).await;
        let server = build_server(db.clone()).await;
        let registered = server
            .register_stock(RegisterStockParams {
                country: "KR".into(),
                code: "QZ9012".into(),
                name: "Sample issuer".into(),
                exchange: "Synthetic exchange".into(),
            })
            .await
            .expect("register foreign stock");
        server
            .register_stock(RegisterStockParams {
                country: "US".into(),
                code: "QZ-7".into(),
                name: "Sample US issuer".into(),
                exchange: "Synthetic US exchange".into(),
            })
            .await
            .expect("register US stock");
        let stock_row = stock::Entity::find_by_id("KR:QZ9012")
            .one(&db)
            .await
            .expect("query stock")
            .expect("registered stock exists");
        let instrument_row = instruments::Entity::find_by_id("KR:QZ9012")
            .one(&db)
            .await
            .expect("query instrument")
            .expect("registered instrument exists");
        let us_instrument_row = instruments::Entity::find_by_id("US:QZ-7")
            .one(&db)
            .await
            .expect("query US instrument")
            .expect("registered US instrument exists");
        let target_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"stock:KR:QZ9012");
        let history = change_history_for(&db, target_id).await;
        create_group(&server).await.expect("create stock group");
        let membership = server
            .add_stock_to_group(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "KR:QZ9012".into(),
            })
            .await
            .expect("add foreign stock to group");
        let members = server
            .list_stock_group_members(ListStockGroupMembersParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
            })
            .await
            .expect("list group members");

        assert_eq!(
            (
                registered.into_value(),
                (
                    stock_row.id,
                    stock_row.name,
                    stock_row.market,
                    stock_row.product_category
                ),
                (
                    instrument_row.id,
                    instrument_row.name,
                    instrument_row.market,
                    instrument_row.sector
                ),
                (
                    us_instrument_row.id,
                    us_instrument_row.name,
                    us_instrument_row.market,
                    us_instrument_row.sector
                ),
                history,
                membership.into_value(),
                members.into_value(),
            ),
            (
                super::super::stock_registration::RegisterStockResult {
                    id: "KR:QZ9012".into(),
                    name: "Sample issuer".into(),
                    exchange: "Synthetic exchange".into(),
                },
                (
                    "KR:QZ9012".into(),
                    "Sample issuer".into(),
                    Some("Synthetic exchange".into()),
                    None,
                ),
                (
                    "KR:QZ9012".into(),
                    "Sample issuer".into(),
                    "OTHER".into(),
                    None,
                ),
                (
                    "US:QZ-7".into(),
                    "Sample US issuer".into(),
                    "US".into(),
                    None,
                ),
                vec![ChangeHistoryShape {
                    id: Uuid::nil(),
                    target_kind: "stock".into(),
                    target_id,
                    actor_kind: "llm".into(),
                    actor_label: "analyst".into(),
                    op: "create".into(),
                    diff_json: json!({
                        "stock_id": "KR:QZ9012",
                        "name": "Sample issuer",
                        "exchange": "Synthetic exchange",
                    }),
                    summary: None,
                    created_at: ts_sentinel(),
                }],
                StockGroupMemberChangeResult { changed: true },
                ListStockGroupMembersResult {
                    axis_key: "sample-axis".into(),
                    group_key: "sample-group".into(),
                    stock_ids: vec!["KR:QZ9012".into()],
                },
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn register_stock_rejects_japan_and_malformed_codes(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = build_server(db).await;
        let invalid_params = [
            RegisterStockParams {
                country: "JP".into(),
                code: "QZ9012".into(),
                name: "Sample issuer".into(),
                exchange: "Synthetic exchange".into(),
            },
            RegisterStockParams {
                country: "US".into(),
                code: "qz9012".into(),
                name: "Sample issuer".into(),
                exchange: "Synthetic exchange".into(),
            },
        ];
        let mut errors = Vec::new();
        for params in invalid_params {
            errors.push(server.register_stock(params).await.expect_err("invalid ID"));
        }

        assert_eq!(
            errors,
            vec![
                McpError::invalid_params("Japanese stocks must not use a country prefix", None,),
                McpError::invalid_params(
                    "code must contain only uppercase ASCII letters, digits, or hyphens",
                    None,
                ),
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn update_stock_group_records_changed_fields(db: gateway_postgres::DatabaseHandle) {
        insert_axis(&db, None).await;
        let server = build_server(db.clone()).await;
        let created = create_group(&server).await.expect("create stock group");
        clear_change_history(&db, created.id).await;

        let updated = server
            .update_stock_group(UpdateStockGroupParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                name: Some("Renamed group".into()),
                description: Some(None),
            })
            .await
            .expect("update stock group");
        let history = change_history_for(&db, created.id).await;

        assert_eq!(
            (normalize_group(updated).as_json().clone(), history),
            (
                json!({
                    "id": Uuid::nil(),
                    "axis_key": "sample-axis",
                    "group_key": "sample-group",
                    "name": "Renamed group",
                    "description": null,
                }),
                vec![history_shape(
                    created.id,
                    "update",
                    json!({
                        "description": { "from": "Sample description", "to": null },
                        "name": { "from": "Sample group", "to": "Renamed group" },
                    }),
                )],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_stock_group_preserves_omitted_description(
        db: gateway_postgres::DatabaseHandle,
    ) {
        insert_axis(&db, None).await;
        let server = build_server(db.clone()).await;
        let created = create_group(&server).await.expect("create stock group");
        clear_change_history(&db, created.id).await;

        let updated = server
            .update_stock_group(UpdateStockGroupParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                name: None,
                description: None,
            })
            .await
            .expect("update stock group");

        assert_eq!(
            (
                normalize_group(updated).as_json().clone(),
                change_history_for(&db, created.id).await,
            ),
            (
                json!({
                    "id": Uuid::nil(),
                    "axis_key": "sample-axis",
                    "group_key": "sample-group",
                    "name": "Sample group",
                    "description": "Sample description",
                }),
                vec![],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn add_stock_to_group_is_idempotent_and_audited(db: gateway_postgres::DatabaseHandle) {
        insert_axis(&db, None).await;
        insert_stock(&db, "0002").await;
        let server = build_server(db.clone()).await;
        let created = create_group(&server).await.expect("create stock group");
        clear_change_history(&db, created.id).await;

        let added_first = server
            .add_stock_to_group(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0002".into(),
            })
            .await
            .expect("add stock");
        let added_second = server
            .add_stock_to_group(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0002".into(),
            })
            .await
            .expect("repeat stock add");
        let history = change_history_for(&db, created.id).await;

        assert_eq!(
            (
                added_first.as_json().clone(),
                added_second.as_json().clone(),
                history,
            ),
            (
                json!({"changed": true}),
                json!({"changed": false}),
                vec![history_shape(
                    created.id,
                    "update",
                    json!({
                        "stock_id": "0002",
                        "membership": { "from": false, "to": true },
                    }),
                )],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_stock_group_members_returns_sorted_ids(db: gateway_postgres::DatabaseHandle) {
        insert_axis(&db, None).await;
        insert_stock(&db, "0001").await;
        insert_stock(&db, "0002").await;
        let server = build_server(db.clone()).await;
        create_group(&server).await.expect("create stock group");
        for stock_id in ["0002", "0001"] {
            server
                .add_stock_to_group(StockGroupMemberParams {
                    axis_key: "sample-axis".into(),
                    group_key: "sample-group".into(),
                    stock_id: stock_id.into(),
                })
                .await
                .expect("add stock");
        }

        let members = server
            .list_stock_group_members(ListStockGroupMembersParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
            })
            .await
            .expect("list group members");

        assert_eq!(
            members,
            ListStockGroupMembersResult {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_ids: vec!["0001".into(), "0002".into()],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn remove_stock_from_group_is_idempotent_and_audited(
        db: gateway_postgres::DatabaseHandle,
    ) {
        insert_axis(&db, None).await;
        insert_stock(&db, "0002").await;
        let server = build_server(db.clone()).await;
        let created = create_group(&server).await.expect("create stock group");
        server
            .add_stock_to_group(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0002".into(),
            })
            .await
            .expect("add stock");
        clear_change_history(&db, created.id).await;

        let removed_first = server
            .remove_stock_from_group(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0002".into(),
            })
            .await
            .expect("remove stock");
        let removed_second = server
            .remove_stock_from_group(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0002".into(),
            })
            .await
            .expect("repeat stock removal");
        let members = server
            .list_stock_group_members(ListStockGroupMembersParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
            })
            .await
            .expect("list group members");
        let history = change_history_for(&db, created.id).await;

        assert_eq!(
            (
                removed_first.as_json().clone(),
                removed_second.as_json().clone(),
                members.stock_ids.clone(),
                history
            ),
            (
                json!({"changed": true}),
                json!({"changed": false}),
                Vec::new(),
                vec![history_shape(
                    created.id,
                    "update",
                    json!({
                        "stock_id": "0002",
                        "membership": { "from": true, "to": false },
                    }),
                )],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn add_stock_to_group_rejects_unknown_stock(db: gateway_postgres::DatabaseHandle) {
        insert_axis(&db, None).await;
        let server = build_server(db.clone()).await;
        let created = create_group(&server).await.expect("create stock group");
        clear_change_history(&db, created.id).await;

        let error = server
            .add_stock_to_group(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0009".into(),
            })
            .await
            .expect_err("unknown stock is rejected");
        let history = change_history_for(&db, created.id).await;

        assert_eq!(
            (error, history),
            (
                McpError::invalid_params("stock 0009 not found", None),
                Vec::new(),
            ),
        );
    }
    #[backend_test_macros::database_test]
    async fn synchronized_axes_reject_all_mutations(db: gateway_postgres::DatabaseHandle) {
        insert_axis(&db, Some("sample-sync")).await;
        let server = build_server(db.clone()).await;
        let create = server
            .create_stock_group(CreateStockGroupParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                name: "Sample group".into(),
                description: None,
            })
            .await;
        let update = server
            .update_stock_group(UpdateStockGroupParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                name: Some("Renamed group".into()),
                description: None,
            })
            .await;
        let add = server
            .add_stock_to_group(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0001".into(),
            })
            .await;
        let remove = server
            .remove_stock_from_group(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0001".into(),
            })
            .await;
        let errors = vec![create.err(), update.err(), add.err(), remove.err()];
        let history = change_history::Entity::find()
            .all(&db)
            .await
            .expect("find change history")
            .into_iter()
            .map(|row| (row.target_kind, row.op))
            .collect::<Vec<_>>();

        assert_eq!(
            (errors, history),
            (
                vec![
                    Some(McpError::invalid_params(
                        "stock groups on synchronized axes cannot be changed by MCP",
                        None,
                    ));
                    4
                ],
                Vec::new()
            ),
        );
    }
}
