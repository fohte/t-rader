use chrono::Utc;
use uuid::Uuid;

use super::error::RssFeedUseCaseError;
use super::repository::SharedRssFeedRepository;
use super::types::{CreateRssFeedCommand, NewRssFeed, RssFeed, UpdateRssFeedPatch};
use super::url_validator::SharedRssFeedUrlValidator;
use crate::unit_of_work::SharedUnitOfWork;

/// `source` は機械処理向けの slug、`display_name` は利用者向けの表示名。
const SOURCE_PATTERN_DESC: &str = "^[a-z0-9_-]+$";
const CONTENT_SOURCES: [&str; 3] = ["none", "feed", "crawl"];

#[derive(Clone)]
pub struct RssFeedUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedRssFeedRepository,
    url_validator: SharedRssFeedUrlValidator,
}

impl RssFeedUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedRssFeedRepository,
        url_validator: SharedRssFeedUrlValidator,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            url_validator,
        }
    }

    pub async fn list(&self, enabled_only: bool) -> Result<Vec<RssFeed>, RssFeedUseCaseError> {
        self.repository.list(enabled_only).await.map_err(Into::into)
    }

    pub async fn get(&self, id: Uuid) -> Result<RssFeed, RssFeedUseCaseError> {
        self.repository
            .find_by_id(id)
            .await?
            .ok_or(RssFeedUseCaseError::NotFound(id))
    }

    pub async fn create(
        &self,
        command: CreateRssFeedCommand,
    ) -> Result<RssFeed, RssFeedUseCaseError> {
        let source = validate_source(&command.source)?;
        let display_name = validate_display_name(&command.display_name)?;
        let url = validate_url(&command.url, self.url_validator.as_ref())?;
        let content_source = validate_content_source(&command.content_source)?;
        let transaction = self.unit_of_work.begin().await?;
        let feed = self
            .repository
            .create(
                &transaction,
                NewRssFeed {
                    id: Uuid::new_v4(),
                    source,
                    display_name,
                    url,
                    enabled: command.enabled.unwrap_or(true),
                    content_source,
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(feed)
    }

    pub async fn update(
        &self,
        id: Uuid,
        patch: UpdateRssFeedPatch,
    ) -> Result<RssFeed, RssFeedUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        if self
            .repository
            .find_by_id_in_transaction(&transaction, id)
            .await?
            .is_none()
        {
            return Err(RssFeedUseCaseError::NotFound(id));
        }
        let patch = UpdateRssFeedPatch {
            display_name: patch
                .display_name
                .map(|name| validate_display_name(&name))
                .transpose()?,
            url: patch
                .url
                .map(|url| validate_url(&url, self.url_validator.as_ref()))
                .transpose()?,
            enabled: patch.enabled,
            content_source: patch
                .content_source
                .map(|source| validate_content_source(&source))
                .transpose()?,
        };
        let updated = self
            .repository
            .update(&transaction, id, patch, Utc::now().fixed_offset())
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(updated)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), RssFeedUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        if self.repository.delete(&transaction, id).await? {
            self.unit_of_work.commit(transaction).await?;
            Ok(())
        } else {
            Err(RssFeedUseCaseError::NotFound(id))
        }
    }
}

fn validate_source(source: &str) -> Result<String, RssFeedUseCaseError> {
    let trimmed = source.trim();
    if trimmed.is_empty()
        || !trimmed.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '_'
                || character == '-'
        })
    {
        return Err(RssFeedUseCaseError::Validation(format!(
            "source must match {SOURCE_PATTERN_DESC} (got '{source}')"
        )));
    }
    Ok(trimmed.to_string())
}

fn validate_url(
    url: &str,
    validator: &dyn super::url_validator::RssFeedUrlValidator,
) -> Result<String, RssFeedUseCaseError> {
    let trimmed = url.trim();
    if !validator.is_valid_http_url(trimmed) {
        return Err(RssFeedUseCaseError::Validation(format!(
            "url must be a valid http(s) URL (got '{url}')"
        )));
    }
    Ok(trimmed.to_string())
}

