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

    /// ニュースは全戦略で共有するため、検索結果には戦略スコープの絞り込みを適用しない。
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
                // news_item.source には既存の表示名が入るため、slug ではなく display_name を渡す。
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

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;

    use chrono::{DateTime, Utc};
    use rstest::rstest;
    use uuid::Uuid;

    use crate::news::FakeNewsItemRepository;
    use crate::news::repository::NewsSearchCriteria;
    use crate::news::types::{AggregationStats, SearchNewsQuery};
    use crate::news_aggregator::{FakeNewsAggregator, NewsItem};
    use crate::rss_feed::{FakeRssFeedRepository, RssFeed};
    use crate::strategy_scope::StrategyScope;

    use super::NewsUseCases;

    fn feed(source: &str, display_name: &str, enabled: bool) -> RssFeed {
        let timestamp = DateTime::<Utc>::UNIX_EPOCH.fixed_offset();
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
        let news_repository = Arc::new(FakeNewsItemRepository::new());
        let use_cases = NewsUseCases::new(
            Arc::new(crate::unit_of_work::FakeUnitOfWork::new()),
            Arc::new(FakeRssFeedRepository::new(feeds)),
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
            published_at: DateTime::<Utc>::UNIX_EPOCH,
        };
        *aggregator.items.lock().await = vec![article.clone()];

        let stats = use_cases
            .run_aggregation_cycle(&aggregator)
            .await
            .expect("aggregation cycle succeeds");
        let requested_feeds = aggregator.requested_feeds.lock().await.clone();
        let upserts = news_repository.upserts.lock().await.clone();

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
        let upsert_count = news_repository.upserts.lock().await.len();

        assert_eq!(
            (result, upsert_count),
            (Err("network error: sample failure".into()), 0),
        );
    }

    #[tokio::test]
    async fn aggregation_cycle_returns_zero_when_no_items_are_fetched() {
        let (use_cases, news_repository) = use_cases(Vec::new());
        let aggregator = FakeNewsAggregator::new();

        let result = use_cases
            .run_aggregation_cycle(&aggregator)
            .await
            .map_err(|error| error.to_string());
        let requested_feeds = aggregator.requested_feeds.lock().await.clone();
        let upserts = news_repository.upserts.lock().await.clone();

        assert_eq!(
            (result, requested_feeds, upserts),
            (
                Ok(AggregationStats { fetched: 0 }),
                vec![Vec::new()],
                Vec::new(),
            ),
        );
    }

    #[tokio::test]
    async fn aggregation_cycle_propagates_duplicate_url_batch_error() {
        let (use_cases, news_repository) = use_cases(Vec::new());
        let aggregator = FakeNewsAggregator::new();
        let item = NewsItem {
            source: "Sample publication".into(),
            url: "https://example.invalid/article".into(),
            title: "Sample headline".into(),
            body_snippet: None,
            published_at: DateTime::<Utc>::UNIX_EPOCH,
        };
        *aggregator.items.lock().await = vec![item.clone(), item];

        let result = use_cases
            .run_aggregation_cycle(&aggregator)
            .await
            .map_err(|error| error.to_string());
        let upserts = news_repository.upserts.lock().await.clone();

        assert_eq!(
            (result, upserts),
            (
                Err("news item batch contains duplicate URLs".into()),
                Vec::new(),
            ),
        );
    }

    async fn run_search(
        use_cases: &NewsUseCases,
        news_repository: &FakeNewsItemRepository,
        query: SearchNewsQuery,
    ) -> (
        Result<Vec<crate::news::NewsArticle>, String>,
        Vec<NewsSearchCriteria>,
    ) {
        let result = use_cases
            .search_news(StrategyScope::from(Uuid::nil()), query)
            .await
            .map_err(|error| error.to_string());
        let criteria = news_repository.searches.lock().await.clone();
        (result, criteria)
    }

    #[rstest]
    #[case::missing(None, None)]
    #[case::empty(Some(""), None)]
    #[case::whitespace(Some("   "), None)]
    #[case::trimmed(Some("  Sample query  "), Some("Sample query"))]
    #[tokio::test]
    async fn search_news_trims_keyword_and_omits_blank_values(
        #[case] keyword: Option<&str>,
        #[case] expected_keyword: Option<&str>,
    ) {
        let (use_cases, news_repository) = use_cases(Vec::new());
        let query = SearchNewsQuery {
            keyword: keyword.map(str::to_string),
            from: None,
            to: None,
            limit: Some(13),
        };
        let search_result = run_search(&use_cases, &news_repository, query).await;

        assert_eq!(
            search_result,
            (
                Ok(Vec::new()),
                vec![NewsSearchCriteria {
                    keyword: expected_keyword.map(str::to_string),
                    from: None,
                    to: None,
                    limit: 13,
                }],
            ),
        );
    }

    #[rstest]
    #[case::default(None, 50)]
    #[case::minimum(Some(0), 1)]
    #[case::maximum(Some(500), 200)]
    #[tokio::test]
    async fn search_news_clamps_limit(#[case] limit: Option<u32>, #[case] expected_limit: u64) {
        let (use_cases, news_repository) = use_cases(Vec::new());
        let search_result = run_search(
            &use_cases,
            &news_repository,
            SearchNewsQuery {
                keyword: None,
                from: None,
                to: None,
                limit,
            },
        )
        .await;

        assert_eq!(
            search_result,
            (
                Ok(Vec::new()),
                vec![NewsSearchCriteria {
                    keyword: None,
                    from: None,
                    to: None,
                    limit: expected_limit,
                }],
            ),
        );
    }
}
