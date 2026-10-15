use std::sync::Arc;

use core_application::strategy_task::StrategyTaskReconcileUseCases;

#[derive(Clone)]
pub struct AgentWebhookState {
    pub strategy_task_reconcile_use_cases: StrategyTaskReconcileUseCases,
    pub webhook_token: Arc<str>,
}
