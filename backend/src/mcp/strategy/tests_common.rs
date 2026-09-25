//! 戦略実行 MCP の統合テストで共有するヘルパー。

use chrono::{DateTime, FixedOffset};
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::DatabaseConnection;
use sea_orm::TransactionTrait;
use sea_orm::{ActiveModelTrait, EntityTrait};
use uuid::Uuid;

use crate::entities::{annotation, comment, hypothesis, note, note_kind, note_version, strategy};

use super::StrategyServer;
use super::dto::{AnnotationDto, CommentDto, NoteDto};

pub(super) async fn insert_strategy(db: &DatabaseConnection, name: &str) -> Uuid {
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

pub(super) async fn insert_note_kind(db: &DatabaseConnection, key: &str, requires_approval: bool) {
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

pub(super) fn build_server(db: DatabaseConnection) -> StrategyServer {
    StrategyServer::new(db, None)
}

/// DTO の比較で動的な timestamp を差し替えるための sentinel 値。
pub(super) fn ts_sentinel() -> DateTime<FixedOffset> {
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

/// 現行バージョンの status を直接書き換える (レビュー確定状態からの遷移をテストするため)
pub(super) async fn set_note_status(db: &DatabaseConnection, note_id: Uuid, status: &str) {
    let version = crate::services::note_versions::find_current_version(db, note_id)
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
    db: &DatabaseConnection,
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

pub(super) async fn current_note_version_id(db: &DatabaseConnection, note_id: Uuid) -> Uuid {
    crate::services::note_versions::find_current_version(db, note_id)
        .await
        .expect("find current note version")
        .expect("current note version exists")
        .id
}

/// 指定戦略の所有として固定タイトルの note を seed する (cross-strategy violation 用)
pub(super) async fn seed_foreign_note(db: &DatabaseConnection, owner: Uuid, title: &str) -> Uuid {
    let id = Uuid::new_v4();
    let txn = db.begin().await.expect("begin note transaction");
    note::Entity::insert(note::ActiveModel {
        id: Set(id),
        strategy_id: Set(Some(owner)),
        kind: Set(None),
        trigger: Set(None),
        trigger_label: Set(None),
        created_at: NotSet,
        updated_at: NotSet,
        execution_id: Set(None),
    })
    .exec_without_returning(&txn)
    .await
    .expect("seed note");
    crate::services::note_versions::append_version(
        &txn,
        id,
        crate::services::note_versions::AppendVersion {
            title: title.to_string(),
            body_md: "body".into(),
            frontmatter_json: serde_json::json!({}),
            graphs_json: serde_json::json!([]),
            created_by_kind: super::STRATEGY_AGENT_ACTOR.into(),
            execution_id: None,
            change_reason: None,
            change_diff: None,
            actor: crate::services::change_history::Actor::Llm {
                label: super::STRATEGY_AGENT_ACTOR,
            },
        },
    )
    .await
    .expect("append note version");
    txn.commit().await.expect("commit note transaction");
    id
}

/// 指定戦略の所有として固定パラメータの annotation を seed する (cross-strategy violation 用)
pub(super) async fn seed_foreign_annotation(db: &DatabaseConnection, owner: Uuid) -> Uuid {
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
        anchor_side: Set(None),
        start_line: Set(None),
        end_line: Set(None),
    }
    .insert(db)
    .await
    .expect("seed comment");
    id
}

/// 指定 strategy_id (`None` なら global) の仮説を seed する
pub(super) async fn seed_hypothesis(
    db: &DatabaseConnection,
    strategy_id: Option<Uuid>,
    title: &str,
    body: &str,
    status: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    hypothesis::ActiveModel {
        hypothesis_id: Set(id),
        strategy_id: Set(strategy_id),
        title: Set(title.to_string()),
        body: Set(body.to_string()),
        status: Set(status.to_string()),
        related_note_ids: Set(vec![]),
        related_interest_ids: Set(vec![]),
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(db)
    .await
    .expect("seed hypothesis");
    id
}

/// note_version にトップレベルの行コメントを seed する。
pub(super) async fn seed_note_version_comment_with_anchor(
    db: &DatabaseConnection,
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
