//! 戦略実行 MCP のニュース検索 tool 実装。

use core_application::news::{NewsArticle, SearchNewsQuery};
use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;

use super::dto::{NewsItemDto, SearchNewsParams, SearchNewsResult};
use super::{StrategyServer, internal_error};

impl StrategyServer {
    /// news_item を title/body_snippet のキーワードと published_at の期間で検索する。
    pub(crate) async fn search_news_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: SearchNewsParams,
    ) -> Result<SearchNewsResult, McpError> {
        let scope = scope.into();
        let rows = self
            .use_cases
            .news
            .search_news(
                scope,
                SearchNewsQuery {
                    keyword: params.keyword,
                    from: params.from,
                    to: params.to,
                    limit: params.limit,
                },
            )
            .await
            .map_err(map_news_error)?;

        Ok(SearchNewsResult {
            items: rows.into_iter().map(Into::into).collect(),
        })
    }
}

impl From<NewsArticle> for NewsItemDto {
    fn from(article: NewsArticle) -> Self {
        Self {
            id: article.id,
            source: article.source,
            url: article.url,
            title: article.title,
            body_snippet: article.body_snippet,
            published_at: article.published_at,
        }
    }
}

fn map_news_error(error: core_application::news::NewsUseCaseError) -> McpError {
    tracing::error!(error = %error, "strategy mcp news search failed");
    internal_error(format!("database error: {error}"))
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset, NaiveDate};
    use uuid::Uuid;

    use super::super::dto::{NewsItemDto, SearchNewsParams, SearchNewsResult};
    use super::super::tests_common::build_server;
    use core_application::news::{NewsItemRepository, NewsSearchCriteria};
    use core_application::news_aggregator::NewsItem;
    use core_application::unit_of_work::UnitOfWork;
    use gateway_postgres::{DatabaseHandle, PostgresNewsItemRepository, PostgresUnitOfWork};

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
        db: &DatabaseHandle,
        url: &str,
        title: &str,
        body_snippet: Option<&str>,
        published_at: DateTime<FixedOffset>,
    ) -> Uuid {
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let repository = PostgresNewsItemRepository::new(db.clone());
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        repository
            .upsert(
                &transaction,
                &[NewsItem {
                    source: "Sample publication".into(),
                    url: url.into(),
                    title: title.into(),
                    body_snippet: body_snippet.map(str::to_string),
                    published_at: published_at.with_timezone(&chrono::Utc),
                }],
            )
            .await
            .expect("insert news item");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");
        repository
            .search(NewsSearchCriteria {
                keyword: None,
                from: None,
                to: None,
                limit: 200,
            })
            .await
            .expect("search inserted news item")
            .into_iter()
            .find(|item| item.url == url)
            .map(|item| item.id)
            .expect("inserted news item exists")
    }

    fn result_urls(result: &SearchNewsResult) -> Vec<String> {
        result.items.iter().map(|i| i.url.clone()).collect()
    }

    #[backend_test_macros::database_test]
    async fn search_news_returns_full_item_shape(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db.clone());

        let published_at = at_noon(ymd(2026, 6, 1));
        let id = insert_news_item_with(
            &db,
            "https://ex.com/1",
            "Example Ventures earnings update",
            Some("quarterly results"),
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
                    source: "Sample publication".into(),
                    url: "https://ex.com/1".into(),
                    title: "Example Ventures earnings update".into(),
                    body_snippet: Some("quarterly results".into()),
                    published_at,
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_news_matches_keyword_case_insensitively_in_title_or_body(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = build_server(db.clone());

        insert_news_item_with(
            &db,
            "https://ex.com/1",
            "EXAMPLE VENTURES earnings update",
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
                    keyword: Some("example ventures".into()),
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect("search_news");

        assert_eq!(result_urls(&result), vec!["https://ex.com/1".to_string()]);
    }

    #[backend_test_macros::database_test]
    async fn search_news_does_not_treat_underscore_as_single_char_wildcard(
        db: gateway_postgres::DatabaseHandle,
    ) {
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

    #[backend_test_macros::database_test]
    async fn search_news_filters_by_published_at_range_inclusive(
        db: gateway_postgres::DatabaseHandle,
    ) {
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

    #[backend_test_macros::database_test]
    async fn search_news_orders_newest_first_and_respects_limit(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
