use std::time::{Duration, Instant};

use crate::news::NewsItemContentStatus;
use chrono::{Duration as ChronoDuration, Utc};
use core::fmt::Display;
use tracing::info;

use super::{
    NewsContentFetchOutcome, NewsContentFetchStats, NewsContentFetcher, NewsContentRunError,
    SharedNewsContentRepository,
};

const MAX_PENDING_ITEMS: u64 = 20;
const MAX_RUN_DURATION: Duration = Duration::from_secs(8 * 60);
const MAX_CONTENT_CHARS: usize = 100_000;
const EXPIRE_AFTER: ChronoDuration = ChronoDuration::hours(48);

#[derive(Clone)]
pub struct NewsContentUseCases {
    repository: SharedNewsContentRepository,
}

impl NewsContentUseCases {
    pub fn new(repository: SharedNewsContentRepository) -> Self {
        Self { repository }
    }

    pub async fn fetch_pending(
        &self,
        fetcher: &dyn NewsContentFetcher,
    ) -> Result<NewsContentFetchStats, NewsContentRunError> {
        self.fetch_pending_with_limits(fetcher, MAX_PENDING_ITEMS, MAX_RUN_DURATION)
            .await
    }

    async fn fetch_pending_with_limits(
        &self,
        fetcher: &dyn NewsContentFetcher,
        max_pending_items: u64,
        max_run_duration: Duration,
    ) -> Result<NewsContentFetchStats, NewsContentRunError> {
        let started_at = Instant::now();
        let mut stats = NewsContentFetchStats::default();
        let cutoff = (Utc::now() - EXPIRE_AFTER).fixed_offset();
        stats.expired = self
            .repository
            .expire_pending_before(cutoff)
            .await
            .map_err(|error| run_error(stats.clone(), error))?;
        let pending = self
            .repository
            .list_pending(max_pending_items)
            .await
            .map_err(|error| run_error(stats.clone(), error))?;

        for item in pending {
            if started_at.elapsed() >= max_run_duration {
                stats.interrupted_reason = Some("time_budget_exhausted".to_owned());
                break;
            }

            if is_unsupported_type(&item.url) {
                self.repository
                    .mark_failed(item.news_item_id, "unsupported_type".to_owned())
                    .await
                    .map_err(|error| run_error(stats.clone(), error))?;
                stats.failed += 1;
                log_status(&item.url, NewsItemContentStatus::Failed);
                continue;
            }

            let outcome = fetcher.fetch(&item.url).await.map_err(|error| {
                run_error(
                    stats.clone(),
                    format_args!("news content fetch failed: {error}"),
                )
            })?;
            match outcome {
                NewsContentFetchOutcome::Fetched(body) if body.trim().is_empty() => {
                    self.repository
                        .mark_failed(item.news_item_id, "empty".to_owned())
                        .await
                        .map_err(|error| run_error(stats.clone(), error))?;
                    stats.failed += 1;
                    log_status(&item.url, NewsItemContentStatus::Failed);
                }
                NewsContentFetchOutcome::Fetched(body) => {
                    self.repository
                        .mark_fetched(item.news_item_id, truncate_body(&body))
                        .await
                        .map_err(|error| run_error(stats.clone(), error))?;
                    stats.fetched += 1;
                    log_status(&item.url, NewsItemContentStatus::Fetched);
                }
                NewsContentFetchOutcome::Failed(error) => {
                    self.repository
                        .mark_failed(item.news_item_id, error)
                        .await
                        .map_err(|error| run_error(stats.clone(), error))?;
                    stats.failed += 1;
                    log_status(&item.url, NewsItemContentStatus::Failed);
                }
                NewsContentFetchOutcome::Retry => {
                    stats.retried += 1;
                    log_status(&item.url, NewsItemContentStatus::Pending);
                }
                NewsContentFetchOutcome::Abort(reason) => {
                    stats.retried += 1;
                    stats.interrupted_reason = Some(reason.as_str().to_owned());
                    log_status(&item.url, NewsItemContentStatus::Pending);
                    return Err(NewsContentRunError {
                        stats,
                        message: reason.as_str().to_owned(),
                    });
                }
            }
        }

        Ok(stats)
    }
}

fn run_error(stats: NewsContentFetchStats, error: impl Display) -> NewsContentRunError {
    NewsContentRunError {
        stats,
        message: error.to_string(),
    }
}

