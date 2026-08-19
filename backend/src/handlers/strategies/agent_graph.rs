use axum::Json;
use axum::extract::State;
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::AgentGraphBody;
use crate::services::agent_graph as agent_graph_svc;

/// 戦略 Agent の多段フェーズ実行設定 (YAML) を取得
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/agent-graph",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = AgentGraphBody),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_agent_graph(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<AgentGraphBody>, AppError> {
    let content = agent_graph_svc::get_agent_graph(&state.db, id).await?;
    Ok(Json(AgentGraphBody { content }))
}

/// 戦略 Agent の多段フェーズ実行設定 (YAML) を上書き保存する。
/// パースできない YAML や、`for_each` の参照先が手前のフェーズに実在しない配列である
/// といった不正な設定は 400 で弾く。
#[utoipa::path(
    put,
    path = "/api/strategies/{id}/agent-graph",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = AgentGraphBody,
    responses(
        (status = 200, body = AgentGraphBody),
        (status = 400, description = "YAML が不正、またはフェーズ定義が不正 (キー重複・for_each の参照先不備など)", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn put_agent_graph(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<AgentGraphBody>,
) -> Result<Json<AgentGraphBody>, AppError> {
    let content = agent_graph_svc::save_agent_graph(&state.db, id, &payload.content).await?;
    Ok(Json(AgentGraphBody { content }))
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use crate::testing::{create_strategy, create_test_server};

    #[sqlx::test(migrations = false)]
    async fn agent_graph_defaults_to_empty_content(pool: PgPool) {
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "s").await;

        let get = server
            .get(&format!("/api/strategies/{id}/agent-graph"))
            .await;
        get.assert_status_ok();
        assert_eq!(
            get.json::<serde_json::Value>(),
            serde_json::json!({ "content": "" }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn put_then_get_agent_graph_round_trips(pool: PgPool) {
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "s").await;

        let yaml = indoc::indoc! {"
            phases:
              - key: plan
                label: 調査計画
                model: claude-opus-4
                prompt: 仮説を立てよ
                output:
                  hypotheses:
                    type: array
                    description: 検証すべき仮説
              - key: investigate
                label: 仮説の調査
                model: deepseek-v4-flash
                for_each: plan.hypotheses
                label_field: title
                max_parallel: 4
                prompt: 割り当てられた仮説を検証せよ
        "};

        let put = server
            .put(&format!("/api/strategies/{id}/agent-graph"))
            .json(&serde_json::json!({ "content": yaml }))
            .await;
        put.assert_status_ok();
        assert_eq!(
            put.json::<serde_json::Value>(),
            serde_json::json!({ "content": yaml }),
        );

        let get = server
            .get(&format!("/api/strategies/{id}/agent-graph"))
            .await;
        get.assert_status_ok();
        assert_eq!(
            get.json::<serde_json::Value>(),
            serde_json::json!({ "content": yaml })
        );
    }

    #[sqlx::test(migrations = false)]
    async fn put_agent_graph_rejects_invalid_yaml(pool: PgPool) {
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "s").await;

        let put = server
            .put(&format!("/api/strategies/{id}/agent-graph"))
            .json(&serde_json::json!({ "content": "phases: [" }))
            .await;
        put.assert_status(axum::http::StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn put_agent_graph_rejects_dangling_for_each_reference(pool: PgPool) {
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "s").await;

        let yaml = indoc::indoc! {"
            phases:
              - key: investigate
                label: 仮説の調査
                model: m
                prompt: p
                for_each: plan.hypotheses
        "};
        let put = server
            .put(&format!("/api/strategies/{id}/agent-graph"))
            .json(&serde_json::json!({ "content": yaml }))
            .await;
        put.assert_status(axum::http::StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = false)]
    async fn agent_graph_get_404_for_unknown_strategy(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000/agent-graph")
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }
}
