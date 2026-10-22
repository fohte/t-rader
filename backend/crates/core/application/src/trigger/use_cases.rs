use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::agent_task_client::AgentTaskClient;
use crate::change_history::{Actor, ChangeHistoryRecord, Op, SharedChangeHistoryPort, TargetKind};
use crate::strategy::SharedStrategyRepository;
use crate::strategy_existence::SharedStrategyExistence;
use crate::strategy_scope::StrategyScope;
use crate::strategy_task::{
    PreparedStrategyTaskSubmission, StrategyTaskUseCases, SubmittedTask, TaskSource,
};
use crate::unit_of_work::{SharedUnitOfWork, UnitOfWorkTransaction};

use self::commands::{apply_update, ensure_scope, validate_create};
use self::validation::validate_event_match;
use super::error::TriggerUseCaseError;
use super::repository::SharedTriggerRepository;
use super::template::{build_standard_context, evaluate_event_match, expand_template};
use super::types::{CreateTriggerCommand, NewTrigger, Trigger, TriggerKind, UpdateTriggerCommand};

mod commands;
mod cron_timer;
mod validation;

#[derive(Clone)]
pub struct TriggerUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedTriggerRepository,
    strategy_existence: SharedStrategyExistence,
    strategy_repository: SharedStrategyRepository,
    strategy_tasks: StrategyTaskUseCases,
    change_history: SharedChangeHistoryPort,
}

