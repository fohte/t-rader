//! 管理 MCP の戦略一覧・タスク投入・タスク status tool。

use rmcp::ErrorData as McpError;

use core_application::agent_task_client::AgentTaskError;
use core_application::strategy_task::{
    GetTaskError, ResumeTaskError, StrategyTaskRepositoryError, SubmitTaskError, TaskSource,
};
use core_application::unit_of_work::UnitOfWorkError;

use super::dto::{
    GetStrategyTaskStatusParams, GetStrategyTaskStatusResult, ListStrategiesResult,
    ResumeStrategyTaskParams, ResumeStrategyTaskResult, StrategySummary, SubmitStrategyTaskParams,
    SubmitStrategyTaskResult,
};
use super::{MgmtServer, internal_error, invalid_params, map_strategy_use_case_error};

impl MgmtServer {
    pub(super) async fn list_strategies_inner(&self) -> Result<ListStrategiesResult, McpError> {
        let strategies = self
            .dependencies
            .strategies
            .list_summaries()
            .await
            .map_err(map_strategy_use_case_error)?
            .into_iter()
            .map(|summary| StrategySummary {
                strategy_id: summary.id,
                name: summary.name,
                updated_at: summary.updated_at,
            })
            .collect();
        Ok(ListStrategiesResult { strategies })
    }

    pub(super) async fn submit_strategy_task_inner(
        &self,
        params: SubmitStrategyTaskParams,
    ) -> Result<SubmitStrategyTaskResult, McpError> {
        let submitted = self
            .dependencies
            .strategy_tasks
            .submit_task(
                self.dependencies.agent_client.as_ref(),
                params.strategy_id,
                &params.prompt,
                TaskSource::MgmtMcp,
                params.purpose,
            )
            .await
            .map_err(map_submit_error)?;
        Ok(SubmitStrategyTaskResult {
            task_id: submitted.task_id,
            a2a_task_id: submitted.a2a_task_id,
        })
    }

    pub(super) async fn resume_strategy_task_inner(
        &self,
        params: ResumeStrategyTaskParams,
    ) -> Result<ResumeStrategyTaskResult, McpError> {
        let submitted = self
            .dependencies
            .strategy_tasks
            .resume(self.dependencies.agent_client.as_ref(), params.task_id)
            .await
            .map_err(map_resume_error)?;
        Ok(ResumeStrategyTaskResult {
            task_id: submitted.task_id,
            a2a_task_id: submitted.a2a_task_id,
        })
    }

    pub(super) async fn get_strategy_task_status_inner(
        &self,
        params: GetStrategyTaskStatusParams,
    ) -> Result<GetStrategyTaskStatusResult, McpError> {
        let view = self
            .dependencies
            .strategy_tasks
            .get_by_a2a_task_id(&params.a2a_task_id)
            .await
            .map_err(map_get_task_error)?
            .ok_or_else(|| McpError::resource_not_found("strategy task not found", None))?;
        Ok(GetStrategyTaskStatusResult {
            task_id: view.task_id,
            strategy_id: view.strategy_id,
            a2a_task_id: view.a2a_task_id,
            phase: view.phase.as_str().to_string(),
            error_summary: view.error_summary,
            result_text: view.result_text,
            updated_at: view.updated_at,
        })
    }
}
fn map_submit_error(err: SubmitTaskError) -> McpError {
    match err {
        SubmitTaskError::EmptyPrompt => invalid_params("prompt must not be empty"),
        SubmitTaskError::StrategyNotFound(id) => invalid_params(format!("strategy {id} not found")),
        SubmitTaskError::PurposeNotFound(purpose) => {
            invalid_params(format!("agent_config for purpose '{purpose}' not found"))
        }
        SubmitTaskError::Repository(error) => map_repository_error(error),
        SubmitTaskError::UnitOfWork(error) => map_unit_of_work_error(error),
        SubmitTaskError::AgentTask(agent_err) => map_agent_task_error(&agent_err),
    }
}

fn map_resume_error(err: ResumeTaskError) -> McpError {
    match err {
        ResumeTaskError::NotFound(id) => {
            McpError::resource_not_found(format!("strategy task {id} not found"), None)
        }
        ResumeTaskError::NotResumable(id, phase) => invalid_params(format!(
            "strategy task {id} is not resumable (current phase: {phase})"
        )),
        ResumeTaskError::Repository(error) => map_repository_error(error),
        ResumeTaskError::UnitOfWork(error) => map_unit_of_work_error(error),
        ResumeTaskError::AgentTask(agent_err) => map_agent_task_error(&agent_err),
    }
}

fn map_get_task_error(error: GetTaskError) -> McpError {
    match error {
        GetTaskError::NotFound(id) => {
            McpError::resource_not_found(format!("strategy task {id} not found"), None)
        }
        GetTaskError::StrategyMismatch { task_id, .. } => {
            McpError::resource_not_found(format!("strategy task {task_id} not found"), None)
        }
        GetTaskError::Repository(error) => map_repository_error(error),
    }
}

fn map_repository_error(error: StrategyTaskRepositoryError) -> McpError {
    tracing::error!(error = %error, "strategy task repository error");
    internal_error(format!("strategy task persistence error: {error}"))
}

fn map_unit_of_work_error(error: UnitOfWorkError) -> McpError {
    match error {
        UnitOfWorkError::Begin(error) | UnitOfWorkError::Commit(error) => {
            tracing::error!(error = %error, "strategy task transaction error");
            internal_error(format!("database error: {error}"))
        }
        UnitOfWorkError::InvalidTransaction => internal_error("invalid strategy task transaction"),
    }
}

fn map_agent_task_error(err: &AgentTaskError) -> McpError {
    match err {
        AgentTaskError::NotConfigured => internal_error("agent task client is not configured"),
        AgentTaskError::NotFound(name) => {
            McpError::resource_not_found(format!("agent task not found: {name}"), None)
        }
        AgentTaskError::Api { status, message } => {
            internal_error(format!("agent task api error (status {status}): {message}"))
        }
        AgentTaskError::Network(msg) => internal_error(format!("agent task network error: {msg}")),
        AgentTaskError::Parse(msg) => internal_error(format!("agent task parse error: {msg}")),
        AgentTaskError::Init(msg) => internal_error(format!("agent task init error: {msg}")),
    }
}
