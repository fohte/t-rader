mod error;
mod repository;
mod types;
mod use_cases;

pub use error::NewsUseCaseError;
pub use repository::{
    NewsItemRepository, NewsItemRepositoryError, NewsSearchCriteria, SharedNewsItemRepository,
};
pub use types::{AggregationStats, NewsArticle, SearchNewsQuery};
pub use use_cases::NewsUseCases;
