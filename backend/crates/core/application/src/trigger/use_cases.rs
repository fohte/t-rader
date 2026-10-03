use std::time::Duration;

use chrono::Utc;
use futures_util::{FutureExt, StreamExt, stream};
use serde_json::Value;
use uuid::Uuid;

use crate::agent_task_client::AgentTaskClient;
use crate::strategy::SharedStrategyRepository;
use crate::strategy_existence::SharedStrategyExistence;
use crate::strategy_scope::StrategyScope;
use crate::strategy_task::{StrategyTaskUseCases, SubmittedTask, TaskSource};
use crate::unit_of_work::SharedUnitOfWork;

use super::error::TriggerUseCaseError;
use super::repository::SharedTriggerRepository;
use super::schedule::{parse_schedule, should_fire};
use super::template::{build_standard_context, evaluate_event_match, expand_template};
use super::types::{CreateTriggerCommand, NewTrigger, Trigger, TriggerKind, UpdateTriggerCommand};

const MAX_CONCURRENT_FIRES: usize = 8;

#[derive(Clone)]
pub struct TriggerUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedTriggerRepository,
    strategy_existence: SharedStrategyExistence,
    strategy_repository: SharedStrategyRepository,
    strategy_tasks: StrategyTaskUseCases,
}

impl TriggerUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedTriggerRepository,
        strategy_existence: SharedStrategyExistence,
        strategy_repository: SharedStrategyRepository,
        strategy_tasks: StrategyTaskUseCases,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            strategy_existence,
            strategy_repository,
            strategy_tasks,
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
        let (schedule, hook_slug, prompt_template) = validate_create(&command)?;
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
                    schedule,
                    hook_slug,
                    event_match: command.event_match,
                    prompt_template,
                    enabled: command.enabled.unwrap_or(true),
                },
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
            .find_by_id_in_transaction(&transaction, trigger_id)
            .await?
            .ok_or(TriggerUseCaseError::NotFound(trigger_id))?;
        ensure_scope(&current, scope)?;

        let updated = apply_update(current, command)?;
        let updated = self.repository.update(&transaction, updated).await?;
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
            .find_by_id_in_transaction(&transaction, trigger_id)
            .await?
            .ok_or(TriggerUseCaseError::NotFound(trigger_id))?;
        ensure_scope(&current, scope)?;
        if !self.repository.delete(&transaction, trigger_id).await? {
            return Err(TriggerUseCaseError::NotFound(trigger_id));
        }
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
        let strategy_id = trigger
            .strategy_id
            .ok_or(TriggerUseCaseError::NoStrategy(trigger_id))?;
        let strategy = self
            .strategy_repository
            .find_by_id(strategy_id)
            .await?
            .ok_or(crate::strategy_task::SubmitTaskError::StrategyNotFound(
                strategy_id,
            ))?;

        let now = Utc::now();
        let context = build_standard_context(&strategy, now);
        let prompt = expand_template(&trigger.prompt_template, &payload, &context);
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

    pub async fn run_cron_tick(
        &self,
        agent_client: &dyn AgentTaskClient,
        interval: Duration,
    ) -> usize {
        let triggers = match self.repository.list_enabled_cron().await {
            Ok(triggers) => triggers,
            Err(error) => {
                tracing::warn!(error = %error, "failed to list cron triggers");
                return 0;
            }
        };

        let now = Utc::now();
        let targets: Vec<Uuid> = triggers
            .into_iter()
            .filter_map(|trigger| {
                let Some(expression) = trigger.schedule.as_deref() else {
                    tracing::warn!(trigger_id = %trigger.trigger_id, "cron trigger has no schedule; skip");
                    return None;
                };
                match parse_schedule(expression) {
                    Ok(schedule) => {
                        let last_fired_at = trigger.last_fired_at.map(|value| value.with_timezone(&Utc));
                        should_fire(&schedule, last_fired_at, now, interval)
                            .then_some(trigger.trigger_id)
                    }
                    Err(error) => {
                        tracing::warn!(
                            trigger_id = %trigger.trigger_id,
                            schedule = expression,
                            error = %error,
                            "failed to parse cron schedule; skip",
                        );
                        None
                    }
                }
            })
            .collect();

        let attempted = targets.len();
        let results = stream::iter(targets)
            .map(|trigger_id| async move {
                std::panic::AssertUnwindSafe(self.fire(
                    agent_client,
                    trigger_id,
                    serde_json::json!({}),
                    TaskSource::Cron,
                ))
                .catch_unwind()
                .await
                .map(|result| (trigger_id, result))
                .map_err(|_| trigger_id)
            })
            .buffer_unordered(MAX_CONCURRENT_FIRES)
            .collect::<Vec<_>>()
            .await;
        for result in results {
            match result {
                Ok((_, Ok(_))) => {}
                Ok((_, Err(TriggerUseCaseError::Disabled(_)))) => {}
                Ok((trigger_id, Err(error))) => {
                    tracing::warn!(error = %error, trigger_id = %trigger_id, "cron trigger fire failed");
                }
                Err(trigger_id) => {
                    tracing::error!(trigger_id = %trigger_id, "cron trigger worker task panicked");
                }
            }
        }
        attempted
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
}