fn validate_display_name(name: &str) -> Result<String, RssFeedUseCaseError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(RssFeedUseCaseError::Validation(
            "display_name must not be empty".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_content_source(source: &str) -> Result<String, RssFeedUseCaseError> {
    if !CONTENT_SOURCES.contains(&source) {
        return Err(RssFeedUseCaseError::Validation(format!(
            "content_source must be one of {} (got '{source}')",
            CONTENT_SOURCES.join(", "),
        )));
    }
    Ok(source.to_string())
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;

    use chrono::{DateTime, Utc};
    use rstest::rstest;
    use uuid::Uuid;

    use super::RssFeedUseCases;
    use crate::rss_feed::{
        CreateRssFeedCommand, FakeRssFeedRepository, RssFeed, RssFeedRepository,
        RssFeedUrlValidator, UpdateRssFeedPatch,
    };
    use crate::unit_of_work::FakeUnitOfWork;

    struct TestUrlValidator;

    impl RssFeedUrlValidator for TestUrlValidator {
        fn is_valid_http_url(&self, url: &str) -> bool {
            url.starts_with("http://") || url.starts_with("https://")
        }
    }

    fn timestamp() -> DateTime<chrono::FixedOffset> {
        DateTime::<Utc>::UNIX_EPOCH.fixed_offset()
    }

    fn normalize(mut feed: RssFeed) -> RssFeed {
        feed.id = Uuid::nil();
        feed.created_at = timestamp();
        feed.updated_at = timestamp();
        feed
    }

    fn feed(source: &str, display_name: &str, url: &str, enabled: bool) -> RssFeed {
        RssFeed {
            id: Uuid::new_v4(),
            source: source.into(),
            display_name: display_name.into(),
            url: url.into(),
            enabled,
            content_source: "none".into(),
            created_at: timestamp(),
            updated_at: timestamp(),
        }
    }

    fn use_cases(feeds: Vec<RssFeed>) -> (RssFeedUseCases, Arc<FakeRssFeedRepository>) {
        let repository = Arc::new(FakeRssFeedRepository::new(feeds));
        let use_cases = RssFeedUseCases::new(
            Arc::new(FakeUnitOfWork::new()),
            repository.clone(),
            Arc::new(TestUrlValidator),
        );
        (use_cases, repository)
    }

    fn create_command(source: &str, display_name: &str, url: &str) -> CreateRssFeedCommand {
        CreateRssFeedCommand {
            source: source.into(),
            display_name: display_name.into(),
            url: url.into(),
            enabled: Some(true),
            content_source: "none".into(),
        }
    }

    fn expected_feed(source: &str, display_name: &str, url: &str) -> RssFeed {
        RssFeed {
            id: Uuid::nil(),
            source: source.into(),
            display_name: display_name.into(),
            url: url.into(),
            enabled: true,
            content_source: "none".into(),
            created_at: timestamp(),
            updated_at: timestamp(),
        }
    }

    async fn create_and_normalize(
        use_cases: &RssFeedUseCases,
        source: &str,
        display_name: &str,
        url: &str,
    ) -> Result<RssFeed, String> {
        use_cases
            .create(create_command(source, display_name, url))
            .await
            .map(normalize)
            .map_err(|error| error.to_string())
    }

    #[rstest]
    #[case::lowercase_slug("feed-source")]
    #[case::underscore("feed_source")]
    #[case::digits("feed-225")]
    #[tokio::test]
    async fn source_accepts_ascii_slug(#[case] source: &str) {
        let (use_cases, _) = use_cases(Vec::new());
        let actual = create_and_normalize(
            &use_cases,
            source,
            "Sample publication",
            "https://example.invalid/feed.xml",
        )
        .await;

        assert_eq!(
            actual,
            Ok(expected_feed(
                source,
                "Sample publication",
                "https://example.invalid/feed.xml",
            )),
        );
    }

    #[rstest]
    #[case::uppercase("Feed", "source must match ^[a-z0-9_-]+$ (got 'Feed')")]
    #[case::space("feed source", "source must match ^[a-z0-9_-]+$ (got 'feed source')")]
    #[case::non_ascii("配信元", "source must match ^[a-z0-9_-]+$ (got '配信元')")]
    #[case::empty("", "source must match ^[a-z0-9_-]+$ (got '')")]
    #[tokio::test]
    async fn source_rejects_invalid_slug(#[case] source: &str, #[case] expected_error: &str) {
        let (use_cases, _) = use_cases(Vec::new());
        let actual = create_and_normalize(
            &use_cases,
            source,
            "Sample publication",
            "https://example.invalid/feed.xml",
        )
        .await;

        assert_eq!(actual, Err(expected_error.to_string()));
    }

    #[rstest]
    #[case::none("none")]
    #[case::feed("feed")]
    #[case::crawl("crawl")]
    #[tokio::test]
    async fn content_source_accepts_supported_values(#[case] content_source: &str) {
        let (use_cases, _) = use_cases(Vec::new());
        let mut command = create_command(
            "feed-alpha",
            "Sample publication",
            "https://example.invalid/feed.xml",
        );
        command.content_source = content_source.into();

        let actual = use_cases
            .create(command)
            .await
            .map(normalize)
            .map_err(|error| error.to_string());
        let mut expected = expected_feed(
            "feed-alpha",
            "Sample publication",
            "https://example.invalid/feed.xml",
        );
        expected.content_source = content_source.into();

        assert_eq!(actual, Ok(expected));
    }

    #[rstest]
    #[case::empty("")]
    #[case::unknown("external")]
    #[tokio::test]
    async fn content_source_rejects_unsupported_values(#[case] content_source: &str) {
        let (use_cases, _) = use_cases(Vec::new());
        let mut command = create_command(
            "feed-alpha",
            "Sample publication",
            "https://example.invalid/feed.xml",
        );
        command.content_source = content_source.into();

        assert_eq!(
            use_cases
                .create(command)
                .await
                .map(|_| ())
                .map_err(|error| error.to_string()),
            Err(format!(
                "content_source must be one of none, feed, crawl (got '{content_source}')"
            )),
        );
    }

    #[rstest]
    #[case::http("http://example.invalid/feed.xml")]
    #[case::https("https://feeds.example.invalid/rss")]
    #[tokio::test]
    async fn url_accepts_http_schemes(#[case] url: &str) {
        let (use_cases, _) = use_cases(Vec::new());
        let actual =
            create_and_normalize(&use_cases, "feed-alpha", "Sample publication", url).await;

        assert_eq!(
            actual,
            Ok(expected_feed("feed-alpha", "Sample publication", url)),
        );
    }

    #[rstest]
    #[case::missing_scheme(
        "example.invalid/feed",
        "url must be a valid http(s) URL (got 'example.invalid/feed')"
    )]
    #[case::ftp(
        "ftp://example.invalid/feed",
        "url must be a valid http(s) URL (got 'ftp://example.invalid/feed')"
    )]
    #[case::invalid("not-a-url", "url must be a valid http(s) URL (got 'not-a-url')")]
    #[tokio::test]
    async fn url_rejects_invalid_values(#[case] url: &str, #[case] expected_error: &str) {
        let (use_cases, _) = use_cases(Vec::new());
        let actual =
            create_and_normalize(&use_cases, "feed-alpha", "Sample publication", url).await;

        assert_eq!(actual, Err(expected_error.to_string()));
    }

    #[rstest]
    #[case::trimmed(" Sample publication ", "Sample publication")]
    #[case::ordinary("Sample publication", "Sample publication")]
    #[tokio::test]
    async fn display_name_trims_surrounding_whitespace(#[case] name: &str, #[case] expected: &str) {
        let (use_cases, _) = use_cases(Vec::new());
        let actual = create_and_normalize(
            &use_cases,
            "feed-alpha",
            name,
            "https://example.invalid/feed.xml",
        )
        .await;

        assert_eq!(
            actual,
            Ok(expected_feed(
                "feed-alpha",
                expected,
                "https://example.invalid/feed.xml",
            )),
        );
    }

    #[rstest]
    #[case::empty("")]
    #[case::whitespace("   ")]
    #[tokio::test]
    async fn display_name_rejects_empty_values(#[case] name: &str) {
        let (use_cases, _) = use_cases(Vec::new());
        let actual = create_and_normalize(
            &use_cases,
            "feed-alpha",
            name,
            "https://example.invalid/feed.xml",
        )
        .await;

        assert_eq!(actual, Err("display_name must not be empty".into()));
    }

    #[tokio::test]
    async fn create_trims_fields_and_defaults_enabled() {
        let (use_cases, repository) = use_cases(Vec::new());
        let created = use_cases
            .create(CreateRssFeedCommand {
                source: " feed-alpha ".into(),
                display_name: " Alpha publication ".into(),
                url: " https://feeds.example.invalid/feed-alpha.xml ".into(),
                enabled: None,
                content_source: "none".into(),
            })
            .await
            .expect("feed creates");
        let listed = repository.list(false).await.expect("feeds list");
        let expected = RssFeed {
            id: Uuid::nil(),
            source: "feed-alpha".into(),
            display_name: "Alpha publication".into(),
            url: "https://feeds.example.invalid/feed-alpha.xml".into(),
            enabled: true,
            content_source: "none".into(),
            created_at: timestamp(),
            updated_at: timestamp(),
        };

        assert_eq!(
            (
                normalize(created),
                listed.into_iter().map(normalize).collect::<Vec<_>>()
            ),
            (expected.clone(), vec![expected]),
        );
    }

    #[tokio::test]
    async fn create_reports_duplicate_source() {
        let existing = feed(
            "feed-alpha",
            "Alpha publication",
            "https://feeds.example.invalid/feed-alpha.xml",
            true,
        );
        let (use_cases, repository) = use_cases(vec![existing]);
        let result = use_cases
            .create(CreateRssFeedCommand {
                source: "feed-alpha".into(),
                display_name: "Another publication".into(),
                url: "https://feeds.example.invalid/another.xml".into(),
                enabled: None,
                content_source: "none".into(),
            })
            .await
            .map(|_| ())
            .map_err(|error| error.to_string());
        let listed = repository.list(false).await.expect("feeds list");

        assert_eq!(
            (
                result,
                listed.into_iter().map(normalize).collect::<Vec<_>>()
            ),
            (
                Err("rss feed with source 'feed-alpha' already exists".into()),
                vec![RssFeed {
                    id: Uuid::nil(),
                    source: "feed-alpha".into(),
                    display_name: "Alpha publication".into(),
                    url: "https://feeds.example.invalid/feed-alpha.xml".into(),
                    enabled: true,
                    content_source: "none".into(),
                    created_at: timestamp(),
                    updated_at: timestamp(),
                }],
            ),
        );
    }

    #[tokio::test]
    async fn update_applies_patch_and_preserves_unpatched_fields() {
        let existing = feed(
            "feed-alpha",
            "Alpha publication",
            "https://feeds.example.invalid/feed-alpha.xml",
            true,
        );
        let id = existing.id;
        let (use_cases, repository) = use_cases(vec![existing]);
        let updated = use_cases
            .update(
                id,
                UpdateRssFeedPatch {
                    display_name: Some("Updated publication".into()),
                    url: None,
                    enabled: None,
                    content_source: Some("crawl".into()),
                },
            )
            .await
            .expect("feed updates");
        let listed = repository.list(false).await.expect("feeds list");
        let transaction_ids = repository.transaction_ids.lock().await.clone();
        let used_one_transaction = matches!(
            transaction_ids.as_slice(),
            [find_transaction, update_transaction] if find_transaction == update_transaction
        );
        let expected = RssFeed {
            id: Uuid::nil(),
            source: "feed-alpha".into(),
            display_name: "Updated publication".into(),
            url: "https://feeds.example.invalid/feed-alpha.xml".into(),
            enabled: true,
            content_source: "crawl".into(),
            created_at: timestamp(),
            updated_at: timestamp(),
        };

        assert_eq!(
            (
                normalize(updated),
                listed.into_iter().map(normalize).collect::<Vec<_>>(),
                used_one_transaction,
            ),
            (expected.clone(), vec![expected], true),
        );
    }

    #[tokio::test]
    async fn delete_removes_feed() {
        let existing = feed(
            "feed-alpha",
            "Alpha publication",
            "https://feeds.example.invalid/feed-alpha.xml",
            true,
        );
        let id = existing.id;
        let (use_cases, repository) = use_cases(vec![existing]);

        let result = use_cases
            .delete(id)
            .await
            .map_err(|error| error.to_string());
        let listed = repository.list(false).await.expect("feeds list");

        assert_eq!((result, listed), (Ok(()), Vec::new()));
    }

    #[tokio::test]
    async fn delete_reports_missing_feed() {
        let (use_cases, repository) = use_cases(Vec::new());
        let id = Uuid::nil();
        let result = use_cases
            .delete(id)
            .await
            .map_err(|error| error.to_string());
        let listed = repository.list(false).await.expect("feeds list");

        assert_eq!(
            (result, listed),
            (Err(format!("rss feed {id} not found")), Vec::new()),
        );
    }
}
