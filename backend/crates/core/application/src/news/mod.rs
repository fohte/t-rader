mod error;
#[cfg(feature = "test-support")]
mod fake;
mod repository;
mod types;
mod use_cases;

pub use error::NewsUseCaseError;
#[cfg(feature = "test-support")]
pub use fake::FakeNewsItemRepository;
pub use repository::{
    NewsItemRepository, NewsItemRepositoryError, NewsSearchCriteria, SharedNewsItemRepository,
};
pub use types::{AggregationStats, NewsArticle, SearchNewsQuery};
pub use use_cases::NewsUseCases;
