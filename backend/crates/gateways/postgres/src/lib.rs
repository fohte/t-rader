use std::{future::Future, pin::Pin, sync::Arc};

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, DbErr, ExecResult,
    IsolationLevel, QueryResult, Statement, TransactionError, TransactionOptions, TransactionTrait,
};

const JQUANTS_SYNC_SOURCE: &str = "jquants";

extern crate self as gateway_postgres;

mod account_risk_policy;
mod agent_config;
mod annotation;
mod annotation_read_query;
mod bars;
mod calendar;
mod change_history;
mod change_history_query;
mod comment;
mod comment_read_query;
mod custom_indicator;
mod earnings_schedule;
pub mod entities;
mod equity_master;
mod financial_summary;
mod group_axis;
mod indicator_observation;
mod ingest_run_log;
mod ingest_status;
mod margin;
mod mcp_tool_call_count;
mod news;
mod news_content;
mod note;
mod note_kind;
mod note_read_query;
mod note_status_change_aggregate;
mod persistence;
mod prediction;
mod refs;
pub mod repositories;
mod rss_feed;
mod shareholding_structure;
mod short_ratio;
mod short_sale_report;
mod stock_group;
mod stock_registration;
mod strategy;
mod strategy_earnings_target;
mod strategy_existence;
mod strategy_scope;
mod strategy_summary_query;
mod strategy_task;
mod strategy_task_reconcile_job_queue;
mod strategy_task_step_evidence;
mod trade;
mod transaction;
mod trigger;
mod unit_of_work;
mod us_stock_master;
mod valuation;

pub use account_risk_policy::PostgresAccountRiskPolicyRepository;
pub use agent_config::PostgresAgentConfigRepository;
pub use annotation::PostgresAnnotationRepository;
pub use annotation_read_query::PostgresAnnotationReadQuery;
pub use bars::PostgresBarsRepository;
pub use calendar::PostgresCalendarEventRepository;
pub use change_history::PostgresChangeHistory;
pub use change_history_query::PostgresChangeHistoryQuery;
pub use comment::PostgresCommentRepository;
pub use comment_read_query::PostgresCommentReadQuery;
pub use custom_indicator::PostgresCustomIndicatorRepository;
pub use earnings_schedule::PostgresEarningsScheduleRepository;
pub use equity_master::PostgresEquityMasterRepository;
pub use financial_summary::PostgresFinancialSummaryRepository;
pub use group_axis::PostgresGroupAxisRepository;
pub use indicator_observation::PostgresIndicatorObservationRepository;
pub use ingest_run_log::PostgresIngestRunLog;
pub use ingest_status::PostgresIngestStatusRepository;
pub use margin::PostgresMarginRepository;
pub use mcp_tool_call_count::PostgresMcpToolCallCountRepository;
pub use news::PostgresNewsItemRepository;
pub use news_content::PostgresNewsContentRepository;
pub use note::{PostgresNoteRepository, supersede_pending_versions_before};
pub use note_kind::PostgresNoteKindRepository;
pub use note_read_query::PostgresNoteReadQuery;
pub use note_status_change_aggregate::PostgresNoteStatusChangeAggregateQuery;
pub use prediction::PostgresPredictionRepository;
pub use refs::PostgresRefRepository;
pub use rss_feed::PostgresRssFeedRepository;
pub use shareholding_structure::PostgresShareholdingStructureRepository;
pub use short_ratio::PostgresShortRatioRepository;
pub use short_sale_report::PostgresShortSaleReportRepository;
pub use stock_group::PostgresStockGroupRepository;
pub use stock_registration::PostgresStockRegistrationRepository;
pub use strategy::PostgresStrategyRepository;
pub use strategy_earnings_target::PostgresStrategyEarningsTargetRepository;
pub use strategy_existence::PostgresStrategyExistence;
pub use strategy_scope::PostgresStrategyScopeSource;
pub use strategy_summary_query::PostgresStrategySummaryQuery;
pub use strategy_task::PostgresStrategyTaskRepository;
pub use strategy_task_reconcile_job_queue::PostgresStrategyTaskReconcileJobQueue;
pub use strategy_task_step_evidence::PostgresStrategyTaskStepEvidenceRepository;
pub use trade::PostgresTradeRepository;
pub use trigger::PostgresTriggerRepository;
pub use unit_of_work::PostgresUnitOfWork;
pub use us_stock_master::PostgresUsStockMasterRepository;
pub use valuation::PostgresValuationRepository;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

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
