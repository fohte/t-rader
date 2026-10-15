use core_application::event_dispatch_job::{
    EVENT_DISPATCH_JOB_IDENTIFIER, EVENT_DISPATCH_QUEUE_NAME, EventDispatchJobQueue,
    EventDispatchJobQueueError,
};
use sea_orm::ConnectionTrait;

use crate::DatabaseHandle;

pub struct PostgresEventDispatchJobQueue {
    db: DatabaseHandle,
}

impl PostgresEventDispatchJobQueue {
    pub fn new(db: impl Into<DatabaseHandle>) -> Self {
        Self { db: db.into() }
    }
}

#[async_trait::async_trait]
impl EventDispatchJobQueue for PostgresEventDispatchJobQueue {
    async fn enqueue_dispatch(&self) -> Result<(), EventDispatchJobQueueError> {
        self.db
            .execute_unprepared(&format!(
                "SELECT graphile_worker.add_job(\
                    '{EVENT_DISPATCH_JOB_IDENTIFIER}',\
                    '{{}}'::json,\
                    queue_name := '{EVENT_DISPATCH_QUEUE_NAME}',\
                    max_attempts := 3,\
                    job_key := '{EVENT_DISPATCH_JOB_IDENTIFIER}'\
                )"
            ))
            .await
            .map_err(|error| EventDispatchJobQueueError::new(error.to_string()))?;

        Ok(())
    }
}
