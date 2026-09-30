mod error;
mod repository;
mod schedule;
mod template;
mod types;
mod use_cases;

#[cfg(feature = "test-support")]
mod fake;

pub use error::TriggerUseCaseError;
pub use repository::{SharedTriggerRepository, TriggerRepository, TriggerRepositoryError};
pub use template::{evaluate_event_match, expand_template};
pub use types::{CreateTriggerCommand, NewTrigger, Trigger, TriggerKind, UpdateTriggerCommand};
pub use use_cases::TriggerUseCases;

#[cfg(feature = "test-support")]
pub use fake::FakeTriggerRepository;
