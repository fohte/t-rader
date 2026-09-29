use chrono::Utc;
use uuid::Uuid;

use super::error::RssFeedUseCaseError;
use super::repository::SharedRssFeedRepository;
use super::types::{CreateRssFeedCommand, NewRssFeed, RssFeed, UpdateRssFeedPatch};
use super::url_validator::SharedRssFeedUrlValidator;
use crate::unit_of_work::SharedUnitOfWork;

const SOURCE_PATTERN_DESC: &str = "^[a-z0-9_-]+$";

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
        let mut feed = self
            .repository
            .find_by_id_in_transaction(&transaction, id)
            .await?
            .ok_or(RssFeedUseCaseError::NotFound(id))?;
        if let Some(display_name) = patch.display_name {
            feed.display_name = validate_display_name(&display_name)?;
        }
        if let Some(url) = patch.url {
            feed.url = validate_url(&url, self.url_validator.as_ref())?;
        }
        if let Some(enabled) = patch.enabled {
            feed.enabled = enabled;
        }
        feed.updated_at = Utc::now().fixed_offset();
        let updated = self.repository.update(&transaction, feed).await?;
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

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{RssFeedUseCaseError, validate_display_name, validate_source, validate_url};
    use crate::rss_feed::RssFeedUrlValidator;

    struct TestUrlValidator;

    impl RssFeedUrlValidator for TestUrlValidator {
        fn is_valid_http_url(&self, url: &str) -> bool {
            url.starts_with("http://") || url.starts_with("https://")
        }
    }

    #[rstest]
    #[case::lowercase_slug("feed-source")]
    #[case::underscore("feed_source")]
    #[case::digits("feed-225")]
    fn source_accepts_ascii_slug(#[case] source: &str) {
        assert_eq!(validate_source(source).unwrap(), source);
    }

    #[rstest]
    #[case::uppercase("Feed")]
    #[case::space("feed source")]
    #[case::non_ascii("配信元")]
    #[case::empty("")]
    fn source_rejects_invalid_slug(#[case] source: &str) {
        assert!(matches!(
            validate_source(source),
            Err(RssFeedUseCaseError::Validation(_))
        ));
    }

    #[rstest]
    #[case::http("http://example.invalid/feed.xml")]
    #[case::https("https://feeds.example.invalid/rss")]
    fn url_accepts_http_schemes(#[case] url: &str) {
        assert_eq!(validate_url(url, &TestUrlValidator).unwrap(), url);
    }

    #[rstest]
    #[case::missing_scheme("example.invalid/feed")]
    #[case::ftp("ftp://example.invalid/feed")]
    #[case::invalid("not-a-url")]
    fn url_rejects_invalid_values(#[case] url: &str) {
        assert!(matches!(
            validate_url(url, &TestUrlValidator),
            Err(RssFeedUseCaseError::Validation(_))
        ));
    }

    #[rstest]
    #[case::trimmed(" Sample publication ", "Sample publication")]
    #[case::ordinary("Sample publication", "Sample publication")]
    fn display_name_trims_surrounding_whitespace(#[case] name: &str, #[case] expected: &str) {
        assert_eq!(validate_display_name(name).unwrap(), expected);
    }

    #[rstest]
    #[case::empty("")]
    #[case::whitespace("   ")]
    fn display_name_rejects_empty_values(#[case] name: &str) {
        assert!(matches!(
            validate_display_name(name),
            Err(RssFeedUseCaseError::Validation(_))
        ));
    }
}
