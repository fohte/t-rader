//! `agent_config` (目的 (purpose) をキーとする AGENTS.md / skills / agent_graph) の
//! CRUD HTTP handler。
//!
//! `agent_config` の application use case を呼び出す thin wrapper。

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{
    AgentConfigItemResponse, AgentConfigResponse, AgentGraphBody, AgentsMdBody,
    CreateAgentConfigRequest, SkillBody, SkillsBody,
};
use core_application::agent_config::{
    AgentConfigRepositoryError, AgentConfigUseCaseError, AgentConfigUseCases,
};
use core_application::unit_of_work::UnitOfWorkError;

fn map_err(error: AgentConfigUseCaseError) -> AppError {
    match error {
        AgentConfigUseCaseError::InvalidPurpose(_)
        | AgentConfigUseCaseError::InvalidSkillName(_)
        | AgentConfigUseCaseError::InvalidAgentGraph(_) => AppError::Validation(error.to_string()),
        AgentConfigUseCaseError::DuplicatePurpose(_) => AppError::Conflict(error.to_string()),
        AgentConfigUseCaseError::NotFound(_) | AgentConfigUseCaseError::SkillNotFound(_) => {
            AppError::NotFound(error.to_string())
        }
        AgentConfigUseCaseError::Repository(AgentConfigRepositoryError::Database(error))
        | AgentConfigUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | AgentConfigUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error)) => error.into(),
        error => AppError::Internal(error.to_string()),
    }
}

fn use_cases(state: &FrontendApiState) -> AgentConfigUseCases {
    state.agent_config_use_cases.clone()
}

