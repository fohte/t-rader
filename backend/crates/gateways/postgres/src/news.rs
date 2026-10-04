use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Utc};
use core_application::news::{
    FetchedNewsItemContent, NewsArticle, NewsArticleContent, NewsItemContentStatus,
    NewsItemRepository, NewsItemRepositoryError, NewsSearchCriteria, UpsertedNewsItem,
    sanitize_search_keyword,
};
use core_application::news_aggregator::NewsItem;
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::{news_item, news_item_content};
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresNewsItemRepository {
    db: DatabaseHandle,
}

impl PostgresNewsItemRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl NewsItemRepository for PostgresNewsItemRepository {
    async fn upsert(
        &self,
        transaction: &UnitOfWorkTransaction,
        items: &[NewsItem],
    ) -> Result<Vec<UpsertedNewsItem>, NewsItemRepositoryError> {
        if items.is_empty() {
            return Ok(Vec::new());
        }
        let transaction = transaction_ref(transaction)?;
        let fetched_at = Utc::now().fixed_offset();
        let unique_items = unique_items(items);
        let urls = unique_items
            .iter()
            .map(|item| item.url.clone())
            .collect::<Vec<_>>();
        let existing_urls = news_item::Entity::find()
            .filter(news_item::Column::Url.is_in(urls.clone()))
            .all(transaction)
            .await
            .map_err(|error| NewsItemRepositoryError::Persistence(persistence_error(error)))?
            .into_iter()
            .map(|item| item.url)
            .collect::<HashSet<_>>();
        let active_models = unique_items
            .iter()
            .map(|item| news_item::ActiveModel {
                id: Set(Uuid::new_v4()),
                source: Set(item.source.clone()),
                url: Set(item.url.clone()),
                title: Set(item.title.clone()),
                body_snippet: Set(item.body_snippet.clone()),
                published_at: Set(item.published_at.fixed_offset()),
                fetched_at: Set(fetched_at),
            })
            .collect::<Vec<_>>();

        news_item::Entity::insert_many(active_models)
            .on_conflict(
                OnConflict::column(news_item::Column::Url)
                    .update_columns([
                        news_item::Column::Title,
                        news_item::Column::Source,
                        news_item::Column::PublishedAt,
                        news_item::Column::BodySnippet,
                        news_item::Column::FetchedAt,
                    ])
                    .to_owned(),
            )
            .exec(transaction)
            .await
            .map_err(|error| NewsItemRepositoryError::Persistence(persistence_error(error)))?;

        let rows = news_item::Entity::find()
            .filter(news_item::Column::Url.is_in(urls))
            .all(transaction)
            .await
            .map_err(|error| NewsItemRepositoryError::Persistence(persistence_error(error)))?;
        let ids_by_url = rows
            .into_iter()
            .map(|item| (item.url, item.id))
            .collect::<HashMap<_, _>>();
        let upserted_items = unique_items
            .into_iter()
            .map(|item| {
                let id = ids_by_url.get(&item.url).copied().ok_or_else(|| {
                    NewsItemRepositoryError::Persistence(
                        core_application::persistence::PersistenceError::Database(format!(
                            "news item was not returned after upsert: {}",
                            item.url
                        )),
                    )
                })?;
                Ok(UpsertedNewsItem {
                    id,
                    inserted: !existing_urls.contains(&item.url),
                    item,
                })
            })
            .collect::<Result<Vec<_>, NewsItemRepositoryError>>()?;

        Ok(upserted_items)
    }