impl TriggerUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedTriggerRepository,
        strategy_existence: SharedStrategyExistence,
        strategy_repository: SharedStrategyRepository,
        strategy_tasks: StrategyTaskUseCases,
        change_history: SharedChangeHistoryPort,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            strategy_existence,
            strategy_repository,
            strategy_tasks,
            change_history,
        }
    }

    pub async fn list_for_strategy(
        &self,
        scope: StrategyScope,
        kind: Option<TriggerKind>,
    ) -> Result<Vec<Trigger>, TriggerUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let triggers = self
            .repository
            .list_for_strategy(&transaction, scope.id(), kind)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(triggers)
    }

    pub async fn get(&self, trigger_id: Uuid) -> Result<Trigger, TriggerUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let trigger = self
            .repository
            .find_by_id_in_transaction(&transaction, trigger_id)
            .await?
            .ok_or(TriggerUseCaseError::NotFound(trigger_id))?;
        self.unit_of_work.commit(transaction).await?;
        Ok(trigger)
    }

    pub async fn create(
        &self,
        scope: StrategyScope,
        command: CreateTriggerCommand,
    ) -> Result<Trigger, TriggerUseCaseError> {
        let purpose = match command.purpose.clone() {
            Some(purpose) => Some(self.validate_purpose(purpose).await?),
            None => None,
        };
        let validated = validate_create(&command)?;
        validate_event_match(command.event_match.as_ref())?;

        let transaction = self.unit_of_work.begin().await?;
        if !self
            .strategy_existence
            .exists(&transaction, scope.id())
            .await?
        {
            return Err(TriggerUseCaseError::NotFound(scope.id()));
        }
        let trigger = self
            .repository
            .create(
                &transaction,
                NewTrigger {
                    trigger_id: Uuid::new_v4(),
                    strategy_id: scope.id(),
                    purpose,
                    kind: command.kind,
                    schedule: validated.schedule,
                    hook_slug: validated.hook_slug,
                    event_type: validated.event_type,
                    event_match: command.event_match,
                    prompt_template: validated.prompt_template,
                    enabled: command.enabled.unwrap_or(true),
                    business_days_only: command.business_days_only.unwrap_or(false),
                },
            )
            .await?;
        self.record_history(
            &transaction,
            trigger.trigger_id,
            Op::Create,
            None,
            Some(&trigger),
        )
        .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(trigger)
    }

    pub async fn update(
        &self,
        scope: StrategyScope,
        trigger_id: Uuid,
        mut command: UpdateTriggerCommand,
    ) -> Result<Trigger, TriggerUseCaseError> {
        command.purpose = match command.purpose.take() {
            Some(Some(purpose)) => Some(Some(self.validate_purpose(purpose).await?)),
            other => other,
        };
        let transaction = self.unit_of_work.begin().await?;
        let current = self
            .repository
            .find_by_id_for_update_in_transaction(&transaction, trigger_id)
            .await?
            .ok_or(TriggerUseCaseError::NotFound(trigger_id))?;
        ensure_scope(&current, scope)?;

        let before = current.clone();
        let updated = apply_update(current, command)?;
        let updated = self.repository.update(&transaction, updated).await?;
        self.record_history(
            &transaction,
            trigger_id,
            Op::Update,
            Some(&before),
            Some(&updated),
        )
        .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(updated)
    }

    pub async fn delete(
        &self,
        scope: StrategyScope,
        trigger_id: Uuid,
    ) -> Result<(), TriggerUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let current = self
            .repository
            .find_by_id_for_update_in_transaction(&transaction, trigger_id)
            .await?
            .ok_or(TriggerUseCaseError::NotFound(trigger_id))?;
        ensure_scope(&current, scope)?;
        if !self.repository.delete(&transaction, trigger_id).await? {
            return Err(TriggerUseCaseError::NotFound(trigger_id));
        }
        self.record_history(&transaction, trigger_id, Op::Delete, Some(&current), None)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }

    pub async fn fire(
        &self,
        agent_client: &dyn AgentTaskClient,
        trigger_id: Uuid,
        payload: Value,
        source: TaskSource,
    ) -> Result<SubmittedTask, TriggerUseCaseError> {
        let trigger = self
            .repository
            .find_by_id(trigger_id)
            .await?
            .ok_or(TriggerUseCaseError::NotFound(trigger_id))?;
        if !trigger.enabled {
            return Err(TriggerUseCaseError::Disabled(trigger_id));
        }
        let now = Utc::now();
        let (strategy_id, prompt) = self.resolve_trigger_prompt(&trigger, &payload, now).await?;
        let submitted = self
            .strategy_tasks
            .submit_task(
                agent_client,
                strategy_id,
                &prompt,
                source,
                trigger.purpose.clone(),
            )
            .await?;

        let transaction = match self.unit_of_work.begin().await {
            Ok(transaction) => transaction,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    trigger_id = %trigger_id,
                    task_id = %submitted.task_id,
                    "trigger fired but last_fired_at transaction could not begin; may re-fire",
                );
                return Err(error.into());
            }
        };
        let marked = match self
            .repository
            .mark_fired(&transaction, trigger_id, now.fixed_offset())
            .await
        {
            Ok(marked) => marked,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    trigger_id = %trigger_id,
                    task_id = %submitted.task_id,
                    "trigger fired but last_fired_at update failed; may re-fire",
                );
                return Err(error.into());
            }
        };
        if !marked {
            tracing::warn!(
                trigger_id = %trigger_id,
                task_id = %submitted.task_id,
                "trigger fired but last_fired_at update found no row",
            );
            return Err(TriggerUseCaseError::NotFound(trigger_id));
        }
        if let Err(error) = self.unit_of_work.commit(transaction).await {
            tracing::warn!(
                error = %error,
                trigger_id = %trigger_id,
                task_id = %submitted.task_id,
                "trigger fired but last_fired_at update failed; may re-fire",
            );
            return Err(error.into());
        }
        Ok(submitted)
    }

    async fn resolve_trigger_prompt(
        &self,
        trigger: &Trigger,
        payload: &Value,
        now: DateTime<Utc>,
    ) -> Result<(Uuid, String), TriggerUseCaseError> {
        let strategy_id = trigger
            .strategy_id
            .ok_or(TriggerUseCaseError::NoStrategy(trigger.trigger_id))?;
        let strategy = self
            .strategy_repository
            .find_by_id(strategy_id)
            .await?
            .ok_or(crate::strategy_task::SubmitTaskError::StrategyNotFound(
                strategy_id,
            ))?;
        let context = build_standard_context(&strategy, now);
        let prompt = expand_template(&trigger.prompt_template, payload, &context);
        Ok((strategy_id, prompt))
    }

    pub(crate) async fn list_enabled_event_triggers(
        &self,
        event_type: &str,
    ) -> Result<Vec<Trigger>, TriggerUseCaseError> {
        self.repository
            .list_enabled_event_by_type(event_type)
            .await
            .map_err(Into::into)
    }

    pub(crate) async fn prepare_event_task(
        &self,
        trigger: &Trigger,
        payload: &Value,
    ) -> Result<PreparedStrategyTaskSubmission, TriggerUseCaseError> {
        let (strategy_id, prompt) = self
            .resolve_trigger_prompt(trigger, payload, Utc::now())
            .await?;
        self.strategy_tasks
            .prepare_submission(
                strategy_id,
                &prompt,
                TaskSource::Event,
                trigger.purpose.clone(),
                None,
            )
            .await
            .map_err(Into::into)
    }

    pub(crate) async fn insert_prepared_event_task(
        &self,
        transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        prepared: &PreparedStrategyTaskSubmission,
    ) -> Result<(), TriggerUseCaseError> {
        self.strategy_tasks
            .insert_prepared_submission(transaction, prepared)
            .await
            .map_err(Into::into)
    }

    pub(crate) async fn submit_prepared_event_task(
        &self,
        agent_client: &dyn AgentTaskClient,
        prepared: PreparedStrategyTaskSubmission,
    ) -> Result<SubmittedTask, TriggerUseCaseError> {
        self.strategy_tasks
            .submit_prepared_submission(agent_client, prepared)
            .await
            .map_err(Into::into)
    }

    pub async fn fire_hook(
        &self,
        agent_client: &dyn AgentTaskClient,
        hook_slug: &str,
        payload: Value,
    ) -> Result<Option<SubmittedTask>, TriggerUseCaseError> {
        let trigger = self
            .repository
            .find_enabled_hook_by_slug(hook_slug)
            .await?
            .ok_or_else(|| TriggerUseCaseError::HookNotFound(hook_slug.to_string()))?;
        if !evaluate_event_match(trigger.event_match.as_ref(), &payload) {
            tracing::info!(
                trigger_id = %trigger.trigger_id,
                hook_slug,
                "hook payload did not match event_match; ignored",
            );
            return Ok(None);
        }
        match self
            .fire(agent_client, trigger.trigger_id, payload, TaskSource::Hook)
            .await
        {
            Ok(submitted) => Ok(Some(submitted)),
            Err(TriggerUseCaseError::NotFound(_) | TriggerUseCaseError::Disabled(_)) => {
                Err(TriggerUseCaseError::HookNotFound(hook_slug.to_string()))
            }
            Err(TriggerUseCaseError::NoStrategy(trigger_id)) => {
                tracing::warn!(trigger_id = %trigger_id, "hook fire rejected: trigger has no strategy_id");
                Err(TriggerUseCaseError::HookNotFound(hook_slug.to_string()))
            }
            Err(error) => Err(error),
        }
    }

    async fn validate_purpose(&self, purpose: String) -> Result<String, TriggerUseCaseError> {
        let purpose = purpose.trim().to_string();
        if purpose.is_empty() {
            return Err(TriggerUseCaseError::Validation(
                "purpose must not be empty".into(),
            ));
        }
        if !self.strategy_tasks.agent_config_exists(&purpose).await? {
            return Err(TriggerUseCaseError::PurposeNotFound(purpose));
        }
        Ok(purpose)
    }

    async fn record_history(
        &self,
        transaction: &UnitOfWorkTransaction,
        trigger_id: Uuid,
        op: Op,
        before: Option<&Trigger>,
        after: Option<&Trigger>,
    ) -> Result<(), TriggerUseCaseError> {
        self.change_history
            .record(
                transaction,
                ChangeHistoryRecord {
                    actor: Actor::Human,
                    target_kind: TargetKind::Trigger,
                    target_id: trigger_id,
                    op,
                    diff: definition_snapshot(before, after),
                    summary: None,
                },
            )
            .await?;
        Ok(())
    }
}

