//! 戦略実行 MCP の関心追加 / 監視対象一覧 tool。
//!
//! 戦略 Agent からの追加は常に `role=derived` / `origin=llm` で記録する。
//! 同じ (strategy_id, ref_kind, ref_id) が既に存在する場合は idempotent に成功させる
//! (role / origin は変更しない)。
//!
//! 監視対象一覧 (`list_watch_targets`) は人間が `origin=human` で登録した
//! 銘柄 (`ref_kind=stock`) のうち `status=active` のものだけを返す。口座全体の
//! 関心 (`strategy_id IS NULL`) と接続元戦略の関心の両方が対象。同じ銘柄が
//! 両スコープに登録されていても ref_id は重複させず、古い方の登録日時を採用する。
//! 保有状況は見ないため、除外したい場合は呼び出し側で `read_portfolio` 等と
//! 突き合わせること。

use rmcp::ErrorData as McpError;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::{Expr, OnConflict};
use sea_orm::{ColumnTrait, Condition, EntityTrait, ExprTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::entities::strategy_interest;
use crate::services::interests::{DEFAULT_STATUS, ensure_ref_kind, ensure_role};

use super::dto::{
    AddInterestParams, AddInterestResult, ListWatchTargetsParams, ListWatchTargetsResult,
    WatchTargetDto,
};
use super::{StrategyServer, clamp_limit, db_error, ensure_strategy_exists, invalid_params};

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
            id: NotSet,
            strategy_id: Set(Some(session_strategy_id)),
            ref_kind: Set(ref_kind.to_string()),
            ref_id: Set(ref_id.to_string()),
            role: Set(AGENT_INTEREST_ROLE.to_string()),
            origin: Set(AGENT_INTEREST_ORIGIN.to_string()),
            status: Set(DEFAULT_STATUS.to_string()),
            created_at: NotSet,
        };
        // 部分ユニークインデックスを conflict target に指定するため、WHERE 述語を一致させる
        let insert_result = strategy_interest::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([
                    strategy_interest::Column::StrategyId,
                    strategy_interest::Column::RefKind,
                    strategy_interest::Column::RefId,
                ])
                .target_and_where(Expr::col(strategy_interest::Column::StrategyId).is_not_null())
                .do_nothing()
                .to_owned(),
            )
            .exec_with_returning(&self.db)
            .await;
        match insert_result {
            Ok(created) => Ok(AddInterestResult {
                strategy_id: session_strategy_id,
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
                    strategy_id: session_strategy_id,
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

    /// 人間が `origin=human` で登録した監視対象銘柄 (`ref_kind=stock`,
    /// `status=active`) を古い順に返す。口座全体の関心 (`strategy_id IS NULL`) と
    /// 接続元戦略の関心の両方が対象で、他の戦略にだけ属する関心は含まれない。
    /// 同じ銘柄が両スコープに登録されていても ref_id は重複させない。
    /// エージェント自身が追加した interest (`origin=llm`) や `status=archived` の
    /// ものは含まれない。保有状況によるフィルタは行わないため、除外したい場合は
    /// `read_portfolio` / `check_buyable_qty` と組み合わせる。
    pub(crate) async fn list_watch_targets_inner(
        &self,
        session_strategy_id: Uuid,
        params: ListWatchTargetsParams,
    ) -> Result<ListWatchTargetsResult, McpError> {
        let rows = strategy_interest::Entity::find()
            .filter(
                Condition::any()
                    .add(strategy_interest::Column::StrategyId.is_null())
                    .add(strategy_interest::Column::StrategyId.eq(session_strategy_id)),
            )
            .filter(strategy_interest::Column::RefKind.eq("stock"))
            .filter(strategy_interest::Column::Origin.eq("human"))
            .filter(strategy_interest::Column::Status.eq(DEFAULT_STATUS))
            .order_by_asc(strategy_interest::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(db_error)?;

        // 口座全体と接続元戦略それぞれに同じ ref_id が登録され得るため、DB 側の
        // limit では重複除去前の件数を切ってしまう。ここで dedup してから絞る。
        let mut seen_ref_ids = std::collections::HashSet::new();
        let watch_targets = rows
            .into_iter()
            .filter(|row| seen_ref_ids.insert(row.ref_id.clone()))
            .take(clamp_limit(params.limit) as usize)
            .map(|row| WatchTargetDto {
                ref_id: row.ref_id,
                created_at: row.created_at,
            })
            .collect();
        Ok(ListWatchTargetsResult { watch_targets })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sea_orm::DatabaseConnection;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::strategy_interest;
    use crate::testing::create_test_db;

    use super::super::dto::{
        AddInterestParams, ListWatchTargetsParams, ListWatchTargetsResult, WatchTargetDto,
    };
    use super::super::tests_common::{build_server, insert_strategy};

    fn ts(secs: i64) -> DateTime<FixedOffset> {
        DateTime::from_timestamp(secs, 0)
            .expect("valid ts")
            .fixed_offset()
    }

    async fn insert_interest(
        db: &DatabaseConnection,
        strategy_id: Option<Uuid>,
        ref_kind: &str,
        ref_id: &str,
        origin: &str,
        status: &str,
        created_at: DateTime<FixedOffset>,
    ) {
        strategy_interest::ActiveModel {
            id: Set(Uuid::new_v4()),
            strategy_id: Set(strategy_id),
            ref_kind: Set(ref_kind.into()),
            ref_id: Set(ref_id.into()),
            role: Set("seed".into()),
            origin: Set(origin.into()),
            status: Set(status.into()),
            created_at: Set(created_at),
        }
        .insert(db)
        .await
        .expect("insert interest");
    }

    fn watch_target_ref_ids(result: ListWatchTargetsResult) -> Vec<String> {
        result.watch_targets.into_iter().map(|t| t.ref_id).collect()
    }

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

    #[sqlx::test(migrations = false)]
    async fn list_watch_targets_returns_only_active_human_stock_interests(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        let server = build_server(db.clone());

        insert_interest(&db, Some(sid), "stock", "7203", "human", "active", ts(1)).await;
        insert_interest(&db, Some(sid), "stock", "9984", "human", "archived", ts(2)).await;
        insert_interest(&db, Some(sid), "stock", "6758", "llm", "active", ts(3)).await;
        insert_interest(
            &db,
            Some(sid),
            "indicator",
            "USDJPY",
            "human",
            "active",
            ts(4),
        )
        .await;

        let result = server
            .list_watch_targets_inner(sid, ListWatchTargetsParams { limit: None })
            .await
            .expect("list_watch_targets");

        assert_eq!(watch_target_ref_ids(result), vec!["7203".to_string()]);
    }

    #[sqlx::test(migrations = false)]
    async fn list_watch_targets_orders_oldest_first_and_respects_limit(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        let server = build_server(db.clone());

        insert_interest(&db, Some(sid), "stock", "3", "human", "active", ts(3)).await;
        insert_interest(&db, Some(sid), "stock", "1", "human", "active", ts(1)).await;
        insert_interest(&db, Some(sid), "stock", "2", "human", "active", ts(2)).await;

        let result = server
            .list_watch_targets_inner(sid, ListWatchTargetsParams { limit: Some(2) })
            .await
            .expect("list_watch_targets");

        assert_eq!(
            watch_target_ref_ids(result),
            vec!["1".to_string(), "2".to_string()],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn list_watch_targets_excludes_other_strategy_interests(pool: PgPool) {
        let db = create_test_db(pool).await;
        let a = insert_strategy(&db, "a").await;
        let b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());

        insert_interest(&db, Some(a), "stock", "7203", "human", "active", ts(1)).await;
        insert_interest(&db, Some(b), "stock", "9984", "human", "active", ts(1)).await;

        let result = server
            .list_watch_targets_inner(a, ListWatchTargetsParams { limit: None })
            .await
            .expect("list_watch_targets");

        assert_eq!(watch_target_ref_ids(result), vec!["7203".to_string()]);
    }

    #[sqlx::test(migrations = false)]
    async fn list_watch_targets_includes_account_wide_interests(pool: PgPool) {
        let db = create_test_db(pool).await;
        let a = insert_strategy(&db, "a").await;
        let b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());

        insert_interest(&db, None, "stock", "7203", "human", "active", ts(1)).await;
        insert_interest(&db, Some(a), "stock", "9984", "human", "active", ts(2)).await;
        insert_interest(&db, Some(b), "stock", "6758", "human", "active", ts(3)).await;

        let result = server
            .list_watch_targets_inner(a, ListWatchTargetsParams { limit: None })
            .await
            .expect("list_watch_targets");

        assert_eq!(
            watch_target_ref_ids(result),
            vec!["7203".to_string(), "9984".to_string()],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn list_watch_targets_dedups_same_ref_id_across_scopes(pool: PgPool) {
        let db = create_test_db(pool).await;
        let a = insert_strategy(&db, "a").await;
        let server = build_server(db.clone());

        insert_interest(&db, None, "stock", "7203", "human", "active", ts(1)).await;
        insert_interest(&db, Some(a), "stock", "7203", "human", "active", ts(2)).await;

        let result = server
            .list_watch_targets_inner(a, ListWatchTargetsParams { limit: None })
            .await
            .expect("list_watch_targets");

        assert_eq!(
            result.watch_targets,
            vec![WatchTargetDto {
                ref_id: "7203".to_string(),
                created_at: ts(1),
            }],
        );
    }
}
