//! 戦略実行 MCP の参照型横断検索 tool。
//!
//! 一級参照型 (stock / indicator / sector / theme) はそれぞれ独立したテーブルに
//! 分かれており umbrella エンティティを持たない (プロジェクト方針)。横断検索は
//! この 4 テーブルを `UNION ALL` した raw SQL で行う。id / name / `ref_term` の別名
//! いずれかへの部分一致 (大文字小文字・全角半角を区別しない) で検索し、`ref_kind` / `ref_id`
//! / `name` の組で返す。

use core_application::refs::{RefRepositoryError, RefSearchMatch, RefUseCaseError};
use core_application::unit_of_work::UnitOfWorkError;
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{StrategyServer, clamp_limit, internal_error, invalid_params};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchRefsParams {
    /// id / name / 別名 (ref_term) の部分一致 (大文字小文字・全角半角を区別しない) で
    /// 検索する自由文字列
    pub query: String,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct RefDto {
    /// 参照型 (`stock` / `indicator` / `sector` / `theme`)
    pub ref_kind: String,
    pub ref_id: String,
    pub name: String,
    /// J-Quants の商品区分コード (例: "014" = ETF)。`ref_kind` が `stock` 以外では常に None
    pub product_category: Option<String>,
}

impl From<RefSearchMatch> for RefDto {
    fn from(reference: RefSearchMatch) -> Self {
        Self {
            ref_kind: reference.ref_kind,
            ref_id: reference.ref_id,
            name: reference.name,
            product_category: reference.product_category,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct SearchRefsResult {
    pub refs: Vec<RefDto>,
}

impl StrategyServer {
    /// 参照型 4 種 (stock / indicator / sector / theme) を横断して id / name / 別名
    /// (ref_term) の部分一致で検索する。戦略スコープを持たないマスタデータのため
    /// `session_strategy_id` は使わない。
    pub(crate) async fn search_refs_inner(
        &self,
        params: SearchRefsParams,
    ) -> Result<SearchRefsResult, McpError> {
        let refs = self
            .use_cases
            .refs()
            .search_all(&params.query, clamp_limit(params.limit))
            .await
            .map_err(ref_use_case_error)?
            .into_iter()
            .map(RefDto::from)
            .collect();

        Ok(SearchRefsResult { refs })
    }
}

fn ref_use_case_error(error: RefUseCaseError) -> McpError {
    match error {
        RefUseCaseError::Validation(message) => invalid_params(message),
        error @ (RefUseCaseError::Repository(RefRepositoryError::Database(_))
        | RefUseCaseError::UnitOfWork(UnitOfWorkError::Begin(_))
        | RefUseCaseError::UnitOfWork(UnitOfWorkError::Commit(_))) => {
            tracing::error!(error = %error, "strategy mcp refs failed");
            match error {
                RefUseCaseError::Repository(RefRepositoryError::Database(error))
                | RefUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
                | RefUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error)) => {
                    internal_error(format!("database error: {error}"))
                }
                _ => internal_error("unexpected reference error"),
            }
        }
        error @ (RefUseCaseError::Repository(RefRepositoryError::InvalidTransaction)
        | RefUseCaseError::UnitOfWork(UnitOfWorkError::InvalidTransaction)) => {
            tracing::error!(error = %error, "strategy mcp refs failed");
            internal_error("reference transaction has an unexpected type")
        }
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};

    use super::super::tests_common::build_server;
    use super::{RefDto, SearchRefsParams, SearchRefsResult};
    use gateway_postgres::entities::{indicator, ref_term, sector, stock, theme};

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
    async fn search_refs_matches_across_all_kinds_ordered_by_name(
        db: gateway_postgres::DatabaseHandle,
    ) {
        seed_indicator(&db, "IND1", "Alpha Indicator").await;
        seed_sector(&db, "SEC1", "Alpha Sector").await;
        seed_stock(&db, "STK1", "Alpha Stock").await;
        seed_theme(&db, "THM1", "Alpha Theme").await;
        seed_stock(&db, "STK2", "Beta Stock").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(SearchRefsParams {
                query: "Alpha".into(),
                limit: None,
            })
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
    async fn search_refs_matches_by_id_substring(db: gateway_postgres::DatabaseHandle) {
        seed_stock(&db, "TOY7203", "Something").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(SearchRefsParams {
                query: "7203".into(),
                limit: None,
            })
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
    async fn search_refs_is_case_insensitive(db: gateway_postgres::DatabaseHandle) {
        seed_sector(&db, "semi", "Semiconductors").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(SearchRefsParams {
                query: "semicon".into(),
                limit: None,
            })
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
    async fn search_refs_does_not_treat_underscore_as_single_char_wildcard(
        db: gateway_postgres::DatabaseHandle,
    ) {
        // "_" は ILIKE の単一文字ワイルドカードなので、素通しすると "AXB" が
        // "A_B" にマッチしてしまう。sanitize_like で除去され、マッチしないことを確認する。
        seed_theme(&db, "u1", "AXB").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(SearchRefsParams {
                query: "A_B".into(),
                limit: None,
            })
            .await
            .expect("search_refs");

        assert_eq!(result, SearchRefsResult { refs: vec![] });
    }

    #[backend_test_macros::database_test]
    async fn search_refs_respects_limit_after_ordering(db: gateway_postgres::DatabaseHandle) {
        seed_theme(&db, "t1", "Match A").await;
        seed_theme(&db, "t2", "Match B").await;
        seed_theme(&db, "t3", "Match C").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(SearchRefsParams {
                query: "Match".into(),
                limit: Some(2),
            })
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
    async fn search_refs_includes_stock_product_category(db: gateway_postgres::DatabaseHandle) {
        seed_stock_with_product_category(&db, "ETF1", "Alpha ETF", Some("014")).await;
        seed_stock(&db, "STK1", "Alpha Stock").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(SearchRefsParams {
                query: "Alpha".into(),
                limit: None,
            })
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
    async fn search_refs_rejects_empty_query(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db);

        let err = server
            .search_refs_inner(SearchRefsParams {
                query: "   ".into(),
                limit: None,
            })
            .await
            .expect_err("empty query");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn search_refs_matches_full_width_query_against_half_width_name(
        db: gateway_postgres::DatabaseHandle,
    ) {
        seed_stock(&db, "STK1", "Alpha Motors").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(SearchRefsParams {
                query: "Ａｌｐｈａ".into(),
                limit: None,
            })
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
    async fn search_refs_matches_full_width_query_against_half_width_id(
        db: gateway_postgres::DatabaseHandle,
    ) {
        seed_indicator(&db, "USDJPY", "US Dollar / Japanese Yen").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(SearchRefsParams {
                query: "ＵＳＤＪＰＹ".into(),
                limit: None,
            })
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
    async fn search_refs_does_not_treat_full_width_underscore_as_wildcard(
        db: gateway_postgres::DatabaseHandle,
    ) {
        seed_theme(&db, "u1", "AXB").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(SearchRefsParams {
                query: "Ａ＿Ｂ".into(),
                limit: None,
            })
            .await
            .expect("search_refs");

        assert_eq!(result, SearchRefsResult { refs: vec![] });
    }

    #[backend_test_macros::database_test]
    async fn search_refs_matches_ref_term_alias(db: gateway_postgres::DatabaseHandle) {
        seed_stock(&db, "7203", "Alpha Motors").await;
        seed_ref_term(&db, "stock", "7203", "Ａｌｐｈａ Ｍｏｔｏｒｓ Ｇｒｏｕｐ").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(SearchRefsParams {
                query: "motors group".into(),
                limit: None,
            })
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
    async fn search_refs_returns_one_row_when_both_name_and_alias_match(
        db: gateway_postgres::DatabaseHandle,
    ) {
        seed_stock(&db, "7203", "Alpha Motors").await;
        seed_ref_term(&db, "stock", "7203", "Alpha Auto").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(SearchRefsParams {
                query: "Alpha".into(),
                limit: None,
            })
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
    async fn search_refs_ignores_dangling_alias_not_in_master(
        db: gateway_postgres::DatabaseHandle,
    ) {
        seed_ref_term(&db, "stock", "9999", "Ghost Co").await;
        let server = build_server(db);

        let result = server
            .search_refs_inner(SearchRefsParams {
                query: "Ghost".into(),
                limit: None,
            })
            .await
            .expect("search_refs");

        assert_eq!(result, SearchRefsResult { refs: vec![] });
    }
}
