//! 戦略実行 MCP の関心追加 tool。
//!
//! 戦略 Agent からの追加は常に `role=derived` / `origin=llm` で記録する。
//! 同じ (strategy_id, ref_kind, ref_id) が既に存在する場合は idempotent に成功させる
//! (role / origin は変更しない)。

use rmcp::ErrorData as McpError;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::entities::strategy_interest;
use crate::services::interests::{ensure_ref_kind, ensure_role};

use super::dto::{AddInterestParams, AddInterestResult};
use super::{StrategyServer, db_error, ensure_strategy_exists, invalid_params};

/// 戦略 Agent が追加する derived interest の固定 role / origin。
const AGENT_INTEREST_ROLE: &str = "derived";
const AGENT_INTEREST_ORIGIN: &str = "llm";

fn validation_to_mcp(err: crate::error::AppError) -> McpError {
    match err {
        crate::error::AppError::Validation(msg) => invalid_params(msg),
        other => invalid_params(format!("validation failed: {other}")),
    }
}

impl StrategyServer {
    pub(crate) async fn add_interest_inner(
        &self,
        session_strategy_id: Uuid,
        params: AddInterestParams,
    ) -> Result<AddInterestResult, McpError> {
        let ref_kind = params.ref_kind.trim();
        ensure_ref_kind(ref_kind).map_err(validation_to_mcp)?;
        // role / origin は agent 経路では固定だが、列挙の不整合に気付けるよう値域チェックは残す
        ensure_role(AGENT_INTEREST_ROLE).map_err(validation_to_mcp)?;
        let ref_id = params.ref_id.trim();
        if ref_id.is_empty() {
            return Err(invalid_params("ref_id must not be empty"));
        }
        ensure_strategy_exists(&self.db, session_strategy_id).await?;

        // ON CONFLICT DO NOTHING で挿入を試み、衝突時は SELECT で既存行を返す。
        // 単純な check-then-insert だと並行呼び出し時に片方が UNIQUE 違反で失敗し、
        // tool description の「idempotent」契約を破る。
        let model = strategy_interest::ActiveModel {
            strategy_id: Set(session_strategy_id),
            ref_kind: Set(ref_kind.to_string()),
            ref_id: Set(ref_id.to_string()),
            role: Set(AGENT_INTEREST_ROLE.to_string()),
            origin: Set(AGENT_INTEREST_ORIGIN.to_string()),
            created_at: NotSet,
        };
        let insert_result = strategy_interest::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([
                    strategy_interest::Column::StrategyId,
                    strategy_interest::Column::RefKind,
                    strategy_interest::Column::RefId,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec_with_returning(&self.db)
            .await;
        match insert_result {
            Ok(created) => Ok(AddInterestResult {
                strategy_id: created.strategy_id,
                ref_kind: created.ref_kind,
                ref_id: created.ref_id,
                role: created.role,
                origin: created.origin,
                created: true,
            }),
            // ON CONFLICT DO NOTHING で skip されたとき、SeaORM 2.0 では
            // `exec_with_returning` は `RecordNotFound` を返す (RETURNING 行が空のため)。
            // 念のため `RecordNotInserted` も同じパスで扱う。
            Err(sea_orm::DbErr::RecordNotInserted | sea_orm::DbErr::RecordNotFound(_)) => {
                let existing = strategy_interest::Entity::find()
                    .filter(strategy_interest::Column::StrategyId.eq(session_strategy_id))
                    .filter(strategy_interest::Column::RefKind.eq(ref_kind))
                    .filter(strategy_interest::Column::RefId.eq(ref_id))
                    .one(&self.db)
                    .await
                    .map_err(db_error)?
                    .ok_or_else(|| {
                        db_error(sea_orm::DbErr::Custom(
                            "interest disappeared between ON CONFLICT and SELECT".into(),
                        ))
                    })?;
                Ok(AddInterestResult {
                    strategy_id: existing.strategy_id,
                    ref_kind: existing.ref_kind,
                    ref_id: existing.ref_id,
                    role: existing.role,
                    origin: existing.origin,
                    created: false,
                })
            }
            Err(err) => Err(db_error(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use crate::testing::create_test_db;

    use super::super::dto::AddInterestParams;
    use super::super::tests_common::{build_server, insert_strategy};

    #[sqlx::test(migrations = false)]
    async fn add_interest_creates_derived_llm_row(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        let server = build_server(db);

        let result = server
            .add_interest_inner(
                sid,
                AddInterestParams {
                    ref_kind: "stock".into(),
                    ref_id: "7203".into(),
                },
            )
            .await
            .expect("add_interest");

        assert_eq!(
            (
                result.strategy_id,
                result.ref_kind,
                result.ref_id,
                result.role,
                result.origin,
                result.created,
            ),
            (
                sid,
                "stock".into(),
                "7203".into(),
                "derived".into(),
                "llm".into(),
                true
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn add_interest_is_idempotent(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        let server = build_server(db);

        for _ in 0..2 {
            server
                .add_interest_inner(
                    sid,
                    AddInterestParams {
                        ref_kind: "indicator".into(),
                        ref_id: "USDJPY".into(),
                    },
                )
                .await
                .expect("add");
        }
        let second = server
            .add_interest_inner(
                sid,
                AddInterestParams {
                    ref_kind: "indicator".into(),
                    ref_id: "USDJPY".into(),
                },
            )
            .await
            .expect("third");
        assert!(!second.created);
    }

    #[sqlx::test(migrations = false)]
    async fn add_interest_rejects_invalid_ref_kind(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        let server = build_server(db);

        let err = server
            .add_interest_inner(
                sid,
                AddInterestParams {
                    ref_kind: "bogus".into(),
                    ref_id: "x".into(),
                },
            )
            .await
            .expect_err("invalid kind");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn add_interest_rejects_empty_ref_id(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        let server = build_server(db);

        let err = server
            .add_interest_inner(
                sid,
                AddInterestParams {
                    ref_kind: "stock".into(),
                    ref_id: "  ".into(),
                },
            )
            .await
            .expect_err("empty ref_id");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}
