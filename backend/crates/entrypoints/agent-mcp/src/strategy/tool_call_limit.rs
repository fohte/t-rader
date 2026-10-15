use axum::http::{HeaderMap, request::Parts};
use core_application::agent_config::AgentConfigUseCaseError;
use core_application::agent_config::graph::parse_agent_graph;
use rmcp::ErrorData as McpError;
use rmcp::service::{RequestContext, RoleServer};

use super::{
    StrategyServer, execution_task_id_from_ctx, internal_error, invalid_params,
    tool_call_count_error_to_mcp,
};

const SUBAGENT_EXECUTION_ID_HEADER: &str = "x-subagent-execution-id";
const SUBAGENT_DEFINITION_KEY_HEADER: &str = "x-subagent-definition-key";

#[derive(Debug, Clone, PartialEq, Eq)]
struct SubagentExecutionIdentity {
    execution_id: String,
    definition_key: String,
}

#[derive(Debug, Clone)]
pub(super) struct ToolCallLimitContext {
    task_execution_id: Option<String>,
    subagent_counter_id: Option<String>,
    task_max_calls: u32,
    subagent_max_calls: Option<u32>,
}

impl ToolCallLimitContext {
    fn untracked(task_max_calls: u32) -> Self {
        Self {
            task_execution_id: None,
            subagent_counter_id: None,
            task_max_calls,
            subagent_max_calls: None,
        }
    }
}

impl StrategyServer {
    pub(super) async fn tool_call_limit_context_from_ctx(
        &self,
        ctx: &RequestContext<RoleServer>,
        tool_name: &str,
        default_task_max_calls: u32,
    ) -> Result<ToolCallLimitContext, McpError> {
        let Some(task_execution_id) = execution_task_id_from_ctx(ctx) else {
            return Ok(ToolCallLimitContext::untracked(default_task_max_calls));
        };
        let subagent = subagent_execution_identity_from_ctx(ctx)?;

        let mut task_max_calls = default_task_max_calls;
        let task = self
            .dependencies
            .strategy_tasks
            .get_by_a2a_task_id(&task_execution_id)
            .await
            .map_err(|error| {
                tracing::error!(error = %error, "strategy mcp task lookup failed");
                internal_error(format!("strategy task lookup failed: {error}"))
            })?;
        if let Some(task) = task {
            let purpose = task
                .purpose
                .filter(|purpose| !purpose.trim().is_empty())
                .unwrap_or_else(|| core_application::strategy_task::DEFAULT_PURPOSE.to_string());
            let agent_config = match self.dependencies.agent_configs.get(&purpose).await {
                Ok(agent_config) => Some(agent_config),
                Err(AgentConfigUseCaseError::NotFound(_)) => None,
                Err(error) => {
                    tracing::error!(purpose, error = %error, "strategy mcp agent config lookup failed");
                    return Err(internal_error(format!(
                        "agent config lookup failed: {error}"
                    )));
                }
            };
            if let Some(agent_config) = agent_config {
                let graph = parse_agent_graph(&agent_config.agent_graph).map_err(|error| {
                    tracing::error!(purpose, error = %error, "strategy mcp agent graph parse failed");
                    internal_error(format!("agent graph is invalid: {error}"))
                })?;
                task_max_calls = graph
                    .and_then(|graph| graph.tool_limits.get(tool_name).copied())
                    .unwrap_or(default_task_max_calls);
            }
        }

        let Some(subagent) = subagent else {
            return Ok(ToolCallLimitContext {
                task_execution_id: Some(task_execution_id),
                subagent_counter_id: None,
                task_max_calls,
                subagent_max_calls: None,
            });
        };

        let definition = self
            .dependencies
            .subagent_definitions
            .get(&subagent.definition_key)
            .await
            .map_err(|error| {
                tracing::error!(definition_key = subagent.definition_key, error = %error, "strategy mcp subagent definition lookup failed");
                internal_error(format!("subagent definition lookup failed: {error}"))
            })?;
        let subagent_max_calls = definition
            .tool_limits
            .get(tool_name)
            .map(|value| u32::try_from(*value).unwrap_or(if *value < 0 { 0 } else { u32::MAX }));

        Ok(ToolCallLimitContext {
            task_execution_id: Some(task_execution_id.clone()),
            subagent_counter_id: Some(format!(
                "subagent:{task_execution_id}:{}",
                subagent.execution_id
            )),
            task_max_calls,
            subagent_max_calls,
        })
    }

    pub(super) async fn reserve_tool_call_limits(
        &self,
        limits: &ToolCallLimitContext,
        tool_name: &'static str,
    ) -> Result<(), McpError> {
        let Some(task_execution_id) = limits.task_execution_id.as_deref() else {
            return Ok(());
        };
        self.dependencies
            .mcp_tool_call_counts
            .reserve(task_execution_id, tool_name, limits.task_max_calls)
            .await
            .map_err(tool_call_count_error_to_mcp)?;

        if let (Some(subagent_counter_id), Some(subagent_max_calls)) = (
            limits.subagent_counter_id.as_deref(),
            limits.subagent_max_calls,
        ) && let Err(error) = self
            .dependencies
            .mcp_tool_call_counts
            .reserve(subagent_counter_id, tool_name, subagent_max_calls)
            .await
        {
            self.release_reservation(task_execution_id, tool_name).await;
            return Err(tool_call_count_error_to_mcp(error));
        }

        Ok(())
    }

