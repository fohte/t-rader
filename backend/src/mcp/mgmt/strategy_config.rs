//! 管理 MCP の戦略設定 (name / description) の取得・作成・更新・削除と、
//! 戦略に紐づく trigger の一覧取得 (読み取り専用) の tool。
//!
//! HTTP handler と同じ application usecase を呼び出す。

use rmcp::ErrorData as McpError;

use core_application::change_history::Actor;
use core_application::strategy::{
    CreateStrategyCommand, StrategyUpdateCommand, StrategyUseCaseError, validate_name,
};

use super::dto::{
    CreateStrategyParams, CreateStrategyResult, DeleteStrategyParams, DeleteStrategyResult,
    GetStrategyConfigParams, GetStrategyConfigResult, TriggerSummary, UpdateStrategyConfigParams,
    UpdateStrategyConfigResult,
};
use super::{MgmtServer, map_strategy_use_case_error, map_trigger_error};

impl MgmtServer {
    pub(super) async fn get_strategy_config_inner(
        &self,
        params: GetStrategyConfigParams,
    ) -> Result<GetStrategyConfigResult, McpError> {
        let scope = self.strategy_scope(params.strategy_id).await?;
        let row = self
            .use_cases
            .strategies
            .get(scope)
            .await
            .map_err(map_strategy_use_case_error)?;
        let triggers = self
            .use_cases
            .triggers
            .list_for_strategy(scope, None)
            .await
            .map_err(map_trigger_error)?;
        Ok(GetStrategyConfigResult {
            strategy_id: row.id,
            name: row.name,
            description: row.description,
            triggers: triggers.into_iter().map(TriggerSummary::from).collect(),
        })
    }

    pub(super) async fn create_strategy_inner(
        &self,
        params: CreateStrategyParams,
    ) -> Result<CreateStrategyResult, McpError> {
        let mut errors = Vec::new();

        if let Err(err) = validate_name(&params.name) {
            errors.push(validation_message(err));
        }

        if !errors.is_empty() {
            return Ok(CreateStrategyResult {
                ok: false,
                errors,
                strategy_id: None,
            });
        }

        let created = self
            .use_cases
            .strategies
            .create(
                Actor::Llm { label: "mgmt-mcp" },
                CreateStrategyCommand {
                    name: params.name,
                    description: params.description,
                    sort_order: 0,
                },
            )
            .await
            .map_err(map_strategy_use_case_error)?;

        Ok(CreateStrategyResult {
            ok: true,
            errors: vec![],
            strategy_id: Some(created.id),
        })
    }

    pub(super) async fn update_strategy_config_inner(
        &self,
        params: UpdateStrategyConfigParams,
    ) -> Result<UpdateStrategyConfigResult, McpError> {
        let mut errors = Vec::new();

        if let Some(name) = &params.name
            && let Err(err) = validate_name(name)
        {
            errors.push(validation_message(err));
        }

        if !errors.is_empty() {
            return Ok(UpdateStrategyConfigResult { ok: false, errors });
        }

        let scope = self.strategy_scope(params.strategy_id).await?;
        self.use_cases
            .strategies
            .update(
                Actor::Llm { label: "mgmt-mcp" },
                scope,
                StrategyUpdateCommand {
                    name: params.name,
                    description: params.description.map(Some),
                    sort_order: None,
                },
            )
            .await
            .map_err(map_strategy_use_case_error)?;

        Ok(UpdateStrategyConfigResult {
            ok: true,
            errors: vec![],
        })
    }

    pub(super) async fn delete_strategy_inner(
        &self,
        params: DeleteStrategyParams,
    ) -> Result<DeleteStrategyResult, McpError> {
        let scope = self.strategy_scope(params.strategy_id).await?;
        let current = self
            .use_cases
            .strategies
            .get(scope)
            .await
            .map_err(map_strategy_use_case_error)?;
        if params.confirm_name != current.name {
            return Ok(DeleteStrategyResult {
                ok: false,
                errors: vec![format!(
                    "confirm_name {:?} does not match strategy name {:?}",
                    params.confirm_name, current.name
                )],
            });
        }
        self.use_cases
            .strategies
            .delete_confirmed(
                Actor::Llm { label: "mgmt-mcp" },
                scope,
                &params.confirm_name,
            )
            .await
            .map_err(map_strategy_use_case_error)?;
        Ok(DeleteStrategyResult {
            ok: true,
            errors: vec![],
        })
    }
}

