//! change_history テーブルへの記録ヘルパー。
//!
//! note / annotation / strategy / trade / comment / custom_indicator / note_kind の CRUD・status 変更を記録する。
//! HTTP API 経由の記録は `record` (= "human"/"user" 固定) を使う。MCP 経由など human 以外の
//! actor を記録したい呼び出し元は `record_as` に `Actor` を渡す。

use sea_orm::ActiveValue::Set;
use sea_orm::{ConnectionTrait, EntityTrait};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::error::AppError;
pub use core_application::change_history::{Actor, Op, TargetKind};
use gateway_postgres::entities::change_history;

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
    use super::*;
    use crate::testing::insert_test_strategy;
    use sea_orm::EntityTrait;
    use serde_json::json;

    #[backend_test_macros::database_test]
    async fn record_as_persists_the_given_actor(db: gateway_postgres::DatabaseHandle) {
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
