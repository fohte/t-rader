//! 戦略実行 MCP の参照型横断検索 tool。
//!
//! 一級参照型 (stock / indicator / sector / theme) はそれぞれ独立したテーブルに
//! 分かれており umbrella エンティティを持たない (プロジェクト方針)。横断検索は
//! この 4 テーブルを `UNION ALL` した raw SQL で行う。id / name / `ref_term` の別名
//! いずれかへの部分一致 (大文字小文字・全角半角を区別しない) で検索し、`ref_kind` / `ref_id`
//! / `name` の組で返す。

use indoc::indoc;
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::handlers::refs::sanitize_like;
use crate::text_normalize::normalize;

use super::{StrategyServer, clamp_limit, db_error, invalid_params};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchRefsParams {
    /// id / name / 別名 (ref_term) の部分一致 (大文字小文字・全角半角を区別しない) で
    /// 検索する自由文字列
    pub query: String,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq, FromQueryResult)]
pub struct RefDto {
    /// 参照型 (`stock` / `indicator` / `sector` / `theme`)
    pub ref_kind: String,
    pub ref_id: String,
    pub name: String,
    /// J-Quants の商品区分コード (例: "014" = ETF)。`ref_kind` が `stock` 以外では常に None
    pub product_category: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct SearchRefsResult {
    pub refs: Vec<RefDto>,
}

const SEARCH_REFS_SQL: &str = indoc! {"
    SELECT 'stock' AS ref_kind, s.id AS ref_id, s.name, s.product_category
        FROM stock s
        WHERE normalize(s.id, NFKC) ILIKE $1
           OR normalize(s.name, NFKC) ILIKE $1
           OR EXISTS (
               SELECT 1 FROM ref_term t
               WHERE t.ref_kind = 'stock' AND t.ref_id = s.id
                 AND normalize(t.term, NFKC) ILIKE $1
           )
    UNION ALL
    SELECT 'indicator', i.id, i.name, NULL
        FROM indicator i
        WHERE normalize(i.id, NFKC) ILIKE $1
           OR normalize(i.name, NFKC) ILIKE $1
           OR EXISTS (
               SELECT 1 FROM ref_term t
               WHERE t.ref_kind = 'indicator' AND t.ref_id = i.id
                 AND normalize(t.term, NFKC) ILIKE $1
           )
    UNION ALL
    SELECT 'sector', se.id, se.name, NULL
        FROM sector se
        WHERE normalize(se.id, NFKC) ILIKE $1
           OR normalize(se.name, NFKC) ILIKE $1
           OR EXISTS (
               SELECT 1 FROM ref_term t
               WHERE t.ref_kind = 'sector' AND t.ref_id = se.id
                 AND normalize(t.term, NFKC) ILIKE $1
           )
    UNION ALL
    SELECT 'theme', th.id, th.name, NULL
        FROM theme th
        WHERE normalize(th.id, NFKC) ILIKE $1
           OR normalize(th.name, NFKC) ILIKE $1
           OR EXISTS (
               SELECT 1 FROM ref_term t
               WHERE t.ref_kind = 'theme' AND t.ref_id = th.id
                 AND normalize(t.term, NFKC) ILIKE $1
           )
    ORDER BY name, ref_kind
    LIMIT $2
"};

impl StrategyServer {
    /// 参照型 4 種 (stock / indicator / sector / theme) を横断して id / name / 別名
    /// (ref_term) の部分一致で検索する。戦略スコープを持たないマスタデータのため
    /// `session_strategy_id` は使わない。
    pub(crate) async fn search_refs_inner(
        &self,
        _session_strategy_id: Uuid,
        params: SearchRefsParams,
    ) -> Result<SearchRefsResult, McpError> {
        let query = params.query.trim();
        if query.is_empty() {
            return Err(invalid_params("query must not be empty"));
        }
        // 列側は SQL 内で normalize(NFKC) してから比較するため、入力側も先に
        // normalize してから sanitize_like に通す。順序を逆にすると全角の `％` `＿` が
        // NFKC で `%` `_` に変わり、ワイルドカードとして効いてしまう
        let pattern = format!("%{}%", sanitize_like(&normalize(query)));
        let limit = clamp_limit(params.limit) as i64;

        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                SEARCH_REFS_SQL,
                [pattern.into(), limit.into()],
            ))
            .await
            .map_err(db_error)?;

        let refs = rows
            .iter()
            .map(|row| RefDto::from_query_result(row, ""))
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_error)?;

