//! 戦略タスク照合を DB と fake agent client で検証する integration test。

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
    use std::sync::atomic::{AtomicUsize, Ordering};

    use chrono::{DateTime, Utc};
    use core_application::agent_task_client::{
        AgentTaskClient, AgentTaskError, AgentTaskRef, AgentTaskState, AgentTaskStatus,
        EXECUTION_LOST_ERROR_KIND, FakeAgentTaskClient, SharedAgentTaskClient, SubmitAgentTask,
    };
    use core_application::event::{EventRepository, NewEvent};
    use core_application::event_dispatch_job::{
        EventDispatchJobQueue, EventDispatchJobQueueError, EventDispatchJobUseCases,
    };
    use core_application::strategy_task::{
        StrategyTaskRepository, StrategyTaskUseCases, TaskSource,
    };
    use core_application::trigger::{NewTrigger, TriggerKind, TriggerRepository};
    use core_application::unit_of_work::UnitOfWork;
    use gateway_postgres::entities::sea_orm_active_enums::{
        StrategyTaskPhase, StrategyTaskStepStatus,
    };
    use gateway_postgres::entities::{
        agent_config, event as event_entity, strategy, strategy_task, strategy_task_step,
    };
    use gateway_postgres::{
        PostgresEventRepository, PostgresStrategyTaskRepository, PostgresTriggerRepository,
        PostgresUnitOfWork,
    };
    use sea_orm::{
        ActiveModelTrait,
        ActiveValue::{NotSet, Set},
        ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, QueryFilter, QueryOrder,
        Statement,
    };
    use serde_json::{Value, json};
    use uuid::Uuid;

    use super::*;

    async fn insert_strategy(db: &impl sea_orm::ConnectionTrait) -> Uuid {
        insert_strategy_with_id(db, Uuid::new_v4()).await
    }

    async fn insert_strategy_with_id(db: &impl sea_orm::ConnectionTrait, id: Uuid) -> Uuid {
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
        insert_task_with_id(
            db,
            Uuid::new_v4(),
            strategy_id,
            a2a_task_id,
            phase,
            deadline_offset,
        )
        .await
    }

    async fn insert_task_with_id(
        db: &impl sea_orm::ConnectionTrait,
        task_id: Uuid,
        strategy_id: Uuid,
        a2a_task_id: Option<&str>,
        phase: StrategyTaskPhase,
        deadline_offset: chrono::Duration,
    ) -> Uuid {
        insert_task_with_id_and_purpose(
            db,
            task_id,
            strategy_id,
            a2a_task_id,
            phase,
            deadline_offset,
            None,
        )
        .await
    }

    async fn insert_task_with_id_and_purpose(
        db: &impl sea_orm::ConnectionTrait,
        task_id: Uuid,
        strategy_id: Uuid,
        a2a_task_id: Option<&str>,
        phase: StrategyTaskPhase,
        deadline_offset: chrono::Duration,
        purpose: Option<&str>,
    ) -> Uuid {
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
            purpose: Set(purpose.map(str::to_string)),
            as_of: NotSet,
            context_kind: NotSet,
            context_id: NotSet,
            auto_resumed_at: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .unwrap();
        task_id
    }

    async fn create_event_origin(
        db: &gateway_postgres::DatabaseHandle,
        strategy_id: Uuid,
        task_id: Uuid,
        chain_depth: i64,
    ) {
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let event_repository = PostgresEventRepository::new(db.clone());
        let trigger_repository = PostgresTriggerRepository::new(db.clone());
        let transaction = unit_of_work.begin().await.unwrap();
        let event = event_repository
            .create(
                &transaction,
                NewEvent {
                    event_type: "sample_process.started".to_string(),
                    payload: json!({}),
                    chain_depth: Some(chain_depth),
                },
            )
            .await
            .unwrap();
        let trigger_id = Uuid::new_v4();
        trigger_repository
            .create(
                &transaction,
                NewTrigger {
                    trigger_id,
                    strategy_id,
                    purpose: None,
                    kind: TriggerKind::Event,
                    schedule: None,
                    hook_slug: None,
                    event_type: Some("sample_process.started".to_string()),
                    event_match: None,
                    prompt_template: "sample prompt".to_string(),
                    enabled: true,
                    business_days_only: false,
                },
            )
            .await
            .unwrap();
        event_repository
            .insert_firing(&transaction, event.event_id, trigger_id, task_id)
            .await
            .unwrap();
        unit_of_work.commit(transaction).await.unwrap();
    }

    async fn ensure_event_dispatch_job_schema() {
        let pool = gateway_postgres::test_support::create_test_pool().await;
        crate::migrations::migrate_graphile_worker_schema(pool)
            .await
            .expect("migrate Graphile Worker schema");
    }

    async fn event_dispatch_job_count(db: &impl ConnectionTrait) -> i64 {
        let statement = Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT count(*) AS job_count FROM graphile_worker._private_jobs WHERE key = $1"
                .to_string(),
            ["event_dispatch".into()],
        );
        db.query_one_raw(statement)
            .await
            .unwrap()
            .unwrap()
            .try_get("", "job_count")
            .unwrap()
    }

    async fn fetch_events(db: &impl ConnectionTrait, event_types: &[&str]) -> Vec<Value> {
        event_entity::Entity::find()
            .filter(event_entity::Column::EventType.is_in(event_types.iter().copied()))
            .all(db)
            .await
            .unwrap()
            .into_iter()
            .map(|event| {
                json!({
                    "event_type": event.event_type,
                    "payload": event.payload,
                    "chain_depth": event.chain_depth,
                })
            })
            .collect()
    }

    async fn insert_agent_config(db: &impl ConnectionTrait, purpose: &str) {
        agent_config::ActiveModel {
            id: Set(Uuid::new_v4()),
            purpose: Set(purpose.to_string()),
            agents_md: NotSet,
            skills: NotSet,
            agent_graph: Set(String::new()),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .unwrap();
    }

    fn sort_events(events: &mut [Value]) {
        events.sort_by(|left, right| {
            left["payload"]["task_id"]
                .as_str()
                .cmp(&right["payload"]["task_id"].as_str())
        });
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

    #[derive(Default)]
    struct FailingEventDispatchJobQueue {
        enqueue_calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl EventDispatchJobQueue for FailingEventDispatchJobQueue {
        async fn enqueue_dispatch(&self) -> Result<(), EventDispatchJobQueueError> {
            self.enqueue_calls.fetch_add(1, Ordering::SeqCst);
            Err(EventDispatchJobQueueError::new(
                "simulated queue failure".to_string(),
            ))
        }
    }

    struct BlockingSubmitAgentTaskClient {
        fake: FakeAgentTaskClient,
        submit_started: tokio::sync::Notify,
        release_submit: tokio::sync::Notify,
    }

    impl BlockingSubmitAgentTaskClient {
        fn new() -> Self {
            Self {
                fake: FakeAgentTaskClient::new(),
                submit_started: tokio::sync::Notify::new(),
                release_submit: tokio::sync::Notify::new(),
            }
        }
    }

    #[async_trait::async_trait]
    impl AgentTaskClient for BlockingSubmitAgentTaskClient {
        async fn submit(&self, request: SubmitAgentTask) -> Result<AgentTaskRef, AgentTaskError> {
            self.submit_started.notify_one();
            self.release_submit.notified().await;
            self.fake.submit(request).await
        }

        async fn get(&self, task_id: &str) -> Result<AgentTaskStatus, AgentTaskError> {
            self.fake.get(task_id).await
        }
    }

    #[backend_test_macros::database_test]
    async fn reconciles_completed_running_and_failed_states(db: gateway_postgres::DatabaseHandle) {
        ensure_event_dispatch_job_schema().await;
        let strategy_id = insert_strategy_with_id(&db, Uuid::from_u128(0x100)).await;
        let completed_id = insert_task_with_id_and_purpose(
            &db,
            Uuid::from_u128(0x101),
            strategy_id,
            Some("t-completed"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
            Some("demo-purpose"),
        )
        .await;
        let failed_id = insert_task_with_id(
            &db,
            Uuid::from_u128(0x102),
            strategy_id,
            Some("t-failed"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        let running_id = insert_task_with_id(
            &db,
            Uuid::from_u128(0x103),
            strategy_id,
            Some("t-running"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        let event_started_task_id = insert_task_with_id(
            &db,
            Uuid::from_u128(0x104),
            strategy_id,
            Some("t-event-started"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        create_event_origin(&db, strategy_id, event_started_task_id, 4).await;

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
                error_message: Some("sample failure".to_string()),
                error_kind: None,
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
        fake.set_status(
            "t-event-started",
            AgentTaskStatus {
                state: AgentTaskState::Completed,
                result_text: Some("linked result".to_string()),
                error_message: None,
                error_kind: None,
                steps: None,
            },
        )
        .await;

        let agent_client: SharedAgentTaskClient = fake.clone();
        let updated = run_once(&db, &agent_client).await;

        let completed = fetch_task(&db, completed_id).await;
        let failed = fetch_task(&db, failed_id).await;
        let running = fetch_task(&db, running_id).await;
        let mut events =
            fetch_events(&db, &["strategy_task.completed", "strategy_task.failed"]).await;
        sort_events(&mut events);
        let mut expected_events = vec![
            json!({
                "event_type": "strategy_task.completed",
                "payload": {
                    "task_id": completed_id,
                    "strategy_id": strategy_id,
                    "purpose": "demo-purpose",
                    "result_text": "all good",
                    "error_summary": null,
                },
                "chain_depth": 0,
            }),
            json!({
                "event_type": "strategy_task.failed",
                "payload": {
                    "task_id": failed_id,
                    "strategy_id": strategy_id,
                    "purpose": null,
                    "result_text": null,
                    "error_summary": "sample failure",
                },
                "chain_depth": 0,
            }),
            json!({
                "event_type": "strategy_task.completed",
                "payload": {
                    "task_id": event_started_task_id,
                    "strategy_id": strategy_id,
                    "purpose": null,
                    "result_text": "linked result",
                    "error_summary": null,
                },
                "chain_depth": 5,
            }),
        ];
        sort_events(&mut expected_events);
        let actual = (
            updated,
            (
                completed.phase,
                completed.result_text,
                completed.error_summary,
            ),
            (failed.phase, failed.result_text, failed.error_summary),
            (running.phase, running.result_text, running.error_summary),
            events,
            event_dispatch_job_count(&db).await,
        );

        assert_eq!(
            actual,
            (
                3,
                (
                    StrategyTaskPhase::Completed,
                    Some("all good".to_string()),
                    None,
                ),
                (
                    StrategyTaskPhase::Failed,
                    None,
                    Some("sample failure".to_string()),
                ),
                (StrategyTaskPhase::Running, None, None),
                expected_events,
                1,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn submit_failure_emits_event_and_enqueues_dispatch(
        db: gateway_postgres::DatabaseHandle,
    ) {
        ensure_event_dispatch_job_schema().await;
        let strategy_id = insert_strategy(&db).await;
        insert_agent_config(&db, "demo-purpose").await;
        let fake = FakeAgentTaskClient::new();
        fake.set_submit_error(AgentTaskError::NotConfigured).await;

        let result = crate::services::use_cases::build_use_cases(db.clone())
            .strategy_tasks()
            .submit_task(
                &fake,
                strategy_id,
                "sample prompt",
                TaskSource::Review,
                Some("demo-purpose".to_string()),
            )
            .await;
        let row = strategy_task::Entity::find()
            .filter(strategy_task::Column::StrategyId.eq(strategy_id))
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        let events = fetch_events(&db, &["strategy_task.failed"]).await;

        assert_eq!(
            (
                matches!(
                    result,
                    Err(core_application::strategy_task::SubmitTaskError::AgentTask(
                        _
                    ))
                ),
                row.phase,
                events,
                event_dispatch_job_count(&db).await,
            ),
            (
                true,
                StrategyTaskPhase::Failed,
                vec![json!({
                    "event_type": "strategy_task.failed",
                    "payload": {
                        "task_id": row.task_id,
                        "strategy_id": strategy_id,
                        "purpose": "demo-purpose",
                        "result_text": null,
                        "error_summary": "agent task submission failed: agent task client is not configured",
                    },
                    "chain_depth": 0,
                })],
                1,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn resume_submission_failure_emits_event_and_enqueues_dispatch(
        db: gateway_postgres::DatabaseHandle,
    ) {
        ensure_event_dispatch_job_schema().await;
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-resume-failure"),
            StrategyTaskPhase::Failed,
            FAR_FUTURE,
        )
        .await;
        let fake = FakeAgentTaskClient::new();
        fake.set_submit_error(AgentTaskError::NotConfigured).await;

        let result = crate::services::use_cases::build_use_cases(db.clone())
            .strategy_tasks()
            .resume(&fake, task_id)
            .await;
        let row = fetch_task(&db, task_id).await;
        let events = fetch_events(&db, &["strategy_task.failed"]).await;

        assert_eq!(
            (
                matches!(
                    result,
                    Err(core_application::strategy_task::ResumeTaskError::AgentTask(
                        _
                    ))
                ),
                row.phase,
                row.error_summary,
                events,
                event_dispatch_job_count(&db).await,
            ),
            (
                true,
                StrategyTaskPhase::Failed,
                Some(
                    "agent task resume submission failed: agent task client is not configured"
                        .to_string()
                ),
                vec![json!({
                    "event_type": "strategy_task.failed",
                    "payload": {
                        "task_id": task_id,
                        "strategy_id": strategy_id,
                        "purpose": null,
                        "result_text": null,
                        "error_summary": "agent task resume submission failed: agent task client is not configured",
                    },
                    "chain_depth": 0,
                })],
                1,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn resume_claim_refreshes_deadline_before_agent_submission(
        db: gateway_postgres::DatabaseHandle,
    ) {
        ensure_event_dispatch_job_schema().await;
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-resume-deadline"),
            StrategyTaskPhase::Failed,
            PAST,
        )
        .await;
        let client = Arc::new(BlockingSubmitAgentTaskClient::new());
        client.fake.set_next_task_id("t-resumed-deadline").await;
        let strategy_tasks = StrategyTaskUseCases::new(
            Arc::new(PostgresUnitOfWork::new(db.clone())),
            Arc::new(PostgresStrategyTaskRepository::new(db.clone())),
        )
        .with_completion_events(
            Arc::new(PostgresEventRepository::new(db.clone())),
            EventDispatchJobUseCases::new(Arc::new(FailingEventDispatchJobQueue::default())),
        );
        let resume_client = client.clone();
        let resume =
            tokio::spawn(
                async move { strategy_tasks.resume(resume_client.as_ref(), task_id).await },
            );
        client.submit_started.notified().await;

        let claimed = fetch_task(&db, task_id).await;
        let client_shared: SharedAgentTaskClient = client.clone();
        let reconciled = run_once(&db, &client_shared).await;
        let after_reconcile = fetch_task(&db, task_id).await;
        client.release_submit.notify_one();
        let resume_result = resume.await.unwrap();
        let finished = fetch_task(&db, task_id).await;
        let submitted_deadline = client.fake.submitted.lock().await[0].deadline_at;
        let events = fetch_events(&db, &["strategy_task.failed"]).await;

        assert_eq!(
            (
                resume_result.is_ok(),
                claimed.phase,
                claimed.a2a_task_id,
                claimed.deadline_at > Utc::now().fixed_offset(),
                reconciled,
                after_reconcile.phase,
                after_reconcile.a2a_task_id,
                finished.phase,
                finished.a2a_task_id,
                finished.deadline_at == submitted_deadline,
                events,
            ),
            (
                true,
                StrategyTaskPhase::Running,
                None,
                true,
                0,
                StrategyTaskPhase::Running,
                None,
                StrategyTaskPhase::Running,
                Some("t-resumed-deadline".to_string()),
                true,
                vec![],
            ),
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
    async fn stale_reconcile_does_not_update_steps_for_terminal_task(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-stale-steps"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        let step_id = Uuid::new_v4();
        let started_at = DateTime::parse_from_rfc3339("2026-01-01T00:00:00.000Z").unwrap();
        strategy_task_step::ActiveModel {
            execution_step_id: Set(step_id),
            task_id: Set(task_id),
            phase_key: Set("sample-phase".to_string()),
            label: Set("Sample phase".to_string()),
            model: Set("example-model".to_string()),
            status: Set(StrategyTaskStepStatus::Running),
            item: Set(None),
            item_label: Set(None),
            output: Set(None),
            started_at: Set(started_at),
            finished_at: Set(None),
            trace_id: Set("trace-before".to_string()),
            span_id: Set("span-before".to_string()),
            error: Set(None),
            seq: NotSet,
        }
        .insert(&db)
        .await
        .unwrap();
        let repository = Arc::new(PostgresStrategyTaskRepository::new(db.clone()));
        let stale_task = repository.find_by_id(task_id).await.unwrap().unwrap();
        strategy_task::ActiveModel {
            task_id: Set(task_id),
            phase: Set(StrategyTaskPhase::Completed),
            updated_at: Set(Utc::now().fixed_offset()),
            ..Default::default()
        }
        .update(&db)
        .await
        .unwrap();

        let fake = FakeAgentTaskClient::new();
        fake.set_status(
            "t-stale-steps",
            AgentTaskStatus {
                state: AgentTaskState::Working,
                result_text: None,
                error_message: None,
                error_kind: None,
                steps: Some(json!([{
                    "execution_step_id": step_id,
                    "phase_key": "sample-phase",
                    "label": "Sample phase",
                    "model": "example-model",
                    "status": "completed",
                    "output": {"summary": "stale report"},
                    "started_at": "2026-01-01T00:00:00.000Z",
                    "finished_at": "2026-01-01T00:00:05.000Z",
                    "trace_id": "trace-after",
                    "span_id": "span-after",
                }])),
            },
        )
        .await;
        let updated = crate::services::use_cases::build_use_cases(db.clone())
            .strategy_tasks()
            .reconcile_task(&fake, stale_task, Utc::now().fixed_offset())
            .await
            .unwrap();

        assert_eq!(
            (
                updated,
                fetch_task(&db, task_id).await.phase,
                without_sequence(fetch_steps(&db, task_id).await),
            ),
            (
                false,
                StrategyTaskPhase::Completed,
                vec![strategy_task_step::Model {
                    execution_step_id: step_id,
                    task_id,
                    phase_key: "sample-phase".to_string(),
                    label: "Sample phase".to_string(),
                    model: "example-model".to_string(),
                    status: StrategyTaskStepStatus::Running,
                    item: None,
                    item_label: None,
                    output: None,
                    started_at,
                    finished_at: None,
                    trace_id: "trace-before".to_string(),
                    span_id: "span-before".to_string(),
                    error: None,
                    seq: 0,
                }],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn stale_reconcile_does_not_update_steps_after_a2a_task_id_changes(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-stale-steps-old"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        let step_id = Uuid::new_v4();
        let started_at = DateTime::parse_from_rfc3339("2026-01-01T00:00:00.000Z").unwrap();
        strategy_task_step::ActiveModel {
            execution_step_id: Set(step_id),
            task_id: Set(task_id),
            phase_key: Set("sample-phase".to_string()),
            label: Set("Sample phase".to_string()),
            model: Set("example-model".to_string()),
            status: Set(StrategyTaskStepStatus::Running),
            item: Set(None),
            item_label: Set(None),
            output: Set(None),
            started_at: Set(started_at),
            finished_at: Set(None),
            trace_id: Set("trace-before".to_string()),
            span_id: Set("span-before".to_string()),
            error: Set(None),
            seq: NotSet,
        }
        .insert(&db)
        .await
        .unwrap();
        let repository = Arc::new(PostgresStrategyTaskRepository::new(db.clone()));
        let stale_task = repository.find_by_id(task_id).await.unwrap().unwrap();
        strategy_task::ActiveModel {
            task_id: Set(task_id),
            a2a_task_id: Set(Some("t-stale-steps-new".to_string())),
            updated_at: Set(Utc::now().fixed_offset()),
            ..Default::default()
        }
        .update(&db)
        .await
        .unwrap();

        let fake = FakeAgentTaskClient::new();
        fake.set_status(
            "t-stale-steps-old",
            AgentTaskStatus {
                state: AgentTaskState::Working,
                result_text: None,
                error_message: None,
                error_kind: None,
                steps: Some(json!([{
                    "execution_step_id": step_id,
                    "phase_key": "sample-phase",
                    "label": "Sample phase",
                    "model": "example-model",
                    "status": "completed",
                    "output": {"summary": "stale report"},
                    "started_at": "2026-01-01T00:00:00.000Z",
                    "finished_at": "2026-01-01T00:00:05.000Z",
                    "trace_id": "trace-after",
                    "span_id": "span-after",
                }])),
            },
        )
        .await;
        let updated = crate::services::use_cases::build_use_cases(db.clone())
            .strategy_tasks()
            .reconcile_task(&fake, stale_task, Utc::now().fixed_offset())
            .await
            .unwrap();
        let row = fetch_task(&db, task_id).await;

        assert_eq!(
            (
                updated,
                row.phase,
                row.a2a_task_id,
                without_sequence(fetch_steps(&db, task_id).await),
            ),
            (
                false,
                StrategyTaskPhase::Running,
                Some("t-stale-steps-new".to_string()),
                vec![strategy_task_step::Model {
                    execution_step_id: step_id,
                    task_id,
                    phase_key: "sample-phase".to_string(),
                    label: "Sample phase".to_string(),
                    model: "example-model".to_string(),
                    status: StrategyTaskStepStatus::Running,
                    item: None,
                    item_label: None,
                    output: None,
                    started_at,
                    finished_at: None,
                    trace_id: "trace-before".to_string(),
                    span_id: "span-before".to_string(),
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
    async fn execution_lost_auto_resume_does_not_emit_failed_event(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
        let repository = Arc::new(PostgresStrategyTaskRepository::new(db.clone()));
        let dispatch_queue = Arc::new(FailingEventDispatchJobQueue::default());
        let strategy_tasks = StrategyTaskUseCases::new(
            Arc::new(PostgresUnitOfWork::new(db.clone())),
            repository.clone(),
        )
        .with_completion_events(
            Arc::new(PostgresEventRepository::new(db.clone())),
            EventDispatchJobUseCases::new(dispatch_queue.clone()),
        );
        let task = repository.find_by_id(task_id).await.unwrap().unwrap();

        let updated = strategy_tasks
            .reconcile_task(fake.as_ref(), task.clone(), Utc::now().fixed_offset())
            .await
            .unwrap();
        let stale_reconcile_updated = strategy_tasks
            .reconcile_task(fake.as_ref(), task, Utc::now().fixed_offset())
            .await
            .unwrap();

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
        let events = fetch_events(&db, &["strategy_task.failed"]).await;
        let actual = (
            updated,
            stale_reconcile_updated,
            fake.submitted.lock().await.len(),
            events,
            dispatch_queue.enqueue_calls.load(Ordering::SeqCst),
        );
        assert_eq!(actual, (true, false, 1, vec![], 0),);
    }

    #[backend_test_macros::database_test]
    async fn execution_lost_that_cannot_be_auto_resumed_emits_failed_event(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-lost-already-resumed"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        let repository = Arc::new(PostgresStrategyTaskRepository::new(db.clone()));
        let stale_task = repository.find_by_id(task_id).await.unwrap().unwrap();
        strategy_task::ActiveModel {
            task_id: Set(task_id),
            auto_resumed_at: Set(Some(Utc::now().fixed_offset())),
            updated_at: Set(Utc::now().fixed_offset()),
            ..Default::default()
        }
        .update(&db)
        .await
        .unwrap();

        let fake = FakeAgentTaskClient::new();
        fake.set_status(
            "t-lost-already-resumed",
            AgentTaskStatus {
                state: AgentTaskState::Failed,
                result_text: None,
                error_message: None,
                error_kind: Some(EXECUTION_LOST_ERROR_KIND.to_string()),
                steps: None,
            },
        )
        .await;
        let dispatch_queue = Arc::new(FailingEventDispatchJobQueue::default());
        let strategy_tasks =
            StrategyTaskUseCases::new(Arc::new(PostgresUnitOfWork::new(db.clone())), repository)
                .with_completion_events(
                    Arc::new(PostgresEventRepository::new(db.clone())),
                    EventDispatchJobUseCases::new(dispatch_queue.clone()),
                );

        let updated = strategy_tasks
            .reconcile_task(&fake, stale_task, Utc::now().fixed_offset())
            .await
            .unwrap();
        let row = fetch_task(&db, task_id).await;
        let events = fetch_events(&db, &["strategy_task.failed"]).await;

        assert_eq!(
            (
                updated,
                row.phase,
                fake.submitted.lock().await.len(),
                events,
                dispatch_queue.enqueue_calls.load(Ordering::SeqCst),
            ),
            (
                true,
                StrategyTaskPhase::Failed,
                0,
                vec![json!({
                    "event_type": "strategy_task.failed",
                    "payload": {
                        "task_id": task_id,
                        "strategy_id": strategy_id,
                        "purpose": null,
                        "result_text": null,
                        "error_summary": EXECUTION_LOST_ERROR_KIND,
                    },
                    "chain_depth": 0,
                })],
                1,
            ),
        );
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

    struct BarrierAgentTaskClient {
        barrier: tokio::sync::Barrier,
        status: AgentTaskStatus,
    }

    #[async_trait::async_trait]
    impl AgentTaskClient for BarrierAgentTaskClient {
        async fn submit(
            &self,
            _request: core_application::agent_task_client::SubmitAgentTask,
        ) -> Result<AgentTaskRef, AgentTaskError> {
            Err(AgentTaskError::NotConfigured)
        }

        async fn get(&self, _task_id: &str) -> Result<AgentTaskStatus, AgentTaskError> {
            self.barrier.wait().await;
            Ok(self.status.clone())
        }
    }

    #[backend_test_macros::database_test]
    async fn concurrent_reconcile_emits_one_completion_event(
        _database: gateway_postgres::DatabaseHandle,
    ) {
        ensure_event_dispatch_job_schema().await;
        let pool = gateway_postgres::test_support::create_test_pool_with_max_connections(4).await;
        let connection = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
        let db = gateway_postgres::DatabaseHandle::from(connection);
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("sample-agent-task"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        let task = PostgresStrategyTaskRepository::new(db.clone())
            .find_by_id(task_id)
            .await
            .unwrap()
            .unwrap();
        let strategy_tasks =
            crate::services::use_cases::build_use_cases(db.clone()).strategy_tasks();
        let client = Arc::new(BarrierAgentTaskClient {
            barrier: tokio::sync::Barrier::new(2),
            status: AgentTaskStatus {
                state: AgentTaskState::Completed,
                result_text: Some("sample result".to_string()),
                error_message: None,
                error_kind: None,
                steps: None,
            },
        });
        let now = Utc::now().fixed_offset();
        let (left, right) = tokio::join!(
            strategy_tasks.reconcile_task(client.as_ref(), task.clone(), now),
            strategy_tasks.reconcile_task(client.as_ref(), task, now),
        );
        let mut changed = vec![left.unwrap(), right.unwrap()];
        changed.sort();

        let task_id_text = task_id.to_string();
        let event_rows = event_entity::Entity::find()
            .filter(event_entity::Column::EventType.eq("strategy_task.completed"))
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .filter(|event| event.payload["task_id"].as_str() == Some(task_id_text.as_str()))
            .collect::<Vec<_>>();
        let event_ids = event_rows
            .iter()
            .map(|event| event.event_id)
            .collect::<Vec<_>>();
        let event_output = event_rows
            .iter()
            .map(|event| {
                json!({
                    "event_type": event.event_type,
                    "task_id": "task",
                    "chain_depth": event.chain_depth,
                })
            })
            .collect::<Vec<_>>();
        let actual = (
            changed,
            event_output,
            fetch_task(&db, task_id).await.phase,
            event_dispatch_job_count(&db).await,
        );

        for event_id in event_ids {
            event_entity::Entity::delete_by_id(event_id)
                .exec(&db)
                .await
                .unwrap();
        }
        db.execute_unprepared(
            "DELETE FROM graphile_worker._private_jobs WHERE key = 'event_dispatch'",
        )
        .await
        .unwrap();
        strategy_task::Entity::delete_by_id(task_id)
            .exec(&db)
            .await
            .unwrap();
        strategy::Entity::delete_by_id(strategy_id)
            .exec(&db)
            .await
            .unwrap();

        assert_eq!(
            actual,
            (
                vec![false, true],
                vec![json!({
                    "event_type": "strategy_task.completed",
                    "task_id": "task",
                    "chain_depth": 0,
                })],
                StrategyTaskPhase::Completed,
                1,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn execution_lost_auto_resume_submission_failure_emits_one_failed_event(
        db: gateway_postgres::DatabaseHandle,
    ) {
        ensure_event_dispatch_job_schema().await;
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-lost-resume-failure"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_status(
            "t-lost-resume-failure",
            AgentTaskStatus {
                state: AgentTaskState::Failed,
                result_text: None,
                error_message: None,
                error_kind: Some(EXECUTION_LOST_ERROR_KIND.to_string()),
                steps: None,
            },
        )
        .await;
        fake.set_submit_error(AgentTaskError::NotConfigured).await;

        let agent_client: SharedAgentTaskClient = fake.clone();
        let updated = run_once(&db, &agent_client).await;
        let row = fetch_task(&db, task_id).await;
        let events = fetch_events(&db, &["strategy_task.failed"]).await;

        assert_eq!(
            (
                updated,
                row.phase,
                row.error_summary,
                row.auto_resumed_at.is_some(),
                events,
                event_dispatch_job_count(&db).await,
            ),
            (
                1,
                StrategyTaskPhase::Failed,
                Some(
                    "agent task resume submission failed: agent task client is not configured"
                        .to_string()
                ),
                true,
                vec![json!({
                    "event_type": "strategy_task.failed",
                    "payload": {
                        "task_id": task_id,
                        "strategy_id": strategy_id,
                        "purpose": null,
                        "result_text": null,
                        "error_summary": "agent task resume submission failed: agent task client is not configured",
                    },
                    "chain_depth": 0,
                })],
                1,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn dispatch_enqueue_failure_keeps_failed_transition_and_event(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db).await;
        let task_id = insert_task(
            &db,
            strategy_id,
            Some("t-dispatch-failure"),
            StrategyTaskPhase::Running,
            FAR_FUTURE,
        )
        .await;
        let fake = FakeAgentTaskClient::new();
        fake.set_status(
            "t-dispatch-failure",
            AgentTaskStatus {
                state: AgentTaskState::Failed,
                result_text: None,
                error_message: Some("sample failure".to_string()),
                error_kind: None,
                steps: None,
            },
        )
        .await;
        let repository = Arc::new(PostgresStrategyTaskRepository::new(db.clone()));
        let dispatch_queue = Arc::new(FailingEventDispatchJobQueue::default());
        let strategy_tasks = StrategyTaskUseCases::new(
            Arc::new(PostgresUnitOfWork::new(db.clone())),
            repository.clone(),
        )
        .with_completion_events(
            Arc::new(PostgresEventRepository::new(db.clone())),
            EventDispatchJobUseCases::new(dispatch_queue.clone()),
        );
        let task = repository.find_by_id(task_id).await.unwrap().unwrap();

        let updated = strategy_tasks
            .reconcile_task(&fake, task, Utc::now().fixed_offset())
            .await
            .unwrap();
        let events = fetch_events(&db, &["strategy_task.failed"]).await;

        assert_eq!(
            (
                updated,
                fetch_task(&db, task_id).await.phase,
                events,
                dispatch_queue.enqueue_calls.load(Ordering::SeqCst),
            ),
            (
                true,
                StrategyTaskPhase::Failed,
                vec![json!({
                    "event_type": "strategy_task.failed",
                    "payload": {
                        "task_id": task_id,
                        "strategy_id": strategy_id,
                        "purpose": null,
                        "result_text": null,
                        "error_summary": "sample failure",
                    },
                    "chain_depth": 0,
                })],
                1,
            ),
        );
    }
}
