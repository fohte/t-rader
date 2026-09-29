use std::sync::Arc;

use core_application::change_history::SharedChangeHistoryPort;
use core_application::strategy_existence::SharedStrategyExistence;
use core_application::strategy_task::{SharedStrategyTaskRepository, StrategyTaskUseCases};
use core_application::trade::{SharedTradeRepository, TradeUseCases};
use core_application::unit_of_work::SharedUnitOfWork;
use gateway_postgres::{
    DatabaseHandle, PostgresChangeHistory, PostgresStrategyExistence,
    PostgresStrategyTaskRepository, PostgresTradeRepository, PostgresUnitOfWork,
};

#[derive(Clone)]
pub struct UseCases {
    pub trades: TradeUseCases,
    pub strategy_tasks: StrategyTaskUseCases,
}

pub fn build_use_cases(db: impl Into<DatabaseHandle>) -> UseCases {
    let db = db.into();
    let unit_of_work: SharedUnitOfWork = Arc::new(PostgresUnitOfWork::new(db.clone()));
    let repository: SharedTradeRepository = Arc::new(PostgresTradeRepository::new(db.clone()));
    let strategy_existence: SharedStrategyExistence = Arc::new(PostgresStrategyExistence);
    let change_history: SharedChangeHistoryPort = Arc::new(PostgresChangeHistory);
    let trades = TradeUseCases::new(unit_of_work, repository, strategy_existence, change_history);

    let strategy_tasks = build_strategy_task_use_cases(db);

    UseCases {
        trades,
        strategy_tasks,
    }
}

pub fn build_strategy_task_use_cases(db: impl Into<DatabaseHandle>) -> StrategyTaskUseCases {
    let db = db.into();
    let task_unit_of_work: SharedUnitOfWork = Arc::new(PostgresUnitOfWork::new(db.clone()));
    let task_repository: SharedStrategyTaskRepository =
        Arc::new(PostgresStrategyTaskRepository::new(db));
    StrategyTaskUseCases::new(task_unit_of_work, task_repository)
}
