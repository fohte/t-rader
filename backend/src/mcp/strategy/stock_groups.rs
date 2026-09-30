use core_application::stock_group::{
    CreateStockGroupCommand, StockGroup, StockGroupUseCaseError, StockGroupUseCases,
    UpdateStockGroupCommand,
};
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
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
    #[serde(default, deserialize_with = "deserialize_nullable_string")]
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

    fn stock_group_use_cases(&self) -> StockGroupUseCases {
        self.use_cases.stock_groups()
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

fn stock_group_error(error: StockGroupUseCaseError) -> McpError {
    match error {
        error @ (StockGroupUseCaseError::Validation(_)
        | StockGroupUseCaseError::AxisNotFound(_)
        | StockGroupUseCaseError::GroupNotFound { .. }) => invalid_params(error.to_string()),
        error => {
            tracing::error!(error = %error, "strategy mcp stock group operation failed");
            internal_error("stock group operation failed")
        }
    }
}

fn deserialize_nullable_string<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Some)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{ActiveModelTrait, EntityTrait};
    use serde_json::json;
    use uuid::Uuid;

    use gateway_postgres::entities::{change_history, group_axis, stock};

    use super::super::tests_common::{ChangeHistoryShape, build_server, change_history_for};
    use super::{
        CreateStockGroupParams, ListStockGroupMembersParams, StockGroupMemberParams,
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
            sector_id: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
            product_category: Set(None),
        }
        .insert(db)
        .await
        .expect("insert stock");
    }

    #[backend_test_macros::database_test]
    async fn stock_group_changes_are_persisted_and_audited(db: gateway_postgres::DatabaseHandle) {
        insert_axis(&db, None).await;
        insert_stock(&db, "0001").await;
        insert_stock(&db, "0002").await;
        let server = build_server(db.clone());
        let created = server
            .create_stock_group_inner(CreateStockGroupParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                name: "Sample group".into(),
                description: Some("Sample description".into()),
            })
            .await
            .expect("create stock group");
        let updated = server
            .update_stock_group_inner(UpdateStockGroupParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                name: Some("Renamed group".into()),
                description: Some(None),
            })
            .await
            .expect("update stock group");
        let added_first = server
            .add_stock_to_group_inner(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0002".into(),
            })
            .await
            .expect("add stock")
            .changed;
        let added_second = server
            .add_stock_to_group_inner(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0002".into(),
            })
            .await
            .expect("repeat stock add")
            .changed;
        server
            .add_stock_to_group_inner(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0001".into(),
            })
            .await
            .expect("add second stock");
        let members = server
            .list_stock_group_members_inner(ListStockGroupMembersParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
            })
            .await
            .expect("list group members");
        let removed_first = server
            .remove_stock_from_group_inner(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0002".into(),
            })
            .await
            .expect("remove stock")
            .changed;
        let removed_second = server
            .remove_stock_from_group_inner(StockGroupMemberParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
                stock_id: "0002".into(),
            })
            .await
            .expect("repeat stock removal")
            .changed;
        let remaining = server
            .list_stock_group_members_inner(ListStockGroupMembersParams {
                axis_key: "sample-axis".into(),
                group_key: "sample-group".into(),
            })
            .await
            .expect("list remaining members");
        let history = change_history_for(&db, created.id).await;

        assert_eq!(
            (
                created.clone(),
                updated,
                added_first,
                added_second,
                members.stock_ids,
                removed_first,
                removed_second,
                remaining.stock_ids,
                history,
            ),
            (
                super::StockGroupDto {
                    id: created.id,
                    axis_key: "sample-axis".into(),
                    group_key: "sample-group".into(),
                    name: "Sample group".into(),
                    description: Some("Sample description".into()),
                },
                super::StockGroupDto {
                    id: created.id,
                    axis_key: "sample-axis".into(),
                    group_key: "sample-group".into(),
                    name: "Renamed group".into(),
                    description: None,
                },
                true,
                false,
                vec!["0001".to_string(), "0002".to_string()],
                true,
                false,
                vec!["0001".to_string()],
                vec![
                    ChangeHistoryShape {
                        id: Uuid::nil(),
                        target_kind: "stock_group".into(),
                        target_id: created.id,
                        actor_kind: "llm".into(),
                        actor_label: "analyst".into(),
                        op: "create".into(),
                        diff_json: json!({
                            "axis_key": "sample-axis",
                            "group_key": "sample-group",
                            "name": "Sample group",
                            "description": "Sample description",
                        }),
                        summary: None,
                        created_at: super::super::tests_common::ts_sentinel(),
                    },
                    ChangeHistoryShape {
                        id: Uuid::nil(),
                        target_kind: "stock_group".into(),
                        target_id: created.id,
                        actor_kind: "llm".into(),
                        actor_label: "analyst".into(),
                        op: "update".into(),
                        diff_json: json!({
                            "description": { "from": "Sample description", "to": null },
                            "name": { "from": "Sample group", "to": "Renamed group" },
                        }),
                        summary: None,
                        created_at: super::super::tests_common::ts_sentinel(),
                    },
                    ChangeHistoryShape {
                        id: Uuid::nil(),
                        target_kind: "stock_group".into(),
                        target_id: created.id,
                        actor_kind: "llm".into(),
                        actor_label: "analyst".into(),
                        op: "update".into(),
                        diff_json: json!({
                            "stock_id": "0002",
                            "membership": { "from": false, "to": true },
                        }),
                        summary: None,
                        created_at: super::super::tests_common::ts_sentinel(),
                    },
                    ChangeHistoryShape {
                        id: Uuid::nil(),
                        target_kind: "stock_group".into(),
                        target_id: created.id,
                        actor_kind: "llm".into(),
                        actor_label: "analyst".into(),
                        op: "update".into(),
                        diff_json: json!({
                            "stock_id": "0001",
                            "membership": { "from": false, "to": true },
                        }),
                        summary: None,
                        created_at: super::super::tests_common::ts_sentinel(),
                    },
                    ChangeHistoryShape {
                        id: Uuid::nil(),
                        target_kind: "stock_group".into(),
                        target_id: created.id,
                        actor_kind: "llm".into(),
                        actor_label: "analyst".into(),
                        op: "update".into(),
                        diff_json: json!({
                            "stock_id": "0002",
                            "membership": { "from": true, "to": false },
                        }),
                        summary: None,
                        created_at: super::super::tests_common::ts_sentinel(),
                    },
                ],
            ),
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
