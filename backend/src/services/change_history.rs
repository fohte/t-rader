//! change_history テーブルへの記録ヘルパー。
//!
//! note / annotation / strategy / trade / comment の CRUD・status 変更を記録する。
//! HTTP API 経由の記録は `record` (= "human"/"user" 固定) を使う。MCP 経由など human 以外の
//! actor を記録したい呼び出し元は `record_as` に `Actor` を渡す。

use sea_orm::ActiveValue::Set;
use sea_orm::{ConnectionTrait, EntityTrait};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::entities::change_history;
use crate::error::AppError;

/// 記録する変更の actor。`change_history.actor_kind` の CHECK 制約 (`human`/`llm`) に対応する。
#[derive(Debug, Clone, Copy)]
pub enum Actor {
    Human,
    Llm { label: &'static str },
}

impl Actor {
    fn kind(self) -> &'static str {
        match self {
            Actor::Human => "human",
            Actor::Llm { .. } => "llm",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Actor::Human => "user",
            Actor::Llm { label } => label,
        }
    }
}

/// 対象種別。`change_history.target_kind` の CHECK 制約と enum 値を対応させる必要がある
#[derive(Debug, Clone, Copy)]
pub enum TargetKind {
    Note,
    Annotation,
    Strategy,
    Trade,
    Comment,
}

impl TargetKind {
    fn as_str(self) -> &'static str {
        match self {
            TargetKind::Note => "note",
            TargetKind::Annotation => "annotation",
            TargetKind::Strategy => "strategy",
            TargetKind::Trade => "trade",
            TargetKind::Comment => "comment",
        }
    }
}

/// 操作種別
#[derive(Debug, Clone, Copy)]
pub enum Op {
    Create,
    Update,
    Delete,
    StatusChange,
}

impl Op {
    fn as_str(self) -> &'static str {
        match self {
            Op::Create => "create",
            Op::Update => "update",
            Op::Delete => "delete",
            Op::StatusChange => "status_change",
        }
    }
}

/// change_history に 1 件記録する。actor は "human" / "user" 固定。
pub async fn record<C>(
    db: &C,
    target_kind: TargetKind,
    target_id: Uuid,
    op: Op,
    diff: JsonValue,
    summary: Option<String>,
) -> Result<(), AppError>
where
    C: ConnectionTrait,
{
    record_as(db, Actor::Human, target_kind, target_id, op, diff, summary).await
}

/// change_history に 1 件記録する。actor を明示的に指定できる版。
pub async fn record_as<C>(
    db: &C,
    actor: Actor,
    target_kind: TargetKind,
    target_id: Uuid,
    op: Op,
    diff: JsonValue,
    summary: Option<String>,
) -> Result<(), AppError>
where
    C: ConnectionTrait,
{
    let model = change_history::ActiveModel {
        id: Set(Uuid::new_v4()),
        target_kind: Set(target_kind.as_str().to_string()),
        target_id: Set(target_id),
        actor_kind: Set(actor.kind().to_string()),
        actor_label: Set(actor.label().to_string()),
        op: Set(op.as_str().to_string()),
        diff_json: Set(diff),
        summary: Set(summary),
        created_at: sea_orm::ActiveValue::NotSet,
    };
    change_history::Entity::insert(model)
        .exec_without_returning(db)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sea_orm::EntityTrait;
    use serde_json::json;
    use sqlx::PgPool;

    use super::*;
    use crate::testing::{create_test_db, insert_test_strategy};

    #[sqlx::test(migrations = false)]
    async fn record_as_persists_the_given_actor(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "s").await;

        record_as(
            &db,
            Actor::Llm { label: "mgmt-mcp" },
            TargetKind::Strategy,
            strategy_id,
            Op::Update,
            json!({}),
            None,
        )
        .await
        .expect("record");

        let row = change_history::Entity::find()
            .one(&db)
            .await
            .expect("query")
            .expect("row exists");
        assert_eq!(
            (row.actor_kind, row.actor_label),
            ("llm".to_string(), "mgmt-mcp".to_string()),
        );
    }
}
