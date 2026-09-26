use std::collections::HashSet;
use std::time::Duration;

use chrono::Utc;
use core_application::{
    NewsAggregator, NewsAggregatorError, NewsFeed, NewsItem, SharedNewsAggregator,
};
use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, EntityTrait, Set};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::entities::news_item;
use crate::services::rss_feed::{self, RssFeedError};

#[derive(Debug, thiserror::Error)]
pub enum NewsAggregationError {
    #[error(transparent)]
    Aggregator(#[from] NewsAggregatorError),

    #[error(transparent)]
    FeedList(#[from] RssFeedError),

    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),
}

/// fetch と upsert を行う poll task の 1 サイクルの結果統計
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AggregationStats {
    pub fetched: usize,
}

/// RSS を取得して `news_item` に保存する
pub async fn run_aggregation_cycle(
    db: &impl sea_orm::ConnectionTrait,
    aggregator: &dyn NewsAggregator,
) -> Result<AggregationStats, NewsAggregationError> {
    let rows = rss_feed::list(db, true).await?;
    // 既存の news_item.source と表示名を揃えるため、slug ではなく display_name を渡す。
    let feeds = rows
        .into_iter()
        .map(|row| NewsFeed {
            source: row.display_name,
            url: row.url,
        })
        .collect::<Vec<_>>();
    let fetched = aggregator.fetch_news(&feeds).await?;
    let fetched_count = upsert_news_items(db, &fetched).await?;
    Ok(AggregationStats {
        fetched: fetched_count,
    })
}

/// `news_item` テーブルに upsert し、対象 URL の件数を返す
pub async fn upsert_news_items(
    db: &impl sea_orm::ConnectionTrait,
    items: &[NewsItem],
) -> Result<usize, sea_orm::DbErr> {
    if items.is_empty() {
        return Ok(0);
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

    Ok(items
        .iter()
        .map(|item| item.url.as_str())
        .collect::<HashSet<_>>()
        .len())
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す
pub fn spawn_poll(
    db: DatabaseConnection,
    aggregator: SharedNewsAggregator,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::news_item;
    use crate::services::rss_feed::{self, CreateInput};
    use core_application::{FakeNewsAggregator, NewsAggregatorError, NewsFeed};
    use sea_orm::{EntityTrait, PaginatorTrait};
    async fn create_feed(
        db: &impl sea_orm::ConnectionTrait,
        source: &str,
        name: &str,
        enabled: bool,
    ) {
        rss_feed::create(
            db,
            CreateInput {
                source: source.into(),
                display_name: name.into(),
                url: format!("https://example.invalid/{source}"),
                enabled: Some(enabled),
            },
        )
        .await
        .expect("feed creates");
    }

    #[backend_test_macros::database_test]
    async fn run_aggregation_cycle_passes_enabled_feeds_by_display_name(
        db: crate::database::DatabaseHandle,
    ) {
        create_feed(&db, "feed_zulu", "Zulu publication", true).await;
        create_feed(&db, "feed_alpha", "Alpha publication", true).await;
        create_feed(&db, "feed_disabled", "Disabled publication", false).await;
        let aggregator = FakeNewsAggregator::new();

        let stats = run_aggregation_cycle(&db, &aggregator)
            .await
            .expect("cycle succeeds");
        let requested_feeds = aggregator.requested_feeds.lock().await.clone();

        assert_eq!(
            (stats, requested_feeds),
            (
                AggregationStats { fetched: 0 },
                vec![vec![
                    NewsFeed {
                        source: "Alpha publication".into(),
                        url: "https://example.invalid/feed_alpha".into(),
                    },
                    NewsFeed {
                        source: "Zulu publication".into(),
                        url: "https://example.invalid/feed_zulu".into(),
                    },
                ]],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn run_aggregation_cycle_stops_before_upsert_when_aggregator_fails(
        db: crate::database::DatabaseHandle,
    ) {
        let aggregator = FakeNewsAggregator::new();
        *aggregator.fetch_error.lock().await =
            Some(NewsAggregatorError::Network("test failure".to_string()));

        let result = run_aggregation_cycle(&db, &aggregator)
            .await
            .map(|stats| stats.fetched)
            .map_err(|error| error.to_string());
        let stored_items = news_item::Entity::find()
            .count(&db)
            .await
            .expect("news items count");

        assert_eq!(
            (result, stored_items),
            (Err("network error: test failure".to_string()), 0),
        );
    }
}
