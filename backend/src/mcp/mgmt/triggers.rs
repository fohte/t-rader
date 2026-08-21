//! 管理 MCP の trigger 書き込み tool (create/update/delete)。
//!
//! REST (`backend/src/handlers/triggers.rs`) と同じ `services::trigger_crud` を経由するため、
//! 検証ロジックは一本化されている。trigger は `change_history` の対象外 (CHECK 制約が
//! `"trigger"` を含まない) なので actor の指定は発生しない。

use rmcp::ErrorData as McpError;

use crate::error::AppError;
use crate::models::{CreateTriggerRequest, UpdateTriggerRequest};
use crate::services::trigger_crud;

use super::dto::{
    CreateStrategyTriggerParams, CreateStrategyTriggerResult, DeleteStrategyTriggerParams,
    DeleteStrategyTriggerResult, UpdateStrategyTriggerParams, UpdateStrategyTriggerResult,
};
use super::{MgmtServer, map_app_error};

impl MgmtServer {
    pub(super) async fn create_strategy_trigger_inner(
        &self,
        params: CreateStrategyTriggerParams,
    ) -> Result<CreateStrategyTriggerResult, McpError> {
        let payload = CreateTriggerRequest {
            kind: params.kind.into(),
            schedule: params.schedule,
            hook_slug: params.hook_slug,
            event_match: params.event_match,
            prompt_template: params.prompt_template,
            enabled: params.enabled,
        };
        match trigger_crud::create_trigger(&self.db, params.strategy_id, payload).await {
            Ok(created) => Ok(CreateStrategyTriggerResult {
                ok: true,
                errors: vec![],
                trigger_id: Some(created.trigger_id),
            }),
            Err(err) => Ok(CreateStrategyTriggerResult {
                ok: false,
                errors: validation_errors(err)?,
                trigger_id: None,
            }),
        }
    }

    pub(super) async fn update_strategy_trigger_inner(
        &self,
        params: UpdateStrategyTriggerParams,
    ) -> Result<UpdateStrategyTriggerResult, McpError> {
        let payload = UpdateTriggerRequest {
            schedule: params.schedule,
            hook_slug: params.hook_slug,
            event_match: params.event_match,
            prompt_template: params.prompt_template,
            enabled: params.enabled,
        };
        match trigger_crud::update_trigger(&self.db, params.trigger_id, payload).await {
            Ok(_) => Ok(UpdateStrategyTriggerResult {
                ok: true,
                errors: vec![],
            }),
            Err(err) => Ok(UpdateStrategyTriggerResult {
                ok: false,
                errors: validation_errors(err)?,
            }),
        }
    }

    pub(super) async fn delete_strategy_trigger_inner(
        &self,
        params: DeleteStrategyTriggerParams,
    ) -> Result<DeleteStrategyTriggerResult, McpError> {
        match trigger_crud::delete_trigger(&self.db, params.trigger_id).await {
            Ok(()) => Ok(DeleteStrategyTriggerResult {
                ok: true,
                errors: vec![],
            }),
            Err(err) => Ok(DeleteStrategyTriggerResult {
                ok: false,
                errors: validation_errors(err)?,
            }),
        }
    }
}

