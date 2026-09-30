//! 戦略実行 MCP の参照型別名 (ref_term) 追加/削除 tool。
//!
//! 一級参照型 (stock/indicator/group) には正規化では吸収できない表記揺れや
//! 別称 (旧社名、指数と構成銘柄の関係等) があり、コードの規則では導出できない。
//! そのため語をデータとして LLM または人間が直接登録・削除する。同じ語が複数の
//! 参照に当たること (例: Apple が銘柄にも指数の構成銘柄としても出る) は正常な状態
//! として許容し、曖昧さの解消はコード側で防がず語を消して直すことに委ねるため、
//! 削除 tool を追加時から用意する。
//!
//! `search_refs` と同様、戦略に属さないマスタデータのため `session_strategy_id` は
//! 使わない。

use core_application::refs::{RefRepositoryError, RefUseCaseError};
use core_application::unit_of_work::UnitOfWorkError;
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{StrategyServer, internal_error, invalid_params};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddRefTermsParams {
    /// 参照型 (`stock` / `indicator` / `group`)
    pub ref_kind: String,
    pub ref_id: String,
    pub terms: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct AddRefTermsResult {
    /// 新規に追加された語 (既に登録済みだった語や空文字は含まない)
    pub added: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoveRefTermsParams {
    pub ref_kind: String,
    pub ref_id: String,
    pub terms: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct RemoveRefTermsResult {
    /// 実際に削除された語 (登録されていなかった語は含まない)
    pub removed: Vec<String>,
}

impl StrategyServer {
    /// 別名を追加する。同じ (ref_kind, ref_id, term) が既にあれば idempotent に無視する。
    pub(crate) async fn add_ref_terms_inner(
        &self,
        params: AddRefTermsParams,
    ) -> Result<AddRefTermsResult, McpError> {
        let added = self
            .use_cases
            .refs()
            .add_terms(&params.ref_kind, &params.ref_id, &params.terms)
            .await
            .map_err(ref_terms_error)?;

        Ok(AddRefTermsResult { added })
    }

    /// 別名を削除する。登録されていない語を渡しても idempotent に無視する。
    pub(crate) async fn remove_ref_terms_inner(
        &self,
        params: RemoveRefTermsParams,
    ) -> Result<RemoveRefTermsResult, McpError> {
        let removed = self
            .use_cases
            .refs()
            .remove_terms(&params.ref_kind, &params.ref_id, &params.terms)
            .await
            .map_err(ref_terms_error)?;

        Ok(RemoveRefTermsResult { removed })
    }
}

fn ref_terms_error(error: RefUseCaseError) -> McpError {
    match error {
        RefUseCaseError::Validation(message) => invalid_params(message),
        error @ (RefUseCaseError::Repository(RefRepositoryError::Database(_))
        | RefUseCaseError::UnitOfWork(UnitOfWorkError::Begin(_))
        | RefUseCaseError::UnitOfWork(UnitOfWorkError::Commit(_))) => {
            tracing::error!(error = %error, "strategy mcp ref terms failed");
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
            tracing::error!(error = %error, "strategy mcp ref terms failed");
            internal_error("reference transaction has an unexpected type")
        }
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::EntityTrait;

    use super::super::tests_common::build_server;
    use super::{AddRefTermsParams, RemoveRefTermsParams};
    use gateway_postgres::entities::ref_term;

    async fn seed_term(
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

    #[backend_test_macros::database_test]
    async fn add_ref_terms_inserts_new_terms(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db);

        let result = server
            .add_ref_terms_inner(AddRefTermsParams {
                ref_kind: "stock".into(),
                ref_id: "7203".into(),
                terms: vec!["トヨタ".into(), "Toyota".into()],
            })
            .await
            .expect("add_ref_terms");

        assert_eq!(
            result.added,
            vec!["トヨタ".to_string(), "Toyota".to_string()],
        );
    }

    #[backend_test_macros::database_test]
    async fn add_ref_terms_is_idempotent_and_skips_blank_terms(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = build_server(db.clone());
        seed_term(&db, "stock", "7203", "トヨタ").await;

        let result = server
            .add_ref_terms_inner(AddRefTermsParams {
                ref_kind: "stock".into(),
                ref_id: "7203".into(),
                terms: vec!["トヨタ".into(), "  ".into(), "Toyota".into()],
            })
            .await
            .expect("add_ref_terms");

        assert_eq!(result.added, vec!["Toyota".to_string()]);
    }

    #[backend_test_macros::database_test]
    async fn add_ref_terms_rejects_invalid_ref_kind(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db);

        let err = server
            .add_ref_terms_inner(AddRefTermsParams {
                ref_kind: "bogus".into(),
                ref_id: "7203".into(),
                terms: vec!["トヨタ".into()],
            })
            .await
            .expect_err("invalid kind");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn add_ref_terms_rejects_empty_ref_id(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db);

        let err = server
            .add_ref_terms_inner(AddRefTermsParams {
                ref_kind: "stock".into(),
                ref_id: "  ".into(),
                terms: vec!["トヨタ".into()],
            })
            .await
            .expect_err("empty ref_id");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn remove_ref_terms_deletes_only_matching_terms(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db.clone());
        seed_term(&db, "stock", "7203", "トヨタ").await;
        seed_term(&db, "stock", "7203", "Toyota").await;
        seed_term(&db, "stock", "9984", "トヨタ").await;

        let result = server
            .remove_ref_terms_inner(RemoveRefTermsParams {
                ref_kind: "stock".into(),
                ref_id: "7203".into(),
                terms: vec!["トヨタ".into(), "存在しない".into()],
            })
            .await
            .expect("remove_ref_terms");

        assert_eq!(result.removed, vec!["トヨタ".to_string()]);

        let remaining = ref_term::Entity::find()
            .all(&db)
            .await
            .expect("list remaining terms");
        assert_eq!(remaining.len(), 2);
    }

    #[backend_test_macros::database_test]
    async fn remove_ref_terms_rejects_invalid_ref_kind(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db);

        let err = server
            .remove_ref_terms_inner(RemoveRefTermsParams {
                ref_kind: "bogus".into(),
                ref_id: "7203".into(),
                terms: vec!["トヨタ".into()],
            })
            .await
            .expect_err("invalid kind");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}
