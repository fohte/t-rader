use std::time::Duration;

use core_application::{
    bars::{UsStockBarsWriteLock, UsStockBarsWriteLockGuard},
    persistence::PersistenceError,
};
use sqlx::postgres::{PgAdvisoryLock, PgAdvisoryLockGuard, PgAdvisoryLockKey, PgConnection};
use sqlx::{Connection as _, Either, PgPool};

const LOCK_KEY: i64 = 0x7452_6164_6572_4261;

pub struct PostgresUsStockBarsWriteLock {
    pool: Option<PgPool>,
}

impl PostgresUsStockBarsWriteLock {
    pub fn new(pool: PgPool) -> Self {
        Self { pool: Some(pool) }
    }

    pub fn from_database_handle(database: &crate::DatabaseHandle) -> Self {
        Self {
            pool: database.postgres_connection_pool(),
        }
    }
}

#[async_trait::async_trait]
impl UsStockBarsWriteLock for PostgresUsStockBarsWriteLock {
    async fn acquire(
        &self,
        wait_timeout: Duration,
    ) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, PersistenceError> {
        let Some(pool) = self.pool.as_ref() else {
            return Err(PersistenceError::Database(
                "Postgres bars write lock requires a PostgreSQL connection pool".to_owned(),
            ));
        };
        let options = pool.connect_options();
        let mut connection = PgConnection::connect_with(options.as_ref())
            .await
            .map_err(database_error)?;

        if wait_timeout.is_zero() {
            return self.try_acquire(connection).await;
        }

        let lock_timeout = format!("{}ms", wait_timeout.as_millis().max(1));
        if let Err(error) = sqlx::query("SELECT set_config('lock_timeout', $1, false)")
            .bind(&lock_timeout)
            .fetch_one(&mut connection)
            .await
        {
            let message = error.to_string();
            let _ = connection.close().await;
            return Err(PersistenceError::Database(message));
        }

        match session_lock().acquire(connection).await {
            Ok(guard) => Ok(Some(Box::new(PostgresUsStockBarsWriteLockGuard {
                _guard: guard,
            }))),
            Err(error) if is_lock_timeout(&error) => Ok(None),
            Err(error) => Err(database_error(error)),
        }
    }
}

impl PostgresUsStockBarsWriteLock {
    async fn try_acquire(
        &self,
        connection: PgConnection,
    ) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, PersistenceError> {
        match session_lock().try_acquire(connection).await {
            Ok(Either::Left(guard)) => Ok(Some(Box::new(PostgresUsStockBarsWriteLockGuard {
                _guard: guard,
            }))),
            Ok(Either::Right(_connection)) => Ok(None),
            Err(error) => Err(database_error(error)),
        }
    }
}

struct PostgresUsStockBarsWriteLockGuard {
    _guard: PgAdvisoryLockGuard<PgConnection>,
}

impl UsStockBarsWriteLockGuard for PostgresUsStockBarsWriteLockGuard {}

fn is_lock_timeout(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .is_some_and(|code| is_lock_timeout_code(&code))
}

fn is_lock_timeout_code(code: &str) -> bool {
    code == "55P03"
}

fn session_lock() -> PgAdvisoryLock {
    PgAdvisoryLock::with_key(PgAdvisoryLockKey::BigInt(LOCK_KEY))
}

fn database_error(error: sqlx::Error) -> PersistenceError {
    PersistenceError::Database(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use core_application::bars::UsStockBarsWriteLock;
    use sqlx::PgPool;

    use super::{LOCK_KEY, PostgresUsStockBarsWriteLock};

    #[test]
    fn recognizes_only_postgres_lock_timeout_errors() {
        assert_eq!(
            ["55P03", "22000", ""].map(super::is_lock_timeout_code),
            [true, false, false],
        );
    }

    async fn wait_for_lock_waiter(pool: &PgPool) -> bool {
        for _ in 0..512 {
            let waiting = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM pg_locks \
                 WHERE locktype = 'advisory' \
                   AND database = (SELECT oid FROM pg_database WHERE datname = current_database()) \
                   AND objsubid = 1 \
                   AND ((classid::bigint << 32) | objid::bigint) = $1 \
                   AND NOT granted)",
            )
            .bind(LOCK_KEY)
            .fetch_one(pool)
            .await
            .expect("query lock wait queue");
            if waiting {
                return true;
            }
            tokio::task::yield_now().await;
        }
        false
    }

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
    async fn queued_session_acquires_lock_after_current_holder_releases(
        _db: crate::DatabaseHandle,
    ) {
        let pool = crate::test_support::create_test_pool_with_max_connections(1).await;
        let lock = Arc::new(PostgresUsStockBarsWriteLock::new(pool.clone()));
        let first = lock
            .acquire(Duration::from_secs(30))
            .await
            .expect("first lock acquisition succeeds")
            .expect("first session gets the lock");
        let waiting_lock = lock.clone();
        let waiter =
            tokio::spawn(async move { waiting_lock.acquire(Duration::from_secs(30)).await });
        let queued = wait_for_lock_waiter(&pool).await;
        assert!(
            queued,
            "second session should wait in PostgreSQL lock queue"
        );

        drop(first);
        let second = waiter
            .await
            .expect("queued task completes")
            .expect("queued lock acquisition succeeds")
            .expect("waiting session gets the released lock");
        let third_blocked = matches!(lock.acquire(Duration::ZERO).await, Ok(None));
        drop(second);
        let reacquired = matches!(lock.acquire(Duration::ZERO).await, Ok(Some(_)));

        assert_eq!((third_blocked, reacquired), (true, true));
    }

    #[backend_test_macros::database_test]
    async fn cancelling_a_waiting_acquire_does_not_leave_a_lock_behind(_db: crate::DatabaseHandle) {
        let pool = crate::test_support::create_test_pool_with_max_connections(1).await;
        let lock = Arc::new(PostgresUsStockBarsWriteLock::new(pool.clone()));
        let first = lock
            .acquire(Duration::from_secs(30))
            .await
            .expect("first lock acquisition succeeds")
            .expect("first session gets the lock");
        let waiting_lock = lock.clone();
        let waiter =
            tokio::spawn(async move { waiting_lock.acquire(Duration::from_secs(30)).await });
        let queued = wait_for_lock_waiter(&pool).await;
        assert!(
            queued,
            "second session should wait in PostgreSQL lock queue"
        );

        waiter.abort();
        let cancelled = match waiter.await {
            Err(error) => error.is_cancelled(),
            Ok(_) => false,
        };
        let first_still_holds_lock = matches!(lock.acquire(Duration::ZERO).await, Ok(None));
        drop(first);
        let reacquired = matches!(lock.acquire(Duration::ZERO).await, Ok(Some(_)));

        assert_eq!(
            (cancelled, first_still_holds_lock, reacquired),
            (true, true, true)
        );
    }
}
