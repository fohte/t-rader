use std::sync::Arc;

use async_trait::async_trait;
use axum::http::StatusCode;
use chrono::DateTime;
use core_application::{
    change_history::{ChangeHistoryError, ChangeHistoryPort, ChangeHistoryRecord},
    strategy_existence::SharedStrategyExistence,
    strategy_task::StrategyTaskUseCases,
    trigger::{SharedTriggerRepository, TriggerUseCases, UpdateTriggerCommand},
    unit_of_work::SharedUnitOfWork,
};
use gateway_postgres::{
    DatabaseHandle, PostgresStrategyExistence, PostgresStrategyRepository,
    PostgresStrategyTaskRepository, PostgresTriggerRepository, PostgresUnitOfWork,
    entities::{change_history, strategy, trigger},
};
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, IntoActiveModel, QueryFilter,
};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{create_agent_config, create_strategy};
use crate::testing::create_test_server;

struct Definition<'a> {
    trigger_id: Uuid,
    strategy_id: Uuid,
    purpose: Option<&'a str>,
    kind: &'a str,
    schedule: Option<&'a str>,
    hook_slug: Option<&'a str>,
    event_type: Option<&'a str>,
    event_match: Option<Value>,
    prompt_template: &'a str,
    enabled: bool,
    business_days_only: bool,
}

fn definition(definition: Definition<'_>) -> Value {
    json!({
        "trigger_id": definition.trigger_id,
        "strategy_id": definition.strategy_id,
        "purpose": definition.purpose,
        "kind": definition.kind,
        "schedule": definition.schedule,
        "hook_slug": definition.hook_slug,
        "event_type": definition.event_type,
        "event_match": definition.event_match,
        "prompt_template": definition.prompt_template,
        "enabled": definition.enabled,
        "business_days_only": definition.business_days_only,
    })
}

fn snapshot(before: Option<Value>, after: Option<Value>) -> Value {
    json!({
        "format": "definition_snapshot",
        "version": 1,
        "before": before,
        "after": after,
    })
}

fn history_entry(target_id: Uuid, op: &str, diff_json: Value) -> Value {
    json!({
        "id": "<id>",
        "target_kind": "trigger",
        "target_id": target_id,
        "actor_kind": "human",
        "actor_label": "user",
        "op": op,
        "diff_json": diff_json,
        "summary": null,
        "created_at": "<created_at>",
    })
}

fn normalize_history(mut rows: Vec<Value>) -> Vec<Value> {
    for row in &mut rows {
        row["id"] = json!("<id>");
        row["created_at"] = json!("<created_at>");
    }
    rows.sort_by_key(|row| row["diff_json"].to_string());
    rows
}

fn normalize_trigger_response(mut body: Value) -> Value {
    body["created_at"] = json!("<created_at>");
    body["updated_at"] = json!("<updated_at>");
    if !body["last_fired_at"].is_null() {
        body["last_fired_at"] = json!("<last_fired_at>");
    }
    body
}

fn trigger_response(mut definition: Value, last_fired_at: Value) -> Value {
    definition["last_fired_at"] = last_fired_at;
    definition["created_at"] = json!("<created_at>");
    definition["updated_at"] = json!("<updated_at>");
    definition
}

async fn trigger_history(
    server: &axum_test::TestServer,
    trigger_id: Uuid,
) -> axum_test::TestResponse {
    server
        .get(&format!(
            "/api/history?target_kind=trigger&target_id={trigger_id}"
        ))
        .await
}

async fn clean_concurrent_test_strategy_history(db: &impl ConnectionTrait) {
    let histories = change_history::Entity::find()
        .filter(change_history::Column::TargetKind.eq("strategy"))
        .all(db)
        .await
        .expect("list strategy histories");
    for history in histories
        .into_iter()
        .filter(|history| history.diff_json["name"] == "concurrent-history-example")
    {
        change_history::Entity::delete_by_id(history.id)
            .exec(db)
            .await
            .expect("delete concurrent test strategy history");
    }
}

