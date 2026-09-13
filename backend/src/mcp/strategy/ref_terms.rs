//! 戦略実行 MCP の参照型別名 (ref_term) 追加/削除 tool。
//!
//! 一級参照型 (stock/indicator/sector/theme) には正規化では吸収できない表記揺れや
//! 別称 (旧社名、指数と構成銘柄の関係等) があり、コードの規則では導出できない。
//! そのため語をデータとして LLM または人間が直接登録・削除する。同じ語が複数の
//! 参照に当たること (例: Apple が銘柄にも指数の構成銘柄としても出る) は正常な状態
//! として許容し、曖昧さの解消はコード側で防がず語を消して直すことに委ねるため、
//! 削除 tool を追加時から用意する。
//!
//! `search_refs` と同様、戦略に属さないマスタデータのため `session_strategy_id` は
//! 使わない。

use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::ref_term;
use crate::services::interests::ensure_ref_kind;

use super::{StrategyServer, db_error, invalid_params};

/// 戦略 Agent が追加する別名の固定 origin。
const AGENT_TERM_ORIGIN: &str = "llm";

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddRefTermsParams {
    /// 参照型 (`stock` / `indicator` / `sector` / `theme`)
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

fn validation_to_mcp(err: crate::error::AppError) -> McpError {
    match err {
        crate::error::AppError::Validation(msg) => invalid_params(msg),
        other => invalid_params(format!("validation failed: {other}")),
    }
}

impl StrategyServer {
    /// 別名を追加する。同じ (ref_kind, ref_id, term) が既にあれば idempotent に無視する。
    pub(crate) async fn add_ref_terms_inner(
        &self,
        _session_strategy_id: Uuid,
        params: AddRefTermsParams,
    ) -> Result<AddRefTermsResult, McpError> {
        let ref_kind = params.ref_kind.trim();
        ensure_ref_kind(ref_kind).map_err(validation_to_mcp)?;
        let ref_id = params.ref_id.trim();
        if ref_id.is_empty() {
            return Err(invalid_params("ref_id must not be empty"));
        }

        let mut added = Vec::new();
        for term in &params.terms {
            let term = term.trim();
            if term.is_empty() {
                continue;
            }
            let model = ref_term::ActiveModel {
                ref_kind: Set(ref_kind.to_string()),
                ref_id: Set(ref_id.to_string()),
                term: Set(term.to_string()),
                origin: Set(AGENT_TERM_ORIGIN.to_string()),
                created_at: NotSet,
            };
            let result = ref_term::Entity::insert(model)
                .on_conflict(
                    OnConflict::columns([
                        ref_term::Column::RefKind,
                        ref_term::Column::RefId,
                        ref_term::Column::Term,
                    ])
                    .do_nothing()
                    .to_owned(),
                )
                .exec_without_returning(&self.db)
                .await
                .map_err(db_error)?;
            if result > 0 {
                added.push(term.to_string());
            }
        }

        Ok(AddRefTermsResult { added })
    }

    /// 別名を削除する。登録されていない語を渡しても idempotent に無視する。
    pub(crate) async fn remove_ref_terms_inner(
        &self,
        _session_strategy_id: Uuid,
        params: RemoveRefTermsParams,
    ) -> Result<RemoveRefTermsResult, McpError> {
        let ref_kind = params.ref_kind.trim();
        ensure_ref_kind(ref_kind).map_err(validation_to_mcp)?;
        let ref_id = params.ref_id.trim();
        if ref_id.is_empty() {
            return Err(invalid_params("ref_id must not be empty"));
        }

        let mut removed = Vec::new();
        for term in &params.terms {
            let term = term.trim();
            if term.is_empty() {
                continue;
            }
            let result = ref_term::Entity::delete_many()
                .filter(ref_term::Column::RefKind.eq(ref_kind))
                .filter(ref_term::Column::RefId.eq(ref_id))
                .filter(ref_term::Column::Term.eq(term))
                .exec(&self.db)
                .await
                .map_err(db_error)?;
            if result.rows_affected > 0 {
                removed.push(term.to_string());
            }
        }

        Ok(RemoveRefTermsResult { removed })
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{DatabaseConnection, EntityTrait};
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::ref_term;
    use crate::testing::create_test_db;

    use super::super::tests_common::build_server;
    use super::{AddRefTermsParams, RemoveRefTermsParams};

    async fn seed_term(db: &DatabaseConnection, ref_kind: &str, ref_id: &str, term: &str) {
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

    #[sqlx::test(migrations = false)]
    async fn add_ref_terms_inserts_new_terms(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db);

        let result = server
            .add_ref_terms_inner(
                Uuid::new_v4(),
                AddRefTermsParams {
                    ref_kind: "stock".into(),
                    ref_id: "7203".into(),
                    terms: vec!["トヨタ".into(), "Toyota".into()],
                },
            )
            .await
            .expect("add_ref_terms");

        assert_eq!(
            result.added,
            vec!["トヨタ".to_string(), "Toyota".to_string()],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn add_ref_terms_is_idempotent_and_skips_blank_terms(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());
        seed_term(&db, "stock", "7203", "トヨタ").await;

        let result = server
            .add_ref_terms_inner(
                Uuid::new_v4(),
                AddRefTermsParams {
                    ref_kind: "stock".into(),
                    ref_id: "7203".into(),
                    terms: vec!["トヨタ".into(), "  ".into(), "Toyota".into()],
                },
            )
            .await
            .expect("add_ref_terms");

        assert_eq!(result.added, vec!["Toyota".to_string()]);
    }

    #[sqlx::test(migrations = false)]
    async fn add_ref_terms_rejects_invalid_ref_kind(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db);

        let err = server
            .add_ref_terms_inner(
                Uuid::new_v4(),
                AddRefTermsParams {
                    ref_kind: "bogus".into(),
                    ref_id: "7203".into(),
                    terms: vec!["トヨタ".into()],
                },
            )
            .await
            .expect_err("invalid kind");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn add_ref_terms_rejects_empty_ref_id(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db);

        let err = server
            .add_ref_terms_inner(
                Uuid::new_v4(),
                AddRefTermsParams {
                    ref_kind: "stock".into(),
                    ref_id: "  ".into(),
                    terms: vec!["トヨタ".into()],
                },
            )
            .await
            .expect_err("empty ref_id");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn remove_ref_terms_deletes_only_matching_terms(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());
        seed_term(&db, "stock", "7203", "トヨタ").await;
        seed_term(&db, "stock", "7203", "Toyota").await;
        seed_term(&db, "stock", "9984", "トヨタ").await;

        let result = server
            .remove_ref_terms_inner(
                Uuid::new_v4(),
                RemoveRefTermsParams {
                    ref_kind: "stock".into(),
                    ref_id: "7203".into(),
                    terms: vec!["トヨタ".into(), "存在しない".into()],
                },
            )
            .await
            .expect("remove_ref_terms");

        assert_eq!(result.removed, vec!["トヨタ".to_string()]);

        let remaining = ref_term::Entity::find()
            .all(&db)
            .await
            .expect("list remaining terms");
        assert_eq!(remaining.len(), 2);
    }

    #[sqlx::test(migrations = false)]
    async fn remove_ref_terms_rejects_invalid_ref_kind(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db);

        let err = server
            .remove_ref_terms_inner(
                Uuid::new_v4(),
                RemoveRefTermsParams {
                    ref_kind: "bogus".into(),
                    ref_id: "7203".into(),
                    terms: vec!["トヨタ".into()],
                },
            )
            .await
            .expect_err("invalid kind");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}
