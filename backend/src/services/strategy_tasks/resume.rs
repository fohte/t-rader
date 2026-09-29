use uuid::Uuid;

pub use core_application::strategy_task::ResumeTaskError;
pub use core_application::strategy_task::SubmittedTask;

pub async fn resume_task<C>(
    db: &C,
    agent_client: &crate::agent_client::SharedAgentTaskClient,
    task_id: Uuid,
) -> Result<SubmittedTask, ResumeTaskError>
where
    C: sea_orm::ConnectionTrait + Clone + Into<gateway_postgres::DatabaseHandle>,
{
    crate::services::use_cases::build_use_cases(db.clone())
        .strategy_tasks
        .resume(agent_client.as_ref(), task_id)
        .await
}

pub async fn auto_resume_task<C>(
    db: &C,
    agent_client: &crate::agent_client::SharedAgentTaskClient,
    task_id: Uuid,
) -> Result<SubmittedTask, ResumeTaskError>
where
    C: sea_orm::ConnectionTrait + Clone + Into<gateway_postgres::DatabaseHandle>,
{
    crate::services::use_cases::build_use_cases(db.clone())
        .strategy_tasks
        .auto_resume(agent_client.as_ref(), task_id)
        .await
}
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::super::{StrategyTaskPhase, TaskSource, phase_str};
    use super::*;
    use crate::agent_client::{AgentTaskError, FakeAgentTaskClient, SharedAgentTaskClient};
    use crate::testing::insert_test_strategy;
    use gateway_postgres::entities::sea_orm_active_enums::StrategyTaskStepStatus;
    use gateway_postgres::entities::{strategy_task, strategy_task_step};
    use sea_orm::{
        ActiveModelTrait,
        ActiveValue::{NotSet, Set},
        EntityTrait,
    };

    /// resume 時の `now` と区別できるよう、投入時刻として十分に過去の固定値を使う。
    fn original_as_of() -> chrono::DateTime<chrono::FixedOffset> {
        chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap()
    }

    async fn insert_task_with_phase(
        db: &impl sea_orm::ConnectionTrait,
        strategy_id: Uuid,
        phase: StrategyTaskPhase,
        prompt: &str,
        purpose: Option<&str>,
    ) -> Uuid {
        let task_id = Uuid::new_v4();
        let now = chrono::Utc::now().fixed_offset();
        strategy_task::ActiveModel {
            task_id: Set(task_id),
            strategy_id: Set(strategy_id),
            a2a_task_id: Set(None),
            source: Set(TaskSource::Review.as_str().to_string()),
            prompt: Set(prompt.to_string()),
            phase: Set(phase),
            error_summary: Set(Some("boom".to_string())),
            result_text: Set(Some("stale result".to_string())),
            deadline_at: Set(now),
            purpose: Set(purpose.map(str::to_string)),
            as_of: Set(Some(original_as_of())),
            auto_resumed_at: NotSet,
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await
        .expect("insert test strategy_task");
        task_id
    }

    async fn insert_task_step(
        db: &impl sea_orm::ConnectionTrait,
        task_id: Uuid,
        execution_step_id: Uuid,
        phase_key: &str,
        status: StrategyTaskStepStatus,
        seq: i64,
    ) {
        strategy_task_step::ActiveModel {
            execution_step_id: Set(execution_step_id),
            task_id: Set(task_id),
            phase_key: Set(phase_key.to_string()),
            label: Set(phase_key.to_string()),
            model: Set("m".to_string()),
            status: Set(status),
            item: Set(None),
            item_label: Set(None),
            output: Set(None),
            started_at: Set(chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap()),
            finished_at: Set(None),
            trace_id: Set("trace-1".to_string()),
            span_id: Set("span-1".to_string()),
            error: Set(None),
            seq: Set(seq),
        }
        .insert(db)
        .await
        .expect("insert test strategy_task_step");
    }

    // database_test は rstest の case 引数を扱わないため、for ループで列挙する。
    #[backend_test_macros::database_test]
    async fn resume_task_rejects_a_task_that_is_neither_failed_nor_completed_with_a_failed_step(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_test_strategy(&db, "s").await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();

        let cases = [
            ("running_without_steps", StrategyTaskPhase::Running, None),
            // 実行中の for_each は失敗済みの要素を持ちうるが、まだ決着していないので対象外。
            (
                "running_with_failed_step",
                StrategyTaskPhase::Running,
                Some(StrategyTaskStepStatus::Failed),
            ),
            (
                "pending_with_failed_step",
                StrategyTaskPhase::Pending,
                Some(StrategyTaskStepStatus::Failed),
            ),
            (
                "completed_without_steps",
                StrategyTaskPhase::Completed,
                None,
            ),
            (
                "completed_with_only_completed_steps",
                StrategyTaskPhase::Completed,
                Some(StrategyTaskStepStatus::Completed),
            ),
        ];
        for (name, phase, step_status) in cases {
            let expected_phase = phase_str(&phase);
            let task_id = insert_task_with_phase(&db, strategy_id, phase, "p", None).await;
            if let Some(status) = step_status {
                insert_task_step(&db, task_id, Uuid::new_v4(), "investigate", status, 1).await;
            }

            let err = resume_task(&db, &agent_client, task_id)
                .await
                .expect_err(name);
            assert!(
                matches!(&err, ResumeTaskError::NotResumable(id, phase) if *id == task_id && *phase == expected_phase),
                "{name}: {err:?}",
            );
        }
        assert!(fake.submitted.lock().await.is_empty());
    }

    #[backend_test_macros::database_test]
    async fn resume_task_not_found(db: gateway_postgres::DatabaseHandle) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let task_id = Uuid::new_v4();

        let err = resume_task(&db, &agent_client, task_id)
            .await
            .expect_err("missing task should fail");
        assert!(matches!(err, ResumeTaskError::NotFound(id) if id == task_id));
    }

    // database_test は rstest の case 引数を扱わないため、for ループで列挙する。
    // Completed は for_each の部分失敗で failed なステップを抱えたまま終わったタスク。
    #[backend_test_macros::database_test]
    async fn resume_task_resubmits_all_steps_and_updates_the_row_in_place(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_test_strategy(&db, "s").await;
        for start_phase in [StrategyTaskPhase::Failed, StrategyTaskPhase::Completed] {
            let start_label = phase_str(&start_phase);
            let task_id = insert_task_with_phase(
                &db,
                strategy_id,
                start_phase,
                "prompt text",
                Some("explore"),
            )
            .await;
            let completed_step_id = Uuid::new_v4();
            let failed_step_id = Uuid::new_v4();
            insert_task_step(
                &db,
                task_id,
                completed_step_id,
                "plan",
                StrategyTaskStepStatus::Completed,
                1,
            )
            .await;
            insert_task_step(
                &db,
                task_id,
                failed_step_id,
                "investigate",
                StrategyTaskStepStatus::Failed,
                2,
            )
            .await;
            let fake = Arc::new(FakeAgentTaskClient::new());
            // a2a_task_id は UNIQUE のため周回ごとに変える。
            let a2a_task_id = format!("agent-task-resumed-{start_label}");
            fake.set_next_task_id(&a2a_task_id).await;
            let agent_client: SharedAgentTaskClient = fake.clone();
            let started_at = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                .unwrap()
                .to_rfc3339();

            let submitted = resume_task(&db, &agent_client, task_id)
                .await
                .expect("resume ok");

            let (submitted_resume_steps, submitted_as_of) = {
                let submitted = fake.submitted.lock().await;
                let req = submitted.first().expect("submit called once");
                (
                    req.resume_steps.clone().expect("resume_steps present"),
                    req.as_of,
                )
            };
            let row = strategy_task::Entity::find_by_id(task_id)
                .one(&db)
                .await
                .unwrap()
                .expect("row still exists");

            #[derive(Debug, PartialEq)]
            struct ResumeOutcome {
                submitted_a2a_task_id: String,
                resume_steps: Vec<serde_json::Value>,
                submitted_as_of: Option<chrono::DateTime<chrono::FixedOffset>>,
                row_a2a_task_id: Option<String>,
                row_phase: StrategyTaskPhase,
                row_error_summary: Option<String>,
                row_result_text: Option<String>,
                row_as_of: Option<chrono::DateTime<chrono::FixedOffset>>,
            }

            assert_eq!(
                ResumeOutcome {
                    submitted_a2a_task_id: submitted.a2a_task_id,
                    resume_steps: submitted_resume_steps,
                    submitted_as_of,
                    row_a2a_task_id: row.a2a_task_id,
                    row_phase: row.phase,
                    row_error_summary: row.error_summary,
                    row_result_text: row.result_text,
                    row_as_of: row.as_of,
                },
                ResumeOutcome {
                    submitted_a2a_task_id: a2a_task_id.clone(),
                    resume_steps: vec![
                        serde_json::json!({
                            "execution_step_id": completed_step_id,
                            "phase_key": "plan",
                            "label": "plan",
                            "model": "m",
                            "status": "completed",
                            "started_at": started_at,
                            "trace_id": "trace-1",
                            "span_id": "span-1",
                        }),
                        serde_json::json!({
                            "execution_step_id": failed_step_id,
                            "phase_key": "investigate",
                            "label": "investigate",
                            "model": "m",
                            "status": "failed",
                            "started_at": started_at,
                            "trace_id": "trace-1",
                            "span_id": "span-1",
                        }),
                    ],
                    submitted_as_of: Some(original_as_of()),
                    row_a2a_task_id: Some(a2a_task_id.clone()),
                    row_phase: StrategyTaskPhase::Running,
                    row_error_summary: None,
                    row_result_text: None,
                    row_as_of: Some(original_as_of()),
                },
                "start phase: {start_label}",
            );
        }
    }

    #[backend_test_macros::database_test]
    async fn resume_task_rejects_a_second_call_after_the_first_claims_the_row(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_test_strategy(&db, "s").await;
        let task_id =
            insert_task_with_phase(&db, strategy_id, StrategyTaskPhase::Failed, "p", None).await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();

        resume_task(&db, &agent_client, task_id)
            .await
            .expect("first resume claims the row");

        let err = resume_task(&db, &agent_client, task_id)
            .await
            .expect_err("second resume must not re-claim an already-running row");
        assert!(
            matches!(&err, ResumeTaskError::NotResumable(id, phase) if *id == task_id && *phase == "running")
        );
        assert_eq!(fake.submitted.lock().await.len(), 1);
    }

    #[backend_test_macros::database_test]
    async fn auto_resume_task_resubmits_and_marks_auto_resumed(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_test_strategy(&db, "s").await;
        let task_id =
            insert_task_with_phase(&db, strategy_id, StrategyTaskPhase::Failed, "p", None).await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_next_task_id("agent-task-auto-resumed").await;
        let agent_client: SharedAgentTaskClient = fake.clone();

        let submitted = auto_resume_task(&db, &agent_client, task_id)
            .await
            .expect("auto resume ok");
        let row = strategy_task::Entity::find_by_id(task_id)
            .one(&db)
            .await
            .unwrap()
            .expect("row still exists");

        #[derive(Debug, PartialEq)]
        struct AutoResumeOutcome {
            submitted_a2a_task_id: String,
            row_a2a_task_id: Option<String>,
            row_phase: StrategyTaskPhase,
            row_auto_resumed_at_is_set: bool,
        }

        assert_eq!(
            AutoResumeOutcome {
                submitted_a2a_task_id: submitted.a2a_task_id,
                row_a2a_task_id: row.a2a_task_id,
                row_phase: row.phase,
                row_auto_resumed_at_is_set: row.auto_resumed_at.is_some(),
            },
            AutoResumeOutcome {
                submitted_a2a_task_id: "agent-task-auto-resumed".to_string(),
                row_a2a_task_id: Some("agent-task-auto-resumed".to_string()),
                row_phase: StrategyTaskPhase::Running,
                row_auto_resumed_at_is_set: true,
            },
        );
        assert_eq!(fake.submitted.lock().await.len(), 1);
    }

    #[backend_test_macros::database_test]
    async fn auto_resume_task_rejects_a_second_call_once_already_auto_resumed(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_test_strategy(&db, "s").await;
        let task_id =
            insert_task_with_phase(&db, strategy_id, StrategyTaskPhase::Failed, "p", None).await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();

        auto_resume_task(&db, &agent_client, task_id)
            .await
            .expect("first auto resume claims the row");

        // 手動 resume で失敗を再現する代わりに、直接 phase を Failed に戻す。
        // auto_resumed_at は前回の claim で既に刻まれているため、この UPDATE では触らない。
        strategy_task::ActiveModel {
            task_id: Set(task_id),
            phase: Set(StrategyTaskPhase::Failed),
            updated_at: Set(chrono::Utc::now().fixed_offset()),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("force phase back to failed");

        let err = auto_resume_task(&db, &agent_client, task_id)
            .await
            .expect_err("second auto resume must not re-claim an already auto-resumed row");
        assert!(
            matches!(&err, ResumeTaskError::NotResumable(id, phase) if *id == task_id && *phase == "failed")
        );
        assert_eq!(fake.submitted.lock().await.len(), 1);
    }

    #[backend_test_macros::database_test]
    async fn auto_resume_task_marks_auto_resumed_at_even_when_submission_fails(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_test_strategy(&db, "s").await;
        let task_id =
            insert_task_with_phase(&db, strategy_id, StrategyTaskPhase::Failed, "p", None).await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_submit_error(AgentTaskError::NotConfigured).await;
        let agent_client: SharedAgentTaskClient = fake.clone();

        let err = auto_resume_task(&db, &agent_client, task_id)
            .await
            .expect_err("submission failure must surface as an error");
        assert!(matches!(err, ResumeTaskError::AgentTask(_)));

        let row = strategy_task::Entity::find_by_id(task_id)
            .one(&db)
            .await
            .unwrap()
            .expect("row still exists");
        assert!(row.auto_resumed_at.is_some());
    }

    #[backend_test_macros::database_test]
    async fn resume_task_does_not_touch_auto_resumed_at(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_test_strategy(&db, "s").await;
        let task_id =
            insert_task_with_phase(&db, strategy_id, StrategyTaskPhase::Failed, "p", None).await;
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());

        resume_task(&db, &agent_client, task_id)
            .await
            .expect("manual resume ok");

        let row = strategy_task::Entity::find_by_id(task_id)
            .one(&db)
            .await
            .unwrap()
            .expect("row still exists");
        assert!(row.auto_resumed_at.is_none());
    }
}
