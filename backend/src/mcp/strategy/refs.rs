//! 戦略実行 MCP の参照型横断検索 tool。
//!
//! 一級参照型 (stock / indicator / group) はそれぞれ独立したテーブルに
//! 分かれており umbrella エンティティを持たない (プロジェクト方針)。横断検索は
//! 各型のテーブルを `UNION ALL` した raw SQL で行う。id / name / `ref_term` の別名
//! いずれかへの部分一致 (大文字小文字・全角半角を区別しない) で検索し、`ref_kind` / `ref_id`
//! / `name` の組で返す。

use core_application::refs::{RefRepositoryError, RefSearchMatch, RefUseCaseError};
use core_application::unit_of_work::UnitOfWorkError;
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{StrategyServer, clamp_limit, internal_error, invalid_params};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SearchRefsParams {
    /// id / name / 別名 (ref_term) の部分一致 (大文字小文字・全角半角を区別しない) で
    /// 検索する自由文字列
    pub query: String,
    pub limit: Option<u32>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct RefDto {
    /// 参照型 (`stock` / `indicator` / `group`)
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

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct SearchRefsResult {
    pub refs: Vec<RefDto>,
}

impl StrategyServer {
    /// 参照型 3 種 (stock / indicator / group) を横断して id / name / 別名
    /// (ref_term) の部分一致で検索する。戦略スコープを持たないマスタデータのため
    /// `session_strategy_id` は使わない。
    pub(crate) async fn search_refs_inner(
        &self,
        params: SearchRefsParams,
    ) -> Result<SearchRefsResult, McpError> {
        let refs = self
            .dependencies
            .refs
            .search_all(&params.query, clamp_limit(params.limit))
            .await
            .map_err(ref_use_case_error)?
            .into_iter()
            .map(RefDto::from)
            .collect();

        Ok(SearchRefsResult { refs })
    }
}

pub(super) fn ref_use_case_error(error: RefUseCaseError) -> McpError {
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