#[backend_test_macros::database_test]
async fn trigger_snapshots_cover_create_update_restore_and_delete(
    db: gateway_postgres::DatabaseHandle,
) {
    let server = create_test_server(db.clone()).await;
    let strategy_id =
        Uuid::parse_str(&create_strategy(&server, "history-example").await).expect("strategy id");
    let purpose = "synthetic-purpose";
    create_agent_config(&server, purpose).await;

    let original_template = indoc::indoc! {"
        synthetic opening
        synthetic continuation
    "}
    .trim();
    let restored_template = indoc::indoc! {"
        revised synthetic opening
        revised synthetic continuation
    "}
    .trim();
    let event_type = "synthetic_task.changed";
    let create_event_match = json!({ "state": { "eq": "initial" } });
    let created = server
        .post(&format!("/api/strategies/{strategy_id}/triggers"))
        .json(&json!({
            "kind": "event",
            "purpose": purpose,
            "event_type": event_type,
            "event_match": create_event_match,
            "prompt_template": original_template,
        }))
        .await;
    let trigger_id = Uuid::parse_str(
        created.json::<Value>()["trigger_id"]
            .as_str()
            .expect("trigger id"),
    )
    .expect("valid trigger id");

    let original = definition(Definition {
        trigger_id,
        strategy_id,
        purpose: Some(purpose),
        kind: "event",
        schedule: None,
        hook_slug: None,
        event_type: Some(event_type),
        event_match: Some(create_event_match),
        prompt_template: original_template,
        enabled: true,
        business_days_only: false,
    });
    let after_clear = definition(Definition {
        trigger_id,
        strategy_id,
        purpose: None,
        kind: "event",
        schedule: None,
        hook_slug: None,
        event_type: Some(event_type),
        event_match: None,
        prompt_template: restored_template,
        enabled: true,
        business_days_only: false,
    });
    let after_partial_update = definition(Definition {
        trigger_id,
        strategy_id,
        purpose: None,
        kind: "event",
        schedule: None,
        hook_slug: None,
        event_type: Some(event_type),
        event_match: Some(json!({})),
        prompt_template: restored_template,
        enabled: false,
        business_days_only: false,
    });

    let first_update = server
        .put(&format!("/api/triggers/{trigger_id}"))
        .json(&json!({
            "purpose": null,
            "event_match": null,
            "prompt_template": restored_template,
        }))
        .await;
    let second_update = server
        .put(&format!("/api/triggers/{trigger_id}"))
        .json(&json!({ "enabled": false, "event_match": {} }))
        .await;

    let history_before_restore = trigger_history(&server, trigger_id).await;
    let history_before_restore_rows = history_before_restore.json::<Vec<Value>>();
    let restore_before = history_before_restore_rows
        .iter()
        .find(|row| row["op"] == "update" && row["diff_json"]["after"]["enabled"] == false)
        .expect("second update history")["diff_json"]["before"]
        .clone();
    let last_fired_at =
        DateTime::parse_from_rfc3339("2025-03-04T05:06:07+00:00").expect("fixed timestamp");
    let mut trigger_model = trigger::Entity::find_by_id(trigger_id)
        .one(&db)
        .await
        .expect("find trigger")
        .expect("trigger exists")
        .into_active_model();
    trigger_model.last_fired_at = Set(Some(last_fired_at));
    trigger_model.update(&db).await.expect("set runtime state");

    let restore = server
        .put(&format!("/api/triggers/{trigger_id}"))
        .json(&json!({
            "purpose": restore_before["purpose"].clone(),
            "event_match": restore_before["event_match"].clone(),
            "prompt_template": restore_before["prompt_template"].clone(),
            "enabled": restore_before["enabled"].clone(),
        }))
        .await;
    let restored = server.get(&format!("/api/triggers/{trigger_id}")).await;
    let restored_body = normalize_trigger_response(restored.json::<Value>());
    let runtime_after_restore = trigger::Entity::find_by_id(trigger_id)
        .one(&db)
        .await
        .expect("find restored trigger")
        .expect("restored trigger exists")
        .last_fired_at;

    let deleted = server.delete(&format!("/api/triggers/{trigger_id}")).await;
    let history_after_delete = trigger_history(&server, trigger_id).await;
    let history_rows = normalize_history(history_after_delete.json::<Vec<Value>>());
    let expected_history = normalize_history(vec![
        history_entry(trigger_id, "create", snapshot(None, Some(original.clone()))),
        history_entry(
            trigger_id,
            "update",
            snapshot(Some(original.clone()), Some(after_clear.clone())),
        ),
        history_entry(
            trigger_id,
            "update",
            snapshot(
                Some(after_clear.clone()),
                Some(after_partial_update.clone()),
            ),
        ),
        history_entry(
            trigger_id,
            "update",
            snapshot(Some(after_partial_update), Some(after_clear.clone())),
        ),
        history_entry(
            trigger_id,
            "delete",
            snapshot(Some(after_clear.clone()), None),
        ),
    ]);

    assert_eq!(
        (
            created.status_code(),
            first_update.status_code(),
            second_update.status_code(),
            history_before_restore.status_code(),
            restore.status_code(),
            restored.status_code(),
            deleted.status_code(),
            history_after_delete.status_code(),
            restored_body,
            runtime_after_restore,
            history_rows,
        ),
        (
            StatusCode::CREATED,
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::NO_CONTENT,
            StatusCode::OK,
            trigger_response(after_clear, json!("<last_fired_at>")),
            Some(last_fired_at),
            expected_history,
        ),
    );
}

#[backend_test_macros::database_test]
async fn create_snapshots_preserve_kind_specific_configuration(
    db: gateway_postgres::DatabaseHandle,
) {
    let server = create_test_server(db).await;
    let strategy_id = Uuid::parse_str(&create_strategy(&server, "kind-history-example").await)
        .expect("strategy id");
    let cases = [
        (
            "cron",
            json!({
                "kind": "cron",
                "schedule": "15 8 * * 1-5",
                "business_days_only": true,
                "prompt_template": "synthetic cron template",
            }),
            Some("15 8 * * 1-5"),
            None,
            None,
            None,
            true,
        ),
        (
            "hook",
            json!({
                "kind": "hook",
                "hook_slug": "synthetic-hook",
                "event_match": { "source": { "eq": "synthetic" } },
                "prompt_template": "synthetic hook template",
            }),
            None,
            Some("synthetic-hook"),
            None,
            Some(json!({ "source": { "eq": "synthetic" } })),
            false,
        ),
        (
            "event",
            json!({
                "kind": "event",
                "event_type": "synthetic_job.changed",
                "event_match": { "phase": { "eq": "ready" } },
                "prompt_template": "synthetic event template",
            }),
            None,
            None,
            Some("synthetic_job.changed"),
            Some(json!({ "phase": { "eq": "ready" } })),
            false,
        ),
    ];
    let mut statuses = Vec::new();
    let mut actual_history = Vec::new();
    let mut expected_history = Vec::new();

    for (kind, request, schedule, hook_slug, event_type, event_match, business_days_only) in cases {
        let created = server
            .post(&format!("/api/strategies/{strategy_id}/triggers"))
            .json(&request)
            .await;
        let trigger_id = Uuid::parse_str(
            created.json::<Value>()["trigger_id"]
                .as_str()
                .expect("trigger id"),
        )
        .expect("valid trigger id");
        let history = trigger_history(&server, trigger_id).await;
        actual_history.extend(history.json::<Vec<Value>>());
        expected_history.push(history_entry(
            trigger_id,
            "create",
            snapshot(
                None,
                Some(definition(Definition {
                    trigger_id,
                    strategy_id,
                    purpose: None,
                    kind,
                    schedule,
                    hook_slug,
                    event_type,
                    event_match,
                    prompt_template: match kind {
                        "cron" => "synthetic cron template",
                        "hook" => "synthetic hook template",
                        "event" => "synthetic event template",
                        _ => unreachable!(),
                    },
                    enabled: true,
                    business_days_only,
                })),
            ),
        ));
        statuses.push((created.status_code(), history.status_code()));
    }

    assert_eq!(
        (statuses, normalize_history(actual_history)),
        (
            vec![
                (StatusCode::CREATED, StatusCode::OK),
                (StatusCode::CREATED, StatusCode::OK),
                (StatusCode::CREATED, StatusCode::OK),
            ],
            normalize_history(expected_history),
        ),
    );
}

#[backend_test_macros::database_test]
async fn concurrent_updates_record_the_definition_they_replace(
    _database: gateway_postgres::DatabaseHandle,
) {
    let pool = gateway_postgres::test_support::create_test_pool_with_max_connections(4).await;
    let connection = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
    let db = DatabaseHandle::from(connection);
    clean_concurrent_test_strategy_history(&db).await;
    let server = create_test_server(db.clone()).await;
    let strategy_id = crate::testing::insert_test_strategy(&db, "concurrent-history-example").await;
    let created = server
        .post(&format!("/api/strategies/{strategy_id}/triggers"))
        .json(&json!({
            "kind": "event",
            "event_type": "synthetic_job.started",
            "prompt_template": "concurrent initial template",
        }))
        .await;
    let trigger_id = Uuid::parse_str(
        created.json::<Value>()["trigger_id"]
            .as_str()
            .expect("trigger id"),
    )
    .expect("valid trigger id");

    let initial = definition(Definition {
        trigger_id,
        strategy_id,
        purpose: None,
        kind: "event",
        schedule: None,
        hook_slug: None,
        event_type: Some("synthetic_job.started"),
        event_match: None,
        prompt_template: "concurrent initial template",
        enabled: true,
        business_days_only: false,
    });
    let prompt_first = definition(Definition {
        trigger_id,
        strategy_id,
        purpose: None,
        kind: "event",
        schedule: None,
        hook_slug: None,
        event_type: Some("synthetic_job.started"),
        event_match: None,
        prompt_template: "concurrent replacement template",
        enabled: true,
        business_days_only: false,
    });
    let enabled_first = definition(Definition {
        trigger_id,
        strategy_id,
        purpose: None,
        kind: "event",
        schedule: None,
        hook_slug: None,
        event_type: Some("synthetic_job.started"),
        event_match: None,
        prompt_template: "concurrent initial template",
        enabled: false,
        business_days_only: false,
    });
    let final_definition = definition(Definition {
        trigger_id,
        strategy_id,
        purpose: None,
        kind: "event",
        schedule: None,
        hook_slug: None,
        event_type: Some("synthetic_job.started"),
        event_match: None,
        prompt_template: "concurrent replacement template",
        enabled: false,
        business_days_only: false,
    });

    let (prompt_update, enabled_update) = tokio::join!(
        server
            .put(&format!("/api/triggers/{trigger_id}"))
            .json(&json!({ "prompt_template": "concurrent replacement template" })),
        server
            .put(&format!("/api/triggers/{trigger_id}"))
            .json(&json!({ "enabled": false })),
    );
    let history = trigger_history(&server, trigger_id).await;
    let final_response = server.get(&format!("/api/triggers/{trigger_id}")).await;
    let actual = (
        created.status_code(),
        (
            prompt_update.status_code(),
            normalize_trigger_response(prompt_update.json::<Value>()),
            enabled_update.status_code(),
            normalize_trigger_response(enabled_update.json::<Value>()),
        ),
        (
            history.status_code(),
            normalize_history(history.json::<Vec<Value>>()),
        ),
        (
            final_response.status_code(),
            normalize_trigger_response(final_response.json::<Value>()),
        ),
    );
    let prompt_first_expected = (
        StatusCode::CREATED,
        (
            StatusCode::OK,
            trigger_response(prompt_first.clone(), Value::Null),
            StatusCode::OK,
            trigger_response(final_definition.clone(), Value::Null),
        ),
        (
            StatusCode::OK,
            normalize_history(vec![
                history_entry(trigger_id, "create", snapshot(None, Some(initial.clone()))),
                history_entry(
                    trigger_id,
                    "update",
                    snapshot(Some(initial.clone()), Some(prompt_first.clone())),
                ),
                history_entry(
                    trigger_id,
                    "update",
                    snapshot(Some(prompt_first), Some(final_definition.clone())),
                ),
            ]),
        ),
        (
            StatusCode::OK,
            trigger_response(final_definition.clone(), Value::Null),
        ),
    );
    let enabled_first_expected = (
        StatusCode::CREATED,
        (
            StatusCode::OK,
            trigger_response(final_definition.clone(), Value::Null),
            StatusCode::OK,
            trigger_response(enabled_first.clone(), Value::Null),
        ),
        (
            StatusCode::OK,
            normalize_history(vec![
                history_entry(trigger_id, "create", snapshot(None, Some(initial.clone()))),
                history_entry(
                    trigger_id,
                    "update",
                    snapshot(Some(initial), Some(enabled_first.clone())),
                ),
                history_entry(
                    trigger_id,
                    "update",
                    snapshot(Some(enabled_first), Some(final_definition.clone())),
                ),
            ]),
        ),
        (
            StatusCode::OK,
            trigger_response(final_definition, Value::Null),
        ),
    );

    change_history::Entity::delete_many()
        .filter(change_history::Column::TargetId.is_in([trigger_id, strategy_id]))
        .exec(&db)
        .await
        .expect("delete trigger history");
    trigger::Entity::delete_by_id(trigger_id)
        .exec(&db)
        .await
        .expect("delete trigger");
    strategy::Entity::delete_by_id(strategy_id)
        .exec(&db)
        .await
        .expect("delete strategy");

    assert!(actual == prompt_first_expected || actual == enabled_first_expected);
}

struct FailingChangeHistory;

#[async_trait]
impl ChangeHistoryPort for FailingChangeHistory {
    async fn record(
        &self,
        _transaction: &core_application::unit_of_work::UnitOfWorkTransaction,
        _record: ChangeHistoryRecord,
    ) -> Result<(), ChangeHistoryError> {
        Err(ChangeHistoryError::InvalidTransaction)
    }
}

#[backend_test_macros::database_test]
async fn update_rolls_back_when_change_history_recording_fails(
    db: gateway_postgres::DatabaseHandle,
) {
    let server = create_test_server(db.clone()).await;
    let strategy_id = Uuid::parse_str(&create_strategy(&server, "rollback-history-example").await)
        .expect("strategy id");
    let created = server
        .post(&format!("/api/strategies/{strategy_id}/triggers"))
        .json(&json!({
            "kind": "event",
            "event_type": "synthetic_job.queued",
            "prompt_template": "committed template",
        }))
        .await;
    let trigger_id = Uuid::parse_str(
        created.json::<Value>()["trigger_id"]
            .as_str()
            .expect("trigger id"),
    )
    .expect("valid trigger id");
    let original = definition(Definition {
        trigger_id,
        strategy_id,
        purpose: None,
        kind: "event",
        schedule: None,
        hook_slug: None,
        event_type: Some("synthetic_job.queued"),
        event_match: None,
        prompt_template: "committed template",
        enabled: true,
        business_days_only: false,
    });

    let unit_of_work: SharedUnitOfWork = Arc::new(PostgresUnitOfWork::new(db.clone()));
    let trigger_repository: SharedTriggerRepository =
        Arc::new(PostgresTriggerRepository::new(db.clone()));
    let strategy_existence: SharedStrategyExistence = Arc::new(PostgresStrategyExistence);
    let strategy_repository = Arc::new(PostgresStrategyRepository::new(db.clone()));
    let strategy_tasks = StrategyTaskUseCases::new(
        unit_of_work.clone(),
        Arc::new(PostgresStrategyTaskRepository::new(db.clone())),
    );
    let trigger_use_cases = TriggerUseCases::new(
        unit_of_work,
        trigger_repository,
        strategy_existence,
        strategy_repository,
        strategy_tasks,
        Arc::new(FailingChangeHistory),
    );
    let result = trigger_use_cases
        .update(
            strategy_id.into(),
            trigger_id,
            UpdateTriggerCommand {
                prompt_template: Some("uncommitted template".to_string()),
                ..UpdateTriggerCommand::default()
            },
        )
        .await;
    let stored = server.get(&format!("/api/triggers/{trigger_id}")).await;
    let history = trigger_history(&server, trigger_id).await;

    assert_eq!(
        (
            created.status_code(),
            result.err().map(|error| error.to_string()),
            stored.status_code(),
            normalize_trigger_response(stored.json::<Value>()),
            history.status_code(),
            normalize_history(history.json::<Vec<Value>>()),
        ),
        (
            StatusCode::CREATED,
            Some("transaction has an unexpected type".to_string()),
            StatusCode::OK,
            trigger_response(original.clone(), Value::Null),
            StatusCode::OK,
            normalize_history(vec![history_entry(
                trigger_id,
                "create",
                snapshot(None, Some(original)),
            )]),
        ),
    );
}
