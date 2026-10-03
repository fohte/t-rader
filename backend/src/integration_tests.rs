//! composition root 経由で公開 entrypoint の統合動作を検証する。
//!
//! 戦略タスクの経路横断契約と、mgmt MCP / strategy MCP の tool dispatch を扱う。

pub(crate) mod mcp_tool;
mod mgmt_mcp;
mod prediction_grading;
mod strategy_mcp;
mod trigger;

use std::sync::Arc;
use std::time::Duration;

use chrono::{TimeZone, Utc};
use sea_orm::{ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, QueryFilter, Statement};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::agent_client::{
    AgentTaskState, AgentTaskStatus, FakeAgentTaskClient, SharedAgentTaskClient,
};
use crate::services::use_cases::build_use_cases;
use crate::testing::agent_config;
use crate::testing::{
    create_test_server_with_db_and_agent_client, insert_test_cron_trigger,
    insert_test_hook_trigger, insert_test_strategy,
};
use core_application::strategy_task::DEFAULT_PURPOSE;
use entrypoint_control_plane_mcp::MgmtServer;
use gateway_postgres::entities::sea_orm_active_enums::StrategyTaskPhase;
use gateway_postgres::entities::strategy_task;

#[backend_test_macros::database_test]
async fn postgres_queue_enqueues_one_keyed_strategy_task_reconcile_job(
    db: gateway_postgres::DatabaseHandle,
) {
    use core_application::strategy_task::{
        STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER, STRATEGY_TASK_RECONCILE_QUEUE_NAME,
        StrategyTaskReconcileJobQueue,
    };
    use gateway_postgres::PostgresStrategyTaskReconcileJobQueue;

    let pool = gateway_postgres::test_support::create_test_pool().await;
    crate::migrations::migrate_graphile_worker_schema(pool)
        .await
        .expect("migrate Graphile Worker schema");
    let queue = PostgresStrategyTaskReconcileJobQueue::new(db.clone());
    queue
        .enqueue_reconciliation()
        .await
        .expect("enqueue reconciliation job");
    queue
        .enqueue_reconciliation()
        .await
        .expect("deduplicate reconciliation job");

    let statement = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT tasks.identifier, jobs.payload::text AS payload, \
                jobs.max_attempts::int AS max_attempts, jobs.key, queues.queue_name \
         FROM graphile_worker._private_jobs AS jobs \
         JOIN graphile_worker._private_tasks AS tasks ON tasks.id = jobs.task_id \
         LEFT JOIN graphile_worker._private_job_queues AS queues ON queues.id = jobs.job_queue_id \
         WHERE jobs.key = $1"
            .to_string(),
        [STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER.to_string().into()],
    );
    let rows = db.query_all_raw(statement).await.expect("read queued job");
    let actual = rows
        .into_iter()
        .map(|row| {
            (
                row.try_get::<String>("", "identifier")
                    .expect("job identifier"),
                row.try_get::<String>("", "payload").expect("job payload"),
                row.try_get::<i32>("", "max_attempts")
                    .expect("maximum attempts"),
                row.try_get::<Option<String>>("", "key").expect("job key"),
                row.try_get::<Option<String>>("", "queue_name")
                    .expect("queue name"),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        vec![(
            STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER.to_string(),
            "{}".to_string(),
            3,
            Some(STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER.to_string()),
            Some(STRATEGY_TASK_RECONCILE_QUEUE_NAME.to_string()),
        )],
    );
}

#[backend_test_macros::database_test]
async fn all_five_submission_routes_converge_on_strategy_task_use_case(
    db: gateway_postgres::DatabaseHandle,
) {
    let fake = Arc::new(FakeAgentTaskClient::new());
    let agent_client: SharedAgentTaskClient = fake.clone();
    let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client.clone()).await;
    let strategy_id = insert_test_strategy(&db, "s").await;
    agent_config::create(&db, DEFAULT_PURPOSE.to_string())
        .await
        .expect("insert test agent_config");

    let use_cases = build_use_cases(db.clone());
    let mgmt = MgmtServer::new(crate::mcp::mgmt_dependencies(
        &use_cases,
        agent_client.clone(),
    ));
    let _: Value = mcp_tool::call_tool_output::<_, Value>(
        &mgmt,
        "submit_strategy_task",
        json!({
            "strategy_id": strategy_id,
            "prompt": "from mgmt",
            "purpose": null,
        }),
    )
    .await
    .expect("mgmt submit ok");

    let res = server
        .post(&format!("/api/strategies/{strategy_id}/chat"))
        .json(&json!({ "prompt": "from frontend" }))
        .await;
    res.assert_status(axum::http::StatusCode::ACCEPTED);

    // schedule は毎分発火。last_fired_at を十分に過去へ置き、run_once で確実に発火対象にする。
    let far_past = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
    insert_test_cron_trigger(
        &db,
        strategy_id,
        "* * * * *",
        true,
        Some(far_past),
        "from cron",
        None,
    )
    .await;
    let use_cases = build_use_cases(db.clone());
    let attempts = use_cases
        .triggers()
        .run_cron_tick(agent_client.as_ref(), Duration::from_secs(60))
        .await;
    assert_eq!(attempts, 1);

    insert_test_hook_trigger(&db, strategy_id, "wh", "from hook", None, true).await;
    let res = server.post("/api/hooks/wh").json(&json!({})).await;
    res.assert_status_ok();

    let note_res = server
        .post("/api/notes")
        .json(&json!({
            "strategy_id": strategy_id,
            "title": "note",
            "body_md": "body",
            "created_by_kind": "llm",
        }))
        .await;
    note_res.assert_status(axum::http::StatusCode::CREATED);
    let note_body: Value = note_res.json();
    let note_id = Uuid::parse_str(note_body["id"].as_str().unwrap()).unwrap();
    let version = crate::testing::find_current_note_version(&db, note_id)
        .await
        .unwrap()
        .unwrap();
    let res = server
        .post(&format!(
            "/api/notes/{note_id}/versions/{}/reject",
            version.version_no
        ))
        .json(&json!({"label": "確認事項"}))
        .await;
    res.assert_status_ok();

    // 5 経路すべてが StrategyTaskUseCases を通って strategy_task 行を作ることを source 別に検証する。
    let mut rows: Vec<(String, String, StrategyTaskPhase)> = strategy_task::Entity::find()
        .filter(strategy_task::Column::StrategyId.eq(strategy_id))
        .all(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|r| (r.source, r.prompt, r.phase))
        .collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        rows,
        vec![
            (
                "cron".to_string(),
                "from cron".to_string(),
                StrategyTaskPhase::Running
            ),
            (
                "frontend".to_string(),
                "from frontend".to_string(),
                StrategyTaskPhase::Running
            ),
            (
                "hook".to_string(),
                "from hook".to_string(),
                StrategyTaskPhase::Running
            ),
            (
                "mgmt-mcp".to_string(),
                "from mgmt".to_string(),
                StrategyTaskPhase::Running
            ),
            (
                "review".to_string(),
                format!(
                    "ノート「note」(id: {note_id}) の v{} (version_id: {}) がレビューで却下されました。理由: 確認事項。付いているコメントを確認し、指摘を反映してください。",
                    version.version_no, version.id,
                ),
                StrategyTaskPhase::Running
            ),
        ],
    );

    // 同一の agent_client (= 同一の StrategyTaskUseCases 呼び出し経路) に 5 件とも届いていることを検証する。
    let mut submitted_prompts: Vec<String> = fake
        .submitted
        .lock()
        .await
        .iter()
        .map(|s| s.prompt.clone())
        .collect();
    submitted_prompts.sort();
    assert_eq!(
        submitted_prompts,
        vec![
            "from cron".to_string(),
            "from frontend".to_string(),
            "from hook".to_string(),
            "from mgmt".to_string(),
            format!(
                "ノート「note」(id: {note_id}) の v{} (version_id: {}) がレビューで却下されました。理由: 確認事項。付いているコメントを確認し、指摘を反映してください。",
                version.version_no, version.id,
            ),
        ],
    );
}

