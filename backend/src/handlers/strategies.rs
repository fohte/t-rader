use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, TransactionTrait,
};
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::entities::{strategy, strategy_interest};
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::kubeopencode::{KubeopencodeError, agent_name_for};
use crate::models::{
    AgentConfigResponse, AgentsMdBody, CreateStrategyRequest, SkillBody, SkillsBody,
    StrategyChatRequest, StrategyChatResponse, StrategyTaskStatusResponse, UpdateStrategyRequest,
};
use crate::services::change_history::{self, Op, TargetKind};
use crate::services::strategy_agent;
use crate::services::strategy_tasks::{self, GetTaskError, SubmitTaskError, TaskSource, phase_str};

fn validate_name(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(AppError::Validation("name must not be empty".into()));
    }
    Ok(trimmed)
}

async fn find_strategy_or_404(
    db: &sea_orm::DatabaseConnection,
    id: Uuid,
) -> Result<strategy::Model, AppError> {
    strategy::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("strategy {id} not found")))
}

/// 戦略一覧
#[utoipa::path(
    get,
    path = "/api/strategies",
    tag = "strategies",
    responses(
        (status = 200, description = "戦略一覧", body = Vec<strategy::Model>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategies(
    State(state): State<AppState>,
) -> Result<Json<Vec<strategy::Model>>, AppError> {
    let items = strategy::Entity::find()
        .order_by_asc(strategy::Column::SortOrder)
        .order_by_asc(strategy::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(items))
}

/// 戦略取得
#[utoipa::path(
    get,
    path = "/api/strategies/{id}",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = strategy::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_strategy(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<strategy::Model>, AppError> {
    let model = find_strategy_or_404(&state.db, id).await?;
    Ok(Json(model))
}

/// 戦略作成
#[utoipa::path(
    post,
    path = "/api/strategies",
    tag = "strategies",
    request_body = CreateStrategyRequest,
    responses(
        (status = 201, body = strategy::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_strategy(
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<CreateStrategyRequest>,
) -> Result<(StatusCode, Json<strategy::Model>), AppError> {
    let name = validate_name(&payload.name)?;
    let id = Uuid::new_v4();
    let sort_order = payload.sort_order.unwrap_or(0);

    let model = strategy::ActiveModel {
        id: Set(id),
        name: Set(name.clone()),
        description: Set(payload.description.clone()),
        sort_order: Set(sort_order),
        agents_md: NotSet,
        skills: NotSet,
        agent_status: NotSet,
        agent_error: NotSet,
        created_at: NotSet,
        updated_at: NotSet,
    };
    let txn = state.db.begin().await?;
    let created = strategy::Entity::insert(model)
        .exec_with_returning(&txn)
        .await?;
    change_history::record(
        &txn,
        TargetKind::Strategy,
        id,
        Op::Create,
        json!({ "name": name, "sort_order": sort_order }),
        Some(format!("created strategy {name}")),
    )
    .await?;
    txn.commit().await?;

    // commit 前に spawn すると rollback 時に Agent CR が孤立する
    strategy_agent::spawn_reconcile(state.db.clone(), state.kubeopencode.clone(), id);

    Ok((StatusCode::CREATED, Json(created)))
}

/// 戦略更新
#[utoipa::path(
    patch,
    path = "/api/strategies/{id}",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = UpdateStrategyRequest,
    responses(
        (status = 200, body = strategy::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_strategy(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<UpdateStrategyRequest>,
) -> Result<Json<strategy::Model>, AppError> {
    let current = find_strategy_or_404(&state.db, id).await?;
    let mut active = current.clone().into_active_model();
    let mut diff = serde_json::Map::new();

    if let Some(name) = payload.name.as_ref() {
        let name = validate_name(name)?;
        diff.insert("name".into(), json!({ "from": current.name, "to": name }));
        active.name = Set(name);
    }
    if let Some(description) = payload.description.as_ref() {
        diff.insert(
            "description".into(),
            json!({ "from": current.description, "to": description }),
        );
        active.description = Set(Some(description.clone()));
    }
    if let Some(sort_order) = payload.sort_order {
        diff.insert(
            "sort_order".into(),
            json!({ "from": current.sort_order, "to": sort_order }),
        );
        active.sort_order = Set(sort_order);
    }
    active.updated_at = Set(chrono::Utc::now().fixed_offset());

    let txn = state.db.begin().await?;
    let updated = active.update(&txn).await?;
    if !diff.is_empty() {
        change_history::record(
            &txn,
            TargetKind::Strategy,
            id,
            Op::Update,
            serde_json::Value::Object(diff),
            None,
        )
        .await?;
    }
    txn.commit().await?;

    Ok(Json(updated))
}

/// 戦略削除
#[utoipa::path(
    delete,
    path = "/api/strategies/{id}",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 204),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_strategy(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    // 不在の id なら Kube API を叩く前に 404 を確定させる
    find_strategy_or_404(&state.db, id).await?;

    // Agent CR を先に消す: DB を先に消すと孤児 Agent が一時的に残り、in-flight Task の
    // agentRef は解決できるが履歴 (strategy_task / change_history) との突き合わせが切れる
    let agent_name = agent_name_for(id);
    match state.kubeopencode.delete_strategy_agent(&agent_name).await {
        Ok(()) | Err(KubeopencodeError::NotConfigured) => {}
        Err(err) => {
            tracing::error!(error = %err, strategy_id = %id, "failed to delete strategy agent");
            return Err(AppError::Config(format!(
                "failed to delete strategy agent: {err}"
            )));
        }
    }

    let txn = state.db.begin().await?;
    let result = strategy::Entity::delete_by_id(id).exec(&txn).await?;
    if result.rows_affected == 0 {
        return Err(AppError::NotFound(format!("strategy {id} not found")));
    }
    change_history::record(&txn, TargetKind::Strategy, id, Op::Delete, json!({}), None).await?;
    txn.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 戦略の関心 (シード + LLM 派生) 一覧
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/interests",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = Vec<strategy_interest::Model>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategy_interests(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<Vec<strategy_interest::Model>>, AppError> {
    find_strategy_or_404(&state.db, id).await?;
    let items = strategy_interest::Entity::find()
        .filter(strategy_interest::Column::StrategyId.eq(id))
        .order_by_asc(strategy_interest::Column::Role)
        .order_by_asc(strategy_interest::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(items))
}

pub(crate) fn map_submit_error(err: SubmitTaskError) -> AppError {
    match err {
        SubmitTaskError::EmptyPrompt => AppError::Validation("prompt must not be empty".into()),
        SubmitTaskError::StrategyNotFound(id) => {
            AppError::NotFound(format!("strategy {id} not found"))
        }
        SubmitTaskError::Database(db_err) => AppError::Database(db_err),
        SubmitTaskError::AgentTask(agent_err) => {
            AppError::Config(format!("agent task error: {agent_err}"))
        }
    }
}

/// フローティングチャットから戦略 Agent にタスクを投入する
#[utoipa::path(
    post,
    path = "/api/strategies/{id}/chat",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = StrategyChatRequest,
    responses(
        (status = 202, body = StrategyChatResponse),
        (status = 400, description = "prompt が空 (空白のみを含む)", body = ErrorResponse),
        (status = 404, description = "戦略が存在しない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn submit_strategy_chat(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<StrategyChatRequest>,
) -> Result<(StatusCode, Json<StrategyChatResponse>), AppError> {
    let submitted = strategy_tasks::submit_task(
        &state.db,
        &state.agent_task_client,
        id,
        &payload.prompt,
        TaskSource::Frontend,
    )
    .await
    .map_err(map_submit_error)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(StrategyChatResponse {
            task_id: submitted.task_id,
            a2a_task_id: submitted.a2a_task_id,
        }),
    ))
}

/// 投入済み戦略タスクの phase / error_summary を取得する
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/tasks/{task_id}",
    tag = "strategies",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("task_id" = Uuid, Path, description = "戦略タスク ID"),
    ),
    responses(
        (status = 200, body = StrategyTaskStatusResponse),
        (status = 400, description = "パスパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_strategy_task(
    State(state): State<AppState>,
    JsonPath((strategy_id, task_id)): JsonPath<(Uuid, Uuid)>,
) -> Result<Json<StrategyTaskStatusResponse>, AppError> {
    let view = strategy_tasks::get_task_for_strategy(&state.db, strategy_id, task_id)
        .await
        .map_err(|err| match err {
            GetTaskError::NotFound(id) => {
                AppError::NotFound(format!("strategy task {id} not found"))
            }
            GetTaskError::StrategyMismatch { task_id, .. } => {
                AppError::NotFound(format!("strategy task {task_id} not found"))
            }
            GetTaskError::Database(db_err) => AppError::Database(db_err),
        })?;
    Ok(Json(StrategyTaskStatusResponse {
        task_id: view.task_id,
        strategy_id: view.strategy_id,
        a2a_task_id: view.a2a_task_id,
        source: view.source,
        phase: phase_str(&view.phase).to_string(),
        error_summary: view.error_summary,
        result_text: view.result_text,
        created_at: view.created_at,
        updated_at: view.updated_at,
    }))
}

fn validate_skill_name(name: &str) -> Result<(), AppError> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err(AppError::Validation("skill name must not be empty".into()));
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return Err(AppError::Validation(
            "skill name must start with [a-z0-9]".into(),
        ));
    }
    for c in chars {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-') {
            return Err(AppError::Validation(
                "skill name must match ^[a-z0-9][a-z0-9_-]*$".into(),
            ));
        }
    }
    Ok(())
}

fn skills_object(value: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    value
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new)
}

fn skills_to_btree(value: &serde_json::Value) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    if let Some(map) = value.as_object() {
        for (k, v) in map {
            if let Some(s) = v.as_str() {
                out.insert(k.clone(), s.to_string());
            }
        }
    }
    out
}

