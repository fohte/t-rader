use crate::news_aggregator::{NewsAggregator, NewsFeed};
use crate::rss_feed::SharedRssFeedRepository;
use crate::strategy_scope::StrategyScope;
use crate::unit_of_work::SharedUnitOfWork;

use super::error::NewsUseCaseError;
use super::repository::{NewsSearchCriteria, SharedNewsItemRepository};
use super::types::{AggregationStats, NewsArticle, SearchNewsQuery};

const DEFAULT_SEARCH_LIMIT: u64 = 50;
const MAX_SEARCH_LIMIT: u64 = 200;

#[derive(Clone)]
pub struct NewsUseCases {
    unit_of_work: SharedUnitOfWork,
    feed_repository: SharedRssFeedRepository,
    news_repository: SharedNewsItemRepository,
}

impl NewsUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        feed_repository: SharedRssFeedRepository,
        news_repository: SharedNewsItemRepository,
    ) -> Self {
        Self {
            unit_of_work,
            feed_repository,
            news_repository,
        }
    }

    pub async fn search_news(
        &self,
        _scope: StrategyScope,
        query: SearchNewsQuery,
    ) -> Result<Vec<NewsArticle>, NewsUseCaseError> {
        let keyword = query
            .keyword
            .map(|keyword| keyword.trim().to_string())
            .filter(|keyword| !keyword.is_empty());
        let limit = query
            .limit
            .map(u64::from)
            .unwrap_or(DEFAULT_SEARCH_LIMIT)
            .clamp(1, MAX_SEARCH_LIMIT);
        self.news_repository
            .search(NewsSearchCriteria {
                keyword,
                from: query.from,
                to: query.to,
                limit,
            })
            .await
            .map_err(Into::into)
    }

    pub async fn run_aggregation_cycle(
        &self,
        aggregator: &dyn NewsAggregator,
    ) -> Result<AggregationStats, NewsUseCaseError> {
        let rows = self.feed_repository.list(true).await?;
        let feeds = rows
            .into_iter()
            .map(|feed| NewsFeed {
                source: feed.display_name,
                url: feed.url,
            })
            .collect::<Vec<_>>();
        let fetched = aggregator.fetch_news(&feeds).await?;
        if fetched.is_empty() {
            return Ok(AggregationStats { fetched: 0 });
        }
        let transaction = self.unit_of_work.begin().await?;
        let count = self.news_repository.upsert(&transaction, &fetched).await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(AggregationStats { fetched: count })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use chrono::{DateTime, NaiveDate, Utc};
    use rstest::rstest;
    use uuid::Uuid;

    use crate::news::repository::{
        NewsItemRepository, NewsItemRepositoryError, NewsSearchCriteria,
    };
    use crate::news::types::{AggregationStats, SearchNewsQuery};
    use crate::news_aggregator::{FakeNewsAggregator, NewsItem};
    use crate::rss_feed::{NewRssFeed, RssFeed, RssFeedRepository, RssFeedRepositoryError};
    use crate::strategy_scope::StrategyScope;

    use super::NewsUseCases;

    #[derive(Clone)]
    struct FakeRssFeedRepository {
        feeds: Vec<RssFeed>,
    }

    #[async_trait]
    impl RssFeedRepository for FakeRssFeedRepository {
        async fn list(&self, enabled_only: bool) -> Result<Vec<RssFeed>, RssFeedRepositoryError> {
            let mut feeds = self
                .feeds
                .iter()
                .filter(|feed| !enabled_only || feed.enabled)
                .cloned()
                .collect::<Vec<_>>();
            feeds.sort_by(|left, right| left.display_name.cmp(&right.display_name));
            Ok(feeds)
        }

        async fn find_by_id(&self, id: Uuid) -> Result<Option<RssFeed>, RssFeedRepositoryError> {
            Ok(self.feeds.iter().find(|feed| feed.id == id).cloned())
        }

        async fn find_by_id_in_transaction(
            &self,
            _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
            id: Uuid,
        ) -> Result<Option<RssFeed>, RssFeedRepositoryError> {
            self.find_by_id(id).await
        }

        async fn create(
            &self,
            _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
            feed: NewRssFeed,
        ) -> Result<RssFeed, RssFeedRepositoryError> {
            let now = Utc::now().fixed_offset();
            Ok(RssFeed {
                id: feed.id,
                source: feed.source,
                display_name: feed.display_name,
                url: feed.url,
                enabled: feed.enabled,
                created_at: now,
                updated_at: now,
            })
        }

        async fn update(
            &self,
            _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
            feed: RssFeed,
        ) -> Result<RssFeed, RssFeedRepositoryError> {
            Ok(feed)
        }

        async fn delete(
            &self,
            _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
            _id: Uuid,
        ) -> Result<bool, RssFeedRepositoryError> {
            Ok(false)
        }
    }

    #[derive(Default)]
    struct FakeNewsItemRepository {
        upserts: Mutex<Vec<Vec<NewsItem>>>,
        searches: Mutex<Vec<NewsSearchCriteria>>,
    }

    #[async_trait]
    impl NewsItemRepository for FakeNewsItemRepository {
        async fn upsert(
            &self,
            _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
            items: &[NewsItem],
        ) -> Result<usize, NewsItemRepositoryError> {
            self.upserts
                .lock()
                .expect("mutex is not poisoned")
                .push(items.to_vec());
            Ok(items
                .iter()
                .map(|item| item.url.as_str())
                .collect::<HashSet<_>>()
                .len())
        }

        async fn search(
            &self,
            criteria: NewsSearchCriteria,
        ) -> Result<Vec<crate::news::NewsArticle>, NewsItemRepositoryError> {
            self.searches
                .lock()
                .expect("mutex is not poisoned")
                .push(criteria);
            Ok(Vec::new())
        }
    }

    fn feed(source: &str, display_name: &str, enabled: bool) -> RssFeed {
        let timestamp = DateTime::from_timestamp(0, 0)
            .expect("valid timestamp")
            .fixed_offset();
        RssFeed {
            id: Uuid::new_v4(),
            source: source.into(),
            display_name: display_name.into(),
            url: format!("https://example.invalid/{source}"),
            enabled,
            created_at: timestamp,
            updated_at: timestamp,
        }
    }

    fn use_cases(feeds: Vec<RssFeed>) -> (NewsUseCases, Arc<FakeNewsItemRepository>) {
        let news_repository = Arc::new(FakeNewsItemRepository::default());
        let use_cases = NewsUseCases::new(
            Arc::new(crate::unit_of_work::FakeUnitOfWork::new()),
            Arc::new(FakeRssFeedRepository { feeds }),
            news_repository.clone(),
        );
        (use_cases, news_repository)
    }

    #[tokio::test]
    async fn aggregation_cycle_fetches_enabled_feeds_and_saves_articles() {
        let (use_cases, news_repository) = use_cases(vec![
            feed("feed-zulu", "Zulu publication", true),
            feed("feed-disabled", "Disabled publication", false),
            feed("feed-alpha", "Alpha publication", true),
        ]);
        let aggregator = FakeNewsAggregator::new();
        let article = NewsItem {
            source: "Sample publication".into(),
            url: "https://example.invalid/article".into(),
            title: "Sample headline".into(),
            body_snippet: Some("Sample summary".into()),
            published_at: DateTime::from_timestamp(0, 0).expect("valid timestamp"),
        };
        *aggregator.items.lock().await = vec![article.clone()];

        let stats = use_cases
            .run_aggregation_cycle(&aggregator)
            .await
            .expect("aggregation cycle succeeds");
        let requested_feeds = aggregator.requested_feeds.lock().await.clone();
        let upserts = news_repository
            .upserts
            .lock()
            .expect("mutex is not poisoned")
            .clone();

        assert_eq!(
            (stats, requested_feeds, upserts),
            (
                AggregationStats { fetched: 1 },
                vec![vec![
                    crate::news_aggregator::NewsFeed {
                        source: "Alpha publication".into(),
                        url: "https://example.invalid/feed-alpha".into(),
                    },
                    crate::news_aggregator::NewsFeed {
                        source: "Zulu publication".into(),
                        url: "https://example.invalid/feed-zulu".into(),
                    },
                ]],
                vec![vec![article]],
            ),
        );
    }

    #[tokio::test]
    async fn aggregation_cycle_stops_before_upsert_when_fetch_fails() {
        let (use_cases, news_repository) = use_cases(Vec::new());
        let aggregator = FakeNewsAggregator::new();
        *aggregator.fetch_error.lock().await = Some(
            crate::news_aggregator::NewsAggregatorError::Network("sample failure".into()),
        );

        let result = use_cases
            .run_aggregation_cycle(&aggregator)
            .await
            .map(|stats| stats.fetched)
            .map_err(|error| error.to_string());
        let upsert_count = news_repository
            .upserts
            .lock()
            .expect("mutex is not poisoned")
            .len();

        assert_eq!(
            (result, upsert_count),
            (Err("network error: sample failure".into()), 0),
        );
    }

    #[rstest]
    #[case::default(None, 50)]
    #[case::minimum(Some(0), 1)]
    #[case::maximum(Some(500), 200)]
    #[tokio::test]
    async fn search_news_normalizes_keyword_and_clamps_limit(
        #[case] limit: Option<u32>,
        #[case] expected_limit: u64,
    ) {
        let (use_cases, news_repository) = use_cases(Vec::new());
        let query = SearchNewsQuery {
            keyword: Some("  Sample query  ".into()),
            from: Some(NaiveDate::from_ymd_opt(2026, 1, 2).expect("valid date")),
            to: Some(NaiveDate::from_ymd_opt(2026, 1, 3).expect("valid date")),
            limit,
        };

        let result = use_cases
            .search_news(StrategyScope::from(Uuid::new_v4()), query.clone())
            .await
            .expect("search succeeds");
        let criteria = news_repository
            .searches
            .lock()
            .expect("mutex is not poisoned")
            .clone();

        assert_eq!(
            (result, criteria),
            (
                Vec::new(),
                vec![NewsSearchCriteria {
                    keyword: query.keyword.map(|keyword| keyword.trim().to_string()),
                    from: query.from,
                    to: query.to,
                    limit: expected_limit,
                }],
            ),
        );
    }
}
