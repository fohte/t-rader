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
    FetchedNewsItemContent, NewsItemContentStatus, NewsItemRepository, NewsItemRepositoryError,
    NewsSearchCriteria, SharedNewsItemRepository, UpsertedNewsItem, sanitize_search_keyword,
};
pub use types::{AggregationStats, NewsArticle, NewsArticleContent, SearchNewsQuery};
pub use use_cases::NewsUseCases;
