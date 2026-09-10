//! `agent_config` (目的 (purpose) をキーとする AGENTS.md / skills / agent_graph) の
//! CRUD HTTP handler。
//!
//! バリデーション・DB 操作は `services::agent_config` に委譲する thin wrapper。

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use crate::AppState;
use crate::entities::agent_config;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{
    AgentConfigResponse, AgentGraphBody, AgentsMdBody, CreateAgentConfigRequest, SkillBody,
    SkillsBody,
};
use crate::services::agent_config as svc;

fn map_err(err: svc::AgentConfigError) -> AppError {
    match err {
        svc::AgentConfigError::InvalidPurpose(_)
        | svc::AgentConfigError::InvalidSkillName(_)
        | svc::AgentConfigError::InvalidAgentGraph(_) => AppError::Validation(err.to_string()),
        svc::AgentConfigError::DuplicatePurpose(_) => AppError::Conflict(err.to_string()),
        svc::AgentConfigError::NotFound(_) | svc::AgentConfigError::SkillNotFound(_) => {
            AppError::NotFound(err.to_string())
        }
        svc::AgentConfigError::Database(e) => AppError::Database(e),
    }
}

/// 目的別 agent 設定一覧
#[utoipa::path(
    get,
    path = "/api/agent-configs",
    tag = "agent_config",
    responses(
        (status = 200, body = Vec<agent_config::Model>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_agent_configs(
    State(state): State<AppState>,
) -> Result<Json<Vec<agent_config::Model>>, AppError> {
    let items = svc::list(&state.db).await.map_err(map_err)?;
    Ok(Json(items))
}

/// 目的別 agent 設定を作成 (purpose のみ必須、内容は空で作成し後続の PUT で設定する)
#[utoipa::path(
    post,
    path = "/api/agent-configs",
    tag = "agent_config",
    request_body = CreateAgentConfigRequest,
    responses(
        (status = 201, body = agent_config::Model),
        (status = 400, description = "purpose が不正", body = ErrorResponse),
        (status = 409, description = "purpose が既存と衝突", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_agent_config(
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<CreateAgentConfigRequest>,
) -> Result<(StatusCode, Json<agent_config::Model>), AppError> {
    let created = svc::create(&state.db, payload.purpose)
        .await
        .map_err(map_err)?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// 目的別 agent 設定を取得。
/// `strategies::get_agent_config` (戦略 ID キー、`AgentConfigResponse` を返す) とは
/// 別 API。こちらは purpose キーで `agent_config` テーブルの行をそのまま返す。
#[utoipa::path(
    get,
    operation_id = "agent_config_get_agent_config",
    path = "/api/agent-configs/{purpose}",
    tag = "agent_config",
    params(("purpose" = String, Path, description = "目的キー")),
    responses(
        (status = 200, body = agent_config::Model),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_agent_config(
    State(state): State<AppState>,
    JsonPath(purpose): JsonPath<String>,
) -> Result<Json<agent_config::Model>, AppError> {
    let model = svc::find_or_404(&state.db, &purpose)
        .await
        .map_err(map_err)?;
    Ok(Json(model))
}

/// 目的別 agent 設定一式 (AGENTS.md / skills / モデル設定) の統合取得。
/// t-rader-agent が purpose 付きタスク実行時に呼び出す。
/// `strategies::get_agent_config` (戦略 ID キー) の purpose キー版。
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
    State(state): State<AppState>,
    JsonPath(purpose): JsonPath<String>,
) -> Result<Json<AgentConfigResponse>, AppError> {
    let row = svc::find_or_404(&state.db, &purpose)
        .await
        .map_err(map_err)?;
    let skills = svc::skills_as_btree(&row);
    Ok(Json(svc::build_agent_config_response(
        row.agents_md,
        skills,
        row.agent_graph,
    )))
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
    State(state): State<AppState>,
    JsonPath(purpose): JsonPath<String>,
) -> Result<StatusCode, AppError> {
    svc::delete(&state.db, &purpose).await.map_err(map_err)?;
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
    State(state): State<AppState>,
    JsonPath(purpose): JsonPath<String>,
) -> Result<Json<AgentsMdBody>, AppError> {
    let row = svc::find_or_404(&state.db, &purpose)
        .await
        .map_err(map_err)?;
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
    State(state): State<AppState>,
    JsonPath(purpose): JsonPath<String>,
    JsonBody(payload): JsonBody<AgentsMdBody>,
) -> Result<Json<AgentsMdBody>, AppError> {
    let content = svc::save_agents_md(&state.db, &purpose, payload.content)
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
    State(state): State<AppState>,
    JsonPath(purpose): JsonPath<String>,
) -> Result<Json<SkillsBody>, AppError> {
    let row = svc::find_or_404(&state.db, &purpose)
        .await
        .map_err(map_err)?;
    Ok(Json(SkillsBody {
        skills: svc::skills_as_btree(&row),
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
    State(state): State<AppState>,
    JsonPath(purpose): JsonPath<String>,
    JsonBody(payload): JsonBody<SkillsBody>,
) -> Result<Json<SkillsBody>, AppError> {
    let updated = svc::put_skills(&state.db, &purpose, payload.skills)
        .await
        .map_err(map_err)?;
    Ok(Json(SkillsBody {
        skills: svc::skills_as_btree(&updated),
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
    State(state): State<AppState>,
    JsonPath((purpose, name)): JsonPath<(String, String)>,
    JsonBody(payload): JsonBody<SkillBody>,
) -> Result<Json<SkillBody>, AppError> {
    svc::put_skill(&state.db, &purpose, &name, payload.content.clone())
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
    State(state): State<AppState>,
    JsonPath((purpose, name)): JsonPath<(String, String)>,
) -> Result<StatusCode, AppError> {
    svc::delete_skill(&state.db, &purpose, &name)
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
    State(state): State<AppState>,
    JsonPath(purpose): JsonPath<String>,
) -> Result<Json<AgentGraphBody>, AppError> {
    let row = svc::find_or_404(&state.db, &purpose)
        .await
        .map_err(map_err)?;
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
    State(state): State<AppState>,
    JsonPath(purpose): JsonPath<String>,
    JsonBody(payload): JsonBody<AgentGraphBody>,
) -> Result<Json<AgentGraphBody>, AppError> {
    let content = svc::save_agent_graph(&state.db, &purpose, &payload.content)
        .await
        .map_err(map_err)?;
    Ok(Json(AgentGraphBody { content }))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{Value, json};
    use sqlx::PgPool;

    use crate::testing::create_test_server;

    fn normalize(mut value: Value) -> Value {
        for key in ["id", "created_at", "updated_at"] {
            if let Some(v) = value.get_mut(key) {
                *v = Value::String(format!("<{key}>"));
            }
        }
        value
    }

    #[sqlx::test(migrations = false)]
    async fn create_and_list_roundtrip(pool: PgPool) {
        let server = create_test_server(pool).await;
        let created = server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "explore" }))
            .await;
        created.assert_status(StatusCode::CREATED);
        assert_eq!(
            normalize(created.json()),
            json!({
                "id": "<id>",
                "purpose": "explore",
                "agents_md": "",
                "skills": {},
                "agent_graph": "",
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
        );

        let list = server.get("/api/agent-configs").await;
        list.assert_status_ok();
        let body: Vec<Value> = list.json();
        assert_eq!(body.len(), 1);
        assert_eq!(body[0]["purpose"], "explore");
    }

    #[sqlx::test(migrations = false)]
    async fn duplicate_purpose_is_409(pool: PgPool) {
        let server = create_test_server(pool).await;
        let body = json!({ "purpose": "explore" });
        server
            .post("/api/agent-configs")
            .json(&body)
            .await
            .assert_status(StatusCode::CREATED);
        let res = server.post("/api/agent-configs").json(&body).await;
        res.assert_status(StatusCode::CONFLICT);
    }

    #[sqlx::test(migrations = false)]
    async fn invalid_purpose_is_400(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "Bad Purpose" }))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn get_nonexistent_agent_config_returns_404(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server.get("/api/agent-configs/missing").await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn delete_agent_config_removes_row(pool: PgPool) {
        let server = create_test_server(pool).await;
        server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "explore" }))
            .await
            .assert_status(StatusCode::CREATED);

        server
            .delete("/api/agent-configs/explore")
            .await
            .assert_status(StatusCode::NO_CONTENT);
        server
            .get("/api/agent-configs/explore")
            .await
            .assert_status(StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn put_then_get_agents_md_round_trips(pool: PgPool) {
        let server = create_test_server(pool).await;
        server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "explore" }))
            .await
            .assert_status(StatusCode::CREATED);

        let body = "# 方針\n慎重に運用する";
        let put = server
            .put("/api/agent-configs/explore/agents-md")
            .json(&json!({ "content": body }))
            .await;
        put.assert_status_ok();
        assert_eq!(put.json::<Value>(), json!({ "content": body }));

        let get = server.get("/api/agent-configs/explore/agents-md").await;
        get.assert_status_ok();
        assert_eq!(get.json::<Value>(), json!({ "content": body }));
    }

    #[sqlx::test(migrations = false)]
    async fn agents_md_get_404_for_unknown_purpose(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server.get("/api/agent-configs/missing/agents-md").await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn single_skill_add_update_delete_lifecycle(pool: PgPool) {
        let server = create_test_server(pool).await;
        server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "explore" }))
            .await
            .assert_status(StatusCode::CREATED);

        server
            .put("/api/agent-configs/explore/skills/scout")
            .json(&json!({ "content": "first" }))
            .await
            .assert_status_ok();
        server
            .put("/api/agent-configs/explore/skills/scout")
            .json(&json!({ "content": "second" }))
            .await
            .assert_status_ok();
        server
            .put("/api/agent-configs/explore/skills/review")
            .json(&json!({ "content": "rev" }))
            .await
            .assert_status_ok();

        let after_adds = server.get("/api/agent-configs/explore/skills").await;
        assert_eq!(
            after_adds.json::<Value>(),
            json!({ "skills": { "scout": "second", "review": "rev" } }),
        );

        server
            .delete("/api/agent-configs/explore/skills/scout")
            .await
            .assert_status(StatusCode::NO_CONTENT);

        let after_del = server.get("/api/agent-configs/explore/skills").await;
        assert_eq!(
            after_del.json::<Value>(),
            json!({ "skills": { "review": "rev" } }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn delete_unknown_skill_returns_404(pool: PgPool) {
        let server = create_test_server(pool).await;
        server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "explore" }))
            .await
            .assert_status(StatusCode::CREATED);
        let res = server
            .delete("/api/agent-configs/explore/skills/missing")
            .await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn put_then_get_agent_graph_round_trips(pool: PgPool) {
        let server = create_test_server(pool).await;
        server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "explore" }))
            .await
            .assert_status(StatusCode::CREATED);

        let yaml = indoc::indoc! {"
            phases:
              - key: plan
                label: 調査計画
                model: claude-opus-4
                prompt: 仮説を立てよ
        "};
        let put = server
            .put("/api/agent-configs/explore/agent-graph")
            .json(&json!({ "content": yaml }))
            .await;
        put.assert_status_ok();
        assert_eq!(put.json::<Value>(), json!({ "content": yaml }));

        let get = server.get("/api/agent-configs/explore/agent-graph").await;
        get.assert_status_ok();
        assert_eq!(get.json::<Value>(), json!({ "content": yaml }));
    }

    #[sqlx::test(migrations = false)]
    async fn put_agent_graph_rejects_invalid_yaml(pool: PgPool) {
        let server = create_test_server(pool).await;
        server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "explore" }))
            .await
            .assert_status(StatusCode::CREATED);

        let res = server
            .put("/api/agent-configs/explore/agent-graph")
            .json(&json!({ "content": "phases: [" }))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn get_agent_config_bundle_returns_agents_md_skills_and_model(pool: PgPool) {
        use crate::services::agent_config::{DEFAULT_AGENT_MODEL, DEFAULT_AGENT_SMALL_MODEL};

        let server = create_test_server(pool).await;
        server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "explore" }))
            .await
            .assert_status(StatusCode::CREATED);

        let agents_md = indoc::indoc! {"
            # 方針
            慎重に運用する"};
        server
            .put("/api/agent-configs/explore/agents-md")
            .json(&json!({ "content": agents_md }))
            .await
            .assert_status_ok();
        server
            .put("/api/agent-configs/explore/skills/scout")
            .json(&json!({ "content": "scout body" }))
            .await
            .assert_status_ok();

        let res = server.get("/api/agent-configs/explore/agent-config").await;
        res.assert_status_ok();
        assert_eq!(
            res.json::<Value>(),
            json!({
                "agents_md": agents_md,
                "skills": { "scout": "scout body" },
                "model": DEFAULT_AGENT_MODEL,
                "small_model": DEFAULT_AGENT_SMALL_MODEL,
                "agent_graph": "",
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn get_agent_config_bundle_404_for_unknown_purpose(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server.get("/api/agent-configs/missing/agent-config").await;
        res.assert_status(StatusCode::NOT_FOUND);
    }
}