fn definition_snapshot(before: Option<&Trigger>, after: Option<&Trigger>) -> Value {
    fn definition(trigger: &Trigger) -> Value {
        json!({
            "trigger_id": trigger.trigger_id,
            "strategy_id": trigger.strategy_id,
            "purpose": trigger.purpose,
            "kind": trigger.kind.as_str(),
            "schedule": trigger.schedule,
            "hook_slug": trigger.hook_slug,
            "event_type": trigger.event_type,
            "event_match": trigger.event_match,
            "prompt_template": trigger.prompt_template,
            "enabled": trigger.enabled,
            "business_days_only": trigger.business_days_only,
        })
    }

    json!({
        "format": "definition_snapshot",
        "version": 1,
        "before": before.map(definition),
        "after": after.map(definition),
    })
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use async_trait::async_trait;
    use chrono::Utc;
    use rstest::{fixture, rstest};
    use serde_json::json;
    use uuid::Uuid;

    use crate::{
        agent_task_client::{
            AgentTaskClient, AgentTaskError, AgentTaskRef, AgentTaskStatus, FakeAgentTaskClient,
            SubmitAgentTask,
        },
        change_history::{FakeChangeHistory, SharedChangeHistoryPort},
        strategy::{FakeStrategyRepository, SharedStrategyRepository, Strategy},
        strategy_existence::{FakeStrategyExistence, SharedStrategyExistence},
        strategy_task::{
            SharedStrategyTaskRepository, StrategyTask, StrategyTaskOrigin, StrategyTaskRepository,
            StrategyTaskRepositoryError, StrategyTaskStep, StrategyTaskUpdate,
            StrategyTaskUseCases, TaskListQuery, TaskSource,
        },
        trigger::{
            FakeTriggerRepository, SharedTriggerRepository, Trigger, TriggerKind,
            TriggerRepository, UpdateTriggerCommand,
        },
        unit_of_work::{FakeUnitOfWork, SharedUnitOfWork, UnitOfWorkTransaction},
    };

    use super::TriggerUseCases;

    struct Harness {
        use_cases: TriggerUseCases,
        triggers: Arc<FakeTriggerRepository>,
        strategies: Arc<FakeStrategyRepository>,
        unit_of_work: Arc<FakeUnitOfWork>,
    }

    #[fixture]
    fn harness() -> Harness {
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let triggers = Arc::new(FakeTriggerRepository::new());
        let strategies = Arc::new(FakeStrategyRepository::new());
        let unit_of_work_shared: SharedUnitOfWork = unit_of_work.clone();
        let trigger_repository: SharedTriggerRepository = triggers.clone();
        let strategy_repository: SharedStrategyRepository = strategies.clone();
        let strategy_existence: SharedStrategyExistence = Arc::new(FakeStrategyExistence::new());
        let task_repository: SharedStrategyTaskRepository = Arc::new(TestStrategyTaskRepository);
        let change_history = Arc::new(FakeChangeHistory::new());
        let change_history_shared: SharedChangeHistoryPort = change_history;
        let strategy_tasks =
            StrategyTaskUseCases::new(unit_of_work_shared.clone(), task_repository);

        Harness {
            use_cases: TriggerUseCases::new(
                unit_of_work_shared,
                trigger_repository,
                strategy_existence,
                strategy_repository,
                strategy_tasks,
                change_history_shared,
            ),
            triggers,
            strategies,
            unit_of_work,
        }
    }

    fn trigger(trigger_id: Uuid, strategy_id: Uuid, kind: TriggerKind) -> Trigger {
        let now = Utc::now().fixed_offset();
        Trigger {
            trigger_id,
            strategy_id: Some(strategy_id),
            purpose: None,
            kind,
            schedule: (kind == TriggerKind::Cron).then(|| "* * * * *".to_string()),
            hook_slug: (kind == TriggerKind::Hook).then(|| "sample-hook".to_string()),
            event_type: (kind == TriggerKind::Event).then(|| "sample_process.finished".to_string()),
            event_match: None,
            prompt_template: "sample prompt".to_string(),
            enabled: true,
            business_days_only: false,
            last_fired_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn strategy(id: Uuid) -> Strategy {
        let now = Utc::now().fixed_offset();
        Strategy {
            id,
            name: "sample strategy".to_string(),
            description: None,
            sort_order: 0,
            created_at: now,
            updated_at: now,
        }
    }

    #[rstest]
    #[tokio::test]
    async fn update_rejects_trigger_outside_strategy_scope(harness: Harness) {
        let trigger = trigger(Uuid::new_v4(), Uuid::new_v4(), TriggerKind::Hook);
        let trigger_id = trigger.trigger_id;
        harness.triggers.insert_trigger(trigger.clone()).await;

        let result = harness
            .use_cases
            .update(
                Uuid::new_v4().into(),
                trigger_id,
                UpdateTriggerCommand::default(),
            )
            .await;
        let stored = harness
            .triggers
            .find_by_id(trigger_id)
            .await
            .map_err(|error| error.to_string());

        assert_eq!(
            (result.err().map(|error| error.to_string()), stored,),
            (
                Some(format!("trigger {trigger_id} not found")),
                Ok(Some(trigger)),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn fire_returns_not_found_when_trigger_disappears_after_submission(harness: Harness) {
        let strategy_id = Uuid::new_v4();
        let trigger = trigger(Uuid::new_v4(), strategy_id, TriggerKind::Hook);
        let trigger_id = trigger.trigger_id;
        harness
            .strategies
            .insert_strategy(strategy(strategy_id))
            .await;
        harness.triggers.insert_trigger(trigger).await;
        let agent_client = DeletingAgentTaskClient::new(harness.triggers.clone(), trigger_id);

        let result = harness
            .use_cases
            .fire(&agent_client, trigger_id, json!({}), TaskSource::Hook)
            .await;
        let submitted = agent_client.submitted.lock().await.len();
        let committed = harness.unit_of_work.committed.lock().await.len();

        assert_eq!(
            (
                result.err().map(|error| error.to_string()),
                submitted,
                committed,
            ),
            (Some(format!("trigger {trigger_id} not found")), 1, 2,),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn cron_tick_retries_a_trigger_after_its_submission_panics(harness: Harness) {
        let strategy_id = Uuid::new_v4();
        harness
            .strategies
            .insert_strategy(strategy(strategy_id))
            .await;
        harness
            .triggers
            .insert_trigger(trigger(Uuid::new_v4(), strategy_id, TriggerKind::Cron))
            .await;
        harness
            .triggers
            .insert_trigger(trigger(Uuid::new_v4(), strategy_id, TriggerKind::Cron))
            .await;
        let agent_client = PanickingOnceAgentTaskClient::new();

        let first_attempted = harness
            .use_cases
            .run_cron_tick(&agent_client, Duration::from_secs(60))
            .await;
        let first_submitted = agent_client.inner.submitted.lock().await.len();
        let first_fired = harness
            .triggers
            .triggers
            .lock()
            .await
            .values()
            .filter(|trigger| trigger.last_fired_at.is_some())
            .count();
        let second_attempted = harness
            .use_cases
            .run_cron_tick(&agent_client, Duration::from_secs(60))
            .await;
        let total_submitted = agent_client.inner.submitted.lock().await.len();
        let final_fired = harness
            .triggers
            .triggers
            .lock()
            .await
            .values()
            .filter(|trigger| trigger.last_fired_at.is_some())
            .count();

        assert_eq!(
            (
                first_attempted,
                first_submitted,
                first_fired,
                second_attempted,
                total_submitted,
                final_fired,
            ),
            (2, 1, 1, 1, 2, 2),
        );
    }

    struct TestStrategyTaskRepository;

    #[async_trait]
    impl StrategyTaskRepository for TestStrategyTaskRepository {
        async fn strategy_exists(
            &self,
            _strategy_id: Uuid,
        ) -> Result<bool, StrategyTaskRepositoryError> {
            Ok(true)
        }

        async fn agent_config_exists(
            &self,
            _purpose: &str,
        ) -> Result<bool, StrategyTaskRepositoryError> {
            Ok(true)
        }

        async fn find_agent_graph_by_purpose(
            &self,
            _purpose: &str,
        ) -> Result<Option<String>, StrategyTaskRepositoryError> {
            Ok(Some(String::new()))
        }

        async fn insert(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _task: StrategyTask,
        ) -> Result<(), StrategyTaskRepositoryError> {
            Ok(())
        }

        async fn update(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _task: StrategyTaskUpdate,
        ) -> Result<bool, StrategyTaskRepositoryError> {
            Ok(true)
        }

        async fn apply_status_and_steps(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _task_id: Uuid,
            _expected_a2a_task_id: Option<&str>,
            _task_update: Option<StrategyTaskUpdate>,
            _steps: Option<serde_json::Value>,
        ) -> Result<crate::strategy_task::StrategyTaskApplyResult, StrategyTaskRepositoryError>
        {
            Ok(crate::strategy_task::StrategyTaskApplyResult::default())
        }

        async fn find_by_id(
            &self,
            _task_id: Uuid,
        ) -> Result<Option<StrategyTask>, StrategyTaskRepositoryError> {
            Ok(None)
        }

        async fn find_by_a2a_task_id(
            &self,
            _a2a_task_id: &str,
        ) -> Result<Option<StrategyTask>, StrategyTaskRepositoryError> {
            Ok(None)
        }

        async fn find_origin_by_execution_step_id(
            &self,
            _execution_step_id: Uuid,
        ) -> Result<Option<StrategyTaskOrigin>, StrategyTaskRepositoryError> {
            Ok(None)
        }

        async fn list(
            &self,
            _query: TaskListQuery,
        ) -> Result<Vec<StrategyTask>, StrategyTaskRepositoryError> {
            Ok(Vec::new())
        }

        async fn list_in_flight(&self) -> Result<Vec<StrategyTask>, StrategyTaskRepositoryError> {
            Ok(Vec::new())
        }

        async fn list_steps(
            &self,
            _task_id: Uuid,
        ) -> Result<Vec<StrategyTaskStep>, StrategyTaskRepositoryError> {
            Ok(Vec::new())
        }

        async fn claim_resumable(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _task_id: Uuid,
            _now: chrono::DateTime<chrono::FixedOffset>,
            _mark_auto_resumed: bool,
        ) -> Result<bool, StrategyTaskRepositoryError> {
            Ok(true)
        }
    }

    struct PanickingOnceAgentTaskClient {
        panicked: AtomicBool,
        inner: FakeAgentTaskClient,
    }

    impl PanickingOnceAgentTaskClient {
        fn new() -> Self {
            Self {
                panicked: AtomicBool::new(false),
                inner: FakeAgentTaskClient::new(),
            }
        }
    }

    #[async_trait]
    impl AgentTaskClient for PanickingOnceAgentTaskClient {
        async fn submit(&self, request: SubmitAgentTask) -> Result<AgentTaskRef, AgentTaskError> {
            if !self.panicked.swap(true, Ordering::SeqCst) {
                std::panic::resume_unwind(Box::new("simulated client panic"));
            }
            self.inner.submit(request).await
        }

        async fn get(&self, task_id: &str) -> Result<AgentTaskStatus, AgentTaskError> {
            self.inner.get(task_id).await
        }
    }

    struct DeletingAgentTaskClient {
        repository: Arc<FakeTriggerRepository>,
        trigger_id: Uuid,
        submitted: tokio::sync::Mutex<Vec<SubmitAgentTask>>,
    }

    impl DeletingAgentTaskClient {
        fn new(repository: Arc<FakeTriggerRepository>, trigger_id: Uuid) -> Self {
            Self {
                repository,
                trigger_id,
                submitted: tokio::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl AgentTaskClient for DeletingAgentTaskClient {
        async fn submit(&self, request: SubmitAgentTask) -> Result<AgentTaskRef, AgentTaskError> {
            self.submitted.lock().await.push(request);
            self.repository
                .triggers
                .lock()
                .await
                .remove(&self.trigger_id);
            Ok(AgentTaskRef {
                task_id: "sample-task".to_string(),
            })
        }

        async fn get(&self, task_id: &str) -> Result<AgentTaskStatus, AgentTaskError> {
            Err(AgentTaskError::NotFound(task_id.to_string()))
        }
    }
}