#[backend_test_macros::database_test]
async fn submitted_task_reaches_completed_with_result_text_after_scheduler_reconciles(
    db: gateway_postgres::DatabaseHandle,
) {
    let fake = Arc::new(FakeAgentTaskClient::new());
    let agent_client: SharedAgentTaskClient = fake.clone();
    let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client.clone()).await;
    let strategy_id = insert_test_strategy(&db, "s").await;
    agent_config::create(&db, DEFAULT_PURPOSE.to_string())
        .await
        .expect("insert test agent_config");

    let submit = server
        .post(&format!("/api/strategies/{strategy_id}/chat"))
        .json(&json!({ "prompt": "inspect 7203" }))
        .await;
    submit.assert_status(axum::http::StatusCode::ACCEPTED);
    let submit_body: Value = submit.json();
    let task_id = Uuid::parse_str(submit_body["task_id"].as_str().expect("task_id")).expect("uuid");
    let a2a_task_id = submit_body["a2a_task_id"]
        .as_str()
        .expect("a2a_task_id")
        .to_string();

    // t-rader-agent 側でタスクが完了した状態を模す。
    fake.set_status(
        &a2a_task_id,
        AgentTaskStatus {
            state: AgentTaskState::Completed,
            result_text: Some("7203 は堅調".to_string()),
            error_message: None,
            error_kind: None,
            steps: None,
        },
    )
    .await;

    let updated = entrypoint_scheduler::reconcile_in_flight_tasks(
        &build_use_cases(db.clone()).strategy_tasks(),
        agent_client.as_ref(),
    )
    .await;
    assert_eq!(updated, Ok(1));

    let res = server
        .get(&format!("/api/strategies/{strategy_id}/tasks/{task_id}"))
        .await;
    res.assert_status_ok();
    let mut body: Value = res.json();
    let obj = body.as_object_mut().unwrap();
    obj.remove("created_at");
    obj.remove("updated_at");
    obj.remove("as_of");
    assert_eq!(
        body,
        json!({
            "task_id": task_id,
            "strategy_id": strategy_id,
            "a2a_task_id": a2a_task_id,
            "source": "frontend",
            "prompt": "inspect 7203",
            "phase": "completed",
            "error_summary": null,
            "result_text": "7203 は堅調",
            "steps": [],
            "purpose": "default",
        }),
    );
}
