use std::sync::Arc;

use core_application::strategy_task::StrategyTaskReconcileJobUseCases;
use gateway_postgres::PostgresStrategyTaskReconcileJobQueue;

use super::UseCases;

impl UseCases {
    pub fn strategy_task_reconcile_job(&self) -> StrategyTaskReconcileJobUseCases {
        StrategyTaskReconcileJobUseCases::new(Arc::new(PostgresStrategyTaskReconcileJobQueue::new(
            self.db.clone(),
        )))
    }
}
