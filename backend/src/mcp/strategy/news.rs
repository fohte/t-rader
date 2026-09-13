//! 戦略実行 MCP の `read_news` tool。
//!
//! `news_strategy_link` (interest 一致ごとの match) を `checkpoint` テーブルの
//! high-water-mark (`news_strategy_link.seq`) で追跡し、前回呼び出し以降に増えた分だけを
//! 返す。`checkpoint` は news に限らず任意の外部ストリームの読み進め位置を持てる汎用
//! テーブルで、本 tool は `stream = "news"` の行だけを使う。
//! 返した分だけ checkpoint を進めるため、呼び出し間隔が空いても取りこぼさない。

use chrono::{DateTime, FixedOffset, NaiveDate};
use rmcp::ErrorData as McpError;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::{Expr, OnConflict};
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::entities::{checkpoint, news_item, news_strategy_link};
use crate::handlers::refs::sanitize_like;

use super::dto::{
    NewsItemDto, NewsUpdateDto, ReadNewsParams, ReadNewsResult, SearchNewsParams, SearchNewsResult,
};
use super::{StrategyServer, clamp_limit, db_error, ensure_strategy_exists, internal_error};

/// checkpoint.graph は agent graph 単位でスコープ分離するためのフィールドだが、MCP 層は
/// どの graph からの呼び出しかを受け取っていないため、read_news では空文字で運用する。
const CHECKPOINT_GRAPH: &str = "";
const NEWS_STREAM: &str = "news";

impl StrategyServer {
    pub(crate) async fn read_news_inner(
        &self,
        session_strategy_id: Uuid,
        execution_id: Option<String>,
        params: ReadNewsParams,
    ) -> Result<ReadNewsResult, McpError> {
        ensure_strategy_exists(&self.db, session_strategy_id).await?;

        let existing_checkpoint = checkpoint::Entity::find()
            .filter(checkpoint::Column::StrategyId.eq(session_strategy_id))
            .filter(checkpoint::Column::Graph.eq(CHECKPOINT_GRAPH))
            .filter(checkpoint::Column::Stream.eq(NEWS_STREAM))
            .one(&self.db)
            .await
            .map_err(db_error)?;

        let cursor: i64 = match &existing_checkpoint {
            Some(row) => row.cursor.parse().map_err(|_| {
                internal_error(format!("invalid checkpoint cursor: {}", row.cursor))
            })?,
            // 識別列は 1 から始まるため、行が無ければ「全件未読」を意味する 0 を使う。
            None => 0,
        };

        let limit = clamp_limit(params.limit);

        let mut rows = news_strategy_link::Entity::find()
            .filter(news_strategy_link::Column::StrategyId.eq(session_strategy_id))
            .filter(news_strategy_link::Column::Seq.gt(cursor))
            .order_by_asc(news_strategy_link::Column::Seq)
            .limit(limit + 1)
            .find_also_related(news_item::Entity)
            .all(&self.db)
            .await
            .map_err(db_error)?;

        let has_more = rows.len() as u64 > limit;
        rows.truncate(limit as usize);

        let mut items = Vec::with_capacity(rows.len());
        for (link, news) in &rows {
            let news = news
                .as_ref()
                .ok_or_else(|| internal_error("news_strategy_link row missing its news_item"))?;
            items.push(NewsUpdateDto {
                id: news.id,
                source: news.source.clone(),
                url: news.url.clone(),
                title: news.title.clone(),
                body_snippet: news.body_snippet.clone(),
                published_at: news.published_at,
                ref_kind: link.ref_kind.clone(),
                ref_id: link.ref_id.clone(),
                matched_term: link.matched_term.clone(),
            });
        }

        if let Some((last_link, _)) = rows.last() {
            let new_cursor = last_link.seq;
            checkpoint::Entity::insert(checkpoint::ActiveModel {
                id: NotSet,
                strategy_id: Set(session_strategy_id),
                graph: Set(CHECKPOINT_GRAPH.to_string()),
                stream: Set(NEWS_STREAM.to_string()),
                cursor: Set(new_cursor.to_string()),
                updated_by_run_id: Set(execution_id),
                created_at: NotSet,
                updated_at: Set(chrono::Utc::now().fixed_offset()),
            })
            .on_conflict(
                OnConflict::columns([
                    checkpoint::Column::StrategyId,
                    checkpoint::Column::Graph,
                    checkpoint::Column::Stream,
                ])
                .update_columns([
                    checkpoint::Column::Cursor,
                    checkpoint::Column::UpdatedByRunId,
                    checkpoint::Column::UpdatedAt,
                ])
                // 同一戦略への read_news 同時呼び出しで、後勝ちの書き込みが先勝ちの進んだ
                // cursor を巻き戻さないようにする (巻き戻ると次回呼び出しで再配信が起きる)。
                .action_and_where(Expr::cust(format!(
                    "checkpoint.cursor::bigint < {new_cursor}"
                )))
                .to_owned(),
            )
            .exec_without_returning(&self.db)
            .await
            .map_err(db_error)?;
        }

        Ok(ReadNewsResult { items, has_more })
    }