/// 入力エラーならメッセージ本文だけを、それ以外なら Display 文字列を返す。
/// validate_name は Validation 以外を返さない実装だが、将来変わってもエラーメッセージを
/// 取りこぼさないためのフォールバック。
fn validation_message(err: StrategyUseCaseError) -> String {
    match err {
        StrategyUseCaseError::Validation(msg) => msg,
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rmcp::handler::server::wrapper::{Json, Parameters};
    use sea_orm::EntityTrait;
    use serde_json::json;
    use uuid::Uuid;

    use crate::agent_client::FakeAgentTaskClient;
    use crate::testing::insert_test_cron_trigger;
    use core_application::change_history::Actor;
    use core_application::strategy::{StrategyUpdateCommand, StrategyUseCaseError};
    use core_application::strategy_scope::StrategyScope;
    use gateway_postgres::PostgresStrategyScopeSource;
    use gateway_postgres::entities::strategy;

    use super::super::tests_common::{build_server, insert_strategy};
    use super::*;

    #[backend_test_macros::database_test]
    async fn get_strategy_config_returns_full_row_and_empty_triggers(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "s").await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .get_strategy_config(Parameters(GetStrategyConfigParams { strategy_id }))
            .await
            .expect("ok");

        assert_eq!(
            serde_json::to_value(result).unwrap(),
            json!({
                "strategy_id": strategy_id,
                "name": "s",
                "description": null,
                "triggers": [],
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_strategy_config_includes_triggers(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "s").await;
        let trigger_id =
            insert_test_cron_trigger(&db, strategy_id, "0 9 * * *", true, None, "prompt").await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .get_strategy_config(Parameters(GetStrategyConfigParams { strategy_id }))
            .await
            .expect("ok");

        assert_eq!(
            (
                result.triggers.len(),
                result.triggers.first().map(|t| t.trigger_id)
            ),
            (1, Some(trigger_id)),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_strategy_config_rejects_unknown_strategy(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let err = server
            .get_strategy_config(Parameters(GetStrategyConfigParams {
                strategy_id: Uuid::new_v4(),
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn create_strategy_persists_name_and_description(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .create_strategy(Parameters(CreateStrategyParams {
                name: "s".to_string(),
                description: Some("desc".to_string()),
            }))
            .await
            .expect("ok");
        assert_eq!(
            (result.ok, result.errors.clone()),
            (true, Vec::<String>::new()),
        );
        let strategy_id = result.strategy_id.expect("strategy_id present");

        let Json(fetched) = server
            .get_strategy_config(Parameters(GetStrategyConfigParams { strategy_id }))
            .await
            .expect("ok");
        assert_eq!(
            serde_json::to_value(fetched).unwrap(),
            json!({
                "strategy_id": strategy_id,
                "name": "s",
                "description": "desc",
                "triggers": [],
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_strategy_rejects_invalid_fields_without_writing_anything(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .create_strategy(Parameters(CreateStrategyParams {
                name: "".to_string(),
                description: None,
            }))
            .await
            .expect("tool call itself must succeed");
        assert_eq!(
            (result.ok, result.errors.len(), result.strategy_id),
            (false, 1, None),
        );

        let rows = strategy::Entity::find().all(&db).await.unwrap();
        assert!(rows.is_empty());
    }

    #[backend_test_macros::database_test]
    async fn update_strategy_config_applies_multiple_fields_in_one_call(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "s").await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .update_strategy_config(Parameters(UpdateStrategyConfigParams {
                strategy_id,
                name: Some("renamed".to_string()),
                description: Some("desc".to_string()),
            }))
            .await
            .expect("ok");
        assert_eq!((result.ok, result.errors), (true, Vec::<String>::new()));

        let Json(fetched) = server
            .get_strategy_config(Parameters(GetStrategyConfigParams { strategy_id }))
            .await
            .expect("ok");
        assert_eq!(
            serde_json::to_value(fetched).unwrap(),
            json!({
                "strategy_id": strategy_id,
                "name": "renamed",
                "description": "desc",
                "triggers": [],
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_strategy_config_rejects_invalid_name_without_writing_anything(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "original").await;
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .update_strategy_config(Parameters(UpdateStrategyConfigParams {
                strategy_id,
                name: Some("".to_string()),
                description: Some("desc".to_string()),
            }))
            .await
            .expect("tool call itself must succeed");
        assert!(!result.ok);

        let row = strategy::Entity::find_by_id(strategy_id)
            .one(&db)
            .await
            .expect("query strategy")
            .expect("strategy exists");
        assert_eq!((row.name, row.description), ("original".to_string(), None));
    }

    #[backend_test_macros::database_test]
    async fn delete_strategy_requires_confirm_name_exact_match(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "s").await;
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .delete_strategy(Parameters(DeleteStrategyParams {
                strategy_id,
                confirm_name: "wrong".to_string(),
            }))
            .await
            .expect("tool call itself must succeed");
        assert_eq!((result.ok, result.errors.len()), (false, 1));

        assert!(
            strategy::Entity::find_by_id(strategy_id)
                .one(&db)
                .await
                .expect("query strategy")
                .is_some()
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_confirmed_rejects_name_changed_after_confirmation(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "original").await;
        let scope_source = PostgresStrategyScopeSource::new(&db);
        let scope = StrategyScope::verify(strategy_id, &scope_source)
            .await
            .expect("verify strategy scope");
        let use_cases = crate::services::use_cases::build_use_cases(db.clone());

        use_cases
            .strategies
            .update(
                Actor::Human,
                scope,
                StrategyUpdateCommand {
                    name: Some("renamed".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("rename strategy");
        let delete_result = use_cases
            .strategies
            .delete_confirmed(Actor::Human, scope, "original")
            .await
            .map(|()| "deleted")
            .map_err(|error| match error {
                StrategyUseCaseError::ConfirmationMismatch(_) => {
                    "confirmation mismatch".to_string()
                }
                other => other.to_string(),
            });
        let remaining = strategy::Entity::find_by_id(strategy_id)
            .one(&db)
            .await
            .expect("query strategy")
            .map(|row| (row.id, row.name));

        assert_eq!(
            (delete_result, remaining),
            (
                Err("confirmation mismatch".to_string()),
                Some((strategy_id, "renamed".to_string())),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_strategy_succeeds_with_matching_confirm_name(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "s").await;
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .delete_strategy(Parameters(DeleteStrategyParams {
                strategy_id,
                confirm_name: "s".to_string(),
            }))
            .await
            .expect("ok");
        assert_eq!((result.ok, result.errors), (true, Vec::<String>::new()));

        assert_eq!(
            strategy::Entity::find_by_id(strategy_id)
                .one(&db)
                .await
                .expect("query strategy"),
            None,
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_strategy_rejects_unknown_strategy(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let err = server
            .delete_strategy(Parameters(DeleteStrategyParams {
                strategy_id: Uuid::new_v4(),
                confirm_name: "whatever".to_string(),
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}