        Ok(SearchRefsResult { refs })
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};

    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::{indicator, ref_term, sector, stock, theme};
    use crate::testing::create_test_db;

    use super::super::tests_common::build_server;
    use super::{RefDto, SearchRefsParams, SearchRefsResult};

    async fn seed_ref_term(
        db: &impl sea_orm::ConnectionTrait,
        ref_kind: &str,
        ref_id: &str,
        term: &str,
    ) {
        ref_term::ActiveModel {
            ref_kind: Set(ref_kind.into()),
            ref_id: Set(ref_id.into()),
            term: Set(term.into()),
            origin: Set("human".into()),
            created_at: NotSet,
        }
        .insert(db)
        .await
        .expect("seed ref_term");
    }

    async fn seed_stock(db: &impl sea_orm::ConnectionTrait, id: &str, name: &str) {
        seed_stock_with_product_category(db, id, name, None).await;
    }

    async fn seed_stock_with_product_category(
        db: &impl sea_orm::ConnectionTrait,
        id: &str,
        name: &str,
        product_category: Option<&str>,
    ) {
        stock::ActiveModel {
            id: Set(id.into()),
            name: Set(name.into()),
            market: Set(None),
            sector_id: Set(None),
            product_category: Set(product_category.map(str::to_string)),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("seed stock");
    }

    async fn seed_indicator(db: &impl sea_orm::ConnectionTrait, id: &str, name: &str) {
        indicator::ActiveModel {
            id: Set(id.into()),
            name: Set(name.into()),
            kind: Set("fx".into()),
        }
        .insert(db)
        .await
        .expect("seed indicator");
    }

    async fn seed_sector(db: &impl sea_orm::ConnectionTrait, id: &str, name: &str) {
        sector::ActiveModel {
            id: Set(id.into()),
            name: Set(name.into()),
        }
        .insert(db)
        .await
        .expect("seed sector");
    }

    async fn seed_theme(db: &impl sea_orm::ConnectionTrait, id: &str, name: &str) {
        theme::ActiveModel {
            id: Set(id.into()),
            name: Set(name.into()),
            description: Set(None),
        }
        .insert(db)
        .await
        .expect("seed theme");
    }

    #[backend_test_macros::database_test]
    async fn search_refs_matches_across_all_kinds_ordered_by_name(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_indicator(&db, "IND1", "Alpha Indicator").await;
        seed_sector(&db, "SEC1", "Alpha Sector").await;
        seed_stock(&db, "STK1", "Alpha Stock").await;
        seed_theme(&db, "THM1", "Alpha Theme").await;
        seed_stock(&db, "STK2", "Beta Stock").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(
                Uuid::new_v4(),
                SearchRefsParams {
                    query: "Alpha".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![
                    RefDto {
                        ref_kind: "indicator".into(),
                        ref_id: "IND1".into(),
                        name: "Alpha Indicator".into(),
                        product_category: None,
                    },
                    RefDto {
                        ref_kind: "sector".into(),
                        ref_id: "SEC1".into(),
                        name: "Alpha Sector".into(),
                        product_category: None,
                    },
                    RefDto {
                        ref_kind: "stock".into(),
                        ref_id: "STK1".into(),
                        name: "Alpha Stock".into(),
                        product_category: None,
                    },
                    RefDto {
                        ref_kind: "theme".into(),
                        ref_id: "THM1".into(),
                        name: "Alpha Theme".into(),
                        product_category: None,
                    },
                ],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_matches_by_id_substring(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_stock(&db, "TOY7203", "Something").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(
                Uuid::new_v4(),
                SearchRefsParams {
                    query: "7203".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![RefDto {
                    ref_kind: "stock".into(),
                    ref_id: "TOY7203".into(),
                    name: "Something".into(),
                    product_category: None,
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_is_case_insensitive(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_sector(&db, "semi", "Semiconductors").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(
                Uuid::new_v4(),
                SearchRefsParams {
                    query: "semicon".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![RefDto {
                    ref_kind: "sector".into(),
                    ref_id: "semi".into(),
                    name: "Semiconductors".into(),
                    product_category: None,
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_does_not_treat_underscore_as_single_char_wildcard(pool: PgPool) {
        let db = create_test_db(pool).await;
        // "_" は ILIKE の単一文字ワイルドカードなので、素通しすると "AXB" が
        // "A_B" にマッチしてしまう。sanitize_like で除去され、マッチしないことを確認する。
        seed_theme(&db, "u1", "AXB").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(
                Uuid::new_v4(),
                SearchRefsParams {
                    query: "A_B".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(result, SearchRefsResult { refs: vec![] });
    }

    #[backend_test_macros::database_test]
    async fn search_refs_respects_limit_after_ordering(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_theme(&db, "t1", "Match A").await;
        seed_theme(&db, "t2", "Match B").await;
        seed_theme(&db, "t3", "Match C").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(
                Uuid::new_v4(),
                SearchRefsParams {
                    query: "Match".into(),
                    limit: Some(2),
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![
                    RefDto {
                        ref_kind: "theme".into(),
                        ref_id: "t1".into(),
                        name: "Match A".into(),
                        product_category: None,
                    },
                    RefDto {
                        ref_kind: "theme".into(),
                        ref_id: "t2".into(),
                        name: "Match B".into(),
                        product_category: None,
                    },
                ],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_includes_stock_product_category(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_stock_with_product_category(&db, "ETF1", "Alpha ETF", Some("014")).await;
        seed_stock(&db, "STK1", "Alpha Stock").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(
                Uuid::new_v4(),
                SearchRefsParams {
                    query: "Alpha".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![
                    RefDto {
                        ref_kind: "stock".into(),
                        ref_id: "ETF1".into(),
                        name: "Alpha ETF".into(),
                        product_category: Some("014".into()),
                    },
                    RefDto {
                        ref_kind: "stock".into(),
                        ref_id: "STK1".into(),
                        name: "Alpha Stock".into(),
                        product_category: None,
                    },
                ],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_rejects_empty_query(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db);

        let err = server
            .search_refs_inner(
                Uuid::new_v4(),
                SearchRefsParams {
                    query: "   ".into(),
                    limit: None,
                },
            )
            .await
            .expect_err("empty query");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn search_refs_matches_full_width_query_against_half_width_name(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_stock(&db, "STK1", "Alpha Motors").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(
                Uuid::new_v4(),
                SearchRefsParams {
                    query: "Ａｌｐｈａ".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![RefDto {
                    ref_kind: "stock".into(),
                    ref_id: "STK1".into(),
                    name: "Alpha Motors".into(),
                    product_category: None,
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_matches_full_width_query_against_half_width_id(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_indicator(&db, "USDJPY", "US Dollar / Japanese Yen").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(
                Uuid::new_v4(),
                SearchRefsParams {
                    query: "ＵＳＤＪＰＹ".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![RefDto {
                    ref_kind: "indicator".into(),
                    ref_id: "USDJPY".into(),
                    name: "US Dollar / Japanese Yen".into(),
                    product_category: None,
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_does_not_treat_full_width_underscore_as_wildcard(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_theme(&db, "u1", "AXB").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(
                Uuid::new_v4(),
                SearchRefsParams {
                    query: "Ａ＿Ｂ".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(result, SearchRefsResult { refs: vec![] });
    }

    #[backend_test_macros::database_test]
    async fn search_refs_matches_ref_term_alias(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_stock(&db, "7203", "Alpha Motors").await;
        seed_ref_term(&db, "stock", "7203", "Ａｌｐｈａ Ｍｏｔｏｒｓ Ｇｒｏｕｐ").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(
                Uuid::new_v4(),
                SearchRefsParams {
                    query: "motors group".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![RefDto {
                    ref_kind: "stock".into(),
                    ref_id: "7203".into(),
                    name: "Alpha Motors".into(),
                    product_category: None,
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_returns_one_row_when_both_name_and_alias_match(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_stock(&db, "7203", "Alpha Motors").await;
        seed_ref_term(&db, "stock", "7203", "Alpha Auto").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(
                Uuid::new_v4(),
                SearchRefsParams {
                    query: "Alpha".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(
            result,
            SearchRefsResult {
                refs: vec![RefDto {
                    ref_kind: "stock".into(),
                    ref_id: "7203".into(),
                    name: "Alpha Motors".into(),
                    product_category: None,
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn search_refs_ignores_dangling_alias_not_in_master(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_ref_term(&db, "stock", "9999", "Ghost Co").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(
                Uuid::new_v4(),
                SearchRefsParams {
                    query: "Ghost".into(),
                    limit: None,
                },
            )
            .await
            .expect("search_refs");

        assert_eq!(result, SearchRefsResult { refs: vec![] });
    }
}