    /// news_item を title/body_snippet のキーワードと published_at の期間で直接検索する。
    /// `news_strategy_link` を経由しないため、strategy_interest への語の登録有無に結果は
    /// 左右されない。
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
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter};
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::{checkpoint, news_item, news_strategy_link};
    use crate::testing::create_test_db;

    use super::super::dto::{
        NewsItemDto, ReadNewsParams, ReadNewsResult, SearchNewsParams, SearchNewsResult,
    };
    use super::super::tests_common::{build_server, insert_strategy};

    fn ts(secs: i64) -> DateTime<FixedOffset> {
        DateTime::from_timestamp(secs, 0)
            .expect("valid ts")
            .fixed_offset()
    }

    async fn insert_news_item(db: &DatabaseConnection, url: &str) -> Uuid {
        insert_news_item_with(db, url, &format!("title-{url}"), None, ts(1)).await
    }

    async fn insert_link(
        db: &DatabaseConnection,
        news_id: Uuid,
        strategy_id: Uuid,
        ref_kind: &str,
        ref_id: &str,
        matched_term: &str,
    ) {
        news_strategy_link::ActiveModel {
            news_id: Set(news_id),
            strategy_id: Set(strategy_id),
            ref_kind: Set(ref_kind.into()),
            ref_id: Set(ref_id.into()),
            matched_term: Set(matched_term.into()),
            created_at: Set(ts(1)),
            seq: NotSet,
        }
        .insert(db)
        .await
        .expect("insert link");
    }

    fn result_ref_ids(result: &ReadNewsResult) -> Vec<(String, String)> {
        result
            .items
            .iter()
            .map(|i| (i.url.clone(), i.ref_id.clone()))
            .collect()
    }

    #[sqlx::test(migrations = false)]
    async fn read_news_first_call_returns_all_links_oldest_first_for_own_strategy(pool: PgPool) {
        let db = create_test_db(pool).await;
        let a = insert_strategy(&db, "a").await;
        let b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());

        let news1 = insert_news_item(&db, "https://ex.com/1").await;
        let news2 = insert_news_item(&db, "https://ex.com/2").await;
        insert_link(&db, news1, a, "stock", "7203", "トヨタ").await;
        insert_link(&db, news2, a, "theme", "semi", "半導体").await;
        let news3 = insert_news_item(&db, "https://ex.com/3").await;
        insert_link(&db, news3, b, "stock", "9984", "ソフトバンク").await;

        let result = server
            .read_news_inner(a, None, ReadNewsParams { limit: None })
            .await
            .expect("read_news");

        assert_eq!(
            result_ref_ids(&result),
            vec![
                ("https://ex.com/1".to_string(), "7203".to_string()),
                ("https://ex.com/2".to_string(), "semi".to_string()),
            ],
        );
        assert!(!result.has_more);
    }

    #[sqlx::test(migrations = false)]
    async fn read_news_second_call_only_returns_links_added_after_first_call(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        let server = build_server(db.clone());

        let news1 = insert_news_item(&db, "https://ex.com/1").await;
        insert_link(&db, news1, sid, "stock", "7203", "トヨタ").await;
        let news2 = insert_news_item(&db, "https://ex.com/2").await;
        insert_link(&db, news2, sid, "stock", "9984", "ソフトバンク").await;

        let first = server
            .read_news_inner(sid, None, ReadNewsParams { limit: None })
            .await
            .expect("first read_news");
        assert_eq!(
            result_ref_ids(&first),
            vec![
                ("https://ex.com/1".to_string(), "7203".to_string()),
                ("https://ex.com/2".to_string(), "9984".to_string()),
            ],
        );

        let news3 = insert_news_item(&db, "https://ex.com/3").await;
        insert_link(&db, news3, sid, "theme", "semi", "半導体").await;

        let second = server
            .read_news_inner(sid, None, ReadNewsParams { limit: None })
            .await
            .expect("second read_news");
        assert_eq!(
            result_ref_ids(&second),
            vec![("https://ex.com/3".to_string(), "semi".to_string())],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn read_news_limit_paginates_without_gaps_or_duplicates(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        let server = build_server(db.clone());

        for i in 1..=5 {
            let url = format!("https://ex.com/{i}");
            let news_id = insert_news_item(&db, &url).await;
            insert_link(&db, news_id, sid, "stock", &i.to_string(), "term").await;
        }

        let mut seen = Vec::new();
        let mut has_more = true;
        let mut calls = 0;
        while has_more {
            calls += 1;
            assert!(calls <= 10, "too many calls, pagination likely looping");
            let result = server
                .read_news_inner(sid, None, ReadNewsParams { limit: Some(2) })
                .await
                .expect("read_news page");
            seen.extend(result_ref_ids(&result));
            has_more = result.has_more;
        }

        assert_eq!(
            seen,
            vec![
                ("https://ex.com/1".to_string(), "1".to_string()),
                ("https://ex.com/2".to_string(), "2".to_string()),
                ("https://ex.com/3".to_string(), "3".to_string()),
                ("https://ex.com/4".to_string(), "4".to_string()),
                ("https://ex.com/5".to_string(), "5".to_string()),
            ],
        );
        assert_eq!(calls, 3);
    }

    #[sqlx::test(migrations = false)]
    async fn read_news_returns_one_row_per_interest_match(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        let server = build_server(db.clone());

        let news_id = insert_news_item(&db, "https://ex.com/both").await;
        insert_link(&db, news_id, sid, "stock", "7203", "トヨタ").await;
        insert_link(&db, news_id, sid, "theme", "semi", "半導体").await;

        let result = server
            .read_news_inner(sid, None, ReadNewsParams { limit: None })
            .await
            .expect("read_news");

        assert_eq!(
            result_ref_ids(&result),
            vec![
                ("https://ex.com/both".to_string(), "7203".to_string()),
                ("https://ex.com/both".to_string(), "semi".to_string()),
            ],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn read_news_empty_result_does_not_create_checkpoint_row(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        let server = build_server(db.clone());

        let result = server
            .read_news_inner(sid, None, ReadNewsParams { limit: None })
            .await
            .expect("read_news");

        assert_eq!(
            result,
            ReadNewsResult {
                items: vec![],
                has_more: false,
            },
        );

        let checkpoint_count = checkpoint::Entity::find()
            .filter(checkpoint::Column::StrategyId.eq(sid))
            .count(&db)
            .await
            .expect("count checkpoints");
        assert_eq!(checkpoint_count, 0);
    }

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

    #[sqlx::test(migrations = false)]
    async fn search_news_ignores_news_strategy_link_entirely(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        let server = build_server(db.clone());

        insert_news_item_with(
            &db,
            "https://ex.com/1",
            "未登録の新語について",
            None,
            at_noon(ymd(2026, 6, 1)),
        )
        .await;

        let link_count = news_strategy_link::Entity::find()
            .count(&db)
            .await
            .expect("count links");
        assert_eq!(link_count, 0);

        let result = server
            .search_news_inner(
                sid,
                SearchNewsParams {
                    keyword: None,
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect("search_news");

        assert_eq!(result_urls(&result), vec!["https://ex.com/1".to_string()]);
    }
}
