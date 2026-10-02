use core_application::stock_group::{
    CreateStockGroupCommand, StockGroup, StockGroupUseCaseError, StockGroupUseCases,
    UpdateStockGroupCommand,
};
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{StrategyServer, internal_error, invalid_params};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CreateStockGroupParams {
    pub axis_key: String,
    pub group_key: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
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

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct StockGroupMemberParams {
    pub axis_key: String,
    pub group_key: String,
    pub stock_id: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListStockGroupMembersParams {
    pub axis_key: String,
    pub group_key: String,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct StockGroupDto {
    pub id: Uuid,
    pub axis_key: String,
    pub group_key: String,
    pub name: String,
    pub description: Option<String>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct StockGroupMemberChangeResult {
    pub changed: bool,
}

#[cfg_attr(test, derive(serde::Deserialize))]
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
    use serde_json::json;

    use super::UpdateStockGroupParams;

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
}
