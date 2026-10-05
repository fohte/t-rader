use std::sync::Arc;

use core_application::calendar::{
    read_use_cases::CalendarEventReadUseCases, target_source::CalendarEventTargetSourceAdapter,
    use_cases::CalendarEventUseCases,
};
use gateway_postgres::{PostgresCalendarEventRepository, PostgresUnitOfWork};

use super::UseCases;

impl UseCases {
    pub fn calendar_events(&self) -> CalendarEventUseCases {
        CalendarEventUseCases::new(
            Arc::new(PostgresCalendarEventRepository::new(self.db.clone())),
            Arc::new(PostgresUnitOfWork::new(self.db.clone())),
        )
    }

    pub fn calendar_event_reads(&self) -> CalendarEventReadUseCases {
        CalendarEventReadUseCases::new(
            Arc::new(PostgresCalendarEventRepository::new(self.db.clone())),
            Arc::new(CalendarEventTargetSourceAdapter::new(
                self.strategy_earnings_targets(),
                self.stock_groups(),
            )),
        )
    }
}
