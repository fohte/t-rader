//! change_history テーブルへの記録ヘルパー。
//!
//! note / annotation / strategy / trade / comment の CRUD・status 変更を記録する。
//! actor_kind / actor_label は HTTP API 経由ではユーザー操作扱いとし、"human" / "user" で固定する。

use sea_orm::ActiveValue::Set;
use sea_orm::{ConnectionTrait, EntityTrait};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::entities::change_history;
use crate::error::AppError;

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
    let model = change_history::ActiveModel {
        id: Set(Uuid::new_v4()),
        target_kind: Set(target_kind.as_str().to_string()),
        target_id: Set(target_id),
        actor_kind: Set("human".to_string()),
        actor_label: Set("user".to_string()),
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