fn validate_create(
    command: &CreateTriggerCommand,
) -> Result<(Option<String>, Option<String>, String), TriggerUseCaseError> {
    let schedule = command
        .schedule
        .as_deref()
        .map(str::trim)
        .map(str::to_string);
    let hook_slug = command
        .hook_slug
        .as_deref()
        .map(str::trim)
        .map(str::to_string);
    match command.kind {
        TriggerKind::Cron => {
            if schedule.as_deref().is_none_or(str::is_empty) {
                return Err(TriggerUseCaseError::Validation(
                    "schedule is required for kind=cron".into(),
                ));
            }
            if hook_slug.is_some() {
                return Err(TriggerUseCaseError::Validation(
                    "hook_slug must be omitted for kind=cron".into(),
                ));
            }
        }
        TriggerKind::Hook => {
            if hook_slug.as_deref().is_none_or(str::is_empty) {
                return Err(TriggerUseCaseError::Validation(
                    "hook_slug is required for kind=hook".into(),
                ));
            }
            if command.schedule.is_some() {
                return Err(TriggerUseCaseError::Validation(
                    "schedule must be omitted for kind=hook".into(),
                ));
            }
        }
    }
    let prompt_template = validate_template(&command.prompt_template)?;
    Ok((schedule, hook_slug, prompt_template))
}

fn apply_update(
    mut trigger: Trigger,
    command: UpdateTriggerCommand,
) -> Result<Trigger, TriggerUseCaseError> {
    if let Some(schedule) = command.schedule {
        if trigger.kind != TriggerKind::Cron {
            return Err(TriggerUseCaseError::Validation(
                "schedule can only be set when kind=cron".into(),
            ));
        }
        let schedule = schedule.trim().to_string();
        if schedule.is_empty() {
            return Err(TriggerUseCaseError::Validation(
                "schedule must not be empty".into(),
            ));
        }
        trigger.schedule = Some(schedule);
    }
    if let Some(hook_slug) = command.hook_slug {
        if trigger.kind != TriggerKind::Hook {
            return Err(TriggerUseCaseError::Validation(
                "hook_slug can only be set when kind=hook".into(),
            ));
        }
        let hook_slug = hook_slug.trim().to_string();
        if hook_slug.is_empty() {
            return Err(TriggerUseCaseError::Validation(
                "hook_slug must not be empty".into(),
            ));
        }
        trigger.hook_slug = Some(hook_slug);
    }
    if let Some(event_match) = command.event_match {
        validate_event_match(event_match.as_ref())?;
        trigger.event_match = event_match;
    }
    if let Some(prompt_template) = command.prompt_template {
        trigger.prompt_template = validate_template(&prompt_template)?;
    }
    if let Some(enabled) = command.enabled {
        trigger.enabled = enabled;
    }
    if let Some(purpose) = command.purpose {
        trigger.purpose = purpose;
    }
    trigger.updated_at = Utc::now().fixed_offset();
    Ok(trigger)
}

fn ensure_scope(trigger: &Trigger, scope: StrategyScope) -> Result<(), TriggerUseCaseError> {
    if trigger.strategy_id == Some(scope.id()) {
        Ok(())
    } else {
        Err(TriggerUseCaseError::NotFound(trigger.trigger_id))
    }
}

fn validate_template(template: &str) -> Result<String, TriggerUseCaseError> {
    let template = template.trim().to_string();
    if template.is_empty() {
        return Err(TriggerUseCaseError::Validation(
            "prompt_template must not be empty".into(),
        ));
    }
    Ok(template)
}

fn validate_event_match(event_match: Option<&Value>) -> Result<(), TriggerUseCaseError> {
    match event_match {
        Some(value) if !value.is_object() && !value.is_null() => Err(
            TriggerUseCaseError::Validation("event_match must be an object or null".into()),
        ),
        _ => Ok(()),
    }
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
        strategy::{FakeStrategyRepository, SharedStrategyRepository, Strategy},
        strategy_existence::{FakeStrategyExistence, SharedStrategyExistence},
        strategy_task::{
            SharedStrategyTaskRepository, StrategyTask, StrategyTaskRepository,
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
        let strategy_tasks =
            StrategyTaskUseCases::new(unit_of_work_shared.clone(), task_repository);

        Harness {
            use_cases: TriggerUseCases::new(
                unit_of_work_shared,
                trigger_repository,
                strategy_existence,
                strategy_repository,
                strategy_tasks,
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
            event_match: None,
            prompt_template: "sample prompt".to_string(),
            enabled: true,
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
    async fn cron_tick_continues_after_a_trigger_panics(harness: Harness) {
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

        let attempted = harness
            .use_cases
            .run_cron_tick(&agent_client, Duration::from_secs(60))
            .await;
        let submitted = agent_client.inner.submitted.lock().await.len();
        let fired = harness
            .triggers
            .triggers
            .lock()
            .await
            .values()
            .filter(|trigger| trigger.last_fired_at.is_some())
            .count();

        assert_eq!((attempted, submitted, fired), (2, 1, 1));
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
            _task_update: Option<StrategyTaskUpdate>,
            _steps: Option<serde_json::Value>,
        ) -> Result<bool, StrategyTaskRepositoryError> {
            Ok(false)
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

        async fn find_strategy_id_by_execution_step_id(
            &self,
            _execution_step_id: Uuid,
        ) -> Result<Option<Uuid>, StrategyTaskRepositoryError> {
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
