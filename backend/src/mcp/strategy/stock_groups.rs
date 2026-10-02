use core_application::stock_group::{
    CreateStockGroupCommand, StockGroup, StockGroupUseCaseError, StockGroupUseCases,
    UpdateStockGroupCommand,
};
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{StrategyServer, internal_error, invalid_params};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateStockGroupParams {
    pub axis_key: String,
    pub group_key: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateStockGroupParams {
    pub axis_key: String,
    pub group_key: String,
    pub name: Option<String>,
    #[serde(
        default,
        deserialize_with = "super::serde_helpers::deserialize_nullable_option"
    )]
    pub description: Option<Option<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StockGroupMemberParams {
    pub axis_key: String,
    pub group_key: String,
    pub stock_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListStockGroupMembersParams {
    pub axis_key: String,
    pub group_key: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct StockGroupDto {
    pub id: Uuid,
    pub axis_key: String,
    pub group_key: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct StockGroupMemberChangeResult {
    pub changed: bool,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ListStockGroupMembersResult {
    pub axis_key: String,
    pub group_key: String,
    pub stock_ids: Vec<String>,
}

impl StrategyServer {
    pub(crate) async fn create_stock_group_inner(
        &self,
        params: CreateStockGroupParams,
    ) -> Result<StockGroupDto, McpError> {
        let group = self
            .stock_group_use_cases()
            .create(CreateStockGroupCommand {
                axis_key: params.axis_key,
                key: params.group_key,
                name: params.name,
                description: params.description,
            })
            .await
            .map_err(stock_group_error)?;
        Ok(group_to_dto(group))
    }

    pub(crate) async fn update_stock_group_inner(
        &self,
        params: UpdateStockGroupParams,
    ) -> Result<StockGroupDto, McpError> {
        let group = self
            .stock_group_use_cases()
            .update(UpdateStockGroupCommand {
                axis_key: params.axis_key,
                key: params.group_key,
                name: params.name,
                description: params.description,
            })
            .await
            .map_err(stock_group_error)?;
        Ok(group_to_dto(group))
    }

    pub(crate) async fn add_stock_to_group_inner(
        &self,
        params: StockGroupMemberParams,
    ) -> Result<StockGroupMemberChangeResult, McpError> {
        let changed = self
            .stock_group_use_cases()
            .add_stock(&params.axis_key, &params.group_key, &params.stock_id)
            .await
            .map_err(stock_group_error)?;
        Ok(StockGroupMemberChangeResult { changed })
    }

    pub(crate) async fn remove_stock_from_group_inner(
        &self,
        params: StockGroupMemberParams,
    ) -> Result<StockGroupMemberChangeResult, McpError> {
        let changed = self
            .stock_group_use_cases()
            .remove_stock(&params.axis_key, &params.group_key, &params.stock_id)
            .await
            .map_err(stock_group_error)?;
        Ok(StockGroupMemberChangeResult { changed })
    }

    pub(crate) async fn list_stock_group_members_inner(
        &self,
        params: ListStockGroupMembersParams,
    ) -> Result<ListStockGroupMembersResult, McpError> {
        let stock_ids = self
            .stock_group_use_cases()
            .list_stock_ids(&params.axis_key, &params.group_key)
            .await
            .map_err(stock_group_error)?;
        Ok(ListStockGroupMembersResult {
            axis_key: params.axis_key,
            group_key: params.group_key,
            stock_ids,
        })
    }

    fn stock_group_use_cases(&self) -> &StockGroupUseCases {
        &self.dependencies.stock_groups
    }
}

fn group_to_dto(group: StockGroup) -> StockGroupDto {
    StockGroupDto {
        id: group.id,
        axis_key: group.axis_key,
        group_key: group.key,
        name: group.name,
        description: group.description,
    }
}

pub(super) fn stock_group_error(error: StockGroupUseCaseError) -> McpError {
    match error {
        error @ (StockGroupUseCaseError::Validation(_)
        | StockGroupUseCaseError::AxisNotFound(_)
        | StockGroupUseCaseError::GroupNotFound { .. }) => invalid_params(error.to_string()),
        error => {
            tracing::error!(error = %error, "strategy mcp stock group operation failed");
            internal_error(format!("database error: {error}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};
    use serde_json::json;
    use uuid::Uuid;

    use gateway_postgres::entities::{change_history, group_axis, stock};

    use super::super::StrategyServer;
    use super::super::tests_common::{ChangeHistoryShape, build_server, change_history_for};
    use super::{
        CreateStockGroupParams, ListStockGroupMembersParams, ListStockGroupMembersResult,
        StockGroupDto, StockGroupMemberChangeResult, StockGroupMemberParams,
        UpdateStockGroupParams,
    };

    #[rstest]
    #[case::omitted(json!({ "axis_key": "sample-axis", "group_key": "sample-group" }), None)]
    #[case::cleared(json!({ "axis_key": "sample-axis", "group_key": "sample-group", "description": null }), Some(None))]
    #[case::set(json!({ "axis_key": "sample-axis", "group_key": "sample-group", "description": "Sample description" }), Some(Some("Sample description".into())))]
    fn update_params_distinguish_omitted_null_and_value(
        #[case] input: serde_json::Value,
        #[case] expected_description: Option<Option<String>>,
    ) {
        let params = serde_json::from_value::<UpdateStockGroupParams>(input)
            .expect("update params deserialize");

        assert_eq!(
            (
                params.axis_key,
                params.group_key,
                params.name,
                params.description,
            ),
            (
                "sample-axis".to_string(),
                "sample-group".to_string(),
                None,
                expected_description,
            ),
        );
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

    async fn create_group(server: &StrategyServer) -> Result<StockGroupDto, super::McpError> {
        server
            .create_stock_group_inner(CreateStockGroupParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                name: "Sample group".into(),
                description: Some("Sample description".into()),
            })
            .await
    }

    fn normalize_group(mut group: StockGroupDto) -> StockGroupDto {
        group.id = Uuid::nil();
        group
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
            created_at: super::super::tests_common::ts_sentinel(),
        }
    }

    #[backend_test_macros::database_test]
    async fn create_stock_group_persists_and_audits(db: gateway_postgres::DatabaseHandle) {
        insert_axis(&db, None).await;
        let server = build_server(db.clone());
        let created = create_group(&server).await.expect("create stock group");
        let history = change_history_for(&db, created.id).await;

        assert_eq!(
            (normalize_group(created.clone()), history),
            (
                StockGroupDto {
                    id: Uuid::nil(),
                    axis_key: "sample-axis".into(),
                    group_key: "sample-group".into(),
                    name: "Sample group".into(),
                    description: Some("Sample description".into()),
                },
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
    async fn update_stock_group_records_changed_fields(db: gateway_postgres::DatabaseHandle) {
        insert_axis(&db, None).await;
        let server = build_server(db.clone());
        let created = create_group(&server).await.expect("create stock group");
        clear_change_history(&db, created.id).await;

        let updated = server
            .update_stock_group_inner(UpdateStockGroupParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                name: Some("Renamed group".into()),
                description: Some(None),
            })
            .await
            .expect("update stock group");
        let history = change_history_for(&db, created.id).await;

        assert_eq!(
            (normalize_group(updated), history),
            (
                StockGroupDto {
                    id: Uuid::nil(),
                    axis_key: "sample-axis".into(),
                    group_key: "sample-group".into(),
                    name: "Renamed group".into(),
                    description: None,
                },
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
    async fn add_stock_to_group_is_idempotent_and_audited(db: gateway_postgres::DatabaseHandle) {
        insert_axis(&db, None).await;
        insert_stock(&db, "0002").await;
        let server = build_server(db.clone());
        let created = create_group(&server).await.expect("create stock group");
        clear_change_history(&db, created.id).await;

        let added_first = server
            .add_stock_to_group_inner(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0002".into(),
            })
            .await
            .expect("add stock");
        let added_second = server
            .add_stock_to_group_inner(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0002".into(),
            })
            .await
            .expect("repeat stock add");
        let history = change_history_for(&db, created.id).await;

        assert_eq!(
            (added_first, added_second, history),
            (
                StockGroupMemberChangeResult { changed: true },
                StockGroupMemberChangeResult { changed: false },
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
        let server = build_server(db.clone());
        create_group(&server).await.expect("create stock group");
        for stock_id in ["0002", "0001"] {
            server
                .add_stock_to_group_inner(StockGroupMemberParams {
                    axis_key: "sample-axis".into(),
                    group_key: "sample-group".into(),
                    stock_id: stock_id.into(),
                })
                .await
                .expect("add stock");
        }

        let members = server
            .list_stock_group_members_inner(ListStockGroupMembersParams {
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
        let server = build_server(db.clone());
        let created = create_group(&server).await.expect("create stock group");
        server
            .add_stock_to_group_inner(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0002".into(),
            })
            .await
            .expect("add stock");
        clear_change_history(&db, created.id).await;

        let removed_first = server
            .remove_stock_from_group_inner(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0002".into(),
            })
            .await
            .expect("remove stock");
        let removed_second = server
            .remove_stock_from_group_inner(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0002".into(),
            })
            .await
            .expect("repeat stock removal");
        let members = server
            .list_stock_group_members_inner(ListStockGroupMembersParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
            })
            .await
            .expect("list group members");
        let history = change_history_for(&db, created.id).await;

        assert_eq!(
            (removed_first, removed_second, members.stock_ids, history),
            (
                StockGroupMemberChangeResult { changed: true },
                StockGroupMemberChangeResult { changed: false },
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
        let server = build_server(db.clone());
        let created = create_group(&server).await.expect("create stock group");
        clear_change_history(&db, created.id).await;

        let error = server
            .add_stock_to_group_inner(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0009".into(),
            })
            .await
            .expect_err("unknown stock is rejected");
        let history = change_history_for(&db, created.id).await;

        assert_eq!(
            (error.code, history),
            (rmcp::model::ErrorCode::INVALID_PARAMS, Vec::new()),
        );
    }
    #[backend_test_macros::database_test]
    async fn synchronized_axes_reject_all_mutations(db: gateway_postgres::DatabaseHandle) {
        insert_axis(&db, Some("sample-sync")).await;
        let server = build_server(db.clone());
        let create = server
            .create_stock_group_inner(CreateStockGroupParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                name: "Sample group".into(),
                description: None,
            })
            .await;
        let update = server
            .update_stock_group_inner(UpdateStockGroupParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                name: Some("Renamed group".into()),
                description: None,
            })
            .await;
        let add = server
            .add_stock_to_group_inner(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0001".into(),
            })
            .await;
        let remove = server
            .remove_stock_from_group_inner(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0001".into(),
            })
            .await;
        let errors = vec![
            create.err().map(|error| error.code),
            update.err().map(|error| error.code),
            add.err().map(|error| error.code),
            remove.err().map(|error| error.code),
        ];
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
                vec![Some(rmcp::model::ErrorCode::INVALID_PARAMS); 4],
                Vec::new()
            ),
        );
    }
}