    async fn create_pending_contents(
        &self,
        transaction: &UnitOfWorkTransaction,
        news_item_ids: &[Uuid],
    ) -> Result<(), NewsItemRepositoryError> {
        if news_item_ids.is_empty() {
            return Ok(());
        }
        let transaction = transaction_ref(transaction)?;
        let models = news_item_ids
            .iter()
            .map(|id| news_item_content::ActiveModel {
                news_item_id: Set(*id),
                status: Set(NewsItemContentStatus::Pending.as_str().to_owned()),
                body: Set(None),
                error: Set(None),
                created_at: sea_orm::ActiveValue::NotSet,
                updated_at: sea_orm::ActiveValue::NotSet,
            })
            .collect::<Vec<_>>();
        news_item_content::Entity::insert_many(models)
            .on_conflict(
                OnConflict::column(news_item_content::Column::NewsItemId)
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(transaction)
            .await
            .map_err(|error| NewsItemRepositoryError::Persistence(persistence_error(error)))?;
        Ok(())
    }

    async fn upsert_fetched_contents(
        &self,
        transaction: &UnitOfWorkTransaction,
        contents: &[FetchedNewsItemContent],
    ) -> Result<(), NewsItemRepositoryError> {
        if contents.is_empty() {
            return Ok(());
        }
        let transaction = transaction_ref(transaction)?;
        let models = contents
            .iter()
            .map(|content| news_item_content::ActiveModel {
                news_item_id: Set(content.news_item_id),
                status: Set("fetched".into()),
                body: Set(Some(content.body.clone())),
                error: Set(None),
                created_at: sea_orm::ActiveValue::NotSet,
                updated_at: sea_orm::ActiveValue::NotSet,
            })
            .collect::<Vec<_>>();
        news_item_content::Entity::insert_many(models)
            .on_conflict(
                OnConflict::column(news_item_content::Column::NewsItemId)
                    .update_columns([
                        news_item_content::Column::Status,
                        news_item_content::Column::Body,
                        news_item_content::Column::Error,
                        news_item_content::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec(transaction)
            .await
            .map_err(|error| NewsItemRepositoryError::Persistence(persistence_error(error)))?;
        Ok(())
    }

    async fn search(
        &self,
        criteria: NewsSearchCriteria,
    ) -> Result<Vec<NewsArticle>, NewsItemRepositoryError> {
        let mut query = news_item::Entity::find().find_also_related(news_item_content::Entity);
        if let Some(keyword) = criteria.keyword.as_deref() {
            let pattern = format!("%{}%", sanitize_search_keyword(keyword));
            query = query.filter(
                Condition::any()
                    .add(news_item::Column::Title.ilike(pattern.clone()))
                    .add(news_item::Column::BodySnippet.ilike(pattern.clone()))
                    .add(news_item_content::Column::Body.ilike(pattern)),
            );
        }
        if let Some(from) = criteria.from {
            query = query.filter(news_item::Column::PublishedAt.gte(start_of_day(from)));
        }
        if let Some(to) = criteria.to {
            query = query.filter(news_item::Column::PublishedAt.lte(end_of_day(to)));
        }
        query
            .order_by_desc(news_item::Column::PublishedAt)
            .limit(criteria.limit)
            .all(&self.db)
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|(article, content)| to_application(article, content))
                    .collect()
            })
            .map_err(|error| NewsItemRepositoryError::Persistence(persistence_error(error)))
    }

    async fn get_content(
        &self,
        news_item_id: Uuid,
    ) -> Result<Option<NewsArticleContent>, NewsItemRepositoryError> {
        news_item::Entity::find_by_id(news_item_id)
            .find_also_related(news_item_content::Entity)
            .one(&self.db)
            .await
            .map(|row| row.map(|(article, content)| to_content_application(article, content)))
            .map_err(|error| NewsItemRepositoryError::Persistence(persistence_error(error)))
    }
}

fn unique_items(items: &[NewsItem]) -> Vec<NewsItem> {
    let mut seen = HashSet::new();
    items
        .iter()
        .filter(|item| seen.insert(item.url.as_str()))
        .cloned()
        .collect()
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, NewsItemRepositoryError> {
    crate::transaction::transaction_ref(transaction)
        .ok_or(NewsItemRepositoryError::InvalidTransaction)
}

fn start_of_day(date: chrono::NaiveDate) -> DateTime<FixedOffset> {
    date.and_hms_opt(0, 0, 0)
        .unwrap_or_default()
        .and_utc()
        .fixed_offset()
}

fn end_of_day(date: chrono::NaiveDate) -> DateTime<FixedOffset> {
    date.and_hms_opt(23, 59, 59)
        .unwrap_or_default()
        .and_utc()
        .fixed_offset()
}

fn to_application(
    model: news_item::Model,
    content: Option<news_item_content::Model>,
) -> NewsArticle {
    NewsArticle {
        id: model.id,
        source: model.source,
        url: model.url,
        title: model.title,
        body_snippet: model.body_snippet,
        content_status: content.map(|content| content.status),
        published_at: model.published_at,
    }
}

fn to_content_application(
    model: news_item::Model,
    content: Option<news_item_content::Model>,
) -> NewsArticleContent {
    NewsArticleContent {
        id: model.id,
        source: model.source,
        url: model.url,
        title: model.title,
        published_at: model.published_at,
        content_status: content.as_ref().map(|content| content.status.clone()),
        content: content.as_ref().and_then(|content| content.body.clone()),
        content_error: content.and_then(|content| content.error),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{DateTime, NaiveDate, Utc};
    use core_application::news::{NewsItemRepository, NewsSearchCriteria};
    use core_application::news_aggregator::NewsItem;
    use core_application::rss_feed::ContentSource;
    use core_application::unit_of_work::UnitOfWork;
    use sea_orm::ConnectionTrait;
    use uuid::Uuid;

    use super::PostgresNewsItemRepository;
    use crate::DatabaseHandle;
    use crate::PostgresUnitOfWork;

    fn normalize(
        mut item: core_application::news::NewsArticle,
    ) -> core_application::news::NewsArticle {
        item.id = Uuid::nil();
        item
    }

    #[backend_test_macros::database_test]
    async fn upsert_updates_an_existing_article_by_url(db: DatabaseHandle) {
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let repository = PostgresNewsItemRepository::new(db);
        let published_at = DateTime::<Utc>::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(2026, 1, 2)
                .expect("valid date")
                .and_hms_opt(3, 0, 0)
                .expect("valid time"),
            Utc,
        );
        let initial = NewsItem {
            source: "Sample publication".into(),
            url: "https://example.invalid/article".into(),
            title: "Initial title".into(),
            body_snippet: None,
            content_source: ContentSource::None,
            content: None,
            published_at,
        };
        let updated = NewsItem {
            source: "Updated publication".into(),
            title: "Updated title".into(),
            body_snippet: Some("Updated summary".into()),
            content: Some("Updated body".into()),
            ..initial.clone()
        };
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        let initial_result = repository
            .upsert(&transaction, std::slice::from_ref(&initial))
            .await
            .expect("initial article saves");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        let updated_result = repository
            .upsert(&transaction, std::slice::from_ref(&updated))
            .await
            .expect("article updates");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");
        let articles = repository
            .search(NewsSearchCriteria {
                keyword: None,
                from: None,
                to: None,
                limit: 10,
            })
            .await
            .expect("articles search");

        assert_eq!(
            (
                initial_result.len(),
                initial_result
                    .iter()
                    .map(|item| item.inserted)
                    .collect::<Vec<_>>(),
                updated_result.len(),
                updated_result
                    .iter()
                    .map(|item| item.inserted)
                    .collect::<Vec<_>>(),
                articles.into_iter().map(normalize).collect::<Vec<_>>(),
            ),
            (
                1,
                vec![true],
                1,
                vec![false],
                vec![core_application::news::NewsArticle {
                    id: Uuid::nil(),
                    source: "Updated publication".into(),
                    url: "https://example.invalid/article".into(),
                    title: "Updated title".into(),
                    body_snippet: Some("Updated summary".into()),
                    content_status: None,
                    published_at: published_at.fixed_offset(),
                }],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn upsert_creates_pending_content_once_and_updates_fetched_content(db: DatabaseHandle) {
        use crate::entities::{news_item, news_item_content};
        use sea_orm::EntityTrait;

        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let repository = PostgresNewsItemRepository::new(db.clone());
        let crawl_item = NewsItem {
            source: "Crawl source".into(),
            url: "https://example.invalid/crawl-article".into(),
            title: "Crawl headline".into(),
            body_snippet: None,
            content_source: ContentSource::Crawl,
            content: None,
            published_at: DateTime::<Utc>::UNIX_EPOCH,
        };
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        let crawl_result = repository
            .upsert(&transaction, std::slice::from_ref(&crawl_item))
            .await
            .expect("crawl article saves");
        repository
            .create_pending_contents(&transaction, &[crawl_result[0].id])
            .await
            .expect("pending content saves");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");

        let transaction = unit_of_work.begin().await.expect("transaction begins");
        let crawl_result_again = repository
            .upsert(&transaction, std::slice::from_ref(&crawl_item))
            .await
            .expect("crawl article updates");
        repository
            .create_pending_contents(&transaction, &[crawl_result_again[0].id])
            .await
            .expect("duplicate pending content is ignored");
        let feed_item = NewsItem {
            source: "Feed source".into(),
            url: "https://example.invalid/feed-article".into(),
            title: "Feed headline".into(),
            body_snippet: None,
            content_source: ContentSource::Feed,
            content: Some("Initial body".into()),
            published_at: DateTime::<Utc>::UNIX_EPOCH,
        };
        let feed_result = repository
            .upsert(&transaction, std::slice::from_ref(&feed_item))
            .await
            .expect("feed article saves");
        repository
            .upsert_fetched_contents(
                &transaction,
                &[core_application::news::FetchedNewsItemContent {
                    news_item_id: feed_result[0].id,
                    body: "Initial body".into(),
                }],
            )
            .await
            .expect("fetched content saves");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");

        let transaction = unit_of_work.begin().await.expect("transaction begins");
        let updated_feed_item = NewsItem {
            content: Some("Updated body".into()),
            ..feed_item
        };
        let updated_feed_result = repository
            .upsert(&transaction, std::slice::from_ref(&updated_feed_item))
            .await
            .expect("feed article updates");
        repository
            .upsert_fetched_contents(
                &transaction,
                &[core_application::news::FetchedNewsItemContent {
                    news_item_id: updated_feed_result[0].id,
                    body: "Updated body".into(),
                }],
            )
            .await
            .expect("fetched content updates");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");

        let articles = news_item::Entity::find()
            .all(&db)
            .await
            .expect("articles load");
        let contents = news_item_content::Entity::find()
            .all(&db)
            .await
            .expect("article contents load");
        let urls_by_id = articles
            .into_iter()
            .map(|article| (article.id, article.url))
            .collect::<HashMap<_, _>>();
        let mut content_snapshot = contents
            .into_iter()
            .map(|content| {
                (
                    urls_by_id[&content.news_item_id].clone(),
                    content.status,
                    content.body,
                    content.error,
                )
            })
            .collect::<Vec<_>>();
        content_snapshot.sort_by(|left, right| left.0.cmp(&right.0));

        assert_eq!(
            (
                crawl_result[0].inserted,
                crawl_result_again[0].inserted,
                feed_result[0].inserted,
                updated_feed_result[0].inserted,
                content_snapshot,
            ),
            (
                true,
                false,
                true,
                false,
                vec![
                    (
                        "https://example.invalid/crawl-article".into(),
                        "pending".into(),
                        None,
                        None,
                    ),
                    (
                        "https://example.invalid/feed-article".into(),
                        "fetched".into(),
                        Some("Updated body".into()),
                        None,
                    ),
                ],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn content_constraints_reject_invalid_rows(db: DatabaseHandle) {
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let repository = PostgresNewsItemRepository::new(db.clone());
        let item = NewsItem {
            source: "Sample publication".into(),
            url: "https://example.invalid/constraint-check".into(),
            title: "Sample headline".into(),
            body_snippet: None,
            content_source: ContentSource::None,
            content: None,
            published_at: DateTime::<Utc>::UNIX_EPOCH,
        };
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        let item_id = repository
            .upsert(&transaction, &[item])
            .await
            .expect("article saves")[0]
            .id;
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");

        let statements = [
            format!(
                "INSERT INTO news_item_content (news_item_id, status, body, error, created_at, updated_at) \
                 VALUES ('{item_id}', 'unknown', NULL, NULL, NOW(), NOW())"
            ),
            format!(
                "INSERT INTO news_item_content (news_item_id, status, body, error, created_at, updated_at) \
                 VALUES ('{item_id}', 'pending', 'body', NULL, NOW(), NOW())"
            ),
            format!(
                "INSERT INTO news_item_content (news_item_id, status, body, error, created_at, updated_at) \
                 VALUES ('{item_id}', 'fetched', NULL, NULL, NOW(), NOW())"
            ),
            format!(
                "INSERT INTO news_item_content (news_item_id, status, body, error, created_at, updated_at) \
                 VALUES ('{item_id}', 'failed', NULL, NULL, NOW(), NOW())"
            ),
        ];
        let mut rejected = Vec::new();
        for statement in statements {
            db.execute_unprepared("SAVEPOINT invalid_content_insert")
                .await
                .expect("savepoint starts");
            rejected.push(db.execute_unprepared(&statement).await.is_err());
            db.execute_unprepared("ROLLBACK TO SAVEPOINT invalid_content_insert")
                .await
                .expect("invalid content insert rolls back");
        }

        assert_eq!(rejected, vec![true, true, true, true]);
    }
}
