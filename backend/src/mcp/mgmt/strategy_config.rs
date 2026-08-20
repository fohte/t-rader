//! 管理 MCP の戦略設定 (name / description / agents_md / skills / agent_graph) の
//! 取得・作成・更新・削除と、戦略に紐づく trigger の一覧取得 (読み取り専用) の tool。
//!
//! Web UI/REST と同じ `services::strategy_config` を経由するため、DB 更新・履歴記録は
//! そちらに一本化されている。agent_graph の YAML 検証だけは `services::strategy_config` が
//! `services::agent_graph` に依存できない (循環依存になる) ため、ここで事前に行う。

use rmcp::ErrorData as McpError;

use crate::error::AppError;
use crate::services::agent_graph as agent_graph_svc;
use crate::services::change_history::Actor;
use crate::services::strategy_config;
use crate::services::trigger_crud;

use super::dto::{
    CreateStrategyParams, CreateStrategyResult, DeleteStrategyParams, DeleteStrategyResult,
    GetStrategyConfigParams, GetStrategyConfigResult, TriggerSummary, UpdateStrategyConfigParams,
    UpdateStrategyConfigResult,
};
use super::{MgmtServer, db_error, internal_error, invalid_params};

impl MgmtServer {
    pub(super) async fn get_strategy_config_inner(
        &self,
        params: GetStrategyConfigParams,
    ) -> Result<GetStrategyConfigResult, McpError> {
        let row = strategy_config::find_or_404(&self.db, params.strategy_id)
            .await
            .map_err(map_app_error)?;
        let triggers = trigger_crud::list_triggers(&self.db, params.strategy_id, None)
            .await
            .map_err(map_app_error)?;
        Ok(GetStrategyConfigResult {
            strategy_id: row.id,
            name: row.name,
            description: row.description,
            agents_md: row.agents_md,
            skills: strategy_config::skills_to_btree(&row.skills),
            agent_graph: row.agent_graph,
            triggers: triggers.into_iter().map(TriggerSummary::from).collect(),
        })
    }

    pub(super) async fn create_strategy_inner(
        &self,
        params: CreateStrategyParams,
    ) -> Result<CreateStrategyResult, McpError> {
        let mut errors = Vec::new();

        if let Err(err) = strategy_config::validate_name(&params.name) {
            errors.push(validation_message(err));
        }

        let skills = params.skills.map(|skills| {
            let mut map = serde_json::Map::new();
            for (name, content) in skills {
                push_skill_name_error(&mut errors, &name);
                map.insert(name, serde_json::Value::String(content));
            }
            serde_json::Value::Object(map)
        });

        push_agent_graph_errors(params.agent_graph.as_deref(), &mut errors);

        if !errors.is_empty() {
            return Ok(CreateStrategyResult {
                ok: false,
                errors,
                strategy_id: None,
            });
        }

        let created = strategy_config::create(
            &self.db,
            Actor::Llm { label: "mgmt-mcp" },
            strategy_config::CreateStrategy {
                name: params.name,
                description: params.description,
                sort_order: 0,
                agents_md: params.agents_md,
                skills,
                agent_graph: params.agent_graph,
            },
        )
        .await
        .map_err(map_app_error)?;

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
            && let Err(err) = strategy_config::validate_name(name)
        {
            errors.push(validation_message(err));
        }

        // null (削除) のキーは既存キーの後始末を妨げないよう検証しない。新規に本文を
        // 設定するキーだけ validate_skill_name を通す。
        let skills_patch = params.skills.map(|skills| {
            let mut patch = serde_json::Map::new();
            for (name, content) in skills {
                match content {
                    Some(body) => {
                        push_skill_name_error(&mut errors, &name);
                        patch.insert(name, serde_json::Value::String(body));
                    }
                    None => {
                        patch.insert(name, serde_json::Value::Null);
                    }
                }
            }
            patch
        });

        push_agent_graph_errors(params.agent_graph.as_deref(), &mut errors);

        if !errors.is_empty() {
            return Ok(UpdateStrategyConfigResult { ok: false, errors });
        }

        strategy_config::update(
            &self.db,
            Actor::Llm { label: "mgmt-mcp" },
            params.strategy_id,
            strategy_config::StrategyUpdate {
                name: params.name,
                description: params.description,
                sort_order: None,
                agents_md: params.agents_md,
                skills_patch,
                agent_graph: params.agent_graph,
            },
        )
        .await
        .map_err(map_app_error)?;

        Ok(UpdateStrategyConfigResult {
            ok: true,
            errors: vec![],
        })
    }

    pub(super) async fn delete_strategy_inner(
        &self,
        params: DeleteStrategyParams,
    ) -> Result<DeleteStrategyResult, McpError> {
        let current = strategy_config::find_or_404(&self.db, params.strategy_id)
            .await
            .map_err(map_app_error)?;
        if params.confirm_name != current.name {
            return Ok(DeleteStrategyResult {
                ok: false,
                errors: vec![format!(
                    "confirm_name {:?} does not match strategy name {:?}",
                    params.confirm_name, current.name
                )],
            });
        }
        strategy_config::delete_confirmed(
            &self.db,
            Actor::Llm { label: "mgmt-mcp" },
            params.strategy_id,
            &params.confirm_name,
        )
        .await
        .map_err(map_app_error)?;
        Ok(DeleteStrategyResult {
            ok: true,
            errors: vec![],
        })
    }
}

/// `AppError::Validation` ならメッセージ本文だけを、それ以外なら Display 文字列を返す。
/// validate_name/validate_skill_name は Validation 以外を返さない実装だが、将来変わっても
/// エラーメッセージを取りこぼさないためのフォールバック。
fn validation_message(err: AppError) -> String {
    match err {
        AppError::Validation(msg) => msg,
        other => other.to_string(),
    }
}

