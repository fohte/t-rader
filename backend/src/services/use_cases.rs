use std::sync::Arc;

use core_application::change_history::SharedChangeHistoryPort;
use core_application::strategy_existence::SharedStrategyExistence;
use core_application::unit_of_work::SharedUnitOfWork;
use gateway_postgres::{
    DatabaseHandle, PostgresChangeHistory, PostgresStrategyExistence, PostgresUnitOfWork,
};

mod account_risk_policy;
mod annotation;
mod comment;
mod custom_indicator;
mod indicator_observation;
mod margin;
mod news;
mod note;
mod note_kind;
mod prediction;
mod refs;
mod rss_feed;
mod short_ratio;
mod short_sale_report;
mod strategy;
mod strategy_task;
mod trade;
mod trigger;

#[derive(Clone)]
pub struct UseCases {
    pub(super) db: DatabaseHandle,
    pub(super) unit_of_work: SharedUnitOfWork,
    pub(super) strategy_existence: SharedStrategyExistence,
    pub(super) change_history: SharedChangeHistoryPort,
}

pub fn build_use_cases(db: impl Into<DatabaseHandle>) -> UseCases {
    let db = db.into();
    let unit_of_work: SharedUnitOfWork = Arc::new(PostgresUnitOfWork::new(db.clone()));
    let strategy_existence: SharedStrategyExistence = Arc::new(PostgresStrategyExistence);
    let change_history: SharedChangeHistoryPort = Arc::new(PostgresChangeHistory);

    UseCases {
        db,
        unit_of_work,
        strategy_existence,
        change_history,
    }
}
