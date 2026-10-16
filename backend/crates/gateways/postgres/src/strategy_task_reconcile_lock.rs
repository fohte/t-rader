use core_application::{
    persistence::PersistenceError,
    strategy_task::{StrategyTaskReconcileLock, StrategyTaskReconcileLockGuard},
};
use sqlx::pool::PoolConnection;
use sqlx::{PgPool, Postgres};

const LOCK_KEY: i64 = 0x7452_6164_6572_5263;

pub struct PostgresStrategyTaskReconcileLock {
    pool: PgPool,
}

impl PostgresStrategyTaskReconcileLock {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl StrategyTaskReconcileLock for PostgresStrategyTaskReconcileLock {
    async fn acquire(&self) -> Result<Box<dyn StrategyTaskReconcileLockGuard>, PersistenceError> {
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|error| PersistenceError::Database(error.to_string()))?;
        sqlx::query("SELECT pg_advisory_lock($1)")
            .bind(LOCK_KEY)
            .fetch_one(&mut *connection)
            .await
            .map_err(|error| PersistenceError::Database(error.to_string()))?;

        Ok(Box::new(PostgresStrategyTaskReconcileLockGuard {
            connection: Some(connection),
        }))
    }

    async fn try_acquire(
        &self,
    ) -> Result<Option<Box<dyn StrategyTaskReconcileLockGuard>>, PersistenceError> {
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|error| PersistenceError::Database(error.to_string()))?;
        let acquired = sqlx::query_scalar::<_, bool>("SELECT pg_try_advisory_lock($1)")
            .bind(LOCK_KEY)
            .fetch_one(&mut *connection)
            .await
            .map_err(|error| PersistenceError::Database(error.to_string()))?;

        if !acquired {
            return Ok(None);
        }

        Ok(Some(Box::new(PostgresStrategyTaskReconcileLockGuard {
            connection: Some(connection),
        })))
    }
}

struct PostgresStrategyTaskReconcileLockGuard {
    connection: Option<PoolConnection<Postgres>>,
}

#[async_trait::async_trait]
impl StrategyTaskReconcileLockGuard for PostgresStrategyTaskReconcileLockGuard {
    async fn release(mut self: Box<Self>) -> Result<(), PersistenceError> {
        let Some(mut connection) = self.connection.take() else {
            return Ok(());
        };
        match unlock(&mut connection).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(PersistenceError::Database(
                "strategy task reconcile advisory lock was not held".to_string(),
            )),
            Err(error) => {
                let message = error.to_string();
                let _ = connection.close().await;
                Err(PersistenceError::Database(message))
            }
        }
    }
}

impl Drop for PostgresStrategyTaskReconcileLockGuard {
    fn drop(&mut self) {
        let Some(connection) = self.connection.take() else {
            return;
        };
        drop(connection.detach());
    }
}

async fn unlock(connection: &mut PoolConnection<Postgres>) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>("SELECT pg_advisory_unlock($1)")
        .bind(LOCK_KEY)
        .fetch_one(&mut **connection)
        .await
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use core_application::strategy_task::StrategyTaskReconcileLock;

    use super::PostgresStrategyTaskReconcileLock;

    #[backend_test_macros::database_test]
    async fn advisory_lock_is_exclusive_and_released(_db: crate::DatabaseHandle) {
        let pool = crate::test_support::create_test_pool_with_max_connections(2).await;
        let lock = PostgresStrategyTaskReconcileLock::new(pool);
        let first_guard = lock.try_acquire().await;
        let first_acquired = matches!(&first_guard, Ok(Some(_)));
        let second_is_blocked = matches!(lock.try_acquire().await, Ok(None));
        let first_release = match first_guard {
            Ok(Some(guard)) => guard.release().await.map_err(|error| error.to_string()),
            Ok(None) => Err("first advisory lock was not acquired".to_string()),
            Err(error) => Err(error.to_string()),
        };
        let third_guard = lock.try_acquire().await;
        let third_acquired = matches!(&third_guard, Ok(Some(_)));
        let third_release = match third_guard {
            Ok(Some(guard)) => guard.release().await.map_err(|error| error.to_string()),
            Ok(None) => Err("third advisory lock was not acquired".to_string()),
            Err(error) => Err(error.to_string()),
        };

        assert_eq!(
            (
                first_acquired,
                second_is_blocked,
                first_release,
                third_acquired,
                third_release,
            ),
            (true, true, Ok(()), true, Ok(())),
        );
    }

    #[backend_test_macros::database_test]
    async fn blocking_advisory_lock_waits_until_the_lock_is_released(_db: crate::DatabaseHandle) {
        let pool = crate::test_support::create_test_pool_with_max_connections(2).await;
        let lock = Arc::new(PostgresStrategyTaskReconcileLock::new(pool));
        let first_guard = lock
            .try_acquire()
            .await
            .expect("acquire first advisory lock")
            .expect("first advisory lock should be available");
        let waiter_lock = lock.clone();
        let mut waiter = tokio::spawn(async move { waiter_lock.acquire().await });
        let waited_for_lock = tokio::time::timeout(Duration::from_millis(50), &mut waiter)
            .await
            .is_err();
        let first_release = first_guard
            .release()
            .await
            .map_err(|error| error.to_string());
        let second_guard = tokio::time::timeout(Duration::from_secs(2), waiter).await;
        let (second_acquired, second_release) = match second_guard {
            Ok(Ok(Ok(guard))) => (
                true,
                guard.release().await.map_err(|error| error.to_string()),
            ),
            Ok(Ok(Err(error))) => (false, Err(error.to_string())),
            Ok(Err(error)) => (false, Err(error.to_string())),
            Err(_) => (false, Err("second advisory lock timed out".to_string())),
        };

        assert_eq!(
            (
                waited_for_lock,
                first_release,
                second_acquired,
                second_release,
            ),
            (true, Ok(()), true, Ok(())),
        );
    }
}
