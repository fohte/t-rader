use std::sync::Arc;

use core_application::calendar::{
    read_use_cases::CalendarEventReadUseCases, references::CalendarEventReferenceUseCases,
    repository::SharedCalendarEventRepository, target_source::CalendarEventTargetSourceAdapter,
    use_cases::CalendarEventUseCases,
};
use core_application::sec_filing_reference::{
    SecFilingReferenceUseCases, SecFilingTargetRepository,
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

    pub fn calendar_event_references(&self) -> CalendarEventReferenceUseCases {
        CalendarEventReferenceUseCases::new(
            Arc::new(PostgresCalendarEventRepository::new(self.db.clone())),
            Arc::new(PostgresUnitOfWork::new(self.db.clone())),
        )
    }

    pub fn sec_filing_references(&self) -> SecFilingReferenceUseCases {
        let repository = Arc::new(PostgresCalendarEventRepository::new(self.db.clone()));
        let target_repository: Arc<dyn SecFilingTargetRepository> = repository.clone();
        let calendar_repository: SharedCalendarEventRepository = repository;
        SecFilingReferenceUseCases::new(
            target_repository,
            CalendarEventReferenceUseCases::new(
                calendar_repository,
                Arc::new(PostgresUnitOfWork::new(self.db.clone())),
            ),
        )
    }
}
