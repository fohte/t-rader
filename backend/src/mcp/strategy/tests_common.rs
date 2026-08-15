//! 戦略実行 MCP の統合テストで共有するヘルパー。

use chrono::{DateTime, FixedOffset};
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::entities::{annotation, comment, note, strategy};

use super::StrategyServer;
use super::dto::{AnnotationDto, CommentDto, NoteDto};

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

/// note の status を直接書き換える (レビュー確定状態からの遷移をテストするため)
pub(super) async fn set_note_status(db: &DatabaseConnection, note_id: Uuid, status: &str) {
    note::ActiveModel {
        id: Set(note_id),
        status: Set(status.to_string()),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("set note status");
}

pub(super) fn normalize_comment(mut c: CommentDto) -> CommentDto {
    c.created_at = ts_sentinel();
    c
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
        graphs_json: Set(serde_json::json!([])),
    }
    .insert(db)
    .await
    .expect("seed note");
    id
}

/// 指定戦略の所有として固定パラメータの annotation を seed する (cross-strategy violation 用)
pub(super) async fn seed_foreign_annotation(db: &DatabaseConnection, owner: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    annotation::ActiveModel {
        id: Set(id),
        strategy_id: Set(owner),
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
    }
    .insert(db)
    .await
    .expect("seed annotation");
    id
}

/// note / annotation にコメントを直接 seed する (MCP に comment 作成 tool は無いため)
pub(super) async fn seed_comment(
    db: &DatabaseConnection,
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
        start_line: Set(None),
        end_line: Set(None),
        drifted: NotSet,
    }
    .insert(db)
    .await
    .expect("seed comment");
    id
}

/// note にトップレベルの anchor_text 付きコメントを seed する (再アンカリングのテスト用)
pub(super) async fn seed_note_comment_with_anchor(
    db: &DatabaseConnection,
    note_id: Uuid,
    anchor_text: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    comment::ActiveModel {
        id: Set(id),
        target_kind: Set("note".into()),
        target_id: Set(note_id),
        parent_id: Set(None),
        body: Set("please fix".into()),
        author_kind: Set("human".into()),
        author_label: Set("user".into()),
        resolved: NotSet,
        created_at: NotSet,
        anchor_text: Set(Some(anchor_text.to_string())),
        start_line: Set(Some(1)),
        end_line: Set(Some(1)),
        drifted: Set(false),
    }
    .insert(db)
    .await
    .expect("seed note comment with anchor");
    id
}
