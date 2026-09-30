use thiserror::Error;

use crate::news_aggregator::NewsAggregatorError;
use crate::rss_feed::RssFeedRepositoryError;
use crate::unit_of_work::UnitOfWorkError;

use super::repository::NewsItemRepositoryError;

#[derive(Debug, Error)]
pub enum NewsUseCaseError {
    #[error(transparent)]
    Aggregator(#[from] NewsAggregatorError),
    #[error(transparent)]
    FeedRepository(#[from] RssFeedRepositoryError),
    #[error(transparent)]
    NewsRepository(#[from] NewsItemRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
}
