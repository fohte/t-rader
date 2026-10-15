use chrono::{Duration, Utc};
use thiserror::Error;
use uuid::Uuid;

use crate::agent_task_client::AgentTaskClient;
use crate::event::{EventRepositoryError, SharedEventRepository};
use crate::strategy_task::SubmitTaskError;
use crate::trigger::{TriggerUseCaseError, TriggerUseCases, evaluate_event_match};
use crate::unit_of_work::{SharedUnitOfWork, UnitOfWorkError};

pub const MAX_EVENT_CHAIN_DEPTH: i64 = 3;
const EVENT_RETENTION_DAYS: i64 = 30;

#[derive(Debug, Error)]
pub enum EventDispatchUseCaseError {
    #[error(transparent)]
    EventRepository(#[from] EventRepositoryError),
    #[error(transparent)]
    Trigger(#[from] TriggerUseCaseError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error("event {event_id} was already fired for trigger {trigger_id}")]
    DuplicateFiring { event_id: Uuid, trigger_id: Uuid },
    #[error("failed to process {0} event trigger(s)")]
    EventTriggerFailures(usize),
    #[error("event {0} could not be marked dispatched")]
    EventNotMarkedDispatched(Uuid),
}

#[derive(Clone)]
pub struct EventDispatchUseCases {
    unit_of_work: SharedUnitOfWork,
    events: SharedEventRepository,
    triggers: TriggerUseCases,
}

impl EventDispatchUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        events: SharedEventRepository,
        triggers: TriggerUseCases,
    ) -> Self {
        Self {
            unit_of_work,
            events,
            triggers,
        }
    }

    pub async fn dispatch_pending_events(
        &self,
        agent_client: &dyn AgentTaskClient,
    ) -> Result<usize, EventDispatchUseCaseError> {
        let mut dispatched_events = 0;
        let mut trigger_failures = 0;

        loop {
            let transaction = self.unit_of_work.begin().await?;
            let Some(event) = self.events.lock_next_undispatched(&transaction).await? else {
                self.unit_of_work.commit(transaction).await?;
                break;
            };
            let mut submissions = Vec::new();

            if event.chain_depth <= MAX_EVENT_CHAIN_DEPTH {
                let triggers = self
                    .triggers
                    .list_enabled_event_triggers(&event.event_type)
                    .await?;
                for trigger in triggers {
                    if !evaluate_event_match(trigger.event_match.as_ref(), &event.payload)
                        || self
                            .events
                            .has_firing(&transaction, event.event_id, trigger.trigger_id)
                            .await?
                    {
                        continue;
                    }

                    let prepared = match self
                        .triggers
                        .prepare_event_task(&trigger, &event.payload)
                        .await
                    {
                        Ok(prepared) => prepared,
                        Err(error) if is_trigger_configuration_error(&error) => {
                            tracing::warn!(
                                %error,
                                event_id = %event.event_id,
                                trigger_id = %trigger.trigger_id,
                                "event trigger configuration rejected",
                            );
                            trigger_failures += 1;
                            continue;
                        }
                        Err(error) => return Err(error.into()),
                    };
                    self.triggers
                        .insert_prepared_event_task(&transaction, &prepared)
                        .await?;
                    if !self
                        .events
                        .insert_firing(
                            &transaction,
                            event.event_id,
                            trigger.trigger_id,
                            prepared.task.task_id,
                        )
                        .await?
                    {
                        return Err(EventDispatchUseCaseError::DuplicateFiring {
                            event_id: event.event_id,
                            trigger_id: trigger.trigger_id,
                        });
                    }
                    submissions.push((trigger.trigger_id, prepared));
                }
            }

            if !self
                .events
                .mark_dispatched(&transaction, event.event_id, Utc::now().fixed_offset())
                .await?
            {
                return Err(EventDispatchUseCaseError::EventNotMarkedDispatched(
                    event.event_id,
                ));
            }
            self.unit_of_work.commit(transaction).await?;
            dispatched_events += 1;

            for (trigger_id, prepared) in submissions {
                if let Err(error) = self
                    .triggers
                    .submit_prepared_event_task(agent_client, prepared)
                    .await
                {
                    tracing::warn!(
                        %error,
                        event_id = %event.event_id,
                        %trigger_id,
                        "event-trigger task submission failed",
                    );
                    trigger_failures += 1;
                }
            }
        }

        if trigger_failures > 0 {
            return Err(EventDispatchUseCaseError::EventTriggerFailures(
                trigger_failures,
            ));
        }
        Ok(dispatched_events)
    }

    pub async fn delete_expired_events(&self) -> Result<u64, EventDispatchUseCaseError> {
        let cutoff = Utc::now().fixed_offset() - Duration::days(EVENT_RETENTION_DAYS);
        self.events.delete_before(cutoff).await.map_err(Into::into)
    }

    pub async fn dispatch_and_cleanup(
        &self,
        agent_client: &dyn AgentTaskClient,
    ) -> Result<(usize, u64), EventDispatchUseCaseError> {
        let deleted = self.delete_expired_events().await?;
        let dispatched = self.dispatch_pending_events(agent_client).await?;
        Ok((dispatched, deleted))
    }
}

fn is_trigger_configuration_error(error: &TriggerUseCaseError) -> bool {
    matches!(
        error,
        TriggerUseCaseError::NoStrategy(_)
            | TriggerUseCaseError::PurposeNotFound(_)
            | TriggerUseCaseError::Submit(
                SubmitTaskError::StrategyNotFound(_)
                    | SubmitTaskError::EmptyPrompt
                    | SubmitTaskError::PurposeNotFound(_)
            )
    )
}
