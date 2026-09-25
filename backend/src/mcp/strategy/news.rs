//! 戦略実行 MCP のニュース検索 tool 実装。

use chrono::{DateTime, FixedOffset, NaiveDate};
use rmcp::ErrorData as McpError;
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::entities::news_item;
use crate::handlers::refs::sanitize_like;

use super::dto::{NewsItemDto, SearchNewsParams, SearchNewsResult};
use super::{StrategyServer, clamp_limit, db_error};

impl StrategyServer {
    /// news_item を title/body_snippet のキーワードと published_at の期間で直接検索する。
    pub(crate) async fn search_news_inner(
        &self,
        _session_strategy_id: Uuid,
        params: SearchNewsParams,
    ) -> Result<SearchNewsResult, McpError> {
        let mut query = news_item::Entity::find();

        if let Some(keyword) = params
            .keyword
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty())
        {
            let pattern = format!("%{}%", sanitize_like(keyword));
            query = query.filter(
                Condition::any()
                    .add(news_item::Column::Title.ilike(pattern.clone()))
                    .add(news_item::Column::BodySnippet.ilike(pattern)),
            );
        }
        if let Some(from) = params.from {
            query = query.filter(news_item::Column::PublishedAt.gte(start_of_day(from)));
        }
        if let Some(to) = params.to {
            query = query.filter(news_item::Column::PublishedAt.lte(end_of_day(to)));
        }

        let rows = query
            .order_by_desc(news_item::Column::PublishedAt)
            .limit(clamp_limit(params.limit))
            .all(&self.db)
            .await
            .map_err(db_error)?;

        let items = rows
            .into_iter()
            .map(|n| NewsItemDto {
                id: n.id,
                source: n.source,
                url: n.url,
                title: n.title,
                body_snippet: n.body_snippet,
                published_at: n.published_at,
            })
            .collect();

        Ok(SearchNewsResult { items })
    }
}

fn start_of_day(date: NaiveDate) -> DateTime<FixedOffset> {
    date.and_hms_opt(0, 0, 0)
        .unwrap_or_default()
        .and_utc()
        .fixed_offset()
}

