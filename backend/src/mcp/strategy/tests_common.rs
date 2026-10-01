//! 戦略実行 MCP の統合テストで共有するヘルパー。

use chrono::{DateTime, FixedOffset};
use core_application::change_history::Actor;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use gateway_postgres::entities::{
    annotation, change_history, comment, note, note_kind, note_version, strategy,
};

use crate::data_provider::SharedDailyBarSource;

use super::StrategyServer;
use super::dto::{AnnotationDto, CommentDto, NoteDto};

pub(super) async fn insert_strategy(db: &impl sea_orm::ConnectionTrait, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    strategy::ActiveModel {
        id: Set(id),
        name: Set(name.to_string()),
        description: Set(None),
        sort_order: Set(0),
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(db)
    .await
    .expect("insert strategy");
    id
}

pub(super) async fn insert_note_kind(
    db: &impl sea_orm::ConnectionTrait,
    key: &str,
    requires_approval: bool,
) {
    note_kind::ActiveModel {
        key: Set(key.to_string()),
        display_name: Set(key.to_string()),
        requires_approval: Set(requires_approval),
        description: Set(None),
        sort_order: Set(0),
    }
    .insert(db)
    .await
    .expect("insert note kind");
}

pub(super) fn build_server(db: impl Into<gateway_postgres::DatabaseHandle>) -> StrategyServer {
    build_server_with_source(db, None)
}

pub(super) fn build_server_with_source(
    db: impl Into<gateway_postgres::DatabaseHandle>,
    daily_bar_source: Option<SharedDailyBarSource>,
) -> StrategyServer {
    StrategyServer::new(
        crate::services::use_cases::build_use_cases(db),
        daily_bar_source,
    )
}

/// DTO の比較で動的な timestamp を差し替えるための sentinel 値。
pub(in crate::mcp) fn ts_sentinel() -> DateTime<FixedOffset> {
    chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset()
}

pub(super) fn normalize_note(mut n: NoteDto) -> NoteDto {
    n.version_id = Uuid::nil();
    n.created_at = ts_sentinel();
    n.updated_at = ts_sentinel();
    n
}

pub(super) fn normalize_annotation(mut a: AnnotationDto) -> AnnotationDto {
    a.created_at = ts_sentinel();
    a.updated_at = ts_sentinel();
    a
}

#[derive(Debug, PartialEq)]
pub(super) struct ChangeHistoryShape {
    pub id: Uuid,
    pub target_kind: String,
    pub target_id: Uuid,
    pub actor_kind: String,
    pub actor_label: String,
    pub op: String,
    pub diff_json: serde_json::Value,
    pub summary: Option<String>,
    pub created_at: DateTime<FixedOffset>,
}

pub(super) async fn change_history_for(
    db: &impl sea_orm::ConnectionTrait,
    target_id: Uuid,
) -> Vec<ChangeHistoryShape> {
    change_history::Entity::find()
        .filter(change_history::Column::TargetId.eq(target_id))
        .order_by_asc(change_history::Column::CreatedAt)
        .all(db)
        .await
        .expect("find change history")
        .into_iter()
        .map(|row| ChangeHistoryShape {
            id: Uuid::nil(),
            target_kind: row.target_kind,
            target_id: row.target_id,
            actor_kind: row.actor_kind,
            actor_label: row.actor_label,
            op: row.op,
            diff_json: row.diff_json,
            summary: row.summary,
            created_at: ts_sentinel(),
        })
        .collect()
}

/// 現行バージョンの status を直接書き換える (レビュー確定状態からの遷移をテストするため)
pub(super) async fn set_note_status(
    db: &impl sea_orm::ConnectionTrait,
    note_id: Uuid,
    status: &str,
) {
    let version = crate::testing::find_current_note_version(db, note_id)
        .await
        .expect("find current note version")
        .expect("current note version exists");
    note_version::ActiveModel {
        id: Set(version.id),
        status: Set(status.to_string()),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("set note status");
}

/// note の updated_at を直接書き換える (`updated_after` フィルタの境界値テスト用)
pub(super) async fn set_note_updated_at(
    db: &impl sea_orm::ConnectionTrait,
    note_id: Uuid,
    updated_at: DateTime<FixedOffset>,
) {
    note::ActiveModel {
        id: Set(note_id),
        updated_at: Set(updated_at),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("set note updated_at");
}

pub(super) fn normalize_comment(mut c: CommentDto) -> CommentDto {
    c.created_at = ts_sentinel();
    c
}

pub(super) fn normalize_comment_model(mut c: comment::Model) -> comment::Model {
    c.created_at = ts_sentinel();
    c
}

pub(super) async fn current_note_version_id(
    db: &impl sea_orm::ConnectionTrait,
    note_id: Uuid,
) -> Uuid {
    crate::testing::find_current_note_version(db, note_id)
        .await
        .expect("find current note version")
        .expect("current note version exists")
        .id
}

/// 指定戦略の所有として固定タイトルの note を seed する (cross-strategy violation 用)
pub(super) async fn seed_foreign_note(
    db: &gateway_postgres::DatabaseHandle,
    owner: Uuid,
    title: &str,
) -> Uuid {
    crate::testing::insert_test_note_as(
        db,
        Some(owner),
        title,
        "body",
        super::STRATEGY_AGENT_ACTOR,
        Actor::Llm {
            label: super::STRATEGY_AGENT_ACTOR,
        },
    )
    .await
}

/// 指定戦略の所有として固定パラメータの annotation を seed する (cross-strategy violation 用)
pub(super) async fn seed_foreign_annotation(
    db: &impl sea_orm::ConnectionTrait,
    owner: Uuid,
) -> Uuid {
    let id = Uuid::new_v4();
    annotation::ActiveModel {
        id: Set(id),
        strategy_id: Set(Some(owner)),
        target_symbol: Set("7203".into()),
        target_kind: Set("signal".into()),
        timestamp: Set("2026-06-01T00:00:00Z".parse().expect("ts")),
        price: Set(None),
        text: Set("breakout".into()),
        status: Set(super::DEFAULT_ANNOTATION_STATUS.into()),
        linked_note_id: Set(None),
        created_by_kind: Set(super::STRATEGY_AGENT_ACTOR.into()),
        created_at: NotSet,
        updated_at: NotSet,
        execution_step_id: Set(None),
        execution_task_id: Set(None),
    }
    .insert(db)
    .await
    .expect("seed annotation");
    id
}

/// note / annotation にコメントを直接 seed する (MCP に comment 作成 tool は無いため)
pub(super) async fn seed_comment(
    db: &impl sea_orm::ConnectionTrait,
    target_kind: &str,
    target_id: Uuid,
    parent_id: Option<Uuid>,
    body: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    comment::ActiveModel {
        id: Set(id),
        target_kind: Set(target_kind.to_string()),
        target_id: Set(target_id),
        parent_id: Set(parent_id),
        body: Set(body.to_string()),
        author_kind: Set("human".into()),
        author_label: Set("user".into()),
        resolved: NotSet,
        created_at: NotSet,
        anchor_text: Set(None),
        anchor_side: Set(None),
        start_line: Set(None),
        end_line: Set(None),
    }
    .insert(db)
    .await
    .expect("seed comment");
    id
}

/// note_version にトップレベルの行コメントを seed する。
pub(super) async fn seed_note_version_comment_with_anchor(
    db: &impl sea_orm::ConnectionTrait,
    version_id: Uuid,
    anchor_text: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    comment::ActiveModel {
        id: Set(id),
        target_kind: Set("note_version".into()),
        target_id: Set(version_id),
        parent_id: Set(None),
        body: Set("please fix".into()),
        author_kind: Set("human".into()),
        author_label: Set("user".into()),
        resolved: NotSet,
        created_at: NotSet,
        anchor_text: Set(Some(anchor_text.to_string())),
        anchor_side: Set(Some("new".into())),
        start_line: Set(Some(2)),
        end_line: Set(Some(2)),
    }
    .insert(db)
    .await
    .expect("seed note version comment with anchor");
    id
}
