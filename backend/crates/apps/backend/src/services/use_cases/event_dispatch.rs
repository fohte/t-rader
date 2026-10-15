use std::sync::Arc;

use core_application::{event_dispatch::EventDispatchUseCases, event_dispatch_job::EventDispatchJobUseCases};
use gateway_postgres::{
    PostgresEventDispatchJobQueue, PostgresEventRepository,
};

use super::UseCases;

impl UseCases {
    pub fn event_dispatch(&self) -> EventDispatchUseCases {
        EventDispatchUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresEventRepository::new(self.db.clone())),
            self.triggers(),
        )
    }

    pub fn event_dispatch_job(&self) -> EventDispatchJobUseCases {
        EventDispatchJobUseCases::new(Arc::new(PostgresEventDispatchJobQueue::new(
            self.db.clone(),
        )))
    }
}