fn end_of_day(date: NaiveDate) -> DateTime<FixedOffset> {
    date.and_hms_opt(23, 59, 59)
        .unwrap_or_default()
        .and_utc()
        .fixed_offset()
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset, NaiveDate};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sea_orm::DatabaseConnection;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::news_item;
    use crate::testing::create_test_db;

    use super::super::dto::{NewsItemDto, SearchNewsParams, SearchNewsResult};
    use super::super::tests_common::build_server;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    fn at_noon(d: NaiveDate) -> DateTime<FixedOffset> {
        d.and_hms_opt(12, 0, 0)
            .expect("valid time")
            .and_utc()
            .fixed_offset()
    }

    async fn insert_news_item_with(
        db: &DatabaseConnection,
        url: &str,
        title: &str,
        body_snippet: Option<&str>,
        published_at: DateTime<FixedOffset>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        news_item::ActiveModel {
            id: Set(id),
            source: Set("Test".into()),
            url: Set(url.into()),
            title: Set(title.into()),
            body_snippet: Set(body_snippet.map(str::to_string)),
            published_at: Set(published_at),
            fetched_at: Set(published_at),
        }
        .insert(db)
        .await
        .expect("insert news item");
        id
    }

    fn result_urls(result: &SearchNewsResult) -> Vec<String> {
        result.items.iter().map(|i| i.url.clone()).collect()
    }

    #[sqlx::test(migrations = false)]
    async fn search_news_returns_full_item_shape(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());

        let published_at = at_noon(ymd(2026, 6, 1));
        let id = insert_news_item_with(
            &db,
            "https://ex.com/1",
            "トヨタ自動車 決算発表",
            Some("好調"),
            published_at,
        )
        .await;

        let result = server
            .search_news_inner(
                Uuid::new_v4(),
                SearchNewsParams {
                    keyword: None,
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect("search_news");

        assert_eq!(
            result,
            SearchNewsResult {
                items: vec![NewsItemDto {
                    id,
                    source: "Test".into(),
                    url: "https://ex.com/1".into(),
                    title: "トヨタ自動車 決算発表".into(),
                    body_snippet: Some("好調".into()),
                    published_at,
                }],
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn search_news_matches_keyword_case_insensitively_in_title_or_body(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());

        insert_news_item_with(
            &db,
            "https://ex.com/1",
            "TOYOTA 決算発表",
            None,
            at_noon(ymd(2026, 6, 1)),
        )
        .await;
        insert_news_item_with(
            &db,
            "https://ex.com/2",
            "市況まとめ",
            Some("半導体株が上昇"),
            at_noon(ymd(2026, 6, 1)),
        )
        .await;
        insert_news_item_with(
            &db,
            "https://ex.com/3",
            "無関係のニュース",
            None,
            at_noon(ymd(2026, 6, 1)),
        )
        .await;

        let result = server
            .search_news_inner(
                Uuid::new_v4(),
                SearchNewsParams {
                    keyword: Some("toyota".into()),
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect("search_news");

        assert_eq!(result_urls(&result), vec!["https://ex.com/1".to_string()]);
    }

    #[sqlx::test(migrations = false)]
    async fn search_news_does_not_treat_underscore_as_single_char_wildcard(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());

        insert_news_item_with(
            &db,
            "https://ex.com/1",
            "AXB",
            None,
            at_noon(ymd(2026, 6, 1)),
        )
        .await;

        let result = server
            .search_news_inner(
                Uuid::new_v4(),
                SearchNewsParams {
                    keyword: Some("A_B".into()),
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect("search_news");

        assert_eq!(result_urls(&result), Vec::<String>::new());
    }

    #[sqlx::test(migrations = false)]
    async fn search_news_filters_by_published_at_range_inclusive(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());

        insert_news_item_with(
            &db,
            "https://ex.com/1",
            "1日",
            None,
            at_noon(ymd(2026, 6, 1)),
        )
        .await;
        insert_news_item_with(
            &db,
            "https://ex.com/2",
            "5日",
            None,
            at_noon(ymd(2026, 6, 5)),
        )
        .await;
        insert_news_item_with(
            &db,
            "https://ex.com/3",
            "10日",
            None,
            at_noon(ymd(2026, 6, 10)),
        )
        .await;

        let result = server
            .search_news_inner(
                Uuid::new_v4(),
                SearchNewsParams {
                    keyword: None,
                    from: Some(ymd(2026, 6, 5)),
                    to: Some(ymd(2026, 6, 5)),
                    limit: None,
                },
            )
            .await
            .expect("search_news");

        assert_eq!(result_urls(&result), vec!["https://ex.com/2".to_string()]);
    }

    #[sqlx::test(migrations = false)]
    async fn search_news_orders_newest_first_and_respects_limit(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());

        insert_news_item_with(
            &db,
            "https://ex.com/1",
            "1日",
            None,
            at_noon(ymd(2026, 6, 1)),
        )
        .await;
        insert_news_item_with(
            &db,
            "https://ex.com/2",
            "2日",
            None,
            at_noon(ymd(2026, 6, 2)),
        )
        .await;
        insert_news_item_with(
            &db,
            "https://ex.com/3",
            "3日",
            None,
            at_noon(ymd(2026, 6, 3)),
        )
        .await;

        let result = server
            .search_news_inner(
                Uuid::new_v4(),
                SearchNewsParams {
                    keyword: None,
                    from: None,
                    to: None,
                    limit: Some(2),
                },
            )
            .await
            .expect("search_news");

        assert_eq!(
            result_urls(&result),
            vec![
                "https://ex.com/3".to_string(),
                "https://ex.com/2".to_string(),
            ],
        );
    }
}
