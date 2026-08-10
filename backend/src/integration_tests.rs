//! 戦略タスクの投入 (5 経路) → t-rader-agent 実行 (`FakeAgentTaskClient` でモック) →
//! watcher による決着反映 → 応答取得までを、実装コンポーネントを跨いで通しで検証する。
//!
//! 各コンポーネント単体の挙動は `services::strategy_tasks` / `mcp::watcher` /
//! `handlers::agent_tasks` 等のテストで既にカバーしているため、ここでは経路横断の契約
//! (5 経路が同一の `submit_task` に収束すること、投入から完了応答までが一気通貫で反映
//! されること) のみを扱う。

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use rmcp::handler::server::wrapper::Parameters;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::agent_client::{
    AgentTaskState, AgentTaskStatus, FakeAgentTaskClient, SharedAgentTaskClient,
};
use crate::entities::sea_orm_active_enums::StrategyTaskPhase;
use crate::entities::strategy_task;
use crate::mcp::mgmt::{MgmtServer, SubmitStrategyTaskParams};
use crate::mcp::watcher;
use crate::services::trigger_worker;
use crate::testing::{
    create_test_server_with_db_and_agent_client, insert_test_cron_trigger,
    insert_test_hook_trigger, insert_test_strategy,
};

#[sqlx::test(migrations = false)]
async fn all_five_submission_routes_converge_on_submit_task(pool: PgPool) {
    let fake = Arc::new(FakeAgentTaskClient::new());
    let agent_client: SharedAgentTaskClient = fake.clone();
    let (db, server) =
        create_test_server_with_db_and_agent_client(pool, agent_client.clone()).await;
    let strategy_id = insert_test_strategy(&db, "s").await;

    let mgmt = MgmtServer::new(db.clone(), agent_client.clone());
    mgmt.submit_strategy_task(Parameters(SubmitStrategyTaskParams {
        strategy_id,
        prompt: "from mgmt".into(),
    }))
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
    )
    .await;
    let attempts =
        trigger_worker::run_once(&db, &agent_client, trigger_worker::DEFAULT_INTERVAL).await;
    assert_eq!(attempts, 1);

    insert_test_hook_trigger(&db, strategy_id, "wh", "from hook", None, true).await;
    let res = server.post("/api/hooks/wh").json(&json!({})).await;
    res.assert_status_ok();

    let note_res = server
        .post("/api/notes")
        .json(&json!({ "strategy_id": strategy_id, "title": "note", "body_md": "body" }))
        .await;
    note_res.assert_status(axum::http::StatusCode::CREATED);
    let note_body: Value = note_res.json();
    let note_id = Uuid::parse_str(note_body["id"].as_str().unwrap()).unwrap();
    let res = server
        .post(&format!("/api/notes/{note_id}/reject"))
        .json(&json!({}))
        .await;
    res.assert_status_ok();

    // 5 経路すべてが submit_task を通って strategy_task 行を作ることを、source 別に検証する。
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
                    "ノート「note」(id: {note_id}) がレビューで却下されました。\
付いているコメントを確認し、指摘を反映してください。"
                ),
                StrategyTaskPhase::Running
            ),
        ],
    );

    // 同一の agent_client (= 同一の submit_task 呼び出し経路) に 5 件とも届いていることを検証する。
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
                "ノート「note」(id: {note_id}) がレビューで却下されました。\
付いているコメントを確認し、指摘を反映してください。"
            ),
        ],
    );
}

#[sqlx::test(migrations = false)]
async fn submitted_task_reaches_completed_with_result_text_after_watcher_reconciles(pool: PgPool) {
    let fake = Arc::new(FakeAgentTaskClient::new());
    let agent_client: SharedAgentTaskClient = fake.clone();
    let (db, server) =
        create_test_server_with_db_and_agent_client(pool, agent_client.clone()).await;
    let strategy_id = insert_test_strategy(&db, "s").await;

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
            error_kind: None,
        },
    )
    .await;

    let updated = watcher::run_once(&db, &agent_client).await;
    assert_eq!(updated, 1);

    let res = server
        .get(&format!("/api/strategies/{strategy_id}/tasks/{task_id}"))
        .await;
    res.assert_status_ok();
    let mut body: Value = res.json();
    let obj = body.as_object_mut().unwrap();
    obj.remove("created_at");
    obj.remove("updated_at");
    assert_eq!(
        body,
        json!({
            "task_id": task_id,
            "strategy_id": strategy_id,
            "a2a_task_id": a2a_task_id,
            "source": "frontend",
            "phase": "completed",
            "error_summary": null,
            "result_text": "7203 は堅調",
        }),
    );
}