/// `AppError::Validation` (schedule/hook_slug の不整合など) はデータとして返し、LLM が
/// 入力を直して再試行できるようにする。`NotFound` (存在しない strategy_id/trigger_id) や
/// `Database` (hook_slug の unique 制約違反など) は参照ミスや DB 制約違反であり content の
/// 修正だけでは直せないため、他の書き込み tool と同様に tool call そのものを失敗させる。
fn validation_errors(err: AppError) -> Result<Vec<String>, McpError> {
    match err {
        AppError::Validation(msg) => Ok(vec![msg]),
        other => Err(map_app_error(other)),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rmcp::handler::server::wrapper::{Json, Parameters};
    use sea_orm::EntityTrait;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::agent_client::FakeAgentTaskClient;
    use crate::entities::trigger;
    use crate::mcp::mgmt::dto::TriggerKindParam;
    use crate::testing::{create_test_db, insert_test_cron_trigger, insert_test_hook_trigger};

    use super::super::tests_common::{build_server, insert_strategy};
    use super::*;

    #[sqlx::test(migrations = false)]
    async fn create_strategy_trigger_inserts_cron_trigger(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "s").await;
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .create_strategy_trigger(Parameters(CreateStrategyTriggerParams {
                strategy_id,
                kind: TriggerKindParam::Cron,
                schedule: Some("0 9 * * *".to_string()),
                hook_slug: None,
                event_match: None,
                prompt_template: "朝の市況を要約せよ".to_string(),
                enabled: None,
            }))
            .await
            .expect("ok");
        let trigger_id = result.trigger_id.expect("trigger_id present");
        assert_eq!(
            serde_json::to_value(CreateStrategyTriggerResult {
                trigger_id: Some(Uuid::nil()),
                ..result
            })
            .unwrap(),
            serde_json::to_value(CreateStrategyTriggerResult {
                ok: true,
                errors: vec![],
                trigger_id: Some(Uuid::nil()),
            })
            .unwrap(),
        );

        let stored = trigger_crud::get_trigger(&db, trigger_id)
            .await
            .expect("trigger persisted");
        assert_eq!(
            (
                stored.strategy_id,
                stored.kind,
                stored.schedule,
                stored.hook_slug,
                stored.prompt_template,
                stored.enabled,
            ),
            (
                strategy_id,
                "cron".to_string(),
                Some("0 9 * * *".to_string()),
                None,
                "朝の市況を要約せよ".to_string(),
                true,
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_strategy_trigger_rejects_cron_without_schedule(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "s").await;
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .create_strategy_trigger(Parameters(CreateStrategyTriggerParams {
                strategy_id,
                kind: TriggerKindParam::Cron,
                schedule: None,
                hook_slug: None,
                event_match: None,
                prompt_template: "prompt".to_string(),
                enabled: None,
            }))
            .await
            .expect("tool call itself must succeed");
        assert_eq!(
            (result.ok, result.errors.len(), result.trigger_id),
            (false, 1, None),
        );

        let rows = trigger::Entity::find().all(&db).await.unwrap();
        assert!(rows.is_empty());
    }

    #[sqlx::test(migrations = false)]
    async fn create_strategy_trigger_rejects_unknown_strategy(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let err = server
            .create_strategy_trigger(Parameters(CreateStrategyTriggerParams {
                strategy_id: Uuid::new_v4(),
                kind: TriggerKindParam::Hook,
                schedule: None,
                hook_slug: Some("earnings".to_string()),
                event_match: None,
                prompt_template: "prompt".to_string(),
                enabled: None,
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn update_strategy_trigger_applies_fields(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "s").await;
        let trigger_id =
            insert_test_cron_trigger(&db, strategy_id, "0 9 * * *", true, None, "old prompt").await;
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .update_strategy_trigger(Parameters(UpdateStrategyTriggerParams {
                trigger_id,
                schedule: Some("0 10 * * *".to_string()),
                hook_slug: None,
                event_match: None,
                prompt_template: Some("new prompt".to_string()),
                enabled: Some(false),
            }))
            .await
            .expect("ok");
        assert_eq!(
            (result.ok, result.errors.clone()),
            (true, Vec::<String>::new()),
        );

        let stored = trigger_crud::get_trigger(&db, trigger_id)
            .await
            .expect("trigger exists");
        assert_eq!(
            (stored.schedule, stored.prompt_template, stored.enabled),
            (
                Some("0 10 * * *".to_string()),
                "new prompt".to_string(),
                false,
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn update_strategy_trigger_rejects_hook_slug_on_cron_trigger(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "s").await;
        let trigger_id =
            insert_test_cron_trigger(&db, strategy_id, "0 9 * * *", true, None, "prompt").await;
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .update_strategy_trigger(Parameters(UpdateStrategyTriggerParams {
                trigger_id,
                schedule: None,
                hook_slug: Some("earnings".to_string()),
                event_match: None,
                prompt_template: None,
                enabled: None,
            }))
            .await
            .expect("tool call itself must succeed");
        assert_eq!((result.ok, result.errors.len()), (false, 1));
    }

    #[sqlx::test(migrations = false)]
    async fn update_strategy_trigger_rejects_unknown_trigger(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let err = server
            .update_strategy_trigger(Parameters(UpdateStrategyTriggerParams {
                trigger_id: Uuid::new_v4(),
                schedule: None,
                hook_slug: None,
                event_match: None,
                prompt_template: Some("prompt".to_string()),
                enabled: None,
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn delete_strategy_trigger_removes_row(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "s").await;
        let trigger_id =
            insert_test_hook_trigger(&db, strategy_id, "earnings", "prompt", None, true).await;
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .delete_strategy_trigger(Parameters(DeleteStrategyTriggerParams { trigger_id }))
            .await
            .expect("ok");
        assert_eq!(
            (result.ok, result.errors.clone()),
            (true, Vec::<String>::new()),
        );

        assert!(trigger_crud::get_trigger(&db, trigger_id).await.is_err());
    }

    #[sqlx::test(migrations = false)]
    async fn delete_strategy_trigger_rejects_unknown_trigger(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let err = server
            .delete_strategy_trigger(Parameters(DeleteStrategyTriggerParams {
                trigger_id: Uuid::new_v4(),
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}