/// 目的別 agent 設定一覧
#[utoipa::path(
    get,
    path = "/api/agent-configs",
    tag = "agent_config",
    responses(
        (status = 200, body = Vec<AgentConfigItemResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_agent_configs(
    State(state): State<FrontendApiState>,
) -> Result<Json<Vec<AgentConfigItemResponse>>, AppError> {
    let items = use_cases(&state).list().await.map_err(map_err)?;
    Ok(Json(
        items
            .into_iter()
            .map(AgentConfigItemResponse::from)
            .collect(),
    ))
}

/// 目的別 agent 設定を作成 (purpose のみ必須、内容は空で作成し後続の PUT で設定する)
#[utoipa::path(
    post,
    path = "/api/agent-configs",
    tag = "agent_config",
    request_body = CreateAgentConfigRequest,
    responses(
        (status = 201, body = AgentConfigItemResponse),
        (status = 400, description = "purpose が不正", body = ErrorResponse),
        (status = 409, description = "purpose が既存と衝突", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_agent_config(
    State(state): State<FrontendApiState>,
    JsonBody(payload): JsonBody<CreateAgentConfigRequest>,
) -> Result<(StatusCode, Json<AgentConfigItemResponse>), AppError> {
    let created = use_cases(&state)
        .create(payload.purpose)
        .await
        .map_err(map_err)?;
    Ok((StatusCode::CREATED, Json(created.into())))
}

/// 目的別 agent 設定を取得。purpose キーで `agent_config` テーブルの行をそのまま返す。
#[utoipa::path(
    get,
    operation_id = "agent_config_get_agent_config",
    path = "/api/agent-configs/{purpose}",
    tag = "agent_config",
    params(("purpose" = String, Path, description = "目的キー")),
    responses(
        (status = 200, body = AgentConfigItemResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_agent_config(
    State(state): State<FrontendApiState>,
    JsonPath(purpose): JsonPath<String>,
) -> Result<Json<AgentConfigItemResponse>, AppError> {
    let model = use_cases(&state).get(&purpose).await.map_err(map_err)?;
    Ok(Json(model.into()))
}

/// 目的別 agent 設定一式 (AGENTS.md / skills / agent_graph) の統合取得。
/// t-rader-agent がタスク実行のたびに呼び出す。
#[utoipa::path(
    get,
    operation_id = "agent_config_get_agent_config_bundle",
    path = "/api/agent-configs/{purpose}/agent-config",
    tag = "agent_config",
    params(("purpose" = String, Path, description = "目的キー")),
    responses(
        (status = 200, body = AgentConfigResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_agent_config_bundle(
    State(state): State<FrontendApiState>,
    JsonPath(purpose): JsonPath<String>,
) -> Result<Json<AgentConfigResponse>, AppError> {
    let row = use_cases(&state).get(&purpose).await.map_err(map_err)?;
    let skills = row.skills_as_btree();
    Ok(Json(AgentConfigResponse {
        agents_md: row.agents_md,
        skills,
        agent_graph: row.agent_graph,
    }))
}

/// 目的別 agent 設定を削除
#[utoipa::path(
    delete,
    path = "/api/agent-configs/{purpose}",
    tag = "agent_config",
    params(("purpose" = String, Path, description = "目的キー")),
    responses(
        (status = 204),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_agent_config(
    State(state): State<FrontendApiState>,
    JsonPath(purpose): JsonPath<String>,
) -> Result<StatusCode, AppError> {
    use_cases(&state).delete(&purpose).await.map_err(map_err)?;
    Ok(StatusCode::NO_CONTENT)
}

/// 目的別 agent の AGENTS.md (方針 / 制約 markdown) を取得
#[utoipa::path(
    get,
    operation_id = "agent_config_get_agents_md",
    path = "/api/agent-configs/{purpose}/agents-md",
    tag = "agent_config",
    params(("purpose" = String, Path, description = "目的キー")),
    responses(
        (status = 200, body = AgentsMdBody),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_agents_md(
    State(state): State<FrontendApiState>,
    JsonPath(purpose): JsonPath<String>,
) -> Result<Json<AgentsMdBody>, AppError> {
    let row = use_cases(&state).get(&purpose).await.map_err(map_err)?;
    Ok(Json(AgentsMdBody {
        content: row.agents_md,
    }))
}

/// 目的別 agent の AGENTS.md を上書き保存
#[utoipa::path(
    put,
    operation_id = "agent_config_put_agents_md",
    path = "/api/agent-configs/{purpose}/agents-md",
    tag = "agent_config",
    params(("purpose" = String, Path, description = "目的キー")),
    request_body = AgentsMdBody,
    responses(
        (status = 200, body = AgentsMdBody),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn put_agents_md(
    State(state): State<FrontendApiState>,
    JsonPath(purpose): JsonPath<String>,
    JsonBody(payload): JsonBody<AgentsMdBody>,
) -> Result<Json<AgentsMdBody>, AppError> {
    let content = use_cases(&state)
        .save_agents_md(&purpose, payload.content)
        .await
        .map_err(map_err)?;
    Ok(Json(AgentsMdBody { content }))
}

/// 目的別 agent の skills 全件取得
#[utoipa::path(
    get,
    operation_id = "agent_config_get_skills",
    path = "/api/agent-configs/{purpose}/skills",
    tag = "agent_config",
    params(("purpose" = String, Path, description = "目的キー")),
    responses(
        (status = 200, body = SkillsBody),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_skills(
    State(state): State<FrontendApiState>,
    JsonPath(purpose): JsonPath<String>,
) -> Result<Json<SkillsBody>, AppError> {
    let row = use_cases(&state).get(&purpose).await.map_err(map_err)?;
    Ok(Json(SkillsBody {
        skills: row.skills_as_btree(),
    }))
}

/// 目的別 agent の skills 全置換
#[utoipa::path(
    put,
    operation_id = "agent_config_put_skills",
    path = "/api/agent-configs/{purpose}/skills",
    tag = "agent_config",
    params(("purpose" = String, Path, description = "目的キー")),
    request_body = SkillsBody,
    responses(
        (status = 200, body = SkillsBody),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn put_skills(
    State(state): State<FrontendApiState>,
    JsonPath(purpose): JsonPath<String>,
    JsonBody(payload): JsonBody<SkillsBody>,
) -> Result<Json<SkillsBody>, AppError> {
    let updated = use_cases(&state)
        .put_skills(&purpose, payload.skills)
        .await
        .map_err(map_err)?;
    Ok(Json(SkillsBody {
        skills: updated.skills_as_btree(),
    }))
}

/// 目的別 agent の単一 skill 追加 / 更新
#[utoipa::path(
    put,
    operation_id = "agent_config_put_skill",
    path = "/api/agent-configs/{purpose}/skills/{name}",
    tag = "agent_config",
    params(
        ("purpose" = String, Path, description = "目的キー"),
        ("name" = String, Path, description = "skill 名 (^[a-z0-9][a-z0-9_-]*$)"),
    ),
    request_body = SkillBody,
    responses(
        (status = 200, body = SkillBody),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn put_skill(
    State(state): State<FrontendApiState>,
    JsonPath((purpose, name)): JsonPath<(String, String)>,
    JsonBody(payload): JsonBody<SkillBody>,
) -> Result<Json<SkillBody>, AppError> {
    use_cases(&state)
        .put_skill(&purpose, &name, payload.content.clone())
        .await
        .map_err(map_err)?;
    Ok(Json(SkillBody {
        content: payload.content,
    }))
}

/// 目的別 agent の単一 skill 削除
#[utoipa::path(
    delete,
    operation_id = "agent_config_delete_skill",
    path = "/api/agent-configs/{purpose}/skills/{name}",
    tag = "agent_config",
    params(
        ("purpose" = String, Path, description = "目的キー"),
        ("name" = String, Path, description = "skill 名"),
    ),
    responses(
        (status = 204),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_skill(
    State(state): State<FrontendApiState>,
    JsonPath((purpose, name)): JsonPath<(String, String)>,
) -> Result<StatusCode, AppError> {
    use_cases(&state)
        .delete_skill(&purpose, &name)
        .await
        .map_err(map_err)?;
    Ok(StatusCode::NO_CONTENT)
}

/// 目的別 agent の多段フェーズ実行設定 (YAML) を取得
#[utoipa::path(
    get,
    operation_id = "agent_config_get_agent_graph",
    path = "/api/agent-configs/{purpose}/agent-graph",
    tag = "agent_config",
    params(("purpose" = String, Path, description = "目的キー")),
    responses(
        (status = 200, body = AgentGraphBody),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_agent_graph(
    State(state): State<FrontendApiState>,
    JsonPath(purpose): JsonPath<String>,
) -> Result<Json<AgentGraphBody>, AppError> {
    let row = use_cases(&state).get(&purpose).await.map_err(map_err)?;
    Ok(Json(AgentGraphBody {
        content: row.agent_graph,
    }))
}

/// 目的別 agent の多段フェーズ実行設定 (YAML) を上書き保存する
#[utoipa::path(
    put,
    operation_id = "agent_config_put_agent_graph",
    path = "/api/agent-configs/{purpose}/agent-graph",
    tag = "agent_config",
    params(("purpose" = String, Path, description = "目的キー")),
    request_body = AgentGraphBody,
    responses(
        (status = 200, body = AgentGraphBody),
        (status = 400, description = "YAML が不正、またはフェーズ定義が不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn put_agent_graph(
    State(state): State<FrontendApiState>,
    JsonPath(purpose): JsonPath<String>,
    JsonBody(payload): JsonBody<AgentGraphBody>,
) -> Result<Json<AgentGraphBody>, AppError> {
    let content = use_cases(&state)
        .save_agent_graph(&purpose, &payload.content)
        .await
        .map_err(map_err)?;
    Ok(Json(AgentGraphBody { content }))
}
