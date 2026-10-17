use std::time::{Duration, Instant};

use core_application::{
    bars::{UsStockBarsWriteLock, UsStockBarsWriteLockGuard},
    persistence::PersistenceError,
};
use sqlx::pool::PoolConnection;
use sqlx::{Connection as _, PgPool, Postgres};

const LOCK_KEY: i64 = 0x7452_6164_6572_4261;

pub struct PostgresUsStockBarsWriteLock {
    pool: PgPool,
}

impl PostgresUsStockBarsWriteLock {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl UsStockBarsWriteLock for PostgresUsStockBarsWriteLock {
    async fn acquire(
        &self,
        wait_timeout: Duration,
    ) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, PersistenceError> {
        if wait_timeout.is_zero() {
            let Some(connection) = self.pool.try_acquire() else {
                return Ok(None);
            };
            return self.try_acquire(connection).await;
        }

        let started_at = Instant::now();
        let connection = match tokio::time::timeout(wait_timeout, self.pool.acquire()).await {
            Ok(Ok(connection)) => connection,
            Ok(Err(error)) => return Err(database_error(error)),
            Err(_) => return Ok(None),
        };

        let mut connection = connection;
        let mut transaction = connection.begin().await.map_err(database_error)?;
        let remaining = wait_timeout.saturating_sub(started_at.elapsed());
        if remaining.is_zero() {
            let _ = transaction.rollback().await;
            return Ok(None);
        }

        let lock_timeout = format!("{}ms", remaining.as_millis().max(1));
        if let Err(error) = sqlx::query("SELECT set_config('lock_timeout', $1, true)")
            .bind(&lock_timeout)
            .fetch_one(&mut *transaction)
            .await
        {
            let message = error.to_string();
            let _ = transaction.rollback().await;
            let _ = connection.close().await;
            return Err(PersistenceError::Database(message));
        }

        match sqlx::query("SELECT pg_advisory_lock($1)")
            .bind(LOCK_KEY)
            .fetch_one(&mut *transaction)
            .await
        {
            Ok(_) => match transaction.commit().await {
                Ok(()) => Ok(Some(Box::new(PostgresUsStockBarsWriteLockGuard {
                    connection: Some(connection),
                }))),
                Err(error) => {
                    let message = error.to_string();
                    let _ = connection.close().await;
                    Err(PersistenceError::Database(message))
                }
            },
            Err(error) if is_lock_timeout(&error) => match transaction.rollback().await {
                Ok(()) => Ok(None),
                Err(rollback_error) => {
                    let message = rollback_error.to_string();
                    let _ = connection.close().await;
                    Err(PersistenceError::Database(message))
                }
            },
            Err(error) => {
                let message = error.to_string();
                let _ = transaction.rollback().await;
                let _ = connection.close().await;
                Err(PersistenceError::Database(message))
            }
        }
    }
}

impl PostgresUsStockBarsWriteLock {
    async fn try_acquire(
        &self,
        mut connection: PoolConnection<Postgres>,
    ) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, PersistenceError> {
        let acquired = match sqlx::query_scalar::<_, bool>("SELECT pg_try_advisory_lock($1)")
            .bind(LOCK_KEY)
            .fetch_one(&mut *connection)
            .await
        {
            Ok(acquired) => acquired,
            Err(error) => {
                let message = error.to_string();
                let _ = connection.close().await;
                return Err(PersistenceError::Database(message));
            }
        };

        if !acquired {
            return Ok(None);
        }

        Ok(Some(Box::new(PostgresUsStockBarsWriteLockGuard {
            connection: Some(connection),
        })))
    }
}

struct PostgresUsStockBarsWriteLockGuard {
    connection: Option<PoolConnection<Postgres>>,
}

impl UsStockBarsWriteLockGuard for PostgresUsStockBarsWriteLockGuard {}

impl Drop for PostgresUsStockBarsWriteLockGuard {
    fn drop(&mut self) {
        let Some(connection) = self.connection.take() else {
            return;
        };
        drop(connection.detach());
    }
}

fn is_lock_timeout(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .is_some_and(|code| code == "55P03")
}

fn database_error(error: sqlx::Error) -> PersistenceError {
    PersistenceError::Database(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use core_application::bars::UsStockBarsWriteLock;

    use super::PostgresUsStockBarsWriteLock;

    #[backend_test_macros::database_test]
    async fn session_advisory_lock_excludes_other_sessions_and_disconnect_releases_it(
        _db: crate::DatabaseHandle,
    ) {
        let pool = crate::test_support::create_test_pool_with_max_connections(2).await;
        let lock = PostgresUsStockBarsWriteLock::new(pool);
        let first = lock.acquire(Duration::from_secs(5)).await;
        let first_acquired = matches!(&first, Ok(Some(_)));
        let second_blocked = matches!(lock.acquire(Duration::ZERO).await, Ok(None));
        drop(first);
        let third = lock.acquire(Duration::from_secs(5)).await;
        let third_acquired = matches!(&third, Ok(Some(_)));

        assert_eq!(
            (first_acquired, second_blocked, third_acquired),
            (true, true, true),
        );
    }

    #[backend_test_macros::database_test]
    async fn waiting_for_session_lock_returns_none_after_lock_timeout(_db: crate::DatabaseHandle) {
        let pool = crate::test_support::create_test_pool_with_max_connections(2).await;
        let lock = PostgresUsStockBarsWriteLock::new(pool);
        let first = lock.acquire(Duration::from_secs(5)).await;
        let first_acquired = matches!(&first, Ok(Some(_)));
        let timed_out = matches!(lock.acquire(Duration::from_secs(1)).await, Ok(None));
        drop(first);
        let reacquired = matches!(lock.acquire(Duration::from_secs(5)).await, Ok(Some(_)));

        assert_eq!((first_acquired, timed_out, reacquired), (true, true, true));
    }
}