fn push_skill_name_error(errors: &mut Vec<String>, name: &str) {
    if let Err(err) = strategy_config::validate_skill_name(name) {
        errors.push(format!("skill {name:?}: {}", validation_message(err)));
    }
}

fn push_agent_graph_errors(yaml: Option<&str>, errors: &mut Vec<String>) {
    if let Some(yaml) = yaml
        && let Err(err) = agent_graph_svc::parse_agent_graph(yaml)
    {
        errors.push(err.to_string());
    }
}

fn map_app_error(err: AppError) -> McpError {
    match err {
        AppError::NotFound(msg) => invalid_params(msg),
        AppError::Database(db_err) => db_error(db_err),
        other => internal_error(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use indoc::indoc;
    use rmcp::handler::server::wrapper::{Json, Parameters};
    use sea_orm::EntityTrait;
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::agent_client::FakeAgentTaskClient;
    use crate::entities::strategy;
    use crate::testing::{create_test_db, insert_test_cron_trigger};

    use super::super::tests_common::{build_server, insert_strategy};
    use super::*;

    #[sqlx::test(migrations = false)]
    async fn get_strategy_config_returns_full_row_and_empty_triggers(pool: PgPool) {
        let db = create_test_db(pool).await;
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
                "agents_md": "",
                "skills": {},
                "agent_graph": "",
                "triggers": [],
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn get_strategy_config_includes_triggers(pool: PgPool) {
        let db = create_test_db(pool).await;
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

    #[sqlx::test(migrations = false)]
    async fn get_strategy_config_rejects_unknown_strategy(pool: PgPool) {
        let db = create_test_db(pool).await;
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

    #[sqlx::test(migrations = false)]
    async fn create_strategy_persists_all_fields_atomically(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let yaml = indoc! {"
            phases:
              - key: plan
                label: 調査計画
                model: claude-opus-4
                prompt: 仮説を立てよ
        "};
        let mut skills = BTreeMap::new();
        skills.insert("scout".to_string(), "scout body".to_string());
        skills.insert("review".to_string(), "review body".to_string());

        let Json(result) = server
            .create_strategy(Parameters(CreateStrategyParams {
                name: "s".to_string(),
                description: Some("desc".to_string()),
                agents_md: Some("# 方針".to_string()),
                skills: Some(skills),
                agent_graph: Some(yaml.to_string()),
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
                "agents_md": "# 方針",
                "skills": { "scout": "scout body", "review": "review body" },
                "agent_graph": yaml,
                "triggers": [],
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_strategy_rejects_invalid_fields_without_writing_anything(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));

        let mut skills = BTreeMap::new();
        skills.insert("Bad Name".to_string(), "x".to_string());

        let Json(result) = server
            .create_strategy(Parameters(CreateStrategyParams {
                name: "".to_string(),
                description: None,
                agents_md: None,
                skills: Some(skills),
                agent_graph: Some("phases: [".to_string()),
            }))
            .await
            .expect("tool call itself must succeed");
        assert_eq!(
            (result.ok, result.errors.len(), result.strategy_id),
            (false, 3, None),
        );

        let rows = strategy::Entity::find().all(&db).await.unwrap();
        assert!(rows.is_empty());
    }

    #[sqlx::test(migrations = false)]
    async fn update_strategy_config_applies_multiple_fields_in_one_call(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "s").await;
        strategy_config::save_skills(
            &db,
            Actor::Human,
            strategy_config::find_or_404(&db, strategy_id)
                .await
                .expect("find strategy"),
            json!({ "old": "old body" }),
            "seed".to_string(),
        )
        .await
        .expect("seed skills");
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let yaml = indoc! {"
            phases:
              - key: plan
                label: 調査計画
                model: claude-opus-4
                prompt: 仮説を立てよ
        "};
        let mut skills = BTreeMap::new();
        skills.insert("old".to_string(), None);
        skills.insert("new".to_string(), Some("new body".to_string()));

        let Json(result) = server
            .update_strategy_config(Parameters(UpdateStrategyConfigParams {
                strategy_id,
                name: None,
                description: None,
                agents_md: Some("# 方針".to_string()),
                skills: Some(skills),
                agent_graph: Some(yaml.to_string()),
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
                "name": "s",
                "description": null,
                "agents_md": "# 方針",
                "skills": { "new": "new body" },
                "agent_graph": yaml,
                "triggers": [],
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn update_strategy_config_rejects_invalid_agent_graph_without_writing_anything(
        pool: PgPool,
    ) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "original").await;
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .update_strategy_config(Parameters(UpdateStrategyConfigParams {
                strategy_id,
                name: Some("renamed".to_string()),
                description: None,
                agents_md: None,
                skills: None,
                agent_graph: Some("phases: [".to_string()),
            }))
            .await
            .expect("tool call itself must succeed");
        assert!(!result.ok);

        let row = strategy_config::find_or_404(&db, strategy_id)
            .await
            .expect("find strategy");
        assert_eq!(row.name, "original".to_string());
    }

    #[sqlx::test(migrations = false)]
    async fn delete_strategy_requires_confirm_name_exact_match(pool: PgPool) {
        let db = create_test_db(pool).await;
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

        assert!(strategy_config::find_or_404(&db, strategy_id).await.is_ok());
    }

    #[sqlx::test(migrations = false)]
    async fn delete_strategy_succeeds_with_matching_confirm_name(pool: PgPool) {
        let db = create_test_db(pool).await;
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

        assert!(
            strategy_config::find_or_404(&db, strategy_id)
                .await
                .is_err()
        );
    }

    #[sqlx::test(migrations = false)]
    async fn delete_strategy_rejects_unknown_strategy(pool: PgPool) {
        let db = create_test_db(pool).await;
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
