use chrono::{DateTime, FixedOffset};
use serde_json::Value;
use uuid::Uuid;

use crate::change_history::Actor;
use crate::strategy_scope::StrategyScope;

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub id: Uuid,
    pub strategy_id: Option<Uuid>,
    pub kind: Option<String>,
    pub trigger: Option<String>,
    pub trigger_label: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub execution_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoteVersion {
    pub id: Uuid,
    pub note_id: Uuid,
    pub version_no: i32,
    pub title: String,
    pub body_md: String,
    pub frontmatter_json: Value,
    pub graphs_json: Value,
    pub status: String,
    pub is_current: bool,
    pub change_reason: Option<String>,
    pub created_by_kind: String,
    pub execution_id: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub reviewed_at: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoteSnapshot {
    pub note: Note,
    pub version: NoteVersion,
    pub created_by_kind: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewNote {
    pub id: Uuid,
    pub strategy_id: Option<Uuid>,
    pub kind: Option<String>,
    pub trigger: Option<String>,
    pub trigger_label: Option<String>,
    pub execution_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoteMetadataUpdate {
    pub id: Uuid,
    pub kind: Option<Option<String>>,
    pub trigger: Option<Option<String>>,
    pub trigger_label: Option<Option<String>>,
    pub updated_at: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewNoteVersion {
    pub id: Uuid,
    pub note_id: Uuid,
    pub version_no: i32,
    pub title: String,
    pub body_md: String,
    pub frontmatter_json: Value,
    pub graphs_json: Value,
    pub status: String,
    pub is_current: bool,
    pub change_reason: Option<String>,
    pub created_by_kind: String,
    pub execution_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoteVersionUpdate {
    pub id: Uuid,
    pub is_current: Option<bool>,
    pub status: Option<String>,
    pub reviewed_at: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteLinkTarget {
    pub id: Uuid,
    pub strategy_id: Option<Uuid>,
    pub current_version_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteLink {
    pub from_version_id: Uuid,
    pub to_note_id: Uuid,
    pub to_version_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteLinkView {
    pub note_id: Uuid,
    pub version_id: Option<Uuid>,
    pub version_no: Option<i32>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteLinks {
    pub outgoing: Vec<NoteLinkView>,
    pub incoming: Vec<NoteLinkView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteListCursor {
    pub updated_at: DateTime<FixedOffset>,
    pub note_id: Uuid,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NoteListQuery {
    pub strategy_id: Option<Uuid>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub reference: Option<(String, String)>,
    pub updated_after: Option<DateTime<FixedOffset>>,
    pub include_pending: bool,
    pub cursor: Option<NoteListCursor>,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoteListPage {
    pub notes: Vec<NoteSnapshot>,
    pub cursor: Option<NoteListCursor>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewNoteLink {
    pub from_version_id: Uuid,
    pub to_note_id: Uuid,
    pub to_version_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct NoteWriteCommand {
    pub scope: Option<StrategyScope>,
    pub strategy_id: Option<Uuid>,
    pub execution_id: Option<String>,
    pub note_id: Option<Uuid>,
    pub title: Option<String>,
    pub body_md: Option<String>,
    pub frontmatter_json: Option<Value>,
    pub graphs_json: Option<Value>,
    pub kind: Option<Option<String>>,
    pub status: Option<String>,
    pub trigger: Option<String>,
    pub trigger_label: Option<String>,
    pub created_by_kind: String,
    pub change_reason: Option<String>,
    pub actor: Actor,
    pub change_diff: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct UpdateNoteCommand {
    pub title: Option<String>,
    pub body_md: Option<String>,
    pub frontmatter_json: Option<Value>,
    pub kind: Option<Option<String>>,
    pub trigger: Option<String>,
    pub trigger_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoteWriteResult {
    pub note_id: Uuid,
    pub created: bool,
    pub snapshot: NoteSnapshot,
}
