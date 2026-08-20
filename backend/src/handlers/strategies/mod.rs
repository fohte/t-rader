use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::AppState;
use crate::entities::{strategy, strategy_interest};
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{
    AgentConfigResponse, AgentsMdBody, CreateStrategyRequest, SkillBody, SkillsBody,
    UpdateStrategyRequest,
};
use crate::services::change_history::Actor;
use crate::services::strategy_config;

mod agent_graph;
mod tasks;

pub use agent_graph::{
    __path_get_agent_graph, __path_put_agent_graph, get_agent_graph, put_agent_graph,
};
pub(crate) use tasks::map_submit_error;
pub use tasks::{
    __path_get_strategy_task, __path_list_strategy_tasks, __path_submit_strategy_chat,
    get_strategy_task, list_strategy_tasks, submit_strategy_chat,
};

pub(super) use strategy_config::find_or_404 as find_strategy_or_404;

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
    let created = strategy_config::create(
        &state.db,
        Actor::Human,
        strategy_config::CreateStrategy {
            name: payload.name,
            description: payload.description,
            sort_order: payload.sort_order.unwrap_or(0),
            agents_md: None,
            skills: None,
            agent_graph: None,
        },
    )
    .await?;

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
    let updated = strategy_config::update(
        &state.db,
        Actor::Human,
        id,
        strategy_config::StrategyUpdate {
            name: payload.name,
            description: payload.description,
            sort_order: payload.sort_order,
            ..Default::default()
        },
    )
    .await?;

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
    find_strategy_or_404(&state.db, id).await?;
    strategy_config::delete(&state.db, Actor::Human, id).await?;
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
    let content =
        strategy_config::save_agents_md(&state.db, Actor::Human, id, payload.content).await?;
    Ok(Json(AgentsMdBody { content }))
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
        skills: strategy_config::skills_to_btree(&row.skills),
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
        strategy_config::validate_skill_name(name).map_err(|err| match err {
            AppError::Validation(msg) => AppError::Validation(format!("skill {name:?}: {msg}")),
            other => other,
        })?;
    }
    let current = strategy_config::find_or_404(&state.db, id).await?;
    let mut map = serde_json::Map::new();
    for (k, v) in &payload.skills {
        map.insert(k.clone(), serde_json::Value::String(v.clone()));
    }
    let updated = strategy_config::save_skills(
        &state.db,
        Actor::Human,
        current,
        serde_json::Value::Object(map),
        "replaced all skills".to_string(),
    )
    .await?;
    Ok(Json(SkillsBody {
        skills: strategy_config::skills_to_btree(&updated.skills),
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
    strategy_config::validate_skill_name(&name)?;
    let current = strategy_config::find_or_404(&state.db, id).await?;
    let mut patch = serde_json::Map::new();
    patch.insert(
        name.clone(),
        serde_json::Value::String(payload.content.clone()),
    );
    let merged = strategy_config::apply_skills_patch(&current.skills, patch);
    strategy_config::save_skills(
        &state.db,
        Actor::Human,
        current,
        serde_json::Value::Object(merged),
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
    strategy_config::validate_skill_name(&name)?;
    strategy_config::delete_skill(&state.db, Actor::Human, id, &name).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) const DEFAULT_AGENT_MODEL: &str = "opencode-go/minimax-m3";
pub(crate) const DEFAULT_AGENT_SMALL_MODEL: &str = "opencode-go/deepseek-v4-flash";

/// モデル設定は DB ではなく env 由来。
fn agent_model_settings() -> (String, String) {
    agent_model_settings_with(|key| std::env::var(key).ok())
}

fn agent_model_settings_with<F>(get: F) -> (String, String)
where
    F: Fn(&str) -> Option<String>,
{
    let model = get("STRATEGY_AGENT_MODEL")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_AGENT_MODEL.to_string());
    let small_model = get("STRATEGY_AGENT_SMALL_MODEL")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_AGENT_SMALL_MODEL.to_string());
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
        skills: strategy_config::skills_to_btree(&row.skills),
        model,
        small_model,
        agent_graph: row.agent_graph,
    }))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use serde_json::json;
    use sqlx::PgPool;

    use uuid::Uuid;

    use super::{DEFAULT_AGENT_MODEL, DEFAULT_AGENT_SMALL_MODEL, agent_model_settings_with};
    use crate::entities::strategy;
    use crate::testing::{create_strategy, create_test_server, create_test_server_with_db};

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
    async fn delete_strategy_removes_row(pool: PgPool) {
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "to-delete").await;

        let deleted = server.delete(&format!("/api/strategies/{id}")).await;
        deleted.assert_status(axum::http::StatusCode::NO_CONTENT);

        let get = server.get(&format!("/api/strategies/{id}")).await;
        get.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn put_then_get_agents_md_round_trips(pool: PgPool) {
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "s").await;

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
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "s").await;

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
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "s").await;

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
    }

    #[sqlx::test(migrations = false)]
    async fn delete_unknown_skill_returns_404(pool: PgPool) {
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "s").await;

        let res = server
            .delete(&format!("/api/strategies/{id}/skills/missing"))
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn invalid_skill_name_returns_400(pool: PgPool) {
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "s").await;

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
    #[case::unset(&[], (DEFAULT_AGENT_MODEL, DEFAULT_AGENT_SMALL_MODEL))]
    #[case::empty(
        &[("STRATEGY_AGENT_MODEL", ""), ("STRATEGY_AGENT_SMALL_MODEL", "")],
        (DEFAULT_AGENT_MODEL, DEFAULT_AGENT_SMALL_MODEL)
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
            agent_graph: NotSet,
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
                "model": DEFAULT_AGENT_MODEL,
                "small_model": DEFAULT_AGENT_SMALL_MODEL,
                "agent_graph": "",
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