async fn save_skills(
    state: &AppState,
    current: strategy::Model,
    skills: serde_json::Value,
    op_desc: String,
) -> Result<strategy::Model, AppError> {
    let id = current.id;
    let prev_skills = current.skills.clone();
    let mut active = current.into_active_model();
    active.skills = Set(skills.clone());
    active.updated_at = Set(chrono::Utc::now().fixed_offset());

    let txn = state.db.begin().await?;
    let updated = active.update(&txn).await?;
    change_history::record(
        &txn,
        TargetKind::Strategy,
        id,
        Op::Update,
        json!({ "skills": { "from": prev_skills, "to": skills } }),
        Some(op_desc),
    )
    .await?;
    txn.commit().await?;

    // rollback 時に reconcile が走ると孤児 Agent CR が残る race を避けるため commit 後に spawn する
    strategy_agent::spawn_reconcile(state.db.clone(), state.kubeopencode.clone(), updated.id);
    Ok(updated)
}

/// 戦略 Agent の AGENTS.md (方針 / 制約 markdown) を取得
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/agents-md",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = AgentsMdBody),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_agents_md(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<AgentsMdBody>, AppError> {
    let row = find_strategy_or_404(&state.db, id).await?;
    Ok(Json(AgentsMdBody {
        content: row.agents_md,
    }))
}

