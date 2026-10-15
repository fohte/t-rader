use async_trait::async_trait;
use core_application::event::{Event, EventRepository, EventRepositoryError, NewEvent};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::{Expr, LockBehavior, LockType, OnConflict};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::{event, event_firing};
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

#[derive(Clone)]
pub struct PostgresEventRepository {
    db: DatabaseHandle,
}

impl PostgresEventRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl EventRepository for PostgresEventRepository {
    async fn create(
        &self,
        transaction: &UnitOfWorkTransaction,
        event: NewEvent,
    ) -> Result<Event, EventRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = event::ActiveModel {
            event_id: NotSet,
            event_type: Set(event.event_type),
            payload: Set(event.payload),
            chain_depth: event.chain_depth.map_or(NotSet, Set),
            occurred_at: NotSet,
            dispatched_at: NotSet,
        };
        event::Entity::insert(model)
            .exec_with_returning(transaction)
            .await
            .map(to_domain)
            .map_err(repository_error)
    }

    async fn lock_next_undispatched(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<Option<Event>, EventRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        event::Entity::find()
            .filter(event::Column::DispatchedAt.is_null())
            .order_by_asc(event::Column::OccurredAt)
            .order_by_asc(event::Column::EventId)
            .limit(1)
            .lock_with_behavior(LockType::Update, LockBehavior::SkipLocked)
            .one(transaction)
            .await
            .map(|model| model.map(to_domain))
            .map_err(repository_error)
    }

    async fn mark_dispatched(
        &self,
        transaction: &UnitOfWorkTransaction,
        event_id: Uuid,
        dispatched_at: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<bool, EventRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        event::Entity::update_many()
            .col_expr(event::Column::DispatchedAt, Expr::value(dispatched_at))
            .filter(event::Column::EventId.eq(event_id))
            .filter(event::Column::DispatchedAt.is_null())
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }

    async fn delete_before(
        &self,
        occurred_before: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<u64, EventRepositoryError> {
        event::Entity::delete_many()
            .filter(event::Column::OccurredAt.lt(occurred_before))
            .exec(&self.db)
            .await
            .map(|result| result.rows_affected)
            .map_err(repository_error)
    }

    async fn insert_firing(
        &self,
        transaction: &UnitOfWorkTransaction,
        event_id: Uuid,
        trigger_id: Uuid,
        task_id: Uuid,
    ) -> Result<bool, EventRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        event_firing::Entity::insert(event_firing::ActiveModel {
            event_id: Set(event_id),
            trigger_id: Set(trigger_id),
            task_id: Set(task_id),
        })
        .on_conflict(
            OnConflict::columns([
                event_firing::Column::EventId,
                event_firing::Column::TriggerId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(transaction)
        .await
        .map(|rows_affected| rows_affected > 0)
        .map_err(repository_error)
    }

    async fn has_firing(
        &self,
        transaction: &UnitOfWorkTransaction,
        event_id: Uuid,
        trigger_id: Uuid,
    ) -> Result<bool, EventRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        event_firing::Entity::find()
            .filter(event_firing::Column::EventId.eq(event_id))
            .filter(event_firing::Column::TriggerId.eq(trigger_id))
            .one(transaction)
            .await
            .map(|firing| firing.is_some())
            .map_err(repository_error)
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, EventRepositoryError> {
    postgres_transaction_ref(transaction).ok_or(EventRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> EventRepositoryError {
    EventRepositoryError::Database(persistence_error(error))
}

fn to_domain(model: event::Model) -> Event {
    Event {
        event_id: model.event_id,
        event_type: model.event_type,
        payload: model.payload,
        chain_depth: model.chain_depth,
        occurred_at: model.occurred_at,
        dispatched_at: model.dispatched_at,
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset, Utc};
    use core_application::{
        event::{Event, EventRepository, NewEvent},
        strategy_task::{StrategyTask, StrategyTaskPhase, StrategyTaskRepository},
        trigger::{NewTrigger, TriggerKind, TriggerRepository},
        unit_of_work::UnitOfWork,
    };
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::sea_query::Expr;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
    use serde_json::json;
    use uuid::Uuid;

    use crate::entities::{event as event_entity, event_firing, strategy};
    use crate::{
        DatabaseHandle, PostgresEventRepository, PostgresStrategyTaskRepository,
        PostgresTriggerRepository, PostgresUnitOfWork,
    };

    fn fixed_time(hours_from_epoch: i64) -> DateTime<FixedOffset> {
        (DateTime::<Utc>::UNIX_EPOCH + chrono::Duration::hours(hours_from_epoch)).fixed_offset()
    }

    fn normalize_event(mut event: Event) -> Event {
        event.event_id = Uuid::nil();
        event.occurred_at = fixed_time(0);
        event
    }

    async fn create_event(
        repository: &PostgresEventRepository,
        unit_of_work: &PostgresUnitOfWork,
        event_type: &str,
        chain_depth: Option<i64>,
    ) -> Event {
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        let event = repository
            .create(
                &transaction,
                NewEvent {
                    event_type: event_type.to_string(),
                    payload: json!({"source": "sample"}),
                    chain_depth,
                },
            )
            .await
            .expect("create event");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");
        event
    }

    async fn set_occurred_at(
        db: &DatabaseHandle,
        event_id: Uuid,
        occurred_at: DateTime<FixedOffset>,
    ) {
        event_entity::Entity::update_many()
            .col_expr(event_entity::Column::OccurredAt, Expr::value(occurred_at))
            .filter(event_entity::Column::EventId.eq(event_id))
            .exec(db)
            .await
            .expect("set event time");
    }

    async fn seed_strategy(db: &DatabaseHandle, strategy_id: Uuid) {
        strategy::ActiveModel {
            id: Set(strategy_id),
            name: Set("Sample strategy".to_string()),
            description: Set(None),
            sort_order: Set(0),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert strategy");
    }

    fn trigger(trigger_id: Uuid, strategy_id: Uuid, event_type: &str, enabled: bool) -> NewTrigger {
        NewTrigger {
            trigger_id,
            strategy_id,
            purpose: None,
            kind: TriggerKind::Event,
            schedule: None,
            hook_slug: None,
            event_type: Some(event_type.to_string()),
            event_match: None,
            prompt_template: "sample prompt".to_string(),
            enabled,
            business_days_only: false,
        }
    }

    fn task(task_id: Uuid, strategy_id: Uuid) -> StrategyTask {
        let now = fixed_time(20_000);
        StrategyTask {
            task_id,
            strategy_id,
            a2a_task_id: None,
            source: "sample source".to_string(),
            prompt: "sample prompt".to_string(),
            phase: StrategyTaskPhase::Pending,
            error_summary: None,
            result_text: None,
            deadline_at: now,
            purpose: None,
            as_of: None,
            context_kind: None,
            context_id: None,
            auto_resumed_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[backend_test_macros::database_test]
    async fn create_event_uses_defaults_and_preserves_chain_depth(db: DatabaseHandle) {
        let repository = PostgresEventRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let default_depth =
            create_event(&repository, &unit_of_work, "sample_process.finished", None).await;
        let explicit_depth =
            create_event(&repository, &unit_of_work, "sample_process.failed", Some(3)).await;

        assert_eq!(
            vec![
                normalize_event(default_depth),
                normalize_event(explicit_depth)
            ],
            vec![
                Event {
                    event_id: Uuid::nil(),
                    event_type: "sample_process.finished".to_string(),
                    payload: json!({"source": "sample"}),
                    chain_depth: 0,
                    occurred_at: fixed_time(0),
                    dispatched_at: None,
                },
                Event {
                    event_id: Uuid::nil(),
                    event_type: "sample_process.failed".to_string(),
                    payload: json!({"source": "sample"}),
                    chain_depth: 3,
                    occurred_at: fixed_time(0),
                    dispatched_at: None,
                },
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn lock_next_undispatched_returns_oldest_event_and_skips_dispatched_events(
        db: DatabaseHandle,
    ) {
        let repository = PostgresEventRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let mut early =
            create_event(&repository, &unit_of_work, "sample_process.early", None).await;
        let middle = create_event(&repository, &unit_of_work, "sample_process.middle", None).await;
        let mut late = create_event(&repository, &unit_of_work, "sample_process.late", None).await;
        early.occurred_at = fixed_time(1);
        late.occurred_at = fixed_time(3);
        set_occurred_at(&db, early.event_id, early.occurred_at).await;
        set_occurred_at(&db, middle.event_id, fixed_time(2)).await;
        set_occurred_at(&db, late.event_id, late.occurred_at).await;
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        repository
            .mark_dispatched(&transaction, middle.event_id, fixed_time(4))
            .await
            .expect("mark dispatched");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");

        let first_transaction = unit_of_work.begin().await.expect("begin first transaction");
        let first = repository
            .lock_next_undispatched(&first_transaction)
            .await
            .expect("lock first undispatched event")
            .expect("first event exists");
        repository
            .mark_dispatched(&first_transaction, first.event_id, fixed_time(5))
            .await
            .expect("mark first event dispatched");
        unit_of_work
            .commit(first_transaction)
            .await
            .expect("commit first transaction");
        let second_transaction = unit_of_work
            .begin()
            .await
            .expect("begin second transaction");
        let second = repository
            .lock_next_undispatched(&second_transaction)
            .await
            .expect("lock second undispatched event")
            .expect("second event exists");
        unit_of_work
            .commit(second_transaction)
            .await
            .expect("commit second transaction");

        assert_eq!(
            (first.event_id, second.event_id),
            (early.event_id, late.event_id)
        );
    }

    #[backend_test_macros::database_test]
    async fn lock_next_undispatched_skips_an_event_locked_by_another_transaction(
        _db: DatabaseHandle,
    ) {
        let pool = crate::test_support::create_test_pool_with_max_connections(2).await;
        let db = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
        let db = DatabaseHandle::from(db);
        let repository = PostgresEventRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let event = create_event(&repository, &unit_of_work, "sample_process.finished", None).await;
        let first_transaction = unit_of_work.begin().await.expect("begin first transaction");
        let locked = repository
            .lock_next_undispatched(&first_transaction)
            .await
            .expect("lock event")
            .map(|event| event.event_id);
        let second_transaction = unit_of_work
            .begin()
            .await
            .expect("begin second transaction");
        let skipped = repository
            .lock_next_undispatched(&second_transaction)
            .await
            .expect("skip locked event")
            .map(|event| event.event_id);
        unit_of_work
            .commit(first_transaction)
            .await
            .expect("commit first transaction");
        unit_of_work
            .commit(second_transaction)
            .await
            .expect("commit second transaction");
        event_entity::Entity::delete_by_id(event.event_id)
            .exec(&db)
            .await
            .expect("delete event fixture");

        assert_eq!((locked, skipped), (Some(event.event_id), None));
    }

    #[backend_test_macros::database_test]
    async fn mark_dispatched_is_idempotent_and_unknown_ids_return_false(db: DatabaseHandle) {
        let repository = PostgresEventRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let event = create_event(&repository, &unit_of_work, "sample_process.finished", None).await;
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        let first = repository
            .mark_dispatched(&transaction, event.event_id, fixed_time(1))
            .await
            .expect("mark dispatched");
        let repeated = repository
            .mark_dispatched(&transaction, event.event_id, fixed_time(2))
            .await
            .expect("mark dispatched again");
        let missing = repository
            .mark_dispatched(&transaction, Uuid::from_u128(999), fixed_time(2))
            .await
            .expect("mark unknown event");
        let has_firing = repository
            .has_firing(&transaction, event.event_id, Uuid::from_u128(100))
            .await
            .expect("check event firing");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");
        let pending_ids = event_entity::Entity::find()
            .filter(event_entity::Column::DispatchedAt.is_null())
            .all(&db)
            .await
            .expect("list pending events")
            .into_iter()
            .map(|event| event.event_id)
            .collect::<Vec<_>>();

        assert_eq!(
            (first, repeated, missing, has_firing, pending_ids),
            (true, false, false, false, vec![])
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_before_removes_events_older_than_the_cutoff(db: DatabaseHandle) {
        let repository = PostgresEventRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let before = create_event(&repository, &unit_of_work, "sample_process.before", None).await;
        let at_cutoff =
            create_event(&repository, &unit_of_work, "sample_process.at_cutoff", None).await;
        let after = create_event(&repository, &unit_of_work, "sample_process.after", None).await;
        set_occurred_at(&db, before.event_id, fixed_time(-1)).await;
        set_occurred_at(&db, at_cutoff.event_id, fixed_time(0)).await;
        set_occurred_at(&db, after.event_id, fixed_time(1)).await;

        let deleted = repository
            .delete_before(fixed_time(0))
            .await
            .expect("delete old events");
        let survivor_ids = event_entity::Entity::find()
            .order_by_asc(event_entity::Column::OccurredAt)
            .order_by_asc(event_entity::Column::EventId)
            .all(&db)
            .await
            .expect("list retained events")
            .into_iter()
            .map(|event| event.event_id)
            .collect::<Vec<_>>();

        assert_eq!(
            (deleted, survivor_ids),
            (1, vec![at_cutoff.event_id, after.event_id]),
        );
    }

    #[backend_test_macros::database_test]
    async fn insert_firing_deduplicates_event_trigger_pair_and_lists_matching_triggers(
        db: DatabaseHandle,
    ) {
        let repository = PostgresEventRepository::new(db.clone());
        let trigger_repository = PostgresTriggerRepository::new(db.clone());
        let task_repository = PostgresStrategyTaskRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let strategy_id = Uuid::from_u128(100);
        let trigger_id = Uuid::from_u128(102);
        let disabled_trigger_id = Uuid::from_u128(103);
        let other_event_trigger_id = Uuid::from_u128(104);
        let task_id = Uuid::from_u128(105);
        seed_strategy(&db, strategy_id).await;
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        let event = repository
            .create(
                &transaction,
                NewEvent {
                    event_type: "sample_process.finished".to_string(),
                    payload: json!({"source": "sample"}),
                    chain_depth: None,
                },
            )
            .await
            .expect("create event");
        let event_id = event.event_id;
        trigger_repository
            .create(
                &transaction,
                trigger(trigger_id, strategy_id, "sample_process.finished", true),
            )
            .await
            .expect("create matching trigger");
        trigger_repository
            .create(
                &transaction,
                trigger(
                    disabled_trigger_id,
                    strategy_id,
                    "sample_process.finished",
                    false,
                ),
            )
            .await
            .expect("create disabled trigger");
        trigger_repository
            .create(
                &transaction,
                trigger(
                    other_event_trigger_id,
                    strategy_id,
                    "sample_process.failed",
                    true,
                ),
            )
            .await
            .expect("create other trigger");
        task_repository
            .insert(&transaction, task(task_id, strategy_id))
            .await
            .expect("insert task");
        let first_insert = repository
            .insert_firing(&transaction, event_id, trigger_id, task_id)
            .await
            .expect("insert event firing");
        let repeated_insert = repository
            .insert_firing(&transaction, event_id, trigger_id, task_id)
            .await
            .expect("repeat event firing");
        let has_firing = repository
            .has_firing(&transaction, event_id, trigger_id)
            .await
            .expect("check event firing");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");

        let firings = event_firing::Entity::find()
            .all(&db)
            .await
            .expect("list event firings")
            .into_iter()
            .map(|row| (row.event_id, row.trigger_id, row.task_id))
            .collect::<Vec<_>>();
        let matching_triggers = trigger_repository
            .list_enabled_event_by_type("sample_process.finished")
            .await
            .expect("list enabled event triggers")
            .into_iter()
            .map(|trigger| trigger.trigger_id)
            .collect::<Vec<_>>();

        assert_eq!(
            (
                first_insert,
                repeated_insert,
                has_firing,
                firings,
                matching_triggers,
            ),
            (
                true,
                false,
                true,
                vec![(event_id, trigger_id, task_id)],
                vec![trigger_id],
            ),
        );
    }
}
