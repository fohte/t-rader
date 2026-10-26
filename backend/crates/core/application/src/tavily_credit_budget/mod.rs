mod repository;
mod types;
mod use_cases;

pub use repository::{
    BootstrapCommand, BudgetReadiness, ExistingReservation, FinishFailureCommand,
    RecordCacheHitCommand, RecordDenialCommand, ReservationClaim, ReservationCommand,
    ReservationFailureState, ReservationOutcome, SharedTavilyCreditBudgetRepository,
    TavilyCreditBudgetRepository, TavilyCreditBudgetRepositoryError,
};
pub use types::{
    SAFE_EXTRACT_RESERVATION, SAFE_SEARCH_RESERVATION, TavilyCreditBudgetConfig,
    TavilyCreditBudgetDenial, TavilyCreditBudgetOperation, TavilyCreditBudgetReason,
    TavilyCreditBudgetScope, TavilyRetrieved,
};
pub use use_cases::{
    SharedTavilyCreditBudgetUseCases, SystemTavilyCreditBudgetClock, TavilyCreditBudgetClock,
    TavilyCreditBudgetUseCases, TavilyCreditBudgetUseCasesError, TavilyCreditBudgetUseCasesPort,
};

#[cfg(test)]
mod use_cases_tests;
