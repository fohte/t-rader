use crate::news_aggregator::{NewsAggregator, NewsFeed};
use crate::rss_feed::{ContentSource, SharedRssFeedRepository};
use crate::strategy_scope::StrategyScope;
use crate::unit_of_work::SharedUnitOfWork;

use super::error::NewsUseCaseError;
use super::repository::{FetchedNewsItemContent, NewsSearchCriteria, SharedNewsItemRepository};
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
                content_source: feed.content_source,
            })
            .collect::<Vec<_>>();
        let fetched = aggregator.fetch_news(&feeds).await?;
        if fetched.is_empty() {
            return Ok(AggregationStats { fetched: 0 });
        }
        let transaction = self.unit_of_work.begin().await?;
        let result = self.news_repository.upsert(&transaction, &fetched).await?;
        let pending_ids = result
            .iter()
            .filter(|item| item.inserted && item.item.content_source == ContentSource::Crawl)
            .map(|item| item.id)
            .collect::<Vec<_>>();
        if !pending_ids.is_empty() {
            self.news_repository
                .create_pending_contents(&transaction, &pending_ids)
                .await?;
        }
        let fetched_contents = result
            .iter()
            .filter_map(|item| {
                (item.item.content_source == ContentSource::Feed)
                    .then(|| item.item.content.as_ref().map(|body| (item.id, body)))
                    .flatten()
            })
            .map(|(news_item_id, body)| FetchedNewsItemContent {
                news_item_id,
                body: body.clone(),
            })
            .collect::<Vec<_>>();
        if !fetched_contents.is_empty() {
            self.news_repository
                .upsert_fetched_contents(&transaction, &fetched_contents)
                .await?;
        }
        self.unit_of_work.commit(transaction).await?;
        Ok(AggregationStats {
            fetched: result.len(),
        })
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;

    use chrono::{DateTime, Utc};
    use rstest::rstest;
    use uuid::Uuid;

    use crate::news::repository::NewsSearchCriteria;
    use crate::news::types::{AggregationStats, NewsArticle, SearchNewsQuery};
    use crate::news::{FakeNewsItemRepository, NewsItemContentStatus};
    use crate::news_aggregator::{FakeNewsAggregator, NewsItem};
    use crate::rss_feed::{ContentSource, FakeRssFeedRepository, RssFeed};
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
            content_source: ContentSource::None,
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

    fn normalize_article(mut article: NewsArticle) -> NewsArticle {
        article.id = Uuid::nil();
        article
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
            content_source: ContentSource::None,
            content: None,
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
                        content_source: ContentSource::None,
                    },
                    crate::news_aggregator::NewsFeed {
                        source: "Zulu publication".into(),
                        url: "https://example.invalid/feed-zulu".into(),
                        content_source: ContentSource::None,
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
    async fn aggregation_cycle_creates_content_rows_for_new_items_and_updates_feed_bodies() {
        let mut crawl_feed = feed("crawl-source", "Crawl source", true);
        crawl_feed.content_source = ContentSource::Crawl;
        let mut feed_content = feed("feed-source", "Feed source", true);
        feed_content.content_source = ContentSource::Feed;
        let mut empty_feed_content = feed("empty-source", "Empty source", true);
        empty_feed_content.content_source = ContentSource::Feed;
        let none_feed = feed("none-source", "No content source", true);
        let (use_cases, repository) = use_cases(vec![
            crawl_feed,
            feed_content,
            empty_feed_content,
            none_feed,
        ]);
        let aggregator = FakeNewsAggregator::new();
        let items = vec![
            NewsItem {
                source: "Crawl source".into(),
                url: "https://example.invalid/crawl-article".into(),
                title: "Crawl headline".into(),
                body_snippet: None,
                content_source: ContentSource::Crawl,
                content: None,
                published_at: DateTime::<Utc>::UNIX_EPOCH,
            },
            NewsItem {
                source: "Feed source".into(),
                url: "https://example.invalid/feed-article".into(),
                title: "Feed headline".into(),
                body_snippet: None,
                content_source: ContentSource::Feed,
                content: Some("Original body".into()),
                published_at: DateTime::<Utc>::UNIX_EPOCH,
            },
            NewsItem {
                source: "Feed source".into(),
                url: "https://example.invalid/feed-without-body".into(),
                title: "Empty feed headline".into(),
                body_snippet: None,
                content_source: ContentSource::Feed,
                content: None,
                published_at: DateTime::<Utc>::UNIX_EPOCH,
            },
            NewsItem {
                source: "No content source".into(),
                url: "https://example.invalid/none-article".into(),
                title: "No content headline".into(),
                body_snippet: None,
                content_source: ContentSource::None,
                content: Some("Ignored body".into()),
                published_at: DateTime::<Utc>::UNIX_EPOCH,
            },
        ];
        *aggregator.items.lock().await = items.clone();

        let first = use_cases
            .run_aggregation_cycle(&aggregator)
            .await
            .expect("first aggregation cycle succeeds");
        let mut updated_items = items;
        updated_items[1].content = Some("Updated body".into());
        *aggregator.items.lock().await = updated_items;
        let second = use_cases
            .run_aggregation_cycle(&aggregator)
            .await
            .expect("second aggregation cycle succeeds");

        let articles = repository.articles.lock().await.clone();
        let content_rows = repository.content_rows.lock().await.clone();
        let requested_feeds = aggregator.requested_feeds.lock().await.clone();
        let mut content_snapshot = articles
            .iter()
            .filter_map(|article| {
                content_rows.get(&article.id).map(|content| {
                    (
                        article.url.clone(),
                        content.status,
                        content.body.clone(),
                        content.error.clone(),
                    )
                })
            })
            .collect::<Vec<_>>();
        content_snapshot.sort_by(|left, right| left.0.cmp(&right.0));

        assert_eq!(
            (first, second, requested_feeds, content_snapshot),
            (
                AggregationStats { fetched: 4 },
                AggregationStats { fetched: 4 },
                vec![
                    vec![
                        crate::news_aggregator::NewsFeed {
                            source: "Crawl source".into(),
                            url: "https://example.invalid/crawl-source".into(),
                            content_source: ContentSource::Crawl,
                        },
                        crate::news_aggregator::NewsFeed {
                            source: "Empty source".into(),
                            url: "https://example.invalid/empty-source".into(),
                            content_source: ContentSource::Feed,
                        },
                        crate::news_aggregator::NewsFeed {
                            source: "Feed source".into(),
                            url: "https://example.invalid/feed-source".into(),
                            content_source: ContentSource::Feed,
                        },
                        crate::news_aggregator::NewsFeed {
                            source: "No content source".into(),
                            url: "https://example.invalid/none-source".into(),
                            content_source: ContentSource::None,
                        },
                    ],
                    vec![
                        crate::news_aggregator::NewsFeed {
                            source: "Crawl source".into(),
                            url: "https://example.invalid/crawl-source".into(),
                            content_source: ContentSource::Crawl,
                        },
                        crate::news_aggregator::NewsFeed {
                            source: "Empty source".into(),
                            url: "https://example.invalid/empty-source".into(),
                            content_source: ContentSource::Feed,
                        },
                        crate::news_aggregator::NewsFeed {
                            source: "Feed source".into(),
                            url: "https://example.invalid/feed-source".into(),
                            content_source: ContentSource::Feed,
                        },
                        crate::news_aggregator::NewsFeed {
                            source: "No content source".into(),
                            url: "https://example.invalid/none-source".into(),
                            content_source: ContentSource::None,
                        },
                    ],
                ],
                vec![
                    (
                        "https://example.invalid/crawl-article".into(),
                        NewsItemContentStatus::Pending,
                        None,
                        None,
                    ),
                    (
                        "https://example.invalid/feed-article".into(),
                        NewsItemContentStatus::Fetched,
                        Some("Updated body".into()),
                        None,
                    ),
                ],
            ),
        );
    }

    #[tokio::test]
    async fn aggregation_cycle_does_not_backfill_when_content_source_changes() {
        let feed_repository = Arc::new(FakeRssFeedRepository::new(vec![feed(
            "sample-source",
            "Sample source",
            true,
        )]));
        let news_repository = Arc::new(FakeNewsItemRepository::new());
        let use_cases = NewsUseCases::new(
            Arc::new(crate::unit_of_work::FakeUnitOfWork::new()),
            feed_repository.clone(),
            news_repository.clone(),
        );
        let aggregator = FakeNewsAggregator::new();
        let mut item = NewsItem {
            source: "Sample source".into(),
            url: "https://example.invalid/source-change".into(),
            title: "Sample headline".into(),
            body_snippet: None,
            content_source: ContentSource::None,
            content: None,
            published_at: DateTime::<Utc>::UNIX_EPOCH,
        };
        *aggregator.items.lock().await = vec![item.clone()];

        let first = use_cases
            .run_aggregation_cycle(&aggregator)
            .await
            .expect("first aggregation cycle succeeds");
        feed_repository.feeds.lock().await[0].content_source = ContentSource::Crawl;
        item.content_source = ContentSource::Crawl;
        *aggregator.items.lock().await = vec![item];
        let second = use_cases
            .run_aggregation_cycle(&aggregator)
            .await
            .expect("second aggregation cycle succeeds");
        let content_rows = news_repository.content_rows.lock().await.clone();
        let requested_feeds = aggregator.requested_feeds.lock().await.clone();

        assert_eq!(
            (first, second, content_rows.len(), requested_feeds),
            (
                AggregationStats { fetched: 1 },
                AggregationStats { fetched: 1 },
                0,
                vec![
                    vec![crate::news_aggregator::NewsFeed {
                        source: "Sample source".into(),
                        url: "https://example.invalid/sample-source".into(),
                        content_source: ContentSource::None,
                    }],
                    vec![crate::news_aggregator::NewsFeed {
                        source: "Sample source".into(),
                        url: "https://example.invalid/sample-source".into(),
                        content_source: ContentSource::Crawl,
                    }],
                ],
            ),
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
    async fn aggregation_cycle_propagates_repository_error() {
        let (use_cases, news_repository) = use_cases(Vec::new());
        let aggregator = FakeNewsAggregator::new();
        let item = NewsItem {
            source: "Sample publication".into(),
            url: "https://example.invalid/article".into(),
            title: "Sample headline".into(),
            body_snippet: None,
            content_source: ContentSource::None,
            content: None,
            published_at: DateTime::<Utc>::UNIX_EPOCH,
        };
        *aggregator.items.lock().await = vec![item];
        *news_repository.upsert_error.lock().await = Some("sample failure".into());

        let result = use_cases
            .run_aggregation_cycle(&aggregator)
            .await
            .map_err(|error| error.to_string());
        let upserts = news_repository.upserts.lock().await.clone();

        assert_eq!(
            (result, upserts),
            (Err("sample failure".into()), Vec::new()),
        );
    }

    #[tokio::test]
    async fn search_news_returns_articles_saved_by_aggregation_cycle() {
        let (use_cases, _) = use_cases(vec![feed("feed-alpha", "Alpha publication", true)]);
        let aggregator = FakeNewsAggregator::new();
        let item = NewsItem {
            source: "Alpha publication".into(),
            url: "https://example.invalid/article".into(),
            title: "Quarterly headline".into(),
            body_snippet: Some("Quarterly summary".into()),
            content_source: ContentSource::None,
            content: None,
            published_at: DateTime::<Utc>::UNIX_EPOCH,
        };
        *aggregator.items.lock().await = vec![item];
        use_cases
            .run_aggregation_cycle(&aggregator)
            .await
            .expect("aggregation cycle succeeds");

        let result = use_cases
            .search_news(
                StrategyScope::from(Uuid::nil()),
                SearchNewsQuery {
                    keyword: Some("headline".into()),
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .map(|articles| {
                articles
                    .into_iter()
                    .map(normalize_article)
                    .collect::<Vec<_>>()
            })
            .map_err(|error| error.to_string());

        assert_eq!(
            result,
            Ok(vec![NewsArticle {
                id: Uuid::nil(),
                source: "Alpha publication".into(),
                url: "https://example.invalid/article".into(),
                title: "Quarterly headline".into(),
                body_snippet: Some("Quarterly summary".into()),
                published_at: DateTime::<Utc>::UNIX_EPOCH.fixed_offset(),
            }]),
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
