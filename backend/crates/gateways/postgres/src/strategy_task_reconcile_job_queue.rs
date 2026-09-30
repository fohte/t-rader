use core_application::strategy_task::{
    STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER, STRATEGY_TASK_RECONCILE_QUEUE_NAME,
    StrategyTaskReconcileJobQueue, StrategyTaskReconcileJobQueueError,
};
use sea_orm::ConnectionTrait;

use crate::DatabaseHandle;

pub struct PostgresStrategyTaskReconcileJobQueue {
    db: DatabaseHandle,
}

impl PostgresStrategyTaskReconcileJobQueue {
    pub fn new(db: impl Into<DatabaseHandle>) -> Self {
        Self { db: db.into() }
    }
}

#[async_trait::async_trait]
impl StrategyTaskReconcileJobQueue for PostgresStrategyTaskReconcileJobQueue {
    async fn enqueue_reconciliation(&self) -> Result<(), StrategyTaskReconcileJobQueueError> {
        self.db
            .execute_unprepared(&format!(
                "SELECT graphile_worker.add_job(\
                    '{STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER}',\
                    '{{}}'::json,\
                    queue_name := '{STRATEGY_TASK_RECONCILE_QUEUE_NAME}',\
                    max_attempts := 3,\
                    job_key := '{STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER}'\
                )"
            ))
            .await
            .map_err(|error| StrategyTaskReconcileJobQueueError::new(error.to_string()))?;

        Ok(())
    }
}
