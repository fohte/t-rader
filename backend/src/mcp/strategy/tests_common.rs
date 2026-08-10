//! 戦略実行 MCP の統合テストで共有するヘルパー。

use chrono::{DateTime, FixedOffset};
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::entities::{note, strategy};

use super::StrategyServer;
use super::dto::{AnnotationDto, NoteDto};

pub(super) async fn insert_strategy(db: &DatabaseConnection, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    strategy::ActiveModel {
        id: Set(id),
        name: Set(name.to_string()),
        description: Set(None),
        sort_order: Set(0),
        agents_md: NotSet,
        skills: NotSet,
        agent_graph: NotSet,
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(db)
    .await
    .expect("insert strategy");
    id
}

pub(super) fn build_server(db: DatabaseConnection) -> StrategyServer {
    StrategyServer::new(db, None)
}

/// DTO の比較で動的な timestamp を差し替えるための sentinel 値。
pub(super) fn ts_sentinel() -> DateTime<FixedOffset> {
    chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset()
}

pub(super) fn normalize_note(mut n: NoteDto) -> NoteDto {
    n.created_at = ts_sentinel();
    n.updated_at = ts_sentinel();
    n
}

pub(super) fn normalize_annotation(mut a: AnnotationDto) -> AnnotationDto {
    a.created_at = ts_sentinel();
    a.updated_at = ts_sentinel();
    a
}

/// 指定戦略の所有として固定タイトルの note を seed する (cross-strategy violation 用)
pub(super) async fn seed_foreign_note(db: &DatabaseConnection, owner: Uuid, title: &str) -> Uuid {
    let id = Uuid::new_v4();
    note::ActiveModel {
        id: Set(id),
        strategy_id: Set(owner),
        title: Set(title.to_string()),
        body_md: Set("body".into()),
        frontmatter_json: Set(serde_json::json!({})),
        type_tag: Set(None),
        status: Set(super::DEFAULT_NOTE_STATUS.into()),
        trigger: Set(None),
        trigger_label: Set(None),
        created_by_kind: Set(super::STRATEGY_AGENT_ACTOR.into()),
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(db)
    .await
    .expect("seed note");
    id
}
