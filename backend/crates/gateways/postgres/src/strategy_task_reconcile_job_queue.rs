use core_application::strategy_task::{
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
            .execute_unprepared(
                "SELECT graphile_worker.add_job(\
                    'strategy_task_reconcile',\
                    '{}'::json,\
                    max_attempts := 3,\
                    job_key := 'strategy_task_reconcile'\
                )",
            )
            .await
            .map_err(|error| StrategyTaskReconcileJobQueueError::new(error.to_string()))?;

        Ok(())
    }
}
