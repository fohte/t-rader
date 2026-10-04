#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset, NaiveDate};
    use uuid::Uuid;

    use super::super::ToolOutput;
    use super::super::dto::{
        GetNewsContentParams, GetNewsContentResult, NewsItemDto, SearchNewsParams, SearchNewsResult,
    };
    use super::super::tests_common::{build_server, insert_strategy};
    use core_application::news::NewsItemRepository;
    use core_application::news_aggregator::NewsItem;
    use core_application::rss_feed::ContentSource;
    use core_application::unit_of_work::UnitOfWork;
    use gateway_postgres::entities::news_item_content;
    use gateway_postgres::{DatabaseHandle, PostgresNewsItemRepository, PostgresUnitOfWork};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};

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
        let rows = repository
            .upsert(
                &transaction,
                &[NewsItem {
                    source: "Sample publication".into(),
                    url: url.into(),
                    title: title.into(),
                    body_snippet: body_snippet.map(str::to_string),
                    content_source: ContentSource::None,
                    content: None,
                    published_at: published_at.with_timezone(&chrono::Utc),
                }],
            )
            .await
            .expect("insert news item");
        let id = rows[0].id;
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");
        id
    }

    async fn insert_content(
        db: &DatabaseHandle,
        news_item_id: Uuid,
        status: &str,
        body: Option<&str>,
        error: Option<&str>,
    ) {
        news_item_content::ActiveModel {
            news_item_id: Set(news_item_id),
            status: Set(status.to_string()),
            body: Set(body.map(str::to_string)),
            error: Set(error.map(str::to_string)),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert news content");
    }

    fn normalize_result(result: ToolOutput<SearchNewsResult>) -> ToolOutput<SearchNewsResult> {
        result.normalize_json(|value| {
            for item in value["items"].as_array_mut().expect("items array") {
                item["id"] = serde_json::json!(Uuid::nil());
            }
        })
    }

    fn expected_item(
        url: &str,
        title: &str,
        body_snippet: Option<&str>,
        published_at: DateTime<FixedOffset>,
    ) -> NewsItemDto {
        NewsItemDto {
            id: Uuid::nil(),
            source: "Sample publication".into(),
            url: url.into(),
            title: title.into(),
            body_snippet: body_snippet.map(str::to_string),
            content_status: None,
            published_at,
        }
    }

    fn expected_item_with_status(
        url: &str,
        title: &str,
        body_snippet: Option<&str>,
        published_at: DateTime<FixedOffset>,
        content_status: Option<&str>,
    ) -> NewsItemDto {
        NewsItemDto {
            content_status: content_status.map(str::to_string),
            ..expected_item(url, title, body_snippet, published_at)
        }
    }

    #[backend_test_macros::database_test]
    async fn search_news_returns_full_item_shape(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db.clone());
        let strategy_id = insert_strategy(&db, "sample").await;

        let published_at = at_noon(ymd(2026, 6, 1));
        insert_news_item_with(
            &db,
            "https://example.invalid/news/one",
            "Example Ventures earnings update",
            Some("quarterly results"),
            published_at,
        )
        .await;

        let result = server
            .search_news(
                strategy_id,
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
            normalize_result(result),
            SearchNewsResult {
                items: vec![expected_item(
                    "https://example.invalid/news/one",
                    "Example Ventures earnings update",
                    Some("quarterly results"),
                    published_at,
                )],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_news_returns_content_status_for_each_article(db: DatabaseHandle) {
        let server = build_server(db.clone());
        let strategy_id = insert_strategy(&db, "sample").await;
        let missing_at = at_noon(ymd(2026, 6, 4));
        let pending_at = at_noon(ymd(2026, 6, 3));
        let fetched_at = at_noon(ymd(2026, 6, 2));
        let failed_at = at_noon(ymd(2026, 6, 1));

        insert_news_item_with(
            &db,
            "https://example.invalid/news/missing-content",
            "Missing content row",
            None,
            missing_at,
        )
        .await;
        let pending_id = insert_news_item_with(
            &db,
            "https://example.invalid/news/pending",
            "Pending article",
            None,
            pending_at,
        )
        .await;
        insert_content(&db, pending_id, "pending", None, None).await;
        let fetched_id = insert_news_item_with(
            &db,
            "https://example.invalid/news/fetched",
            "Fetched article",
            None,
            fetched_at,
        )
        .await;
        insert_content(&db, fetched_id, "fetched", Some("full body"), None).await;
        let failed_id = insert_news_item_with(
            &db,
            "https://example.invalid/news/failed",
            "Failed article",
            None,
            failed_at,
        )
        .await;
        insert_content(&db, failed_id, "failed", None, Some("fetch failed")).await;

        let result = server
            .search_news(
                strategy_id,
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
            normalize_result(result),
            SearchNewsResult {
                items: vec![
                    expected_item_with_status(
                        "https://example.invalid/news/missing-content",
                        "Missing content row",
                        None,
                        missing_at,
                        None,
                    ),
                    expected_item_with_status(
                        "https://example.invalid/news/pending",
                        "Pending article",
                        None,
                        pending_at,
                        Some("pending"),
                    ),
                    expected_item_with_status(
                        "https://example.invalid/news/fetched",
                        "Fetched article",
                        None,
                        fetched_at,
                        Some("fetched"),
                    ),
                    expected_item_with_status(
                        "https://example.invalid/news/failed",
                        "Failed article",
                        None,
                        failed_at,
                        Some("failed"),
                    ),
                ],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_news_matches_keyword_in_saved_article_body(db: DatabaseHandle) {
        let server = build_server(db.clone());
        let strategy_id = insert_strategy(&db, "sample").await;
        let published_at = at_noon(ymd(2026, 6, 1));
        let id = insert_news_item_with(
            &db,
            "https://example.invalid/news/full-body-match",
            "Company update",
            Some("general overview"),
            published_at,
        )
        .await;
        insert_content(
            &db,
            id,
            "fetched",
            Some("The stored article covers distinctive catalyst wording"),
            None,
        )
        .await;

        let result = server
            .search_news(
                strategy_id,
                SearchNewsParams {
                    keyword: Some("DISTINCTIVE CATALYST".into()),
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect("search_news");

        assert_eq!(
            normalize_result(result),
            SearchNewsResult {
                items: vec![expected_item_with_status(
                    "https://example.invalid/news/full-body-match",
                    "Company update",
                    Some("general overview"),
                    published_at,
                    Some("fetched"),
                )],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn get_news_content_returns_full_saved_article_body(db: DatabaseHandle) {
        let server = build_server(db.clone());
        let strategy_id = insert_strategy(&db, "sample").await;
        let published_at = at_noon(ymd(2026, 6, 1));
        let id = insert_news_item_with(
            &db,
            "https://example.invalid/news/full-body",
            "Company update",
            Some("short snippet"),
            published_at,
        )
        .await;
        insert_content(
            &db,
            id,
            "fetched",
            Some("The complete stored article body"),
            None,
        )
        .await;

        let result = server
            .get_news_content(strategy_id, GetNewsContentParams { id })
            .await
            .expect("get_news_content");

        assert_eq!(
            result,
            GetNewsContentResult {
                id,
                source: "Sample publication".into(),
                url: "https://example.invalid/news/full-body".into(),
                title: "Company update".into(),
                published_at,
                content_status: Some("fetched".into()),
                content: Some("The complete stored article body".into()),
                content_error: None,
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn get_news_content_returns_null_fields_when_no_content_record_exists(
        db: DatabaseHandle,
    ) {
        let server = build_server(db.clone());
        let strategy_id = insert_strategy(&db, "sample").await;
        let published_at = at_noon(ymd(2026, 6, 1));
        let id = insert_news_item_with(
            &db,
            "https://example.invalid/news/no-content-row",
            "Company update",
            None,
            published_at,
        )
        .await;

        let result = server
            .get_news_content(strategy_id, GetNewsContentParams { id })
            .await
            .expect("get_news_content");

        assert_eq!(
            result,
            GetNewsContentResult {
                id,
                source: "Sample publication".into(),
                url: "https://example.invalid/news/no-content-row".into(),
                title: "Company update".into(),
                published_at,
                content_status: None,
                content: None,
                content_error: None,
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn get_news_content_returns_failed_retrieval_state(db: DatabaseHandle) {
        let server = build_server(db.clone());
        let strategy_id = insert_strategy(&db, "sample").await;
        let published_at = at_noon(ymd(2026, 6, 1));
        let id = insert_news_item_with(
            &db,
            "https://example.invalid/news/failed-content",
            "Company update",
            None,
            published_at,
        )
        .await;
        insert_content(&db, id, "failed", None, Some("retrieval failed")).await;

        let result = server
            .get_news_content(strategy_id, GetNewsContentParams { id })
            .await
            .expect("get_news_content");

        assert_eq!(
            result,
            GetNewsContentResult {
                id,
                source: "Sample publication".into(),
                url: "https://example.invalid/news/failed-content".into(),
                title: "Company update".into(),
                published_at,
                content_status: Some("failed".into()),
                content: None,
                content_error: Some("retrieval failed".into()),
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn get_news_content_returns_not_found_for_unknown_id(db: DatabaseHandle) {
        let server = build_server(db.clone());
        let strategy_id = insert_strategy(&db, "sample").await;
        let error = server
            .get_news_content(strategy_id, GetNewsContentParams { id: Uuid::new_v4() })
            .await
            .expect_err("unknown news item expected to be rejected");

        assert_eq!(
            error,
            rmcp::ErrorData::resource_not_found("news item not found", None),
        );
    }

    #[backend_test_macros::database_test]
    async fn search_news_matches_keyword_case_insensitively_in_title_or_body(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = build_server(db.clone());
        let strategy_id = insert_strategy(&db, "sample").await;

        insert_news_item_with(
            &db,
            "https://example.invalid/news/one",
            "EXAMPLE VENTURES earnings update",
            None,
            at_noon(ymd(2026, 6, 1)),
        )
        .await;
        insert_news_item_with(
            &db,
            "https://example.invalid/news/two",
            "Market overview",
            Some("sample sector shares rise"),
            at_noon(ymd(2026, 6, 1)),
        )
        .await;
        insert_news_item_with(
            &db,
            "https://example.invalid/news/three",
            "Unrelated update",
            None,
            at_noon(ymd(2026, 6, 1)),
        )
        .await;

        let result = server
            .search_news(
                strategy_id,
                SearchNewsParams {
                    keyword: Some("example ventures".into()),
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect("search_news");

        assert_eq!(
            normalize_result(result),
            SearchNewsResult {
                items: vec![expected_item(
                    "https://example.invalid/news/one",
                    "EXAMPLE VENTURES earnings update",
                    None,
                    at_noon(ymd(2026, 6, 1)),
                )],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_news_does_not_treat_underscore_as_single_char_wildcard(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = build_server(db.clone());
        let strategy_id = insert_strategy(&db, "sample").await;

        insert_news_item_with(
            &db,
            "https://example.invalid/news/one",
            "AXB",
            None,
            at_noon(ymd(2026, 6, 1)),
        )
        .await;

        let result = server
            .search_news(
                strategy_id,
                SearchNewsParams {
                    keyword: Some("A_B".into()),
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect("search_news");

        assert_eq!(
            normalize_result(result),
            SearchNewsResult { items: Vec::new() }
        );
    }

    #[backend_test_macros::database_test]
    async fn search_news_filters_by_published_at_range_inclusive(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = build_server(db.clone());
        let strategy_id = insert_strategy(&db, "sample").await;

        insert_news_item_with(
            &db,
            "https://example.invalid/news/one",
            "Day one",
            None,
            at_noon(ymd(2026, 6, 1)),
        )
        .await;
        insert_news_item_with(
            &db,
            "https://example.invalid/news/five",
            "Day five",
            None,
            at_noon(ymd(2026, 6, 5)),
        )
        .await;
        insert_news_item_with(
            &db,
            "https://example.invalid/news/ten",
            "Day ten",
            None,
            at_noon(ymd(2026, 6, 10)),
        )
        .await;

        let result = server
            .search_news(
                strategy_id,
                SearchNewsParams {
                    keyword: None,
                    from: Some(ymd(2026, 6, 5)),
                    to: Some(ymd(2026, 6, 5)),
                    limit: None,
                },
            )
            .await
            .expect("search_news");

        assert_eq!(
            normalize_result(result),
            SearchNewsResult {
                items: vec![expected_item(
                    "https://example.invalid/news/five",
                    "Day five",
                    None,
                    at_noon(ymd(2026, 6, 5)),
                )],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_news_orders_newest_first_and_respects_limit(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = build_server(db.clone());
        let strategy_id = insert_strategy(&db, "sample").await;

        insert_news_item_with(
            &db,
            "https://example.invalid/news/one",
            "Day one",
            None,
            at_noon(ymd(2026, 6, 1)),
        )
        .await;
        insert_news_item_with(
            &db,
            "https://example.invalid/news/two",
            "Day two",
            None,
            at_noon(ymd(2026, 6, 2)),
        )
        .await;
        insert_news_item_with(
            &db,
            "https://example.invalid/news/three",
            "Day three",
            None,
            at_noon(ymd(2026, 6, 3)),
        )
        .await;

        let result = server
            .search_news(
                strategy_id,
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
            normalize_result(result),
            SearchNewsResult {
                items: vec![
                    expected_item(
                        "https://example.invalid/news/three",
                        "Day three",
                        None,
                        at_noon(ymd(2026, 6, 3)),
                    ),
                    expected_item(
                        "https://example.invalid/news/two",
                        "Day two",
                        None,
                        at_noon(ymd(2026, 6, 2)),
                    ),
                ],
            },
        );
    }
}
