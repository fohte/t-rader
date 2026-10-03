//! 戦略タスク照合の既存 DB テストで使う単発 helper。

use core_application::agent_task_client::SharedAgentTaskClient;
use gateway_postgres::DatabaseHandle;

/// 1 回分の照合を実行する。失敗した個別 task はログに残して次へ進む。
pub async fn run_once<C>(db: &C, agent_client: &SharedAgentTaskClient) -> usize
where
    C: sea_orm::ConnectionTrait + Clone + Into<DatabaseHandle>,
{
    let strategy_tasks = crate::services::use_cases::build_use_cases(db.clone()).strategy_tasks();
    match entrypoint_scheduler::reconcile_in_flight_tasks(&strategy_tasks, agent_client.as_ref())
        .await
    {
        Ok(updated) => updated,
        Err(error) => {
            tracing::warn!(%error, "strategy task reconciliation failed");
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{DateTime, Utc};
    use core_application::agent_task_client::{
        AgentTaskError, AgentTaskState, AgentTaskStatus, EXECUTION_LOST_ERROR_KIND,
        FakeAgentTaskClient,
    };
    use gateway_postgres::entities::sea_orm_active_enums::{
        StrategyTaskPhase, StrategyTaskStepStatus,
    };
    use gateway_postgres::entities::{strategy, strategy_task, strategy_task_step};
    use sea_orm::{
        ActiveModelTrait,
        ActiveValue::{NotSet, Set},
        ColumnTrait, EntityTrait, QueryFilter, QueryOrder,
    };
    use uuid::Uuid;

    use super::*;

    async fn insert_strategy(db: &impl sea_orm::ConnectionTrait) -> Uuid {
        let id = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(id),
            name: Set("test".to_string()),
            description: Set(None),
            sort_order: Set(0),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(db)
        .await
        .unwrap();
        id
    }

    /// `deadline_offset` だけ現在時刻からずらした deadline_at を持つ行を挿入する。
    /// 過去にすれば「deadline 超過」、未来にすれば「deadline 未到来」の状態を作れる。
    async fn insert_task(
        db: &impl sea_orm::ConnectionTrait,
        strategy_id: Uuid,
        a2a_task_id: Option<&str>,
        phase: StrategyTaskPhase,
        deadline_offset: chrono::Duration,
    ) -> Uuid {
        let task_id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();
        strategy_task::ActiveModel {
            task_id: Set(task_id),
            strategy_id: Set(strategy_id),
            a2a_task_id: Set(a2a_task_id.map(|s| s.to_string())),
            source: Set("slack".to_string()),
            prompt: Set("hi".to_string()),
            phase: Set(phase),
            error_summary: Set(None),
            result_text: Set(None),
            deadline_at: Set(now + deadline_offset),
            purpose: NotSet,
            as_of: NotSet,
            auto_resumed_at: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .unwrap();
        task_id
    }

    async fn fetch_task(db: &impl sea_orm::ConnectionTrait, task_id: Uuid) -> strategy_task::Model {
        strategy_task::Entity::find_by_id(task_id)
            .one(db)
            .await
            .unwrap()
            .unwrap()
    }

    async fn fetch_steps(
        db: &impl sea_orm::ConnectionTrait,
        task_id: Uuid,
    ) -> Vec<strategy_task_step::Model> {
        strategy_task_step::Entity::find()
            .filter(strategy_task_step::Column::TaskId.eq(task_id))
            .order_by_asc(strategy_task_step::Column::Seq)
            .all(db)
            .await
            .unwrap()
    }

    fn without_sequence(
        mut steps: Vec<strategy_task_step::Model>,
    ) -> Vec<strategy_task_step::Model> {
        for step in &mut steps {
            step.seq = 0;
        }
        steps
    }

    const FAR_FUTURE: chrono::Duration = chrono::Duration::minutes(15);
    const PAST: chrono::Duration = chrono::Duration::seconds(-1);

    #[backend_test_macros::database_test]
    async fn reconciles_completed_running_and_failed_states(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db).await;
        let completed_id = insert_task(
            &db,
            strategy_id,
            Some("t-completed"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        let failed_id = insert_task(
            &db,
            strategy_id,
            Some("t-failed"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        let running_id = insert_task(
            &db,
            strategy_id,
            Some("t-running"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;

        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_status(
            "t-completed",
            AgentTaskStatus {
                state: AgentTaskState::Completed,
                result_text: Some("all good".to_string()),
                error_message: None,
                error_kind: None,
                steps: None,
            },
        )
        .await;
        fake.set_status(
            "t-failed",
            AgentTaskStatus {
                state: AgentTaskState::Failed,
                result_text: None,
                error_message: None,
                error_kind: Some("usage_limit".to_string()),
                steps: None,
            },
        )
        .await;
        fake.set_status(
            "t-running",
            AgentTaskStatus {
                state: AgentTaskState::Working,
                result_text: None,
                error_message: None,
                error_kind: None,
                steps: None,
            },
        )
        .await;

        let agent_client: SharedAgentTaskClient = fake.clone();
        let updated = run_once(&db, &agent_client).await;
        // running は phase (Running) も error/result も変化しないので更新カウントに含まれない。
        assert_eq!(updated, 2);

        let completed = fetch_task(&db, completed_id).await;
        let failed = fetch_task(&db, failed_id).await;
        let running = fetch_task(&db, running_id).await;

        assert_eq!(
            (
                completed.phase,
                completed.result_text,
                completed.error_summary,
            ),
            (
                StrategyTaskPhase::Completed,
                Some("all good".to_string()),
                None,
            ),
        );
        assert_eq!(
            (failed.phase, failed.result_text, failed.error_summary),
            (
                StrategyTaskPhase::Failed,
                None,
                Some("usage_limit".to_string()),
            ),
        );
        assert_eq!(
            (running.phase, running.result_text, running.error_summary),
            (StrategyTaskPhase::Running, None, None),
        );
    }

    #[backend_test_macros::database_test]
    async fn input_required_maps_to_failed(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-ir"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;

        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_status(
            "t-ir",
            AgentTaskStatus {
                state: AgentTaskState::InputRequired,
                result_text: None,
                error_message: None,
                error_kind: None,
                steps: None,
            },
        )
        .await;
        let agent_client: SharedAgentTaskClient = fake.clone();

        let updated = run_once(&db, &agent_client).await;
        assert_eq!(updated, 1);

        let row = fetch_task(&db, task_id).await;
        assert_eq!(
            (row.phase, row.error_summary),
            (
                StrategyTaskPhase::Failed,
                Some("agent task failed".to_string())
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn failed_error_summary_is_agent_error_message_over_error_kind(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-reason"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;

        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_status(
            "t-reason",
            AgentTaskStatus {
                state: AgentTaskState::Failed,
                result_text: None,
                error_message: Some(
                    "フェーズ「調査」(investigate) の実行に失敗しました: upstream returned 400"
                        .to_string(),
                ),
                error_kind: Some("agent_error".to_string()),
                steps: None,
            },
        )
        .await;
        let agent_client: SharedAgentTaskClient = fake;

        let updated = run_once(&db, &agent_client).await;
        assert_eq!(updated, 1);

        let row = fetch_task(&db, task_id).await;
        assert_eq!(
            (row.phase, row.error_summary),
            (
                StrategyTaskPhase::Failed,
                Some(
                    "フェーズ「調査」(investigate) の実行に失敗しました: upstream returned 400"
                        .to_string()
                ),
            ),
        );
    }

    // deadline 超過時は agent の応答内容 (completed/working 問わず) より deadline を優先して
    // failed に確定する。database_test は rstest の case 引数を扱わないため
    // for ループで列挙する。
    #[backend_test_macros::database_test]
    async fn agent_response_after_deadline_marks_failed_regardless_of_state(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db).await;

        for (label, a2a_task_id, status, expected_result_text) in [
            (
                "completed",
                "t-late",
                AgentTaskStatus {
                    state: AgentTaskState::Completed,
                    result_text: Some("all good".to_string()),
                    error_message: None,
                    error_kind: None,
                    steps: None,
                },
                Some("all good".to_string()),
            ),
            (
                "still_working",
                "t-stuck",
                AgentTaskStatus {
                    state: AgentTaskState::Working,
                    result_text: None,
                    error_message: None,
                    error_kind: None,
                    steps: None,
                },
                None,
            ),
        ] {
            let task_id = insert_task(
                &db,
                strategy_id,
                Some(a2a_task_id),
                StrategyTaskPhase::Running,
                PAST,
            )
            .await;
            let fake = Arc::new(FakeAgentTaskClient::new());
            fake.set_status(a2a_task_id, status).await;
            let agent_client: SharedAgentTaskClient = fake;

            let updated = run_once(&db, &agent_client).await;
            assert_eq!(updated, 1, "case {label}");

            let row = fetch_task(&db, task_id).await;
            assert_eq!(
                (row.phase, row.result_text, row.error_summary),
                (
                    StrategyTaskPhase::Failed,
                    expected_result_text,
                    Some("agent task exceeded deadline".to_string()),
                ),
                "case {label}",
            );
        }
    }

    // database_test は rstest の case 引数を扱わないため、for ループで列挙する。
    #[backend_test_macros::database_test]
    async fn deadline_exceeded_includes_agent_reported_reason(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db).await;

        for (label, a2a_task_id, error_message, error_kind, expected_error_summary) in [
            (
                "kind_only",
                "t-usage-limit",
                None,
                Some("usage_limit"),
                "agent task exceeded deadline (agent reported: usage_limit)",
            ),
            (
                "message_over_kind",
                "t-late-reason",
                Some("upstream returned 400"),
                Some("agent_error"),
                "agent task exceeded deadline (agent reported: upstream returned 400)",
            ),
        ] {
            let task_id = insert_task(
                &db,
                strategy_id,
                Some(a2a_task_id),
                StrategyTaskPhase::Running,
                PAST,
            )
            .await;

            let fake = Arc::new(FakeAgentTaskClient::new());
            fake.set_status(
                a2a_task_id,
                AgentTaskStatus {
                    state: AgentTaskState::Failed,
                    result_text: None,
                    error_message: error_message.map(str::to_string),
                    error_kind: error_kind.map(str::to_string),
                    steps: None,
                },
            )
            .await;
            let agent_client: SharedAgentTaskClient = fake;

            let updated = run_once(&db, &agent_client).await;
            assert_eq!(updated, 1, "case {label}");

            let row = fetch_task(&db, task_id).await;
            assert_eq!(
                (row.phase, row.error_summary),
                (
                    StrategyTaskPhase::Failed,
                    Some(expected_error_summary.to_string()),
                ),
                "case {label}",
            );
        }
    }

    #[backend_test_macros::database_test]
    async fn deadline_exceeded_still_upserts_steps_reported_by_agent(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-progress"),
            StrategyTaskPhase::Running,
            PAST,
        )
        .await;

        let step_id = Uuid::new_v4();
        let step = serde_json::json!({
            "execution_step_id": step_id,
            "phase_key": "investigate",
            "label": "仮説の調査",
            "model": "test-model",
            "status": "running",
            "started_at": "2026-01-01T00:00:00.000Z",
            "trace_id": "trace-1",
            "span_id": "span-1",
        });

        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_status(
            "t-progress",
            AgentTaskStatus {
                state: AgentTaskState::Working,
                result_text: None,
                error_message: None,
                error_kind: None,
                steps: Some(serde_json::json!([step])),
            },
        )
        .await;
        let agent_client: SharedAgentTaskClient = fake;

        let updated = run_once(&db, &agent_client).await;
        assert_eq!(updated, 1);

        let row = fetch_task(&db, task_id).await;
        assert_eq!(row.phase, StrategyTaskPhase::Failed);

        let steps = fetch_steps(&db, task_id).await;
        assert_eq!(steps.len(), 1);
        let seq = steps[0].seq;
        assert_eq!(
            steps[0],
            strategy_task_step::Model {
                execution_step_id: step_id,
                task_id,
                phase_key: "investigate".to_string(),
                label: "仮説の調査".to_string(),
                model: "test-model".to_string(),
                status: StrategyTaskStepStatus::Running,
                item: None,
                item_label: None,
                output: None,
                started_at: DateTime::parse_from_rfc3339("2026-01-01T00:00:00.000Z").unwrap(),
                finished_at: None,
                trace_id: "trace-1".to_string(),
                span_id: "span-1".to_string(),
                error: None,
                seq,
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn not_found_after_deadline_marks_failed(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("ghost"),
            StrategyTaskPhase::Running,
            PAST,
        )
        .await;
        let fake: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());

        let updated = run_once(&db, &fake).await;
        assert_eq!(updated, 1);

        let row = fetch_task(&db, task_id).await;
        assert_eq!(row.phase, StrategyTaskPhase::Failed);
        assert_eq!(
            row.error_summary,
            Some("agent task ghost not found".to_string())
        );
    }

    #[backend_test_macros::database_test]
    async fn not_found_before_deadline_is_skipped(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("fresh"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        let fake: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());

        let updated = run_once(&db, &fake).await;
        assert_eq!(updated, 0);

        let row = fetch_task(&db, task_id).await;
        assert_eq!(row.phase, StrategyTaskPhase::Running);
        assert_eq!(row.error_summary, None);
    }

    #[backend_test_macros::database_test]
    async fn orphaned_row_without_a2a_task_id_failed_after_deadline(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(&db, strategy_id, None, StrategyTaskPhase::Pending, PAST).await;
        let fake: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());

        let updated = run_once(&db, &fake).await;
        assert_eq!(updated, 1);

        let row = fetch_task(&db, task_id).await;
        assert_eq!(row.phase, StrategyTaskPhase::Failed);
        assert_eq!(
            row.error_summary,
            Some("agent task submission was not recorded".to_string())
        );
    }

    #[backend_test_macros::database_test]
    async fn orphaned_row_without_a2a_task_id_skipped_before_deadline(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            None,
            StrategyTaskPhase::Pending,
            FAR_FUTURE,
        )
        .await;
        let fake: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());

        let updated = run_once(&db, &fake).await;
        assert_eq!(updated, 0);

        let row = fetch_task(&db, task_id).await;
        assert_eq!(row.phase, StrategyTaskPhase::Pending);
    }

    #[backend_test_macros::database_test]
    async fn transient_error_after_deadline_marks_failed(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("flaky"),
            StrategyTaskPhase::Running,
            PAST,
        )
        .await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_get_error(AgentTaskError::Network("connection refused".to_string()))
            .await;
        let agent_client: SharedAgentTaskClient = fake;

        let updated = run_once(&db, &agent_client).await;
        assert_eq!(updated, 1);

        let row = fetch_task(&db, task_id).await;
        assert_eq!(row.phase, StrategyTaskPhase::Failed);
    }

    #[backend_test_macros::database_test]
    async fn transient_error_before_deadline_is_skipped(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("flaky"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_get_error(AgentTaskError::Network("connection refused".to_string()))
            .await;
        let agent_client: SharedAgentTaskClient = fake;

        let updated = run_once(&db, &agent_client).await;
        assert_eq!(updated, 0);

        let row = fetch_task(&db, task_id).await;
        assert_eq!(row.phase, StrategyTaskPhase::Running);
    }

    #[backend_test_macros::database_test]
    async fn apply_phase_upserts_steps_and_skips_unchanged(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-steps"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;

        let step_id = Uuid::new_v4();
        let running_step = serde_json::json!({
            "execution_step_id": step_id,
            "phase_key": "investigate",
            "label": "仮説の調査",
            "model": "test-model",
            "status": "running",
            "started_at": "2026-01-01T00:00:00.000Z",
            "trace_id": "trace-1",
            "span_id": "span-1",
        });
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();

        // 新規ステップは insert される。
        fake.set_status(
            "t-steps",
            AgentTaskStatus {
                state: AgentTaskState::Working,
                result_text: None,
                error_message: None,
                error_kind: None,
                steps: Some(serde_json::json!([running_step.clone()])),
            },
        )
        .await;
        let updated = run_once(&db, &agent_client).await;
        assert_eq!(
            (updated, without_sequence(fetch_steps(&db, task_id).await)),
            (
                1,
                vec![strategy_task_step::Model {
                    execution_step_id: step_id,
                    task_id,
                    phase_key: "investigate".to_string(),
                    label: "仮説の調査".to_string(),
                    model: "test-model".to_string(),
                    status: StrategyTaskStepStatus::Running,
                    item: None,
                    item_label: None,
                    output: None,
                    started_at: DateTime::parse_from_rfc3339("2026-01-01T00:00:00.000Z").unwrap(),
                    finished_at: None,
                    trace_id: "trace-1".to_string(),
                    span_id: "span-1".to_string(),
                    error: None,
                    seq: 0,
                }],
            ),
        );

        // 同じ内容の再送は upsert 対象にならない。
        let updated = run_once(&db, &agent_client).await;
        assert_eq!(
            (updated, without_sequence(fetch_steps(&db, task_id).await)),
            (
                0,
                vec![strategy_task_step::Model {
                    execution_step_id: step_id,
                    task_id,
                    phase_key: "investigate".to_string(),
                    label: "仮説の調査".to_string(),
                    model: "test-model".to_string(),
                    status: StrategyTaskStepStatus::Running,
                    item: None,
                    item_label: None,
                    output: None,
                    started_at: DateTime::parse_from_rfc3339("2026-01-01T00:00:00.000Z").unwrap(),
                    finished_at: None,
                    trace_id: "trace-1".to_string(),
                    span_id: "span-1".to_string(),
                    error: None,
                    seq: 0,
                }],
            ),
        );

        // status/output の変化は既存行 (同じ execution_step_id) を更新する。
        let completed_step = serde_json::json!({
            "execution_step_id": step_id,
            "phase_key": "investigate",
            "label": "仮説の調査",
            "model": "test-model",
            "status": "completed",
            "output": {"summary": "ok"},
            "started_at": "2026-01-01T00:00:00.000Z",
            "finished_at": "2026-01-01T00:00:05.000Z",
            "trace_id": "trace-1",
            "span_id": "span-1",
        });
        fake.set_status(
            "t-steps",
            AgentTaskStatus {
                state: AgentTaskState::Working,
                result_text: None,
                error_message: None,
                error_kind: None,
                steps: Some(serde_json::json!([completed_step])),
            },
        )
        .await;
        let updated = run_once(&db, &agent_client).await;
        assert_eq!(
            (updated, without_sequence(fetch_steps(&db, task_id).await)),
            (
                1,
                vec![strategy_task_step::Model {
                    execution_step_id: step_id,
                    task_id,
                    phase_key: "investigate".to_string(),
                    label: "仮説の調査".to_string(),
                    model: "test-model".to_string(),
                    status: StrategyTaskStepStatus::Completed,
                    item: None,
                    item_label: None,
                    output: Some(serde_json::json!({"summary": "ok"})),
                    started_at: DateTime::parse_from_rfc3339("2026-01-01T00:00:00.000Z").unwrap(),
                    finished_at: Some(
                        DateTime::parse_from_rfc3339("2026-01-01T00:00:05.000Z").unwrap(),
                    ),
                    trace_id: "trace-1".to_string(),
                    span_id: "span-1".to_string(),
                    error: None,
                    seq: 0,
                }],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn apply_phase_rolls_back_row_update_when_step_upsert_fails(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-txn"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;

        // 同じ execution_step_id を持つ 2 件を 1 回の upsert に含めると、Postgres が
        // 「ON CONFLICT DO UPDATE command cannot affect row a second time」で拒否する。
        // steps upsert 側の失敗を注入する手段として使う。
        let dup_id = Uuid::new_v4();
        let step = |status: &str| {
            serde_json::json!({
                "execution_step_id": dup_id,
                "phase_key": "investigate",
                "label": "仮説の調査",
                "model": "test-model",
                "status": status,
                "started_at": "2026-01-01T00:00:00.000Z",
                "trace_id": "trace-1",
                "span_id": "span-1",
            })
        };

        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_status(
            "t-txn",
            AgentTaskStatus {
                state: AgentTaskState::Completed,
                result_text: Some("done".to_string()),
                error_message: None,
                error_kind: None,
                steps: Some(serde_json::json!([step("running"), step("completed")])),
            },
        )
        .await;
        let agent_client: SharedAgentTaskClient = fake;
        assert_eq!(run_once(&db, &agent_client).await, 0);

        // steps upsert の失敗で phase 更新もロールバックされ、行は変化していないこと。
        let row = fetch_task(&db, task_id).await;
        assert_eq!(
            (row.phase, row.result_text, fetch_steps(&db, task_id).await),
            (StrategyTaskPhase::Running, None, vec![]),
        );
    }

    #[backend_test_macros::database_test]
    async fn execution_lost_failure_triggers_auto_resume(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-lost"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;

        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_status(
            "t-lost",
            AgentTaskStatus {
                state: AgentTaskState::Failed,
                result_text: None,
                error_message: None,
                error_kind: Some(EXECUTION_LOST_ERROR_KIND.to_string()),
                steps: None,
            },
        )
        .await;
        fake.set_next_task_id("agent-task-resumed").await;
        let agent_client: SharedAgentTaskClient = fake.clone();

        let updated = run_once(&db, &agent_client).await;
        assert_eq!(updated, 1);

        let row = fetch_task(&db, task_id).await;

        #[derive(Debug, PartialEq)]
        struct AutoResumeOutcome {
            phase: StrategyTaskPhase,
            a2a_task_id: Option<String>,
            auto_resumed_at_is_set: bool,
        }

        assert_eq!(
            AutoResumeOutcome {
                phase: row.phase,
                a2a_task_id: row.a2a_task_id,
                auto_resumed_at_is_set: row.auto_resumed_at.is_some(),
            },
            AutoResumeOutcome {
                phase: StrategyTaskPhase::Running,
                a2a_task_id: Some("agent-task-resumed".to_string()),
                auto_resumed_at_is_set: true,
            },
        );
        assert_eq!(fake.submitted.lock().await.len(), 1);
    }

    #[backend_test_macros::database_test]
    async fn execution_lost_failure_is_not_auto_resumed_twice(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-lost-again"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        // 既に自動 resume 済みの行を模擬する。
        strategy_task::ActiveModel {
            task_id: Set(task_id),
            auto_resumed_at: Set(Some(Utc::now().fixed_offset())),
            updated_at: Set(Utc::now().fixed_offset()),
            ..Default::default()
        }
        .update(&db)
        .await
        .unwrap();

        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_status(
            "t-lost-again",
            AgentTaskStatus {
                state: AgentTaskState::Failed,
                result_text: None,
                error_message: None,
                error_kind: Some(EXECUTION_LOST_ERROR_KIND.to_string()),
                steps: None,
            },
        )
        .await;
        let agent_client: SharedAgentTaskClient = fake.clone();

        let updated = run_once(&db, &agent_client).await;
        assert_eq!(updated, 1);

        assert_not_auto_resumed(&db, task_id, &fake).await;
    }

    #[backend_test_macros::database_test]
    async fn non_execution_lost_failure_is_not_auto_resumed(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-usage-limit-inflight"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;

        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_status(
            "t-usage-limit-inflight",
            AgentTaskStatus {
                state: AgentTaskState::Failed,
                result_text: None,
                error_message: None,
                error_kind: Some("usage_limit".to_string()),
                steps: None,
            },
        )
        .await;
        let agent_client: SharedAgentTaskClient = fake.clone();

        let updated = run_once(&db, &agent_client).await;
        assert_eq!(updated, 1);

        assert_not_auto_resumed(&db, task_id, &fake).await;
    }

    #[backend_test_macros::database_test]
    async fn execution_lost_failure_past_deadline_is_not_auto_resumed(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-lost-late"),
            StrategyTaskPhase::Running,
            PAST,
        )
        .await;

        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_status(
            "t-lost-late",
            AgentTaskStatus {
                state: AgentTaskState::Failed,
                result_text: None,
                error_message: None,
                error_kind: Some(EXECUTION_LOST_ERROR_KIND.to_string()),
                steps: None,
            },
        )
        .await;
        let agent_client: SharedAgentTaskClient = fake.clone();

        let updated = run_once(&db, &agent_client).await;
        assert_eq!(updated, 1);

        assert_not_auto_resumed(&db, task_id, &fake).await;
    }

    /// 自動 resume が起きなかったこと (phase が Failed のまま、agent への再投入が
    /// 発生していないこと) をまとめて検証する。
    async fn assert_not_auto_resumed(
        db: &impl sea_orm::ConnectionTrait,
        task_id: Uuid,
        fake: &FakeAgentTaskClient,
    ) {
        let row = fetch_task(db, task_id).await;
        assert_eq!(row.phase, StrategyTaskPhase::Failed);
        assert!(fake.submitted.lock().await.is_empty());
    }
}
