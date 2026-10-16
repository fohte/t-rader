use core_application::strategy_task::{
    StrategyTaskReconcileLockUseCases, StrategyTaskReconcileUseCases,
};

use super::UseCases;

impl UseCases {
    pub fn strategy_task_reconcile(
        &self,
        lock: StrategyTaskReconcileLockUseCases,
    ) -> StrategyTaskReconcileUseCases {
        StrategyTaskReconcileUseCases::new(
            self.strategy_tasks(),
            lock,
            self.agent_task_client.clone(),
        )
    }
}
