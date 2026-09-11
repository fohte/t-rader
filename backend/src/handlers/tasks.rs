use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonQuery;
use crate::models::StrategyTaskSummary;
use crate::services::strategy_tasks::{self, phase_str};

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListTasksQuery {
    pub strategy_id: Option<Uuid>,
    pub purpose: Option<String>,
}

/// 戦略タスクの実行履歴を口座横断で新しい順に一覧取得する。`strategy_id`/`purpose` で絞り込める。
#[utoipa::path(
    get,
    path = "/api/tasks",
    tag = "tasks",
    params(ListTasksQuery),
    responses(
        (status = 200, body = Vec<StrategyTaskSummary>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_tasks(
    State(state): State<AppState>,
    JsonQuery(p): JsonQuery<ListTasksQuery>,
) -> Result<Json<Vec<StrategyTaskSummary>>, AppError> {
    let views = strategy_tasks::list_tasks(&state.db, p.strategy_id, p.purpose)
        .await
        .map_err(AppError::Database)?;
    Ok(Json(
        views
            .into_iter()
            .map(|view| StrategyTaskSummary {
                task_id: view.task_id,
                strategy_id: view.strategy_id,
                source: view.source,
                prompt: view.prompt,
                phase: phase_str(&view.phase).to_string(),
                error_summary: view.error_summary,
                created_at: view.created_at,
                updated_at: view.updated_at,
                purpose: view.purpose,
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sqlx::PgPool;

    use crate::testing::{
        create_test_server, create_test_server_with_db, insert_test_strategy,
        insert_test_strategy_task,
    };

    /// JSON body から動的フィールド (created_at/updated_at) を除去し、
    /// 単一の assert_eq! で残りのフィールドを比較できるようにする。
    fn strip_timestamps(v: &mut serde_json::Value) {
        if let Some(obj) = v.as_object_mut() {
            obj.remove("created_at");
            obj.remove("updated_at");
        }
    }

    #[sqlx::test(migrations = false)]
    async fn list_tasks_returns_all_strategies_newest_first(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let strategy_a = insert_test_strategy(&db, "a").await;
        let strategy_b = insert_test_strategy(&db, "b").await;

        let base = chrono::Utc::now().fixed_offset();
        let task_a = insert_test_strategy_task(&db, strategy_a, "for-a", None, base).await;
        let task_b = insert_test_strategy_task(
            &db,
            strategy_b,
            "for-b",
            None,
            base + chrono::Duration::seconds(1),
        )
        .await;

        let res = server.get("/api/tasks").await;
        res.assert_status_ok();
        let mut body: Vec<serde_json::Value> = res.json();
        body.iter_mut().for_each(strip_timestamps);
        assert_eq!(
            body,
            vec![
                json!({
                    "task_id": task_b,
                    "strategy_id": strategy_b,
                    "source": "frontend",
                    "prompt": "for-b",
                    "phase": "completed",
                    "error_summary": null,
                    "purpose": null,
                }),
                json!({
                    "task_id": task_a,
                    "strategy_id": strategy_a,
                    "source": "frontend",
                    "prompt": "for-a",
                    "phase": "completed",
                    "error_summary": null,
                    "purpose": null,
                }),
            ],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn list_tasks_filters_by_strategy_id(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let strategy_a = insert_test_strategy(&db, "a").await;
        let strategy_b = insert_test_strategy(&db, "b").await;

        let base = chrono::Utc::now().fixed_offset();
        let task_a = insert_test_strategy_task(&db, strategy_a, "for-a", None, base).await;
        insert_test_strategy_task(&db, strategy_b, "for-b", None, base).await;

        let res = server
            .get(&format!("/api/tasks?strategy_id={strategy_a}"))
            .await;
        res.assert_status_ok();
        let mut body: Vec<serde_json::Value> = res.json();
        body.iter_mut().for_each(strip_timestamps);
        assert_eq!(
            body,
            vec![json!({
                "task_id": task_a,
                "strategy_id": strategy_a,
                "source": "frontend",
                "prompt": "for-a",
                "phase": "completed",
                "error_summary": null,
                "purpose": null,
            })],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn list_tasks_filters_by_purpose(pool: PgPool) {
        let (db, server) = create_test_server_with_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "x").await;

        let base = chrono::Utc::now().fixed_offset();
        let task_inspect =
            insert_test_strategy_task(&db, strategy_id, "inspect prompt", Some("inspect"), base)
                .await;
        insert_test_strategy_task(
            &db,
            strategy_id,
            "default prompt",
            None,
            base + chrono::Duration::seconds(1),
        )
        .await;

        let res = server.get("/api/tasks?purpose=inspect").await;
        res.assert_status_ok();
        let mut body: Vec<serde_json::Value> = res.json();
        body.iter_mut().for_each(strip_timestamps);
        assert_eq!(
            body,
            vec![json!({
                "task_id": task_inspect,
                "strategy_id": strategy_id,
                "source": "frontend",
                "prompt": "inspect prompt",
                "phase": "completed",
                "error_summary": null,
                "purpose": "inspect",
            })],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn list_tasks_returns_empty_for_unknown_strategy_id(pool: PgPool) {
        let server = create_test_server(pool).await;

        let res = server
            .get("/api/tasks?strategy_id=00000000-0000-0000-0000-000000000000")
            .await;
        res.assert_status_ok();
        let body: Vec<serde_json::Value> = res.json();
        assert_eq!(body, Vec::<serde_json::Value>::new());
    }
}
