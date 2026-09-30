use core_application::strategy_task::StrategyTaskReconcileJobUseCases;

use super::UseCases;

impl UseCases {
    pub fn strategy_task_reconcile_job(&self) -> StrategyTaskReconcileJobUseCases {
        StrategyTaskReconcileJobUseCases::new(self.strategy_task_reconcile_job_queue.clone())
    }
}
