use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use core_application::rss_feed::{
    ContentSource, NewRssFeed, RssFeed, RssFeedRepository, RssFeedRepositoryError,
    UpdateRssFeedPatch,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{NotSet, Set, Unchanged};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::rss_feed;
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresRssFeedRepository {
    db: DatabaseHandle,
}

impl PostgresRssFeedRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl RssFeedRepository for PostgresRssFeedRepository {
    async fn list(&self, enabled_only: bool) -> Result<Vec<RssFeed>, RssFeedRepositoryError> {
        let mut query = rss_feed::Entity::find();
        if enabled_only {
            query = query.filter(rss_feed::Column::Enabled.eq(true));
        }
        let rows = query
            .order_by_asc(rss_feed::Column::DisplayName)
            .all(&self.db)
            .await
            .map_err(repository_error)?;
        rows.into_iter().map(to_application).collect()
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<RssFeed>, RssFeedRepositoryError> {
        let row = rss_feed::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(repository_error)?;
        row.map(to_application).transpose()
    }

    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<Option<RssFeed>, RssFeedRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let row = rss_feed::Entity::find_by_id(id)
            .one(transaction)
            .await
            .map_err(repository_error)?;
        row.map(to_application).transpose()
    }

    async fn create(
        &self,
        transaction: &UnitOfWorkTransaction,
        feed: NewRssFeed,
    ) -> Result<RssFeed, RssFeedRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let source = feed.source.clone();
        let model = rss_feed::ActiveModel {
            id: Set(feed.id),
            source: Set(feed.source),
            display_name: Set(feed.display_name),
            url: Set(feed.url),
            enabled: Set(feed.enabled),
            content_source: Set(feed.content_source.as_str().to_owned()),
            created_at: NotSet,
            updated_at: NotSet,
        };
        let model = rss_feed::Entity::insert(model)
            .exec_with_returning(transaction)
            .await
            .map_err(|error| match persistence_error(error) {
                core_application::persistence::PersistenceError::Conflict(_) => {
                    RssFeedRepositoryError::DuplicateSource(source)
                }
                error => RssFeedRepositoryError::Persistence(error),
            })?;
        to_application(model)
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
        patch: UpdateRssFeedPatch,
        updated_at: DateTime<FixedOffset>,
    ) -> Result<RssFeed, RssFeedRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = rss_feed::ActiveModel {
            id: Unchanged(id),
            source: NotSet,
            display_name: patch.display_name.map_or(NotSet, Set),
            url: patch.url.map_or(NotSet, Set),
            enabled: patch.enabled.map_or(NotSet, Set),
            content_source: patch
                .content_source
                .map(|content_source| content_source.as_str().to_owned())
                .map_or(NotSet, Set),
            created_at: NotSet,
            updated_at: Set(updated_at),
        };
        let model = model.update(transaction).await.map_err(repository_error)?;
        to_application(model)
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<bool, RssFeedRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        rss_feed::Entity::delete_by_id(id)
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, RssFeedRepositoryError> {
    crate::transaction::transaction_ref(transaction)
        .ok_or(RssFeedRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> RssFeedRepositoryError {
    RssFeedRepositoryError::Persistence(persistence_error(error))
}

fn to_application(model: rss_feed::Model) -> Result<RssFeed, RssFeedRepositoryError> {
    let content_source = ContentSource::parse(&model.content_source).ok_or_else(|| {
        RssFeedRepositoryError::InvalidContentSource(model.content_source.clone())
    })?;
    Ok(RssFeed {
        id: model.id,
        source: model.source,
        display_name: model.display_name,
        url: model.url,
        enabled: model.enabled,
        content_source,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use core_application::rss_feed::{
        ContentSource, NewRssFeed, RssFeed, RssFeedRepository, UpdateRssFeedPatch,
    };
    use core_application::unit_of_work::UnitOfWork;
    use uuid::Uuid;

    use super::PostgresRssFeedRepository;
    use crate::DatabaseHandle;
    use crate::PostgresUnitOfWork;

    fn normalize(mut feed: RssFeed) -> RssFeed {
        feed.id = Uuid::nil();
        let timestamp = epoch();
        feed.created_at = timestamp;
        feed.updated_at = timestamp;
        feed
    }

    fn epoch() -> chrono::DateTime<chrono::FixedOffset> {
        chrono::DateTime::<Utc>::from_timestamp(0, 0)
            .expect("valid timestamp")
            .fixed_offset()
    }

    fn new_feed(source: &str, display_name: &str, enabled: bool) -> NewRssFeed {
        NewRssFeed {
            id: Uuid::new_v4(),
            source: source.into(),
            display_name: display_name.into(),
            url: format!("https://feeds.example.invalid/{source}.xml"),
            enabled,
            content_source: ContentSource::None,
        }
    }

    fn setup(db: DatabaseHandle) -> (PostgresUnitOfWork, PostgresRssFeedRepository) {
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let repository = PostgresRssFeedRepository::new(db);
        (unit_of_work, repository)
    }

    async fn create_feed(
        unit_of_work: &PostgresUnitOfWork,
        repository: &PostgresRssFeedRepository,
        feed: NewRssFeed,
    ) -> RssFeed {
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        let created = repository
            .create(&transaction, feed)
            .await
            .expect("feed creates");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");
        created
    }

    #[backend_test_macros::database_test]
    async fn repository_creates_and_lists_feed(db: DatabaseHandle) {
        let (unit_of_work, repository) = setup(db);
        let created = create_feed(
            &unit_of_work,
            &repository,
            new_feed("feed-alpha", "Alpha publication", true),
        )
        .await;
        let listed = repository.list(true).await.expect("feeds list");

        let expected = RssFeed {
            id: Uuid::nil(),
            source: "feed-alpha".into(),
            display_name: "Alpha publication".into(),
            url: "https://feeds.example.invalid/feed-alpha.xml".into(),
            enabled: true,
            content_source: ContentSource::None,
            created_at: epoch(),
            updated_at: epoch(),
        };

        assert_eq!(
            (
                normalize(created),
                listed.into_iter().map(normalize).collect::<Vec<_>>(),
            ),
            (expected.clone(), vec![expected]),
        );
    }

    #[backend_test_macros::database_test]
    async fn database_defaults_content_source_to_none(db: DatabaseHandle) {
        use sea_orm::ConnectionTrait;

        let id = Uuid::new_v4();
        db.execute_unprepared(&format!(
            "INSERT INTO rss_feed (id, source, display_name, url, enabled) \
             VALUES ('{id}', 'sample-feed', 'Sample publication', 'https://example.invalid/feed.xml', true)"
        ))
        .await
        .expect("feed inserts without content_source");
        let repository = PostgresRssFeedRepository::new(db);
        let feed = repository
            .find_by_id(id)
            .await
            .expect("feed loads")
            .expect("feed exists");

        assert_eq!(
            normalize(feed),
            RssFeed {
                id: Uuid::nil(),
                source: "sample-feed".into(),
                display_name: "Sample publication".into(),
                url: "https://example.invalid/feed.xml".into(),
                enabled: true,
                content_source: ContentSource::None,
                created_at: epoch(),
                updated_at: epoch(),
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn repository_filters_list_by_enabled_state(db: DatabaseHandle) {
        let (unit_of_work, repository) = setup(db);
        let enabled = create_feed(
            &unit_of_work,
            &repository,
            new_feed("feed-alpha", "Alpha publication", true),
        )
        .await;
        let disabled = create_feed(
            &unit_of_work,
            &repository,
            new_feed("feed-zeta", "Zeta publication", false),
        )
        .await;
        let enabled_feeds = repository.list(true).await.expect("enabled feeds list");
        let all_feeds = repository.list(false).await.expect("all feeds list");

        assert_eq!(
            (
                enabled_feeds.into_iter().map(normalize).collect::<Vec<_>>(),
                all_feeds.into_iter().map(normalize).collect::<Vec<_>>(),
            ),
            (
                vec![normalize(enabled.clone())],
                vec![normalize(enabled), normalize(disabled)],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn repository_updates_feed(db: DatabaseHandle) {
        let (unit_of_work, repository) = setup(db);
        let created = create_feed(
            &unit_of_work,
            &repository,
            new_feed("feed-alpha", "Alpha publication", true),
        )
        .await;
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        let updated = repository
            .update(
                &transaction,
                created.id,
                UpdateRssFeedPatch {
                    display_name: Some("Updated publication".into()),
                    url: None,
                    enabled: Some(false),
                    content_source: Some(ContentSource::Crawl),
                },
                Utc::now().fixed_offset(),
            )
            .await
            .expect("feed updates");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");

        assert_eq!(
            normalize(updated),
            normalize(RssFeed {
                display_name: "Updated publication".into(),
                enabled: false,
                content_source: ContentSource::Crawl,
                ..created
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn repository_preserves_concurrent_changes_to_unpatched_fields(db: DatabaseHandle) {
        let (unit_of_work, repository) = setup(db);
        let created = create_feed(
            &unit_of_work,
            &repository,
            new_feed("feed-alpha", "Alpha publication", true),
        )
        .await;
        let stale_transaction = unit_of_work.begin().await.expect("transaction begins");
        repository
            .find_by_id_in_transaction(&stale_transaction, created.id)
            .await
            .expect("feed lookup")
            .expect("feed exists");

        let concurrent_transaction = unit_of_work.begin().await.expect("transaction begins");
        repository
            .update(
                &concurrent_transaction,
                created.id,
                UpdateRssFeedPatch {
                    display_name: None,
                    url: Some("https://feeds.example.invalid/concurrent.xml".into()),
                    enabled: None,
                    content_source: None,
                },
                Utc::now().fixed_offset(),
            )
            .await
            .expect("concurrent URL update succeeds");
        unit_of_work
            .commit(concurrent_transaction)
            .await
            .expect("concurrent transaction commits");

        let updated = repository
            .update(
                &stale_transaction,
                created.id,
                UpdateRssFeedPatch {
                    display_name: Some("Updated publication".into()),
                    url: None,
                    enabled: None,
                    content_source: None,
                },
                Utc::now().fixed_offset(),
            )
            .await
            .expect("partial update succeeds");
        unit_of_work
            .commit(stale_transaction)
            .await
            .expect("stale transaction commits");

        assert_eq!(
            normalize(updated),
            normalize(RssFeed {
                display_name: "Updated publication".into(),
                url: "https://feeds.example.invalid/concurrent.xml".into(),
                ..created
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn repository_deletes_feed(db: DatabaseHandle) {
        let (unit_of_work, repository) = setup(db);
        let created = create_feed(
            &unit_of_work,
            &repository,
            new_feed("feed-alpha", "Alpha publication", true),
        )
        .await;
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        let deleted = repository
            .delete(&transaction, created.id)
            .await
            .expect("feed deletes");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");
        let missing = repository
            .find_by_id(created.id)
            .await
            .expect("feed lookup");

        assert_eq!((deleted, missing), (true, None));
    }

    #[backend_test_macros::database_test]
    async fn repository_returns_false_when_deleting_missing_feed(db: DatabaseHandle) {
        let (unit_of_work, repository) = setup(db);
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        let deleted = repository
            .delete(&transaction, Uuid::new_v4())
            .await
            .expect("missing feed delete succeeds");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");

        assert!(!deleted);
    }
}
