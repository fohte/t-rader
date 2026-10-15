use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{
    CreateSubagentDefinitionRequest, SubagentDefinitionResponse, UpdateSubagentDefinitionRequest,
};
use core_application::agent_config::AgentConfigRepositoryError;
use core_application::subagent_definition::{
    SubagentDefinitionRepositoryError, SubagentDefinitionUseCaseError,
};
use core_application::unit_of_work::UnitOfWorkError;

fn map_err(error: SubagentDefinitionUseCaseError) -> AppError {
    match error {
        SubagentDefinitionUseCaseError::InvalidKey(_)
        | SubagentDefinitionUseCaseError::RunSubagentToolNotAllowed
        | SubagentDefinitionUseCaseError::TimeoutMinutesOutOfRange { .. }
        | SubagentDefinitionUseCaseError::MaxModelCallsOutOfRange(_)
        | SubagentDefinitionUseCaseError::ToolLimitOutOfRange { .. } => {
            AppError::Validation(error.to_string())
        }
        SubagentDefinitionUseCaseError::InvalidAgentGraph(_) => {
            AppError::Internal(error.to_string())
        }
        SubagentDefinitionUseCaseError::DuplicateKey(_)
        | SubagentDefinitionUseCaseError::Referenced { .. } => {
            AppError::Conflict(error.to_string())
        }
        SubagentDefinitionUseCaseError::NotFound(_) => AppError::NotFound(error.to_string()),
        SubagentDefinitionUseCaseError::Repository(
            SubagentDefinitionRepositoryError::Database(error),
        )
        | SubagentDefinitionUseCaseError::AgentConfigRepository(
            AgentConfigRepositoryError::Database(error),
        )
        | SubagentDefinitionUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | SubagentDefinitionUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error)) => {
            error.into()
        }
        error => AppError::Internal(error.to_string()),
    }
}

/// サブエージェント定義一覧
#[utoipa::path(
    get,
    path = "/api/subagent-definitions",
    tag = "subagent_definitions",
    responses(
        (status = 200, body = Vec<SubagentDefinitionResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_subagent_definitions(
    State(state): State<FrontendApiState>,
) -> Result<Json<Vec<SubagentDefinitionResponse>>, AppError> {
    let items = state
        .subagent_definition_use_cases
        .list()
        .await
        .map_err(map_err)?;
    Ok(Json(items.into_iter().map(Into::into).collect()))
}

/// サブエージェント定義を作成
#[utoipa::path(
    post,
    path = "/api/subagent-definitions",
    tag = "subagent_definitions",
    request_body = CreateSubagentDefinitionRequest,
    responses(
        (status = 201, body = SubagentDefinitionResponse),
        (status = 400, body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 415, body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_subagent_definition(
    State(state): State<FrontendApiState>,
    JsonBody(payload): JsonBody<CreateSubagentDefinitionRequest>,
) -> Result<(StatusCode, Json<SubagentDefinitionResponse>), AppError> {
    let (key, fields) = payload.into_fields();
    let definition = state
        .subagent_definition_use_cases
        .create(key, fields)
        .await
        .map_err(map_err)?;
    Ok((StatusCode::CREATED, Json(definition.into())))
}

/// サブエージェント定義を取得
#[utoipa::path(
    get,
    path = "/api/subagent-definitions/{key}",
    tag = "subagent_definitions",
    params(("key" = String, Path, description = "サブエージェント定義の key")),
    responses(
        (status = 200, body = SubagentDefinitionResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_subagent_definition(
    State(state): State<FrontendApiState>,
    JsonPath(key): JsonPath<String>,
) -> Result<Json<SubagentDefinitionResponse>, AppError> {
    let definition = state
        .subagent_definition_use_cases
        .get(&key)
        .await
        .map_err(map_err)?;
    Ok(Json(definition.into()))
}

/// サブエージェント定義を更新
#[utoipa::path(
    put,
    path = "/api/subagent-definitions/{key}",
    tag = "subagent_definitions",
    params(("key" = String, Path, description = "サブエージェント定義の key")),
    request_body = UpdateSubagentDefinitionRequest,
    responses(
        (status = 200, body = SubagentDefinitionResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_subagent_definition(
    State(state): State<FrontendApiState>,
    JsonPath(key): JsonPath<String>,
    JsonBody(payload): JsonBody<UpdateSubagentDefinitionRequest>,
) -> Result<Json<SubagentDefinitionResponse>, AppError> {
    let definition = state
        .subagent_definition_use_cases
        .update(&key, payload.into())
        .await
        .map_err(map_err)?;
    Ok(Json(definition.into()))
}

/// サブエージェント定義を削除
#[utoipa::path(
    delete,
    path = "/api/subagent-definitions/{key}",
    tag = "subagent_definitions",
    params(("key" = String, Path, description = "サブエージェント定義の key")),
    responses(
        (status = 204),
        (status = 404, body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_subagent_definition(
    State(state): State<FrontendApiState>,
    JsonPath(key): JsonPath<String>,
) -> Result<StatusCode, AppError> {
    state
        .subagent_definition_use_cases
        .delete(&key)
        .await
        .map_err(map_err)?;
    Ok(StatusCode::NO_CONTENT)
}
