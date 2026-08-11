use axum::Json;
use axum::extract::State;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;
use sea_orm::{IntoActiveModel, TransactionTrait};
use serde_json::json;
use uuid::Uuid;

use super::find_strategy_or_404;
use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::AgentGraphBody;
use crate::services::agent_graph::{AgentGraphError, parse_agent_graph};
use crate::services::change_history::{self, Op, TargetKind};

fn map_agent_graph_error(err: AgentGraphError) -> AppError {
    AppError::Validation(err.to_string())
}

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
    let row = find_strategy_or_404(&state.db, id).await?;
    Ok(Json(AgentGraphBody {
        content: row.agent_graph,
    }))
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
    parse_agent_graph(&payload.content).map_err(map_agent_graph_error)?;

    let current = find_strategy_or_404(&state.db, id).await?;
    let prev = current.agent_graph.clone();
    let mut active = current.into_active_model();
    active.agent_graph = Set(payload.content.clone());
    active.updated_at = Set(chrono::Utc::now().fixed_offset());

    let txn = state.db.begin().await?;
    let updated = active.update(&txn).await?;
    change_history::record(
        &txn,
        TargetKind::Strategy,
        id,
        Op::Update,
        json!({ "agent_graph": { "from": prev, "to": payload.content } }),
        Some("updated agent_graph".to_string()),
    )
    .await?;
    txn.commit().await?;

    Ok(Json(AgentGraphBody {
        content: updated.agent_graph,
    }))
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
                runs: once
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
