use chrono::{DateTime, FixedOffset};
use uuid::Uuid;

use crate::change_history::Actor;
use crate::strategy_scope::StrategyScope;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommentTargetKind {
    NoteVersion,
    Annotation,
}

impl CommentTargetKind {
    pub const ALL: [Self; 2] = [Self::NoteVersion, Self::Annotation];

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|target_kind| target_kind.as_str() == value)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoteVersion => "note_version",
            Self::Annotation => "annotation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub id: Uuid,
    pub target_kind: String,
    pub target_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub body: String,
    pub author_kind: String,
    pub author_label: String,
    pub resolved: bool,
    pub created_at: DateTime<FixedOffset>,
    pub anchor_text: Option<String>,
    pub anchor_side: Option<String>,
    pub start_line: Option<i32>,
    pub end_line: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewComment {
    pub id: Uuid,
    pub target_kind: CommentTargetKind,
    pub target_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub body: String,
    pub author_kind: String,
    pub author_label: String,
    pub anchor_text: Option<String>,
    pub anchor_side: Option<String>,
    pub start_line: Option<i32>,
    pub end_line: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteVersionAnchorBodies {
    pub version_no: i32,
    pub current_body: String,
    pub previous_body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateCommentCommand {
    pub target_kind: String,
    pub target_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub body: String,
    pub author_kind: String,
    pub author_label: String,
    pub anchor_text: Option<String>,
    pub anchor_side: Option<String>,
    pub start_line: Option<i32>,
    pub end_line: Option<i32>,
    pub scope: Option<StrategyScope>,
    pub actor: Actor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyCommentCommand {
    pub parent_id: Uuid,
    pub body: String,
    pub author_kind: String,
    pub author_label: String,
    pub scope: Option<StrategyScope>,
    pub actor: Actor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveCommentCommand {
    pub id: Uuid,
    pub resolved: bool,
    pub scope: Option<StrategyScope>,
    pub actor: Actor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteCommentCommand {
    pub id: Uuid,
    pub scope: Option<StrategyScope>,
    pub actor: Actor,
}