/// 戦略 Agent の AGENTS.md を上書き保存し、Agent reconcile を再発火
#[utoipa::path(
    put,
    path = "/api/strategies/{id}/agents-md",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = AgentsMdBody,
    responses(
        (status = 200, body = AgentsMdBody),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn put_agents_md(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<AgentsMdBody>,
) -> Result<Json<AgentsMdBody>, AppError> {
    let current = find_strategy_or_404(&state.db, id).await?;
    let prev = current.agents_md.clone();
    let mut active = current.into_active_model();
    active.agents_md = Set(payload.content.clone());
    active.updated_at = Set(chrono::Utc::now().fixed_offset());

    let txn = state.db.begin().await?;
    let updated = active.update(&txn).await?;
    change_history::record(
        &txn,
        TargetKind::Strategy,
        id,
        Op::Update,
        json!({ "agents_md": { "from": prev, "to": payload.content } }),
        Some("updated agents_md".to_string()),
    )
    .await?;
    txn.commit().await?;

    // rollback 時に reconcile が走ると孤児 Agent CR が残る race を避けるため commit 後に spawn する
    strategy_agent::spawn_reconcile(state.db.clone(), state.kubeopencode.clone(), updated.id);
    Ok(Json(AgentsMdBody {
        content: updated.agents_md,
    }))
}

/// 戦略 Agent の skills 全件取得
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/skills",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = SkillsBody),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_skills(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<SkillsBody>, AppError> {
    let row = find_strategy_or_404(&state.db, id).await?;
    Ok(Json(SkillsBody {
        skills: skills_to_btree(&row.skills),
    }))
}

