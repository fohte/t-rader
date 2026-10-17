use std::sync::Arc;

use core_application::agent_task_client::{DisabledAgentTaskClient, SharedAgentTaskClient};
use core_application::bars::SharedUsStockBarsWriteLock;
use core_application::change_history::SharedChangeHistoryPort;
use core_application::strategy_existence::SharedStrategyExistence;
use core_application::unit_of_work::SharedUnitOfWork;
use gateway_postgres::{
    DatabaseHandle, PostgresChangeHistory, PostgresStrategyExistence, PostgresUnitOfWork,
    PostgresUsStockBarsWriteLock,
};
use tokio::sync::Mutex;

automod::dir!("src/services/use_cases");

#[derive(Clone)]
pub struct UseCases {
    pub(super) db: DatabaseHandle,
    pub(super) unit_of_work: SharedUnitOfWork,
    pub(super) strategy_existence: SharedStrategyExistence,
    pub(super) change_history: SharedChangeHistoryPort,
    pub(super) agent_task_client: SharedAgentTaskClient,
    pub(super) us_stock_bars_write_lock: SharedUsStockBarsWriteLock,
    pub(super) event_dispatch_control: Arc<BackgroundEventDispatchControl>,
}

pub(super) struct BackgroundEventDispatchControl {
    pub(super) state: Mutex<BackgroundEventDispatchState>,
}

pub(super) struct BackgroundEventDispatchState {
    pub(super) running: bool,
    pub(super) pending: bool,
}

pub fn build_use_cases(
    db: impl Into<DatabaseHandle>,
    agent_task_client: SharedAgentTaskClient,
) -> Result<UseCases, String> {
    let db = db.into();
    let postgres_pool = db
        .postgres_connection_pool()
        .ok_or_else(|| "US stock bar ingestion requires a PostgreSQL connection pool".to_owned())?;
    let unit_of_work: SharedUnitOfWork = Arc::new(PostgresUnitOfWork::new(db.clone()));
    let strategy_existence: SharedStrategyExistence = Arc::new(PostgresStrategyExistence);
    let change_history: SharedChangeHistoryPort = Arc::new(PostgresChangeHistory);
    let us_stock_bars_write_lock: SharedUsStockBarsWriteLock =
        Arc::new(PostgresUsStockBarsWriteLock::new(postgres_pool));

    Ok(UseCases {
        db,
        unit_of_work,
        strategy_existence,
        change_history,
        agent_task_client,
        us_stock_bars_write_lock,
        event_dispatch_control: Arc::new(BackgroundEventDispatchControl {
            state: Mutex::new(BackgroundEventDispatchState {
                running: false,
                pending: false,
            }),
        }),
    })
}

pub fn build_use_cases_without_agent_task_client(
    db: impl Into<DatabaseHandle>,
) -> Result<UseCases, String> {
    build_use_cases(db, Arc::new(DisabledAgentTaskClient))
}