fn is_unsupported_type(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let path = path.to_ascii_lowercase();
    [".xlsx", ".xls", ".csv"]
        .iter()
        .any(|extension| path.ends_with(extension))
}

fn truncate_body(body: &str) -> String {
    let mut characters = body.chars();
    let Some(_) = characters.nth(MAX_CONTENT_CHARS) else {
        return body.to_owned();
    };

    const TRUNCATION_NOTE: &str = "[記事本文は 100,000 字で切り詰めました]";
    let content_limit = MAX_CONTENT_CHARS - TRUNCATION_NOTE.chars().count() - 1;
    let mut truncated = body.chars().take(content_limit).collect::<String>();
    truncated.push('\n');
    truncated.push_str(TRUNCATION_NOTE);
    truncated
}

fn log_status(url: &str, status: NewsItemContentStatus) {
    info!(
        url,
        status = status.as_str(),
        "news content fetch item completed"
    );
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use chrono::{DateTime, Duration as ChronoDuration, FixedOffset, Utc};
    use rstest::{fixture, rstest};
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use crate::{
        news::NewsItemContentStatus,
        news_content::{
            NewsContentFetchError, NewsContentFetchOutcome, NewsContentFetcher,
            NewsContentInterruption, NewsContentRepository, NewsContentUseCases,
            PendingNewsContent,
        },
        persistence::PersistenceError,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct FakeRow {
        pending: PendingNewsContent,
        created_at: DateTime<FixedOffset>,
        status: NewsItemContentStatus,
        body: Option<String>,
        error: Option<String>,
    }

    #[derive(Default)]
    struct FakeRepository {
        rows: Mutex<Vec<FakeRow>>,
    }

    impl FakeRepository {
        fn with_rows(rows: Vec<FakeRow>) -> Self {
            Self {
                rows: Mutex::new(rows),
            }
        }
    }

    #[async_trait]
    impl NewsContentRepository for FakeRepository {
        async fn expire_pending_before(
            &self,
            cutoff: DateTime<FixedOffset>,
        ) -> Result<u64, PersistenceError> {
            let mut rows = self.rows.lock().await;
            let mut expired = 0;
            for row in rows.iter_mut() {
                if row.status == NewsItemContentStatus::Pending && row.created_at < cutoff {
                    row.status = NewsItemContentStatus::Failed;
                    row.error = Some("expired".to_owned());
                    expired += 1;
                }
            }
            Ok(expired)
        }

        async fn list_pending(
            &self,
            limit: u64,
        ) -> Result<Vec<PendingNewsContent>, PersistenceError> {
            let mut rows = self.rows.lock().await;
            rows.sort_by_key(|row| row.created_at);
            Ok(rows
                .iter()
                .filter(|row| row.status == NewsItemContentStatus::Pending)
                .take(limit as usize)
                .map(|row| row.pending.clone())
                .collect())
        }

        async fn mark_fetched(
            &self,
            news_item_id: Uuid,
            body: String,
        ) -> Result<(), PersistenceError> {
            update_row(&self.rows, news_item_id, |row| {
                row.status = NewsItemContentStatus::Fetched;
                row.body = Some(body);
                row.error = None;
            })
            .await
        }

        async fn mark_failed(
            &self,
            news_item_id: Uuid,
            error: String,
        ) -> Result<(), PersistenceError> {
            update_row(&self.rows, news_item_id, |row| {
                row.status = NewsItemContentStatus::Failed;
                row.body = None;
                row.error = Some(error);
            })
            .await
        }
    }

    async fn update_row(
        rows: &Mutex<Vec<FakeRow>>,
        id: Uuid,
        update: impl FnOnce(&mut FakeRow),
    ) -> Result<(), PersistenceError> {
        let mut rows = rows.lock().await;
        let Some(row) = rows.iter_mut().find(|row| {
            row.pending.news_item_id == id && row.status == NewsItemContentStatus::Pending
        }) else {
            return Err(PersistenceError::RecordNotUpdated(
                "pending news content was not found".to_owned(),
            ));
        };
        update(row);
        Ok(())
    }

    #[derive(Default)]
    struct FakeFetcher {
        outcomes: Mutex<Vec<NewsContentFetchOutcome>>,
        requested_urls: Mutex<Vec<String>>,
    }

    impl FakeFetcher {
        fn with_outcomes(outcomes: Vec<NewsContentFetchOutcome>) -> Self {
            Self {
                outcomes: Mutex::new(outcomes),
                requested_urls: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl NewsContentFetcher for FakeFetcher {
        async fn fetch(&self, url: &str) -> Result<NewsContentFetchOutcome, NewsContentFetchError> {
            self.requested_urls.lock().await.push(url.to_owned());
            let mut outcomes = self.outcomes.lock().await;
            Ok(if outcomes.is_empty() {
                NewsContentFetchOutcome::Retry
            } else {
                outcomes.remove(0)
            })
        }
    }

    #[fixture]
    fn pending_row() -> FakeRow {
        FakeRow {
            pending: PendingNewsContent {
                news_item_id: Uuid::from_u128(1),
                url: "https://example.invalid/article".to_owned(),
            },
            created_at: Utc::now().fixed_offset(),
            status: NewsItemContentStatus::Pending,
            body: None,
            error: None,
        }
    }

    #[fixture]
    fn use_case(pending_row: FakeRow) -> (NewsContentUseCases, Arc<FakeRepository>) {
        let repository = Arc::new(FakeRepository::with_rows(vec![pending_row]));
        (NewsContentUseCases::new(repository.clone()), repository)
    }

    fn normalize_rows(mut rows: Vec<FakeRow>) -> Vec<FakeRow> {
        for row in &mut rows {
            row.created_at = DateTime::<Utc>::UNIX_EPOCH.fixed_offset();
        }
        rows
    }

    #[rstest]
    #[case::fetched(NewsContentFetchOutcome::Fetched("full article".to_owned()), NewsItemContentStatus::Fetched, Some("full article".to_owned()), None)]
    #[case::failed(NewsContentFetchOutcome::Failed("http_404".to_owned()), NewsItemContentStatus::Failed, None, Some("http_404".to_owned()))]
    #[case::retry(
        NewsContentFetchOutcome::Retry,
        NewsItemContentStatus::Pending,
        None,
        None
    )]
    #[tokio::test]
    async fn fetch_pending_updates_state_from_fetch_outcome(
        use_case: (NewsContentUseCases, Arc<FakeRepository>),
        #[case] outcome: NewsContentFetchOutcome,
        #[case] expected_status: NewsItemContentStatus,
        #[case] expected_body: Option<String>,
        #[case] expected_error: Option<String>,
    ) {
        let (use_cases, repository) = use_case;
        let fetcher = FakeFetcher::with_outcomes(vec![outcome]);
        let result = use_cases
            .fetch_pending(&fetcher)
            .await
            .map_err(|error| error.message);
        let rows = normalize_rows(repository.rows.lock().await.clone());
        let requested_urls = fetcher.requested_urls.lock().await.clone();

        assert_eq!(
            (result, rows, requested_urls),
            (
                Ok(crate::news_content::NewsContentFetchStats {
                    expired: 0,
                    fetched: u64::from(expected_status == NewsItemContentStatus::Fetched),
                    failed: u64::from(expected_status == NewsItemContentStatus::Failed),
                    retried: u64::from(expected_status == NewsItemContentStatus::Pending),
                    interrupted_reason: None,
                }),
                vec![FakeRow {
                    pending: PendingNewsContent {
                        news_item_id: Uuid::from_u128(1),
                        url: "https://example.invalid/article".to_owned(),
                    },
                    created_at: DateTime::<Utc>::UNIX_EPOCH.fixed_offset(),
                    status: expected_status,
                    body: expected_body,
                    error: expected_error,
                }],
                vec!["https://example.invalid/article".to_owned()],
            ),
        );
    }

    #[rstest]
    #[case::xlsx("https://example.invalid/report.xlsx?download=1")]
    #[case::xls("https://example.invalid/report.xls#sheet")]
    #[case::csv("https://example.invalid/report.csv")]
    #[tokio::test]
    async fn fetch_pending_marks_spreadsheets_unsupported(pending_row: FakeRow, #[case] url: &str) {
        let mut row = pending_row;
        row.created_at = Utc::now().fixed_offset();
        row.pending.url = url.to_owned();
        let repository = Arc::new(FakeRepository::with_rows(vec![row]));
        let use_cases = NewsContentUseCases::new(repository.clone());
        let fetcher = FakeFetcher::default();
        let result = use_cases.fetch_pending(&fetcher).await;
        let rows = normalize_rows(repository.rows.lock().await.clone());
        let requested_urls = fetcher.requested_urls.lock().await.clone();

        assert_eq!(
            (result, rows, requested_urls),
            (
                Ok(crate::news_content::NewsContentFetchStats {
                    expired: 0,
                    fetched: 0,
                    failed: 1,
                    retried: 0,
                    interrupted_reason: None,
                }),
                vec![FakeRow {
                    pending: PendingNewsContent {
                        news_item_id: Uuid::from_u128(1),
                        url: url.to_owned(),
                    },
                    created_at: DateTime::<Utc>::UNIX_EPOCH.fixed_offset(),
                    status: NewsItemContentStatus::Failed,
                    body: None,
                    error: Some("unsupported_type".to_owned()),
                }],
                Vec::new(),
            ),
        );
    }

    #[rstest]
    #[case::credits_exhausted(NewsContentInterruption::FirecrawlCreditsExhausted, "firecrawl_402")]
    #[case::rate_limited(NewsContentInterruption::FirecrawlRateLimited, "firecrawl_429")]
    #[tokio::test]
    async fn fetch_pending_stops_after_an_abort(
        use_case: (NewsContentUseCases, Arc<FakeRepository>),
        pending_row: FakeRow,
        #[case] reason: NewsContentInterruption,
        #[case] expected_reason: &str,
    ) {
        let (use_cases, repository) = use_case;
        let mut next_row = pending_row;
        next_row.pending.news_item_id = Uuid::from_u128(2);
        next_row.pending.url = "https://example.invalid/next".to_owned();
        next_row.created_at = Utc::now().fixed_offset() + ChronoDuration::seconds(1);
        repository.rows.lock().await.push(next_row);
        let fetcher = FakeFetcher::with_outcomes(vec![
            NewsContentFetchOutcome::Abort(reason),
            NewsContentFetchOutcome::Fetched("must not be fetched".to_owned()),
        ]);
        let result = use_cases
            .fetch_pending(&fetcher)
            .await
            .map(|stats| (stats, None::<String>))
            .map_err(|error| (error.stats, Some(error.message)));
        let rows = normalize_rows(repository.rows.lock().await.clone());
        let requested_urls = fetcher.requested_urls.lock().await.clone();

        assert_eq!(
            (result, rows, requested_urls),
            (
                Err((
                    crate::news_content::NewsContentFetchStats {
                        expired: 0,
                        fetched: 0,
                        failed: 0,
                        retried: 1,
                        interrupted_reason: Some(expected_reason.to_owned()),
                    },
                    Some(expected_reason.to_owned()),
                )),
                vec![
                    FakeRow {
                        pending: PendingNewsContent {
                            news_item_id: Uuid::from_u128(1),
                            url: "https://example.invalid/article".to_owned(),
                        },
                        created_at: DateTime::<Utc>::UNIX_EPOCH.fixed_offset(),
                        status: NewsItemContentStatus::Pending,
                        body: None,
                        error: None,
                    },
                    FakeRow {
                        pending: PendingNewsContent {
                            news_item_id: Uuid::from_u128(2),
                            url: "https://example.invalid/next".to_owned(),
                        },
                        created_at: DateTime::<Utc>::UNIX_EPOCH.fixed_offset(),
                        status: NewsItemContentStatus::Pending,
                        body: None,
                        error: None,
                    },
                ],
                vec!["https://example.invalid/article".to_owned()],
            ),
        );
    }

    #[tokio::test]
    async fn fetch_pending_expires_old_rows_before_selecting_pending_items() {
        let old_time = (Utc::now() - ChronoDuration::hours(49)).fixed_offset();
        let recent_time = Utc::now().fixed_offset();
        let rows = vec![
            FakeRow {
                pending: PendingNewsContent {
                    news_item_id: Uuid::from_u128(1),
                    url: "https://example.invalid/old".to_owned(),
                },
                created_at: old_time,
                status: NewsItemContentStatus::Pending,
                body: None,
                error: None,
            },
            FakeRow {
                pending: PendingNewsContent {
                    news_item_id: Uuid::from_u128(2),
                    url: "https://example.invalid/recent".to_owned(),
                },
                created_at: recent_time,
                status: NewsItemContentStatus::Pending,
                body: None,
                error: None,
            },
        ];
        let repository = Arc::new(FakeRepository::with_rows(rows));
        let use_cases = NewsContentUseCases::new(repository.clone());
        let fetcher = FakeFetcher::with_outcomes(vec![NewsContentFetchOutcome::Retry]);
        let result = use_cases.fetch_pending(&fetcher).await;
        let rows = normalize_rows(repository.rows.lock().await.clone());
        let requested_urls = fetcher.requested_urls.lock().await.clone();

        assert_eq!(
            (result, rows, requested_urls),
            (
                Ok(crate::news_content::NewsContentFetchStats {
                    expired: 1,
                    fetched: 0,
                    failed: 0,
                    retried: 1,
                    interrupted_reason: None,
                }),
                vec![
                    FakeRow {
                        pending: PendingNewsContent {
                            news_item_id: Uuid::from_u128(1),
                            url: "https://example.invalid/old".to_owned(),
                        },
                        created_at: DateTime::<Utc>::UNIX_EPOCH.fixed_offset(),
                        status: NewsItemContentStatus::Failed,
                        body: None,
                        error: Some("expired".to_owned()),
                    },
                    FakeRow {
                        pending: PendingNewsContent {
                            news_item_id: Uuid::from_u128(2),
                            url: "https://example.invalid/recent".to_owned(),
                        },
                        created_at: DateTime::<Utc>::UNIX_EPOCH.fixed_offset(),
                        status: NewsItemContentStatus::Pending,
                        body: None,
                        error: None,
                    },
                ],
                vec!["https://example.invalid/recent".to_owned()],
            ),
        );
    }

    #[tokio::test]
    async fn fetch_pending_does_not_start_an_item_after_the_time_budget() {
        let row = FakeRow {
            pending: PendingNewsContent {
                news_item_id: Uuid::from_u128(1),
                url: "https://example.invalid/article".to_owned(),
            },
            created_at: Utc::now().fixed_offset(),
            status: NewsItemContentStatus::Pending,
            body: None,
            error: None,
        };
        let repository = Arc::new(FakeRepository::with_rows(vec![row.clone()]));
        let use_cases = NewsContentUseCases::new(repository.clone());
        let fetcher = FakeFetcher::default();
        let result = use_cases
            .fetch_pending_with_limits(&fetcher, 20, Duration::ZERO)
            .await;
        let rows = repository.rows.lock().await.clone();
        let requested_urls = fetcher.requested_urls.lock().await.clone();

        assert_eq!(
            (result, rows, requested_urls),
            (
                Ok(crate::news_content::NewsContentFetchStats {
                    expired: 0,
                    fetched: 0,
                    failed: 0,
                    retried: 0,
                    interrupted_reason: Some("time_budget_exhausted".to_owned()),
                }),
                vec![row],
                Vec::new(),
            ),
        );
    }

    #[tokio::test]
    async fn fetch_pending_truncates_long_bodies_and_keeps_the_note_within_the_limit() {
        let (use_cases, repository) = use_case(FakeRow {
            pending: PendingNewsContent {
                news_item_id: Uuid::from_u128(1),
                url: "https://example.invalid/article".to_owned(),
            },
            created_at: Utc::now().fixed_offset(),
            status: NewsItemContentStatus::Pending,
            body: None,
            error: None,
        });
        let fetcher = FakeFetcher::with_outcomes(vec![NewsContentFetchOutcome::Fetched(
            "あ".repeat(100_001),
        )]);
        let result = use_cases.fetch_pending(&fetcher).await;
        let rows = repository.rows.lock().await.clone();
        let expected_body = format!(
            "{}\n[記事本文は 100,000 字で切り詰めました]",
            "あ".repeat(99_974),
        );

        assert_eq!(
            (result, rows[0].status, rows[0].body.as_deref(),),
            (
                Ok(crate::news_content::NewsContentFetchStats {
                    expired: 0,
                    fetched: 1,
                    failed: 0,
                    retried: 0,
                    interrupted_reason: None,
                }),
                NewsItemContentStatus::Fetched,
                Some(expected_body.as_str()),
            ),
        );
    }
}
