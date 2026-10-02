use std::sync::Arc;

use core_application::agent_task_client::SharedAgentTaskClient;
use gateway_postgres::DatabaseHandle;
use rmcp::ErrorData as McpError;
use rmcp::ServerHandler;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{CallToolRequestParams, CallToolResponse, NumberOrString};
use rmcp::service::{RequestContext, RoleServer, serve_directly};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use uuid::Uuid;

use crate::agent_client::FakeAgentTaskClient;
use crate::services::use_cases::build_use_cases;
use entrypoint_control_plane_mcp::{MgmtDependencies, MgmtServer};
use gateway_postgres::entities::strategy;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;

use super::dto::{
    GetStrategyConfigResult, GetStrategyTaskStatusResult, ListRecentAnnotationsResult,
    ListRecentNotesResult, ListRssFeedsParams, ListRssFeedsResult, ListStrategiesResult,
    ResumeStrategyTaskParams, ResumeStrategyTaskResult, SubmitStrategyTaskParams,
    SubmitStrategyTaskResult,
};

#[derive(Clone)]
pub(crate) struct MgmtTestServer {
    pub dependencies: MgmtDependencies,
    server: MgmtServer,
}

pub(crate) fn build_server(
    db: impl Into<DatabaseHandle>,
    fake: Arc<FakeAgentTaskClient>,
) -> MgmtTestServer {
    let use_cases = build_use_cases(db);
    let dependencies = crate::mcp::mgmt_dependencies(&use_cases, fake as SharedAgentTaskClient);
    let server = MgmtServer::new(dependencies.clone());
    MgmtTestServer {
        dependencies,
        server,
    }
}

pub(crate) async fn insert_strategy(db: &impl sea_orm::ConnectionTrait, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    strategy::ActiveModel {
        id: Set(id),
        name: Set(name.to_string()),
        description: Set(None),
        sort_order: Set(0),
        created_at: sea_orm::ActiveValue::NotSet,
        updated_at: sea_orm::ActiveValue::NotSet,
    }
    .insert(db)
    .await
    .expect("insert test strategy");
    id
}

pub(crate) async fn call_tool(
    server: &MgmtServer,
    name: &'static str,
    arguments: Value,
) -> Result<CallToolResponse, McpError> {
    let arguments = arguments
        .as_object()
        .cloned()
        .ok_or_else(|| McpError::invalid_params("tool arguments must be a JSON object", None))?;
    let (server_io, _client_io) = tokio::io::duplex(64);
    let (reader, writer) = tokio::io::split(server_io);
    let running = serve_directly::<RoleServer, _, _, std::io::Error, _>(
        server.clone(),
        (reader, writer),
        None,
    );
    let context = RequestContext::new(NumberOrString::Number(1), running.peer().clone());
    let response = server
        .call_tool(
            CallToolRequestParams::new(name).with_arguments(arguments),
            context,
        )
        .await;
    let _ = running.cancel().await;
    response
}

pub(crate) async fn call_tool_output<T: DeserializeOwned>(
    server: &MgmtServer,
    name: &'static str,
    arguments: Value,
) -> Result<T, McpError> {
    let response = call_tool(server, name, arguments).await?;
    let CallToolResponse::Complete(response) = response else {
        return Err(McpError::internal_error(
            "test tool call did not return a complete result",
            None,
        ));
    };
    if response.is_error == Some(true) {
        return Err(McpError::internal_error(
            "test tool call returned an error result",
            None,
        ));
    }
    let structured_content = response.structured_content.ok_or_else(|| {
        McpError::internal_error("tool result has no structured JSON output", None)
    })?;
    serde_json::from_value(structured_content)
        .map_err(|error| McpError::internal_error(error.to_string(), None))
}

async fn invoke<TInput: Serialize, TOutput: DeserializeOwned>(
    server: &MgmtServer,
    name: &'static str,
    input: TInput,
) -> Result<Json<TOutput>, McpError> {
    let arguments = serde_json::to_value(input)
        .map_err(|error| McpError::internal_error(error.to_string(), None))?;
    call_tool_output(server, name, arguments).await.map(Json)
}

impl MgmtTestServer {
    pub(crate) async fn list_strategies(&self) -> Result<Json<ListStrategiesResult>, McpError> {
        invoke(&self.server, "list_strategies", serde_json::json!({})).await
    }

    pub(crate) async fn submit_strategy_task(
        &self,
        Parameters(params): Parameters<SubmitStrategyTaskParams>,
    ) -> Result<Json<SubmitStrategyTaskResult>, McpError> {
        invoke(&self.server, "submit_strategy_task", params).await
    }

    pub(crate) async fn resume_strategy_task(
        &self,
        Parameters(params): Parameters<ResumeStrategyTaskParams>,
    ) -> Result<Json<ResumeStrategyTaskResult>, McpError> {
        invoke(&self.server, "resume_strategy_task", params).await
    }

    pub(crate) async fn get_strategy_task_status(
        &self,
        Parameters(params): Parameters<super::dto::GetStrategyTaskStatusParams>,
    ) -> Result<Json<GetStrategyTaskStatusResult>, McpError> {
        invoke(&self.server, "get_strategy_task_status", params).await
    }

    pub(crate) async fn get_strategy_config(
        &self,
        Parameters(params): Parameters<super::dto::GetStrategyConfigParams>,
    ) -> Result<Json<GetStrategyConfigResult>, McpError> {
        invoke(&self.server, "get_strategy_config", params).await
    }

    pub(crate) async fn list_recent_notes(
        &self,
        Parameters(params): Parameters<super::dto::ListRecentParams>,
    ) -> Result<Json<ListRecentNotesResult>, McpError> {
        invoke(&self.server, "list_recent_notes", params).await
    }

    pub(crate) async fn list_recent_annotations(
        &self,
        Parameters(params): Parameters<super::dto::ListRecentParams>,
    ) -> Result<Json<ListRecentAnnotationsResult>, McpError> {
        invoke(&self.server, "list_recent_annotations", params).await
    }

    pub(crate) async fn list_rss_feeds(
        &self,
        Parameters(params): Parameters<ListRssFeedsParams>,
    ) -> Result<Json<ListRssFeedsResult>, McpError> {
        invoke(&self.server, "list_rss_feeds", params).await
    }
}
