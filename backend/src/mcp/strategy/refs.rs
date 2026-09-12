//! 戦略実行 MCP の参照型横断検索 tool。
//!
//! 一級参照型 (stock / indicator / sector / theme) はそれぞれ独立したテーブルに
//! 分かれており umbrella エンティティを持たない (プロジェクト方針)。横断検索は
//! この 4 テーブルを `UNION ALL` した raw SQL で行う。id / name いずれかへの部分一致
//! (大文字小文字を区別しない) で検索し、結果は `add_interest` にそのまま渡せる
//! `ref_kind` / `ref_id` の組で返す。

use indoc::indoc;
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{StrategyServer, clamp_limit, db_error, invalid_params};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchRefsParams {
    /// id または name の部分一致 (大文字小文字を区別しない) で検索する自由文字列
    pub query: String,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq, FromQueryResult)]
pub struct RefDto {
    /// 参照型 (`stock` / `indicator` / `sector` / `theme`)
    pub ref_kind: String,
    pub ref_id: String,
    pub name: String,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct SearchRefsResult {
    pub refs: Vec<RefDto>,
}

const SEARCH_REFS_SQL: &str = indoc! {"
    SELECT 'stock' AS ref_kind, id AS ref_id, name FROM stock WHERE id ILIKE $1 OR name ILIKE $1
    UNION ALL
    SELECT 'indicator', id, name FROM indicator WHERE id ILIKE $1 OR name ILIKE $1
    UNION ALL
    SELECT 'sector', id, name FROM sector WHERE id ILIKE $1 OR name ILIKE $1
    UNION ALL
    SELECT 'theme', id, name FROM theme WHERE id ILIKE $1 OR name ILIKE $1
    ORDER BY name, ref_kind
    LIMIT $2
"};

impl StrategyServer {
    /// 参照型 4 種 (stock / indicator / sector / theme) を横断して id / name の部分一致で
    /// 検索する。戦略スコープを持たないマスタデータのため `session_strategy_id` は使わない。
    pub(crate) async fn search_refs_inner(
        &self,
        _session_strategy_id: Uuid,
        params: SearchRefsParams,
    ) -> Result<SearchRefsResult, McpError> {
        let query = params.query.trim();
        if query.is_empty() {
            return Err(invalid_params("query must not be empty"));
        }
        let pattern = format!("%{query}%");
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
    use sea_orm::DatabaseConnection;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::{indicator, sector, stock, theme};
    use crate::testing::create_test_db;

    use super::{RefDto, SearchRefsParams, SearchRefsResult, StrategyServer};

    fn build_server(db: DatabaseConnection) -> StrategyServer {
        StrategyServer::new(db, None)
    }

    async fn seed_stock(db: &DatabaseConnection, id: &str, name: &str) {
        stock::ActiveModel {
            id: Set(id.into()),
            name: Set(name.into()),
            market: Set(None),
            sector_id: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("seed stock");
    }

    async fn seed_indicator(db: &DatabaseConnection, id: &str, name: &str) {
        indicator::ActiveModel {
            id: Set(id.into()),
            name: Set(name.into()),
            kind: Set("fx".into()),
        }
        .insert(db)
        .await
        .expect("seed indicator");
    }

    async fn seed_sector(db: &DatabaseConnection, id: &str, name: &str) {
        sector::ActiveModel {
            id: Set(id.into()),
            name: Set(name.into()),
        }
        .insert(db)
        .await
        .expect("seed sector");
    }

    async fn seed_theme(db: &DatabaseConnection, id: &str, name: &str) {
        theme::ActiveModel {
            id: Set(id.into()),
            name: Set(name.into()),
            description: Set(None),
        }
        .insert(db)
        .await
        .expect("seed theme");
    }

    #[sqlx::test(migrations = false)]
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
                    },
                    RefDto {
                        ref_kind: "sector".into(),
                        ref_id: "SEC1".into(),
                        name: "Alpha Sector".into(),
                    },
                    RefDto {
                        ref_kind: "stock".into(),
                        ref_id: "STK1".into(),
                        name: "Alpha Stock".into(),
                    },
                    RefDto {
                        ref_kind: "theme".into(),
                        ref_id: "THM1".into(),
                        name: "Alpha Theme".into(),
                    },
                ],
            },
        );
    }

    #[sqlx::test(migrations = false)]
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
                }],
            },
        );
    }

    #[sqlx::test(migrations = false)]
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
                }],
            },
        );
    }

    #[sqlx::test(migrations = false)]
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
                    },
                    RefDto {
                        ref_kind: "theme".into(),
                        ref_id: "t2".into(),
                        name: "Match B".into(),
                    },
                ],
            },
        );
    }

    #[sqlx::test(migrations = false)]
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
}
