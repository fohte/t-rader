use std::sync::Arc;

use core_application::calendar::use_cases::CalendarEventUseCases;
use gateway_postgres::PostgresCalendarEventRepository;

use super::UseCases;

impl UseCases {
    pub fn calendar_events(&self) -> CalendarEventUseCases {
        CalendarEventUseCases::new(Arc::new(PostgresCalendarEventRepository::new(
            self.db.clone(),
        )))
    }
}
