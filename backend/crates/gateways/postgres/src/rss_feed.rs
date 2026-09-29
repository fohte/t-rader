use async_trait::async_trait;
use core_application::rss_feed::{NewRssFeed, RssFeed, RssFeedRepository, RssFeedRepositoryError};
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
        query
            .order_by_asc(rss_feed::Column::DisplayName)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_application).collect())
            .map_err(repository_error)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<RssFeed>, RssFeedRepositoryError> {
        rss_feed::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map(|row| row.map(to_application))
            .map_err(repository_error)
    }

    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<Option<RssFeed>, RssFeedRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        rss_feed::Entity::find_by_id(id)
            .one(transaction)
            .await
            .map(|row| row.map(to_application))
            .map_err(repository_error)
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
            created_at: NotSet,
            updated_at: NotSet,
        };
        rss_feed::Entity::insert(model)
            .exec_with_returning(transaction)
            .await
            .map(to_application)
            .map_err(|error| match persistence_error(error) {
                core_application::persistence::PersistenceError::Conflict(_) => {
                    RssFeedRepositoryError::DuplicateSource(source)
                }
                error => RssFeedRepositoryError::Persistence(error),
            })
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        feed: RssFeed,
    ) -> Result<RssFeed, RssFeedRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = rss_feed::ActiveModel {
            id: Unchanged(feed.id),
            source: Unchanged(feed.source),
            display_name: Set(feed.display_name),
            url: Set(feed.url),
            enabled: Set(feed.enabled),
            created_at: Unchanged(feed.created_at),
            updated_at: Set(feed.updated_at),
        };
        model
            .update(transaction)
            .await
            .map(to_application)
            .map_err(repository_error)
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

fn to_application(model: rss_feed::Model) -> RssFeed {
    RssFeed {
        id: model.id,
        source: model.source,
        display_name: model.display_name,
        url: model.url,
        enabled: model.enabled,
        created_at: model.created_at,
        updated_at: model.updated_at,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use core_application::rss_feed::{NewRssFeed, RssFeed, RssFeedRepository};
    use core_application::unit_of_work::UnitOfWork;
    use uuid::Uuid;

    use super::PostgresRssFeedRepository;
    use crate::DatabaseHandle;
    use crate::PostgresUnitOfWork;

    fn normalize(mut feed: RssFeed) -> RssFeed {
        feed.id = Uuid::nil();
        let timestamp = chrono::DateTime::<Utc>::from_timestamp(0, 0)
            .expect("valid timestamp")
            .fixed_offset();
        feed.created_at = timestamp;
        feed.updated_at = timestamp;
        feed
    }

    #[backend_test_macros::database_test]
    async fn repository_lists_updates_and_deletes_feeds(db: DatabaseHandle) {
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let repository = PostgresRssFeedRepository::new(db);
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        let created = repository
            .create(
                &transaction,
                NewRssFeed {
                    id: Uuid::new_v4(),
                    source: "feed-alpha".into(),
                    display_name: "Alpha feed".into(),
                    url: "https://example.invalid/alpha".into(),
                    enabled: true,
                },
            )
            .await
            .expect("feed creates");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");
        let listed = repository.list(true).await.expect("feeds list");
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        let updated = repository
            .update(
                &transaction,
                RssFeed {
                    display_name: "Updated feed".into(),
                    enabled: false,
                    updated_at: Utc::now().fixed_offset(),
                    ..created.clone()
                },
            )
            .await
            .expect("feed updates");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");
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

        assert_eq!(
            (
                listed.into_iter().map(normalize).collect::<Vec<_>>(),
                normalize(updated),
                deleted,
                missing,
            ),
            (
                vec![normalize(created.clone())],
                normalize(RssFeed {
                    display_name: "Updated feed".into(),
                    enabled: false,
                    ..created
                }),
                true,
                None,
            ),
        );
    }
}
