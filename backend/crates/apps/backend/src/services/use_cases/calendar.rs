use std::sync::Arc;

use core_application::calendar::use_cases::CalendarEventUseCases;
use gateway_postgres::{PostgresCalendarEventRepository, PostgresUnitOfWork};

use super::UseCases;

impl UseCases {
    pub fn calendar_events(&self) -> CalendarEventUseCases {
        CalendarEventUseCases::new(
            Arc::new(PostgresCalendarEventRepository::new()),
            Arc::new(PostgresUnitOfWork::new(self.db.clone())),
        )
    }
}
