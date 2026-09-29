use std::collections::HashSet;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Utc};
use core_application::news::{
    NewsArticle, NewsItemRepository, NewsItemRepositoryError, NewsSearchCriteria,
};
use core_application::news_aggregator::NewsItem;
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::news_item;
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
    ) -> Result<usize, NewsItemRepositoryError> {
        if items.is_empty() {
            return Ok(0);
        }
        let transaction = transaction_ref(transaction)?;
        let fetched_at = Utc::now().fixed_offset();
        let active_models = items
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

        Ok(items
            .iter()
            .map(|item| item.url.as_str())
            .collect::<HashSet<_>>()
            .len())
    }

    async fn search(
        &self,
        criteria: NewsSearchCriteria,
    ) -> Result<Vec<NewsArticle>, NewsItemRepositoryError> {
        let mut query = news_item::Entity::find();
        if let Some(keyword) = criteria.keyword.as_deref() {
            let pattern = format!("%{}%", sanitize_like(keyword));
            query = query.filter(
                Condition::any()
                    .add(news_item::Column::Title.ilike(pattern.clone()))
                    .add(news_item::Column::BodySnippet.ilike(pattern)),
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
            .map(|rows| rows.into_iter().map(to_application).collect())
            .map_err(|error| NewsItemRepositoryError::Persistence(persistence_error(error)))
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, NewsItemRepositoryError> {
    crate::transaction::transaction_ref(transaction)
        .ok_or(NewsItemRepositoryError::InvalidTransaction)
}

fn sanitize_like(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '%' | '_' | '\\'))
        .collect()
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

fn to_application(model: news_item::Model) -> NewsArticle {
    NewsArticle {
        id: model.id,
        source: model.source,
        url: model.url,
        title: model.title,
        body_snippet: model.body_snippet,
        published_at: model.published_at,
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDate, Utc};
    use core_application::news::{NewsItemRepository, NewsSearchCriteria};
    use core_application::news_aggregator::NewsItem;
    use core_application::unit_of_work::UnitOfWork;
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
            published_at,
        };
        let updated = NewsItem {
            source: "Updated publication".into(),
            title: "Updated title".into(),
            body_snippet: Some("Updated summary".into()),
            ..initial.clone()
        };
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        let initial_count = repository
            .upsert(&transaction, std::slice::from_ref(&initial))
            .await
            .expect("initial article saves");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        let updated_count = repository
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
                initial_count,
                updated_count,
                articles.into_iter().map(normalize).collect::<Vec<_>>(),
            ),
            (
                1,
                1,
                vec![core_application::news::NewsArticle {
                    id: Uuid::nil(),
                    source: "Updated publication".into(),
                    url: "https://example.invalid/article".into(),
                    title: "Updated title".into(),
                    body_snippet: Some("Updated summary".into()),
                    published_at: published_at.fixed_offset(),
                }],
            ),
        );
    }
}
