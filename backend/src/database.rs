use std::{future::Future, pin::Pin, sync::Arc};

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, DbErr, ExecResult,
    IsolationLevel, QueryResult, Statement, TransactionError, TransactionOptions, TransactionTrait,
};

#[derive(Clone)]
enum DatabaseHandleInner {
    Connection(DatabaseConnection),
    Transaction(Arc<DatabaseTransaction>),
}

/// アプリケーションが接続と transaction のどちらも受け取れる DB handle。
#[derive(Clone)]
pub struct DatabaseHandle {
    inner: DatabaseHandleInner,
}

impl From<DatabaseConnection> for DatabaseHandle {
    fn from(connection: DatabaseConnection) -> Self {
        Self {
            inner: DatabaseHandleInner::Connection(connection),
        }
    }
}

impl From<DatabaseTransaction> for DatabaseHandle {
    fn from(transaction: DatabaseTransaction) -> Self {
        Self {
            inner: DatabaseHandleInner::Transaction(Arc::new(transaction)),
        }
    }
}

impl std::fmt::Debug for DatabaseHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.inner {
            DatabaseHandleInner::Connection(_) => formatter.write_str("DatabaseHandle(Connection)"),
            DatabaseHandleInner::Transaction(_) => {
                formatter.write_str("DatabaseHandle(Transaction)")
            }
        }
    }
}

#[async_trait::async_trait]
impl ConnectionTrait for DatabaseHandle {
    fn get_database_backend(&self) -> DbBackend {
        match &self.inner {
            DatabaseHandleInner::Connection(connection) => connection.get_database_backend(),
            DatabaseHandleInner::Transaction(transaction) => transaction.get_database_backend(),
        }
    }

    async fn execute_raw(&self, statement: Statement) -> Result<ExecResult, DbErr> {
        match &self.inner {
            DatabaseHandleInner::Connection(connection) => connection.execute_raw(statement).await,
            DatabaseHandleInner::Transaction(transaction) => {
                transaction.execute_raw(statement).await
            }
        }
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        match &self.inner {
            DatabaseHandleInner::Connection(connection) => connection.execute_unprepared(sql).await,
            DatabaseHandleInner::Transaction(transaction) => {
                transaction.execute_unprepared(sql).await
            }
        }
    }

    async fn query_one_raw(&self, statement: Statement) -> Result<Option<QueryResult>, DbErr> {
        match &self.inner {
            DatabaseHandleInner::Connection(connection) => {
                connection.query_one_raw(statement).await
            }
            DatabaseHandleInner::Transaction(transaction) => {
                transaction.query_one_raw(statement).await
            }
        }
    }

    async fn query_all_raw(&self, statement: Statement) -> Result<Vec<QueryResult>, DbErr> {
        match &self.inner {
            DatabaseHandleInner::Connection(connection) => {
                connection.query_all_raw(statement).await
            }
            DatabaseHandleInner::Transaction(transaction) => {
                transaction.query_all_raw(statement).await
            }
        }
    }

    fn is_mock_connection(&self) -> bool {
        match &self.inner {
            DatabaseHandleInner::Connection(connection) => connection.is_mock_connection(),
            DatabaseHandleInner::Transaction(transaction) => transaction.is_mock_connection(),
        }
    }
}

#[async_trait::async_trait]
impl TransactionTrait for DatabaseHandle {
    type Transaction = DatabaseTransaction;

    async fn begin(&self) -> Result<Self::Transaction, DbErr> {
        match &self.inner {
            DatabaseHandleInner::Connection(connection) => connection.begin().await,
            DatabaseHandleInner::Transaction(transaction) => transaction.begin().await,
        }
    }

    async fn begin_with_config(
        &self,
        isolation_level: Option<IsolationLevel>,
        access_mode: Option<sea_orm::AccessMode>,
    ) -> Result<Self::Transaction, DbErr> {
        match &self.inner {
            DatabaseHandleInner::Connection(connection) => {
                connection
                    .begin_with_config(isolation_level, access_mode)
                    .await
            }
            DatabaseHandleInner::Transaction(transaction) => {
                transaction
                    .begin_with_config(isolation_level, access_mode)
                    .await
            }
        }
    }

    async fn begin_with_options(
        &self,
        options: TransactionOptions,
    ) -> Result<Self::Transaction, DbErr> {
        match &self.inner {
            DatabaseHandleInner::Connection(connection) => {
                connection.begin_with_options(options).await
            }
            DatabaseHandleInner::Transaction(transaction) => {
                transaction.begin_with_options(options).await
            }
        }
    }

    async fn transaction<F, T, E>(&self, callback: F) -> Result<T, TransactionError<E>>
    where
        F: for<'c> FnOnce(
                &'c Self::Transaction,
            ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'c>>
            + Send,
        T: Send,
        E: std::fmt::Display + std::fmt::Debug + Send,
    {
        match &self.inner {
            DatabaseHandleInner::Connection(connection) => connection.transaction(callback).await,
            DatabaseHandleInner::Transaction(transaction) => {
                transaction.transaction(callback).await
            }
        }
    }

    async fn transaction_with_config<F, T, E>(
        &self,
        callback: F,
        isolation_level: Option<IsolationLevel>,
        access_mode: Option<sea_orm::AccessMode>,
    ) -> Result<T, TransactionError<E>>
    where
        F: for<'c> FnOnce(
                &'c Self::Transaction,
            ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'c>>
            + Send,
        T: Send,
        E: std::fmt::Display + std::fmt::Debug + Send,
    {
        match &self.inner {
            DatabaseHandleInner::Connection(connection) => {
                connection
                    .transaction_with_config(callback, isolation_level, access_mode)
                    .await
            }
            DatabaseHandleInner::Transaction(transaction) => {
                transaction
                    .transaction_with_config(callback, isolation_level, access_mode)
                    .await
            }
        }
    }
}
