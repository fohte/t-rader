//! 外部 hook 受信 endpoint。`/api/hooks/:hook_slug` で payload を受け、
//! 対応する `trigger` 行の `event_match` を満たした場合に共通 service 経由で発火する。

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use utoipa::ToSchema;

use crate::AppState;
use crate::entities::trigger;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::services::strategy_tasks::TaskSource;
use crate::services::triggers::{FireTriggerError, evaluate_event_match, fire_trigger};

/// hook 受信レスポンス。
///
/// `fired = true`: trigger に紐づく strategy_task が作成された。
/// `fired = false`: payload が `event_match` を満たさなかったため no-op (200 OK)。
#[derive(Debug, Serialize, ToSchema)]
pub struct HookResponse {
    /// 発火したか
    pub fired: bool,
    /// 発火時のみ。作成された strategy_task の UUID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<uuid::Uuid>,
}

/// hook を受信する。
///
/// - `hook_slug` に一致する有効な trigger が無ければ 404
/// - trigger が `enabled=false` の場合も 404 (外部に存在を漏らさない)
/// - `event_match` を満たさない payload は 200 OK の no-op
/// - 満たした場合は共通 service 経由で `submit_strategy_task` を呼び、200 OK を返す
#[utoipa::path(
    post,
    path = "/api/hooks/{hook_slug}",
    tag = "triggers",
    params(("hook_slug" = String, Path, description = "hook 識別子")),
    request_body = serde_json::Value,
    responses(
        (status = 200, body = HookResponse),
        (status = 400, description = "リクエストボディに null バイトが含まれる等の汎用エラー", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
        (status = 503, description = "agent task client が未設定", body = ErrorResponse),
    )
)]
pub async fn receive_hook(
    State(state): State<AppState>,
    JsonPath(hook_slug): JsonPath<String>,
    JsonBody(payload): JsonBody<serde_json::Value>,
) -> Result<(StatusCode, Json<HookResponse>), AppError> {
    let trigger_row = trigger::Entity::find()
        .filter(trigger::Column::HookSlug.eq(hook_slug.clone()))
        .filter(trigger::Column::Enabled.eq(true))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("hook {hook_slug} not found")))?;

    if !evaluate_event_match(trigger_row.event_match.as_ref(), &payload) {
        tracing::info!(
            trigger_id = %trigger_row.trigger_id,
            hook_slug,
            "hook payload did not match event_match; ignored",
        );
        return Ok((
            StatusCode::OK,
            Json(HookResponse {
                fired: false,
                task_id: None,
            }),
        ));
    }

    let trigger_id = trigger_row.trigger_id;
    match fire_trigger(
        &state.db,
        &state.agent_task_client,
        trigger_id,
        payload,
        TaskSource::Hook,
    )
    .await
    {
        Ok(outcome) => Ok((
            StatusCode::OK,
            Json(HookResponse {
                fired: true,
                task_id: Some(outcome.task_id),
            }),
        )),
        // SELECT と fire の間に削除 / 無効化された race。
        Err(FireTriggerError::TriggerNotFound(_) | FireTriggerError::Disabled(_)) => {
            Err(AppError::NotFound(format!("hook {hook_slug} not found")))
        }
        Err(FireTriggerError::Submit(err)) => {
            Err(crate::handlers::strategies::map_submit_error(err))
        }
        Err(FireTriggerError::Database(err)) => Err(AppError::Database(err)),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::http::StatusCode;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{EntityTrait, QueryOrder};
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::agent_client::{AgentTaskError, FakeAgentTaskClient, SharedAgentTaskClient};
    use crate::entities::sea_orm_active_enums::StrategyTaskPhase;
    use crate::entities::{strategy, strategy_task, trigger};
    use crate::testing::create_test_server_with_db_and_agent_client;

    /// strategy_task 行の動的フィールド (id / 時刻) を捨てた比較用ビュー。
    #[derive(Debug, PartialEq, Eq)]
    struct TaskShape {
        strategy_id: Uuid,
        source: String,
        prompt: String,
        phase: StrategyTaskPhase,
    }

    impl TaskShape {
        fn from(row: &strategy_task::Model) -> Self {
            Self {
                strategy_id: row.strategy_id,
                source: row.source.clone(),
                prompt: row.prompt.clone(),
                phase: row.phase.clone(),
            }
        }
    }

    async fn seed_strategy(db: &sea_orm::DatabaseConnection, name: &str) -> Uuid {
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
        .unwrap();
        id
    }

    async fn seed_hook_trigger(
        db: &sea_orm::DatabaseConnection,
        strategy_id: Uuid,
        slug: &str,
        prompt_template: &str,
        event_match: Option<Value>,
        enabled: bool,
    ) -> Uuid {
        let id = Uuid::new_v4();
        trigger::ActiveModel {
            trigger_id: Set(id),
            strategy_id: Set(strategy_id),
            kind: Set("hook".to_string()),
            schedule: Set(None),
            hook_slug: Set(Some(slug.to_string())),
            event_match: Set(event_match),
            prompt_template: Set(prompt_template.to_string()),
            enabled: Set(enabled),
            last_fired_at: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .unwrap();
        id
    }

    #[sqlx::test(migrations = false)]
    async fn fires_when_event_match_satisfied(pool: PgPool) {
        let kube: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(pool, kube).await;
        let sid = seed_strategy(&db, "長期").await;
        let _ = seed_hook_trigger(
            &db,
            sid,
            "tv-alert",
            "alert {{payload.symbol}} for {{strategy.name}}",
            Some(json!({"event": {"eq": "fired"}})),
            true,
        )
        .await;

        let res = server
            .post("/api/hooks/tv-alert")
            .json(&json!({"event": "fired", "symbol": "7203"}))
            .await;
        res.assert_status_ok();
        // task_id は実行時 UUID なので body をそのまま比較する前に placeholder 化する。
        let mut body: Value = res.json();
        let task_id_str = body["task_id"].as_str().unwrap().to_string();
        let task_id = Uuid::parse_str(&task_id_str).unwrap();
        body["task_id"] = Value::String("<task_id>".to_string());
        let task = strategy_task::Entity::find_by_id(task_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (body, TaskShape::from(&task)),
            (
                json!({ "fired": true, "task_id": "<task_id>" }),
                TaskShape {
                    strategy_id: sid,
                    source: "hook".to_string(),
                    prompt: "alert 7203 for 長期".to_string(),
                    phase: StrategyTaskPhase::Running,
                },
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn skips_when_event_match_not_satisfied(pool: PgPool) {
        let kube: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(pool, kube).await;
        let sid = seed_strategy(&db, "s").await;
        let _ = seed_hook_trigger(
            &db,
            sid,
            "tv-alert",
            "x",
            Some(json!({"event": {"eq": "fired"}})),
            true,
        )
        .await;

        let res = server
            .post("/api/hooks/tv-alert")
            .json(&json!({"event": "ignored"}))
            .await;
        res.assert_status_ok();
        assert_eq!(res.json::<Value>(), json!({"fired": false}));

        let tasks = strategy_task::Entity::find()
            .order_by_asc(strategy_task::Column::CreatedAt)
            .all(&db)
            .await
            .unwrap();
        assert!(tasks.is_empty());
    }

    #[sqlx::test(migrations = false)]
    async fn disabled_trigger_is_404(pool: PgPool) {
        let kube: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(pool, kube).await;
        let sid = seed_strategy(&db, "s").await;
        let _ = seed_hook_trigger(&db, sid, "off", "x", None, false).await;

        let res = server.post("/api/hooks/off").json(&json!({})).await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn unknown_slug_is_404(pool: PgPool) {
        let kube: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (_db, server) = create_test_server_with_db_and_agent_client(pool, kube).await;
        let res = server.post("/api/hooks/nope").json(&json!({})).await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn placeholders_expand_from_payload(pool: PgPool) {
        let kube: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(pool, kube).await;
        let sid = seed_strategy(&db, "s").await;
        let _ = seed_hook_trigger(
            &db,
            sid,
            "tv",
            "sym={{payload.symbol}} price={{payload.price}}",
            None,
            true,
        )
        .await;

        let res = server
            .post("/api/hooks/tv")
            .json(&json!({"symbol": "7203", "price": 2500}))
            .await;
        res.assert_status_ok();

        let tasks = strategy_task::Entity::find()
            .order_by_asc(strategy_task::Column::CreatedAt)
            .all(&db)
            .await
            .unwrap();
        assert_eq!(
            tasks.iter().map(TaskShape::from).collect::<Vec<_>>(),
            vec![TaskShape {
                strategy_id: sid,
                source: "hook".to_string(),
                prompt: "sym=7203 price=2500".to_string(),
                phase: StrategyTaskPhase::Running,
            }],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn agent_not_configured_returns_503(pool: PgPool) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_submit_error(AgentTaskError::NotConfigured).await;
        let agent_client: SharedAgentTaskClient = fake;
        let (db, server) = create_test_server_with_db_and_agent_client(pool, agent_client).await;
        let sid = seed_strategy(&db, "s").await;
        let _ = seed_hook_trigger(&db, sid, "tv-alert", "x", None, true).await;

        let res = server.post("/api/hooks/tv-alert").json(&json!({})).await;
        res.assert_status(StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            res.json::<Value>(),
            json!({ "error": "agent task client is not configured" }),
        );
    }
}
