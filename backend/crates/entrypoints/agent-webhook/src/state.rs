use std::sync::Arc;

use core_application::strategy_task::StrategyTaskReconcileJobUseCases;

#[derive(Clone)]
pub struct AgentWebhookState {
    pub strategy_task_reconcile_job_use_cases: StrategyTaskReconcileJobUseCases,
    pub webhook_token: Arc<str>,
}
