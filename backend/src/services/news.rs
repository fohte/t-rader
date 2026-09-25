use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::data_provider::DataProviderError;
use crate::data_provider::news::{NewsAggregator, NewsItem};
use crate::entities::news_item;

/// fetch と upsert を行う poll task の 1 サイクルの結果統計
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AggregationStats {
    pub fetched: usize,
}

/// RSS を取得して `news_item` に保存する
pub async fn run_aggregation_cycle(
    db: &DatabaseConnection,
    aggregator: &dyn NewsAggregator,
) -> Result<AggregationStats, DataProviderError> {
    let fetched = aggregator.fetch_news().await?;
    let news_rows = upsert_news_items(db, &fetched).await.map_err(db_err)?;
    Ok(AggregationStats {
        fetched: news_rows.len(),
    })
}

fn db_err(e: sea_orm::DbErr) -> DataProviderError {
    DataProviderError::Database(e.to_string())
}

/// `news_item` テーブルに upsert し、対象行の Model 全件を返す
pub async fn upsert_news_items(
    db: &DatabaseConnection,
    items: &[NewsItem],
) -> Result<Vec<news_item::Model>, sea_orm::DbErr> {
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let now = Utc::now().into();
    let actives: Vec<news_item::ActiveModel> = items
        .iter()
        .map(|n| news_item::ActiveModel {
            id: Set(Uuid::new_v4()),
            source: Set(n.source.clone()),
            url: Set(n.url.clone()),
            title: Set(n.title.clone()),
            body_snippet: Set(n.body_snippet.clone()),
            published_at: Set(n.published_at.into()),
            fetched_at: Set(now),
        })
        .collect();

    // url で conflict したら title / source / published_at / body_snippet / fetched_at を更新
    news_item::Entity::insert_many(actives)
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
        .exec(db)
        .await?;

    let urls: Vec<String> = items.iter().map(|n| n.url.clone()).collect();
    news_item::Entity::find()
        .filter(news_item::Column::Url.is_in(urls))
        .all(db)
        .await
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す
pub fn spawn_poll(
    db: DatabaseConnection,
    aggregator: Arc<dyn NewsAggregator>,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match run_aggregation_cycle(&db, aggregator.as_ref()).await {
                Ok(stats) => {
                    tracing::debug!(fetched = stats.fetched, "news aggregation cycle completed",);
                }
                Err(err) => {
                    tracing::warn!(%err, "news aggregation cycle failed");
                }
            }
        }
    })
}
