use sqlx::postgres::PgListener;
#[cfg(test)]
use sqlx::postgres::PgPool;

const TRIGGER_CHANGE_CHANNEL: &str = "t_rader_trigger_changed";

pub struct PostgresTriggerChangeListener {
    listener: PgListener,
}

impl PostgresTriggerChangeListener {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let listener = PgListener::connect(database_url).await?;
        Self::subscribe(listener).await
    }

    #[cfg(test)]
    async fn connect_with_pool(pool: &PgPool) -> Result<Self, sqlx::Error> {
        let listener = PgListener::connect_with(pool).await?;
        Self::subscribe(listener).await
    }

    async fn subscribe(mut listener: PgListener) -> Result<Self, sqlx::Error> {
        listener.listen(TRIGGER_CHANGE_CHANNEL).await?;
        Ok(Self { listener })
    }

    pub async fn recv(&mut self) -> Result<(), sqlx::Error> {
        self.listener.try_recv().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::Utc;
    use sea_orm::{ActiveModelTrait, EntityTrait, SqlxPostgresConnector};
    use sqlx::Connection;
    use tokio::time::timeout;
    use uuid::Uuid;

    use super::PostgresTriggerChangeListener;
    use crate::entities::strategy;
    use crate::test_support::create_test_pool_with_max_connections;
    use crate::{DatabaseHandle, PostgresTriggerRepository, PostgresUnitOfWork};
    use core_application::trigger::{NewTrigger, TriggerKind, TriggerRepository};
    use core_application::unit_of_work::UnitOfWork;

    #[tokio::test]
    async fn committed_trigger_update_wakes_listener() {
        let pool = create_test_pool_with_max_connections(3).await;
        let mut lock_connection = pool.acquire().await.expect("acquire test lock connection");
        let mut lock_transaction = lock_connection
            .begin()
            .await
            .expect("begin test lock transaction");
        let _lock_result = sqlx::query_scalar::<_, bool>(
            "SELECT pg_advisory_xact_lock(hashtext('t-rader-test-database'), hashtext('test-execution')) IS NULL",
        )
        .fetch_one(&mut *lock_transaction)
        .await
        .expect("lock shared test database");

        let db_connection = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
        let db = DatabaseHandle::from(db_connection);
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let repository = PostgresTriggerRepository::new(db.clone());
        let strategy_id = Uuid::new_v4();
        let trigger_id = Uuid::new_v4();
        strategy::ActiveModel {
            id: sea_orm::ActiveValue::Set(strategy_id),
            name: sea_orm::ActiveValue::Set("sample strategy".to_string()),
            description: sea_orm::ActiveValue::Set(None),
            sort_order: sea_orm::ActiveValue::Set(0),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&db)
        .await
        .expect("insert test strategy");
        let transaction = unit_of_work
            .begin()
            .await
            .expect("begin trigger transaction");
        repository
            .create(
                &transaction,
                NewTrigger {
                    trigger_id,
                    strategy_id,
                    purpose: None,
                    kind: TriggerKind::Cron,
                    schedule: Some("0 * * * *".to_string()),
                    hook_slug: None,
                    event_type: None,
                    event_match: None,
                    prompt_template: "sample prompt".to_string(),
                    enabled: true,
                    business_days_only: false,
                },
            )
            .await
            .expect("insert test cron trigger");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit trigger insert");

        let mut listener = PostgresTriggerChangeListener::connect_with_pool(&pool)
            .await
            .expect("subscribe to trigger changes");
        let mut trigger = repository
            .find_by_id(trigger_id)
            .await
            .expect("read test trigger")
            .expect("test trigger exists");
        trigger.enabled = false;
        trigger.updated_at = Utc::now().fixed_offset();
        let transaction = unit_of_work.begin().await.expect("begin trigger update");
        repository
            .update(&transaction, trigger)
            .await
            .expect("update test trigger");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit trigger update");

        let received = timeout(Duration::from_secs(1), listener.recv())
            .await
            .map_err(|error| error.to_string())
            .and_then(|result| result.map_err(|error| error.to_string()));

        let transaction = unit_of_work
            .begin()
            .await
            .expect("begin cleanup transaction");
        repository
            .delete(&transaction, trigger_id)
            .await
            .expect("delete test trigger");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit trigger cleanup");
        strategy::Entity::delete_by_id(strategy_id)
            .exec(&db)
            .await
            .expect("delete test strategy");
        drop(listener);
        drop(lock_transaction);
        drop(lock_connection);
        pool.close().await;

        assert_eq!(received, Ok(()));
    }
}
