use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Utc};
use core_application::news_content::{NewsContentRepository, PendingNewsContent};
use core_application::persistence::PersistenceError;
use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::{news_item, news_item_content};
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresNewsContentRepository {
    db: DatabaseHandle,
}

impl PostgresNewsContentRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl NewsContentRepository for PostgresNewsContentRepository {
    async fn expire_pending_before(
        &self,
        cutoff: DateTime<FixedOffset>,
    ) -> Result<u64, PersistenceError> {
        news_item_content::Entity::update_many()
            .set(news_item_content::ActiveModel {
                status: Set("failed".to_owned()),
                error: Set(Some("expired".to_owned())),
                updated_at: Set(Utc::now().fixed_offset()),
                ..Default::default()
            })
            .filter(news_item_content::Column::Status.eq("pending"))
            .filter(news_item_content::Column::CreatedAt.lt(cutoff))
            .exec(&self.db)
            .await
            .map(|result| result.rows_affected)
            .map_err(persistence_error)
    }

    async fn list_pending(&self, limit: u64) -> Result<Vec<PendingNewsContent>, PersistenceError> {
        news_item::Entity::find()
            .find_also_related(news_item_content::Entity)
            .filter(news_item_content::Column::Status.eq("pending"))
            .order_by_asc(news_item_content::Column::CreatedAt)
            .order_by_asc(news_item_content::Column::NewsItemId)
            .limit(limit)
            .all(&self.db)
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|(item, _content)| PendingNewsContent {
                        news_item_id: item.id,
                        url: item.url,
                    })
                    .collect()
            })
            .map_err(persistence_error)
    }

    async fn mark_fetched(&self, news_item_id: Uuid, body: String) -> Result<(), PersistenceError> {
        let result = news_item_content::Entity::update_many()
            .set(news_item_content::ActiveModel {
                status: Set("fetched".to_owned()),
                body: Set(Some(body)),
                error: Set(None),
                updated_at: Set(Utc::now().fixed_offset()),
                ..Default::default()
            })
            .filter(news_item_content::Column::NewsItemId.eq(news_item_id))
            .filter(news_item_content::Column::Status.eq("pending"))
            .exec(&self.db)
            .await
            .map_err(persistence_error)?;
        ensure_updated(news_item_id, result.rows_affected)
    }

    async fn mark_failed(&self, news_item_id: Uuid, error: String) -> Result<(), PersistenceError> {
        let result = news_item_content::Entity::update_many()
            .set(news_item_content::ActiveModel {
                status: Set("failed".to_owned()),
                body: Set(None),
                error: Set(Some(error)),
                updated_at: Set(Utc::now().fixed_offset()),
                ..Default::default()
            })
            .filter(news_item_content::Column::NewsItemId.eq(news_item_id))
            .filter(news_item_content::Column::Status.eq("pending"))
            .exec(&self.db)
            .await
            .map_err(persistence_error)?;
        ensure_updated(news_item_id, result.rows_affected)
    }
}

fn ensure_updated(news_item_id: Uuid, rows_affected: u64) -> Result<(), PersistenceError> {
    if rows_affected == 0 {
        return Err(PersistenceError::RecordNotUpdated(format!(
            "pending news content was not found: {news_item_id}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Duration, FixedOffset, Utc};
    use core_application::news_content::{NewsContentRepository, PendingNewsContent};
    use sea_orm::ActiveValue::Set;
    use sea_orm::EntityTrait;
    use uuid::Uuid;

    use super::PostgresNewsContentRepository;
    use crate::DatabaseHandle;
    use crate::entities::{news_item, news_item_content};

    fn timestamp(hours_ago: i64) -> DateTime<FixedOffset> {
        (Utc::now() - Duration::hours(hours_ago)).fixed_offset()
    }

    async fn insert_content(
        db: &DatabaseHandle,
        id: Uuid,
        url: &str,
        status: &str,
        created_at: DateTime<FixedOffset>,
    ) {
        news_item::Entity::insert(news_item::ActiveModel {
            id: Set(id),
            source: Set("Sample source".to_owned()),
            url: Set(url.to_owned()),
            title: Set("Sample headline".to_owned()),
            body_snippet: Set(None),
            published_at: Set(created_at),
            fetched_at: Set(created_at),
        })
        .exec_without_returning(db)
        .await
        .expect("insert news item");
        news_item_content::Entity::insert(news_item_content::ActiveModel {
            news_item_id: Set(id),
            status: Set(status.to_owned()),
            body: Set(None),
            error: Set((status == "failed").then(|| "prior_error".to_owned())),
            created_at: Set(created_at),
            updated_at: Set(created_at),
        })
        .exec_without_returning(db)
        .await
        .expect("insert news item content");
    }

    async fn content_row(db: &DatabaseHandle, id: Uuid) -> news_item_content::Model {
        news_item_content::Entity::find_by_id(id)
            .one(db)
            .await
            .expect("read news item content")
            .expect("news item content exists")
    }

    #[backend_test_macros::database_test]
    async fn list_pending_returns_oldest_pending_items_with_their_urls(db: DatabaseHandle) {
        let older_id = Uuid::from_u128(1);
        let newer_id = Uuid::from_u128(2);
        let ignored_id = Uuid::from_u128(3);
        insert_content(
            &db,
            newer_id,
            "https://example.invalid/newer",
            "pending",
            timestamp(1),
        )
        .await;
        insert_content(
            &db,
            older_id,
            "https://example.invalid/older",
            "pending",
            timestamp(2),
        )
        .await;
        insert_content(
            &db,
            ignored_id,
            "https://example.invalid/failed",
            "failed",
            timestamp(3),
        )
        .await;
        let repository = PostgresNewsContentRepository::new(db);

        let actual = repository
            .list_pending(1)
            .await
            .map_err(|error| error.to_string());

        assert_eq!(
            actual,
            Ok(vec![PendingNewsContent {
                news_item_id: older_id,
                url: "https://example.invalid/older".to_owned(),
            }]),
        );
    }

    #[backend_test_macros::database_test]
    async fn expire_pending_before_marks_only_old_pending_items_failed(db: DatabaseHandle) {
        let expired_id = Uuid::from_u128(4);
        let recent_id = Uuid::from_u128(5);
        let failed_id = Uuid::from_u128(6);
        insert_content(
            &db,
            expired_id,
            "https://example.invalid/expired",
            "pending",
            timestamp(49),
        )
        .await;
        insert_content(
            &db,
            recent_id,
            "https://example.invalid/recent",
            "pending",
            timestamp(47),
        )
        .await;
        insert_content(
            &db,
            failed_id,
            "https://example.invalid/failed",
            "failed",
            timestamp(50),
        )
        .await;
        let repository = PostgresNewsContentRepository::new(db.clone());
        let expired_count = repository
            .expire_pending_before(timestamp(48))
            .await
            .expect("expire pending items");
        let expired = content_row(&db, expired_id).await;
        let recent = content_row(&db, recent_id).await;
        let failed = content_row(&db, failed_id).await;

        assert_eq!(
            (
                expired_count,
                (expired.status, expired.body, expired.error),
                (recent.status, recent.body, recent.error),
                (failed.status, failed.body, failed.error),
            ),
            (
                1,
                ("failed".to_owned(), None, Some("expired".to_owned())),
                ("pending".to_owned(), None, None),
                ("failed".to_owned(), None, Some("prior_error".to_owned())),
            ),
        );
    }
}