/// 戦略 Agent の skills 全置換
#[utoipa::path(
    put,
    path = "/api/strategies/{id}/skills",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = SkillsBody,
    responses(
        (status = 200, body = SkillsBody),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn put_skills(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<SkillsBody>,
) -> Result<Json<SkillsBody>, AppError> {
    for name in payload.skills.keys() {
        validate_skill_name(name).map_err(|err| match err {
            AppError::Validation(msg) => AppError::Validation(format!("skill {name:?}: {msg}")),
            other => other,
        })?;
    }
    let current = find_strategy_or_404(&state.db, id).await?;
    let mut map = serde_json::Map::new();
    for (k, v) in &payload.skills {
        map.insert(k.clone(), serde_json::Value::String(v.clone()));
    }
    let updated = save_skills(
        &state,
        current,
        serde_json::Value::Object(map),
        "replaced all skills".to_string(),
    )
    .await?;
    Ok(Json(SkillsBody {
        skills: skills_to_btree(&updated.skills),
    }))
}

/// 戦略 Agent の単一 skill 追加 / 更新
#[utoipa::path(
    put,
    path = "/api/strategies/{id}/skills/{name}",
    tag = "strategies",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("name" = String, Path, description = "skill 名 (^[a-z0-9][a-z0-9_-]*$)"),
    ),
    request_body = SkillBody,
    responses(
        (status = 200, body = SkillBody),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn put_skill(
    State(state): State<AppState>,
    JsonPath((id, name)): JsonPath<(Uuid, String)>,
    JsonBody(payload): JsonBody<SkillBody>,
) -> Result<Json<SkillBody>, AppError> {
    validate_skill_name(&name)?;
    let current = find_strategy_or_404(&state.db, id).await?;
    let mut map = skills_object(&current.skills);
    map.insert(
        name.clone(),
        serde_json::Value::String(payload.content.clone()),
    );
    save_skills(
        &state,
        current,
        serde_json::Value::Object(map),
        format!("updated skill {name}"),
    )
    .await?;
    Ok(Json(SkillBody {
        content: payload.content,
    }))
}

/// 戦略 Agent の単一 skill 削除
#[utoipa::path(
    delete,
    path = "/api/strategies/{id}/skills/{name}",
    tag = "strategies",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("name" = String, Path, description = "skill 名"),
    ),
    responses(
        (status = 204),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_skill(
    State(state): State<AppState>,
    JsonPath((id, name)): JsonPath<(Uuid, String)>,
) -> Result<StatusCode, AppError> {
    validate_skill_name(&name)?;
    let current = find_strategy_or_404(&state.db, id).await?;
    let mut map = skills_object(&current.skills);
    if map.remove(&name).is_none() {
        return Err(AppError::NotFound(format!("skill {name} not found")));
    }
    save_skills(
        &state,
        current,
        serde_json::Value::Object(map),
        format!("deleted skill {name}"),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// モデル設定は DB ではなく env 由来。kubeopencode reconcile と同じ変数名 / デフォルトを使う。
fn agent_model_settings() -> (String, String) {
    agent_model_settings_with(|key| std::env::var(key).ok())
}

fn agent_model_settings_with<F>(get: F) -> (String, String)
where
    F: Fn(&str) -> Option<String>,
{
    let model = get("STRATEGY_AGENT_MODEL")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| crate::kubeopencode::DEFAULT_AGENT_MODEL.to_string());
    let small_model = get("STRATEGY_AGENT_SMALL_MODEL")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| crate::kubeopencode::DEFAULT_AGENT_SMALL_MODEL.to_string());
    (model, small_model)
}

/// 戦略 Agent 設定一式 (AGENTS.md / skills / モデル設定) の統合取得。
/// t-rader-agent がタスク実行のたびに呼び出し、agent をその場で構成する。
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/agent-config",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = AgentConfigResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_agent_config(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<AgentConfigResponse>, AppError> {
    let row = find_strategy_or_404(&state.db, id).await?;
    let (model, small_model) = agent_model_settings();
    Ok(Json(AgentConfigResponse {
        agents_md: row.agents_md,
        skills: skills_to_btree(&row.skills),
        model,
        small_model,
    }))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rstest::rstest;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{DatabaseConnection, EntityTrait};
    use serde_json::json;
    use sqlx::PgPool;

    use uuid::Uuid;

    use super::agent_model_settings_with;
    use crate::agent_client::{FakeAgentTaskClient, SharedAgentTaskClient};
    use crate::entities::sea_orm_active_enums::StrategyAgentStatus;
    use crate::entities::{strategy, strategy_task};
    use crate::kubeopencode::{FakeKubeopencodeClient, SharedKubeopencodeClient};
    use crate::testing::{
        create_test_server, create_test_server_with_db,
        create_test_server_with_db_and_agent_client, create_test_server_with_kube,
    };

    async fn insert_strategy(db: &DatabaseConnection, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(id),
            name: Set(name.to_string()),
            description: Set(None),
            sort_order: Set(0),
            agents_md: NotSet,
            skills: NotSet,
            agent_status: NotSet,
            agent_error: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert strategy");
        id
    }

    #[sqlx::test(migrations = false)]
    async fn create_and_list_strategy(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .post("/api/strategies")
            .json(&json!({ "name": "長期投資" }))
            .await;
        res.assert_status(axum::http::StatusCode::CREATED);

        let list = server.get("/api/strategies").await;
        list.assert_status_ok();
        let body: Vec<serde_json::Value> = list.json();
        assert_eq!(body.len(), 1);
        assert_eq!(body[0]["name"], "長期投資");
    }

    #[sqlx::test(migrations = false)]
    async fn get_nonexistent_strategy_returns_404(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000")
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn list_interests_of_nonexistent_strategy_returns_404(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000/interests")
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn delete_strategy_calls_kube_delete(pool: PgPool) {
        let fake = Arc::new(FakeKubeopencodeClient::new());
        let kube: SharedKubeopencodeClient = fake.clone();
        let server = create_test_server_with_kube(pool, kube).await;

        let created = server
            .post("/api/strategies")
            .json(&json!({ "name": "to-delete" }))
            .await;
        created.assert_status(axum::http::StatusCode::CREATED);
        let id = created.json::<serde_json::Value>()["id"]
            .as_str()
            .map(str::to_string)
            .expect("id");

        let deleted = server.delete(&format!("/api/strategies/{id}")).await;
        deleted.assert_status(axum::http::StatusCode::NO_CONTENT);

        let uuid = Uuid::parse_str(&id).unwrap();
        let expected_agent = crate::kubeopencode::agent_name_for(uuid);
        assert_eq!(
            fake.deleted_agents.lock().await.as_slice(),
            &[expected_agent],
        );
    }

    async fn wait_for_reconcile_count(fake: &FakeKubeopencodeClient, expected: usize) {
        for _ in 0..200 {
            if fake.reconciled.lock().await.len() >= expected {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!(
            "reconcile count did not reach {expected} within timeout (current: {})",
            fake.reconciled.lock().await.len()
        );
    }

    async fn create_strategy_for_skills(
        server: &axum_test::TestServer,
        fake: &FakeKubeopencodeClient,
        name: &str,
    ) -> String {
        let created = server
            .post("/api/strategies")
            .json(&json!({ "name": name }))
            .await;
        created.assert_status(axum::http::StatusCode::CREATED);
        // 以降の検証で追加分のみをカウントするため、作成時 spawn_reconcile の到着を先に待つ
        wait_for_reconcile_count(fake, 1).await;
        created.json::<serde_json::Value>()["id"]
            .as_str()
            .map(str::to_string)
            .expect("id")
    }

    #[sqlx::test(migrations = false)]
    async fn put_then_get_agents_md_round_trips_and_triggers_reconcile(pool: PgPool) {
        let fake = Arc::new(FakeKubeopencodeClient::new());
        let kube: SharedKubeopencodeClient = fake.clone();
        let server = create_test_server_with_kube(pool, kube).await;
        let id = create_strategy_for_skills(&server, &fake, "s").await;

        let body = "# 方針\n慎重に運用する";
        let put = server
            .put(&format!("/api/strategies/{id}/agents-md"))
            .json(&json!({ "content": body }))
            .await;
        put.assert_status_ok();
        assert_eq!(put.json::<serde_json::Value>(), json!({ "content": body }));

        let get = server.get(&format!("/api/strategies/{id}/agents-md")).await;
        get.assert_status_ok();
        assert_eq!(get.json::<serde_json::Value>(), json!({ "content": body }));

        wait_for_reconcile_count(&fake, 2).await;
        let last = fake.reconciled.lock().await.last().cloned().expect("spec");
        let uuid = Uuid::parse_str(&id).unwrap();
        assert_eq!(
            last,
            crate::kubeopencode::StrategyAgentSpec {
                strategy_id: uuid,
                agent_name: crate::kubeopencode::agent_name_for(uuid),
                agents_md: body.to_string(),
                skills: std::collections::BTreeMap::new(),
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn agents_md_get_404_for_unknown_strategy(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000/agents-md")
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn put_skills_replaces_whole_map(pool: PgPool) {
        let fake = Arc::new(FakeKubeopencodeClient::new());
        let kube: SharedKubeopencodeClient = fake.clone();
        let server = create_test_server_with_kube(pool, kube).await;
        let id = create_strategy_for_skills(&server, &fake, "s").await;

        let put = server
            .put(&format!("/api/strategies/{id}/skills"))
            .json(&json!({ "skills": { "scout": "scout body", "review": "review body" } }))
            .await;
        put.assert_status_ok();
        assert_eq!(
            put.json::<serde_json::Value>(),
            json!({ "skills": { "scout": "scout body", "review": "review body" } }),
        );

        let replaced = server
            .put(&format!("/api/strategies/{id}/skills"))
            .json(&json!({ "skills": { "only": "left" } }))
            .await;
        replaced.assert_status_ok();

        let get = server.get(&format!("/api/strategies/{id}/skills")).await;
        assert_eq!(
            get.json::<serde_json::Value>(),
            json!({ "skills": { "only": "left" } }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn single_skill_add_update_delete_lifecycle(pool: PgPool) {
        let fake = Arc::new(FakeKubeopencodeClient::new());
        let kube: SharedKubeopencodeClient = fake.clone();
        let server = create_test_server_with_kube(pool, kube).await;
        let id = create_strategy_for_skills(&server, &fake, "s").await;

        let add = server
            .put(&format!("/api/strategies/{id}/skills/scout"))
            .json(&json!({ "content": "first" }))
            .await;
        add.assert_status_ok();
        assert_eq!(
            add.json::<serde_json::Value>(),
            json!({ "content": "first" })
        );

        let update = server
            .put(&format!("/api/strategies/{id}/skills/scout"))
            .json(&json!({ "content": "second" }))
            .await;
        update.assert_status_ok();

        let add_other = server
            .put(&format!("/api/strategies/{id}/skills/review"))
            .json(&json!({ "content": "rev" }))
            .await;
        add_other.assert_status_ok();

        let after_adds = server.get(&format!("/api/strategies/{id}/skills")).await;
        assert_eq!(
            after_adds.json::<serde_json::Value>(),
            json!({ "skills": { "scout": "second", "review": "rev" } }),
        );

        let del = server
            .delete(&format!("/api/strategies/{id}/skills/scout"))
            .await;
        del.assert_status(axum::http::StatusCode::NO_CONTENT);

        let after_del = server.get(&format!("/api/strategies/{id}/skills")).await;
        assert_eq!(
            after_del.json::<serde_json::Value>(),
            json!({ "skills": { "review": "rev" } }),
        );

        wait_for_reconcile_count(&fake, 5).await;
    }

    #[sqlx::test(migrations = false)]
    async fn delete_unknown_skill_returns_404(pool: PgPool) {
        let fake = Arc::new(FakeKubeopencodeClient::new());
        let kube: SharedKubeopencodeClient = fake.clone();
        let server = create_test_server_with_kube(pool, kube).await;
        let id = create_strategy_for_skills(&server, &fake, "s").await;

        let res = server
            .delete(&format!("/api/strategies/{id}/skills/missing"))
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn invalid_skill_name_returns_400(pool: PgPool) {
        let fake = Arc::new(FakeKubeopencodeClient::new());
        let kube: SharedKubeopencodeClient = fake.clone();
        let server = create_test_server_with_kube(pool, kube).await;
        let id = create_strategy_for_skills(&server, &fake, "s").await;

        for name in [
            "-bad",
            "Bad",
            "with space",
            "a/b",
            "",
            "..",
            ".hidden",
            "a.b",
        ] {
            let put_map = server
                .put(&format!("/api/strategies/{id}/skills"))
                .json(&json!({ "skills": { name: "x" } }))
                .await;
            put_map.assert_status(axum::http::StatusCode::BAD_REQUEST);

            if name.is_empty()
                || name.contains('/')
                || name.contains(' ')
                || name == ".."
                || name == "."
            {
                // URL path に乗らない / クライアント側で正規化されて route に届かないものはスキップ
                continue;
            }
            let put_single = server
                .put(&format!("/api/strategies/{id}/skills/{name}"))
                .json(&json!({ "content": "x" }))
                .await;
            put_single.assert_status(axum::http::StatusCode::BAD_REQUEST);
        }
    }

    #[sqlx::test(migrations = false)]
    async fn skills_endpoints_404_for_unknown_strategy(pool: PgPool) {
        let server = create_test_server(pool).await;
        let missing = "/api/strategies/00000000-0000-0000-0000-000000000000";

        let g = server.get(&format!("{missing}/skills")).await;
        g.assert_status(axum::http::StatusCode::NOT_FOUND);

        let put_all = server
            .put(&format!("{missing}/skills"))
            .json(&json!({ "skills": {} }))
            .await;
        put_all.assert_status(axum::http::StatusCode::NOT_FOUND);

        let put_one = server
            .put(&format!("{missing}/skills/scout"))
            .json(&json!({ "content": "x" }))
            .await;
        put_one.assert_status(axum::http::StatusCode::NOT_FOUND);

        let del = server.delete(&format!("{missing}/skills/scout")).await;
        del.assert_status(axum::http::StatusCode::NOT_FOUND);

        let put_md = server
            .put(&format!("{missing}/agents-md"))
            .json(&json!({ "content": "x" }))
            .await;
        put_md.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    fn env_get<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[rstest]
    #[case::unset(&[], (crate::kubeopencode::DEFAULT_AGENT_MODEL, crate::kubeopencode::DEFAULT_AGENT_SMALL_MODEL))]
    #[case::empty(
        &[("STRATEGY_AGENT_MODEL", ""), ("STRATEGY_AGENT_SMALL_MODEL", "")],
        (crate::kubeopencode::DEFAULT_AGENT_MODEL, crate::kubeopencode::DEFAULT_AGENT_SMALL_MODEL)
    )]
    #[case::overridden(
        &[("STRATEGY_AGENT_MODEL", "m-x"), ("STRATEGY_AGENT_SMALL_MODEL", "m-y")],
        ("m-x", "m-y")
    )]
    fn agent_model_settings_with_resolves_env(
        #[case] env: &[(&str, &str)],
        #[case] expected: (&str, &str),
    ) {
        assert_eq!(
            agent_model_settings_with(env_get(env)),
            (expected.0.to_string(), expected.1.to_string()),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn get_agent_config_returns_agents_md_skills_and_model(pool: PgPool) {
        let agents_md = indoc::indoc! {"
            # 方針
            慎重に運用する"};

        let (db, server) = create_test_server_with_db(pool).await;
        let id = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(id),
            name: Set("s".to_string()),
            description: Set(None),
            sort_order: Set(0),
            agents_md: Set(agents_md.to_string()),
            skills: Set(json!({ "scout": "scout body", "review": "review body" })),
            agent_status: Set(StrategyAgentStatus::Ready),
            agent_error: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(&db)
        .await
        .expect("insert strategy");

        let res = server
            .get(&format!("/api/strategies/{id}/agent-config"))
            .await;
        res.assert_status_ok();
        assert_eq!(
            res.json::<serde_json::Value>(),
            json!({
                "agents_md": agents_md,
                "skills": { "scout": "scout body", "review": "review body" },
                "model": crate::kubeopencode::DEFAULT_AGENT_MODEL,
                "small_model": crate::kubeopencode::DEFAULT_AGENT_SMALL_MODEL,
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn agent_config_get_404_for_unknown_strategy(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000/agent-config")
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn submit_chat_creates_task_row_and_submits_to_agent(pool: PgPool) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_next_task_id("agent-task-1").await;
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(pool, agent_client).await;
        let strategy_id = insert_strategy(&db, "long").await;

        let res = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": " inspect 7203 " }))
            .await;
        res.assert_status(axum::http::StatusCode::ACCEPTED);

        let mut body: serde_json::Value = res.json();
        let task_id = Uuid::parse_str(body["task_id"].as_str().expect("task_id")).expect("uuid");
        body["task_id"] = json!("<uuid>");
        assert_eq!(
            body,
            json!({
                "task_id": "<uuid>",
                "a2a_task_id": "agent-task-1",
            }),
        );

        let row = strategy_task::Entity::find_by_id(task_id)
            .one(&db)
            .await
            .unwrap()
            .expect("row");
        let row_summary = (
            row.task_id,
            row.strategy_id,
            row.a2a_task_id,
            row.source,
            row.prompt,
            row.phase,
            row.error_summary,
        );
        assert_eq!(
            row_summary,
            (
                task_id,
                strategy_id,
                Some("agent-task-1".to_string()),
                "frontend".to_string(),
                "inspect 7203".to_string(),
                crate::entities::sea_orm_active_enums::StrategyTaskPhase::Running,
                None,
            ),
        );

        let submitted: Vec<(Uuid, String)> = fake
            .submitted
            .lock()
            .await
            .iter()
            .map(|s| (s.strategy_id, s.prompt.clone()))
            .collect();
        assert_eq!(submitted, vec![(strategy_id, "inspect 7203".to_string())]);
    }

    #[sqlx::test(migrations = false)]
    async fn submit_chat_unknown_strategy_returns_404(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .post("/api/strategies/00000000-0000-0000-0000-000000000000/chat")
            .json(&json!({ "prompt": "x" }))
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn submit_chat_empty_prompt_returns_400(pool: PgPool) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(pool, agent_client).await;
        let strategy_id = insert_strategy(&db, "x").await;

        let res = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": "   " }))
            .await;
        res.assert_status(axum::http::StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn get_strategy_task_returns_phase(pool: PgPool) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(pool, agent_client).await;
        let strategy_id = insert_strategy(&db, "x").await;

        let submit = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": "p" }))
            .await;
        submit.assert_status(axum::http::StatusCode::ACCEPTED);
        let task_id = submit.json::<serde_json::Value>()["task_id"]
            .as_str()
            .map(|s| Uuid::parse_str(s).unwrap())
            .expect("task_id");
        let a2a_task_id = submit.json::<serde_json::Value>()["a2a_task_id"]
            .as_str()
            .expect("a2a_task_id")
            .to_string();

        let res = server
            .get(&format!("/api/strategies/{strategy_id}/tasks/{task_id}"))
            .await;
        res.assert_status_ok();
        let body: serde_json::Value = res.json();
        let mut normalized = body.clone();
        let obj = normalized.as_object_mut().unwrap();
        obj.remove("created_at");
        obj.remove("updated_at");
        assert_eq!(
            normalized,
            json!({
                "task_id": task_id,
                "strategy_id": strategy_id,
                "a2a_task_id": a2a_task_id,
                "source": "frontend",
                "phase": "running",
                "error_summary": null,
                "result_text": null,
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn get_strategy_task_unknown_returns_404(pool: PgPool) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(pool, agent_client).await;
        let strategy_id = insert_strategy(&db, "x").await;

        let res = server
            .get(&format!(
                "/api/strategies/{strategy_id}/tasks/00000000-0000-0000-0000-000000000000"
            ))
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn get_strategy_task_strategy_mismatch_returns_404(pool: PgPool) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(pool, agent_client).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;

        let submit = server
            .post(&format!("/api/strategies/{strategy_a}/chat"))
            .json(&json!({ "prompt": "p" }))
            .await;
        submit.assert_status(axum::http::StatusCode::ACCEPTED);
        let task_id = submit.json::<serde_json::Value>()["task_id"]
            .as_str()
            .map(|s| Uuid::parse_str(s).unwrap())
            .expect("task_id");

        let res = server
            .get(&format!("/api/strategies/{strategy_b}/tasks/{task_id}"))
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn list_interests_of_existing_strategy_returns_empty(pool: PgPool) {
        let server = create_test_server(pool).await;
        let created = server
            .post("/api/strategies")
            .json(&json!({ "name": "test" }))
            .await;
        created.assert_status(axum::http::StatusCode::CREATED);
        let id = created.json::<serde_json::Value>()["id"]
            .as_str()
            .map(str::to_string)
            .expect("id");

        let res = server.get(&format!("/api/strategies/{id}/interests")).await;
        res.assert_status_ok();
        let body: Vec<serde_json::Value> = res.json();
        assert!(body.is_empty());
    }
}
