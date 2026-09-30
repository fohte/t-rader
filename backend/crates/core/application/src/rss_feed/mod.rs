mod error;
#[cfg(feature = "test-support")]
mod fake;
mod repository;
mod types;
mod url_validator;
mod use_cases;

pub use error::RssFeedUseCaseError;
#[cfg(feature = "test-support")]
pub use fake::FakeRssFeedRepository;
pub use repository::{RssFeedRepository, RssFeedRepositoryError, SharedRssFeedRepository};
pub use types::{CreateRssFeedCommand, NewRssFeed, RssFeed, UpdateRssFeedPatch};
pub use url_validator::{RssFeedUrlValidator, SharedRssFeedUrlValidator};
pub use use_cases::RssFeedUseCases;
