//! 管理 MCP の trigger 書き込み tool (create/update/delete)。
//!
//! REST (`backend/src/handlers/triggers.rs`) と同じ application use case を呼び出す。

use rmcp::ErrorData as McpError;

use core_application::trigger::{
    CreateTriggerCommand, TriggerKind, TriggerUseCaseError, UpdateTriggerCommand,
};

use super::dto::{
    CreateStrategyTriggerParams, CreateStrategyTriggerResult, DeleteStrategyTriggerParams,
    DeleteStrategyTriggerResult, UpdateStrategyTriggerParams, UpdateStrategyTriggerResult,
};
use super::{MgmtServer, map_trigger_error};

impl MgmtServer {
    pub(super) async fn create_strategy_trigger_inner(
        &self,
        params: CreateStrategyTriggerParams,
    ) -> Result<CreateStrategyTriggerResult, McpError> {
        let scope = self.strategy_scope(params.strategy_id).await?;
        let command = CreateTriggerCommand {
            purpose: params.purpose,
            kind: match params.kind {
                super::dto::TriggerKindParam::Cron => TriggerKind::Cron,
                super::dto::TriggerKindParam::Hook => TriggerKind::Hook,
            },
            schedule: params.schedule,
            hook_slug: params.hook_slug,
            event_match: params.event_match.map(Into::into),
            prompt_template: params.prompt_template,
            enabled: params.enabled,
        };
        match self.use_cases.triggers().create(scope, command).await {
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
        let current = self
            .use_cases
            .triggers()
            .get(params.trigger_id)
            .await
            .map_err(map_trigger_error)?;
        let strategy_id = current.strategy_id.ok_or_else(|| {
            super::invalid_params(format!("trigger {} not found", params.trigger_id))
        })?;
        let scope = self.strategy_scope(strategy_id).await?;
        let command = UpdateTriggerCommand {
            purpose: params.purpose,
            schedule: params.schedule,
            hook_slug: params.hook_slug,
            event_match: params.event_match.map(|value| Some(value.into())),
            prompt_template: params.prompt_template,
            enabled: params.enabled,
        };
        match self
            .use_cases
            .triggers()
            .update(scope, params.trigger_id, command)
            .await
        {
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
        let current = self
            .use_cases
            .triggers()
            .get(params.trigger_id)
            .await
            .map_err(map_trigger_error)?;
        let strategy_id = current.strategy_id.ok_or_else(|| {
            super::invalid_params(format!("trigger {} not found", params.trigger_id))
        })?;
        let scope = self.strategy_scope(strategy_id).await?;
        match self
            .use_cases
            .triggers()
            .delete(scope, params.trigger_id)
            .await
        {
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
fn validation_errors(err: TriggerUseCaseError) -> Result<Vec<String>, McpError> {
    match err {
        TriggerUseCaseError::Validation(msg) => Ok(vec![msg]),
        TriggerUseCaseError::PurposeNotFound(purpose) => {
            Ok(vec![format!("agent_config purpose {purpose} not found")])
        }
        other => Err(map_trigger_error(other)),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rmcp::handler::server::wrapper::{Json, Parameters};
    use uuid::Uuid;

    use crate::agent_client::FakeAgentTaskClient;
    use crate::mcp::mgmt::dto::{TriggerKindParam, TriggerSummary};
    use crate::mcp::strategy::tests_common::ts_sentinel;
    use crate::testing::{insert_test_cron_trigger, insert_test_hook_trigger};

    use super::super::tests_common::{build_server, insert_strategy};
    use super::*;

    fn normalize_trigger_summaries(mut summaries: Vec<TriggerSummary>) -> Vec<TriggerSummary> {
        for summary in &mut summaries {
            summary.created_at = ts_sentinel();
            summary.updated_at = ts_sentinel();
        }
        summaries
    }

    #[backend_test_macros::database_test]
    async fn create_strategy_trigger_inserts_cron_trigger(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "s").await;
        crate::testing::agent_config::create(&db, "synthetic-purpose".to_string())
            .await
            .expect("insert test agent_config");
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .create_strategy_trigger(Parameters(CreateStrategyTriggerParams {
                strategy_id,
                purpose: Some("synthetic-purpose".to_string()),
                kind: TriggerKindParam::Cron,
                schedule: Some("0 9 * * *".to_string()),
                hook_slug: None,
                event_match: None,
                prompt_template: "synthetic prompt".to_string(),
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

        let stored = server
            .use_cases
            .triggers()
            .get(trigger_id)
            .await
            .expect("trigger persisted");
        assert_eq!(
            (
                stored.strategy_id,
                stored.purpose,
                stored.kind.as_str().to_string(),
                stored.schedule,
                stored.hook_slug,
                stored.prompt_template,
                stored.enabled,
            ),
            (
                Some(strategy_id),
                Some("synthetic-purpose".to_string()),
                "cron".to_string(),
                Some("0 9 * * *".to_string()),
                None,
                "synthetic prompt".to_string(),
                true,
            ),
        );

        let Json(config) = server
            .get_strategy_config(Parameters(super::super::dto::GetStrategyConfigParams {
                strategy_id,
            }))
            .await
            .expect("strategy config is available");
        assert_eq!(
            normalize_trigger_summaries(config.triggers),
            vec![TriggerSummary {
                trigger_id,
                purpose: Some("synthetic-purpose".to_string()),
                kind: "cron".to_string(),
                schedule: Some("0 9 * * *".to_string()),
                hook_slug: None,
                event_match: None,
                prompt_template: "synthetic prompt".to_string(),
                enabled: true,
                last_fired_at: None,
                created_at: ts_sentinel(),
                updated_at: ts_sentinel(),
            }],
        );
    }

    #[backend_test_macros::database_test]
    async fn create_strategy_trigger_rejects_cron_without_schedule(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "s").await;
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .create_strategy_trigger(Parameters(CreateStrategyTriggerParams {
                strategy_id,
                purpose: None,
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
            serde_json::to_value(result).unwrap(),
            serde_json::json!({
                "ok": false,
                "errors": ["schedule is required for kind=cron"],
            }),
        );

        let Json(config) = server
            .get_strategy_config(Parameters(super::super::dto::GetStrategyConfigParams {
                strategy_id,
            }))
            .await
            .expect("strategy config is available");
        assert_eq!(
            serde_json::to_value(config.triggers).unwrap(),
            serde_json::json!([]),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_strategy_trigger_rejects_unknown_strategy(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let err = server
            .create_strategy_trigger(Parameters(CreateStrategyTriggerParams {
                strategy_id: Uuid::new_v4(),
                purpose: None,
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

    #[backend_test_macros::database_test]
    async fn update_strategy_trigger_applies_fields(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "s").await;
        crate::testing::agent_config::create(&db, "synthetic-purpose".to_string())
            .await
            .expect("insert test agent_config");
        let trigger_id = insert_test_cron_trigger(
            &db,
            strategy_id,
            "0 9 * * *",
            true,
            None,
            "old prompt",
            None,
        )
        .await;
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .update_strategy_trigger(Parameters(UpdateStrategyTriggerParams {
                trigger_id,
                purpose: Some(Some("synthetic-purpose".to_string())),
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

        let stored = server
            .use_cases
            .triggers()
            .get(trigger_id)
            .await
            .expect("trigger exists");
        assert_eq!(
            (
                stored.purpose,
                stored.schedule,
                stored.prompt_template,
                stored.enabled,
            ),
            (
                Some("synthetic-purpose".to_string()),
                Some("0 10 * * *".to_string()),
                "new prompt".to_string(),
                false,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_strategy_trigger_rejects_hook_slug_on_cron_trigger(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "s").await;
        let trigger_id =
            insert_test_cron_trigger(&db, strategy_id, "0 9 * * *", true, None, "prompt", None)
                .await;
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .update_strategy_trigger(Parameters(UpdateStrategyTriggerParams {
                trigger_id,
                purpose: None,
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

    #[backend_test_macros::database_test]
    async fn update_strategy_trigger_rejects_unknown_trigger(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let err = server
            .update_strategy_trigger(Parameters(UpdateStrategyTriggerParams {
                trigger_id: Uuid::new_v4(),
                purpose: None,
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

    #[backend_test_macros::database_test]
    async fn delete_strategy_trigger_removes_row(db: gateway_postgres::DatabaseHandle) {
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

        assert!(server.use_cases.triggers().get(trigger_id).await.is_err());
    }

    #[backend_test_macros::database_test]
    async fn delete_strategy_trigger_rejects_unknown_trigger(db: gateway_postgres::DatabaseHandle) {
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