    pub(super) async fn release_tool_call_limits(
        &self,
        limits: &ToolCallLimitContext,
        tool_name: &'static str,
    ) {
        if let Some(task_execution_id) = limits.task_execution_id.as_deref() {
            self.release_reservation(task_execution_id, tool_name).await;
        }
        if let (Some(subagent_counter_id), Some(_)) = (
            limits.subagent_counter_id.as_deref(),
            limits.subagent_max_calls,
        ) {
            self.release_reservation(subagent_counter_id, tool_name)
                .await;
        }
    }

    async fn release_reservation(&self, counter_id: &str, tool_name: &'static str) {
        if let Err(error) = self
            .dependencies
            .mcp_tool_call_counts
            .release(counter_id, tool_name)
            .await
        {
            tracing::warn!(
                counter_id,
                tool_name,
                error = %error,
                "failed to release tool call count reservation"
            );
        }
    }
}

fn subagent_execution_identity_from_ctx(
    ctx: &RequestContext<RoleServer>,
) -> Result<Option<SubagentExecutionIdentity>, McpError> {
    let Some(parts) = ctx.extensions.get::<Parts>() else {
        return Ok(None);
    };
    subagent_execution_identity_from_headers(&parts.headers)
}

fn subagent_execution_identity_from_headers(
    headers: &HeaderMap,
) -> Result<Option<SubagentExecutionIdentity>, McpError> {
    let execution_id = optional_header(headers, SUBAGENT_EXECUTION_ID_HEADER)?;
    let definition_key = optional_header(headers, SUBAGENT_DEFINITION_KEY_HEADER)?;
    match (execution_id, definition_key) {
        (None, None) => Ok(None),
        (Some(execution_id), Some(definition_key)) => Ok(Some(SubagentExecutionIdentity {
            execution_id,
            definition_key,
        })),
        _ => Err(invalid_params(format!(
            "{SUBAGENT_EXECUTION_ID_HEADER} and {SUBAGENT_DEFINITION_KEY_HEADER} must be provided together"
        ))),
    }
}

fn optional_header(headers: &HeaderMap, name: &'static str) -> Result<Option<String>, McpError> {
    let Some(header) = headers.get(name) else {
        return Ok(None);
    };
    let value = header
        .to_str()
        .map_err(|_| invalid_params(format!("{name} header is not valid ASCII")))?
        .trim();
    if value.is_empty() {
        return Err(invalid_params(format!("{name} header must not be empty")));
    }
    Ok(Some(value.to_string()))
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};
    use rstest::rstest;

    use super::{
        SUBAGENT_DEFINITION_KEY_HEADER, SUBAGENT_EXECUTION_ID_HEADER, SubagentExecutionIdentity,
        subagent_execution_identity_from_headers,
    };

    fn headers_with_child(execution_id: Option<&[u8]>, definition_key: Option<&[u8]>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in [
            (SUBAGENT_EXECUTION_ID_HEADER, execution_id),
            (SUBAGENT_DEFINITION_KEY_HEADER, definition_key),
        ] {
            if let Some(value) = value {
                headers.insert(name, HeaderValue::from_bytes(value).expect("header value"));
            }
        }
        headers
    }

    #[rstest]
    #[case::missing(None, None, None)]
    #[case::both_present(
        Some(b"run-1".as_slice()),
        Some(b"researcher".as_slice()),
        Some(SubagentExecutionIdentity { execution_id: "run-1".into(), definition_key: "researcher".into() })
    )]
    fn parses_subagent_execution_headers(
        #[case] execution_id: Option<&[u8]>,
        #[case] definition_key: Option<&[u8]>,
        #[case] expected: Option<SubagentExecutionIdentity>,
    ) {
        assert_eq!(
            subagent_execution_identity_from_headers(&headers_with_child(
                execution_id,
                definition_key,
            )),
            Ok(expected),
        );
    }

    #[rstest]
    #[case::missing_definition(Some(b"run-1".as_slice()), None)]
    #[case::missing_execution(None, Some(b"researcher".as_slice()))]
    fn rejects_partial_subagent_execution_headers(
        #[case] execution_id: Option<&[u8]>,
        #[case] definition_key: Option<&[u8]>,
    ) {
        assert_eq!(
            subagent_execution_identity_from_headers(&headers_with_child(
                execution_id,
                definition_key,
            )),
            Err(rmcp::ErrorData::invalid_params(
                "x-subagent-execution-id and x-subagent-definition-key must be provided together",
                None,
            )),
        );
    }
}
