use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

mod query;
mod use_cases;

#[cfg(feature = "test-support")]
mod fake;

pub use query::{
    ChangeHistoryEntry, ChangeHistoryListQuery, ChangeHistoryQuery, ChangeHistoryQueryError,
    SharedChangeHistoryQuery,
};
pub use use_cases::{ChangeHistoryUseCaseError, ChangeHistoryUseCases};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor {
    Human,
    Llm { label: &'static str },
}

impl Actor {
    pub fn kind(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Llm { .. } => "llm",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Human => "user",
            Self::Llm { label } => label,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Note,
    Annotation,
    Strategy,
    Trade,
    Comment,
    CustomIndicator,
    NoteKind,
}

impl TargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Annotation => "annotation",
            Self::Strategy => "strategy",
            Self::Trade => "trade",
            Self::Comment => "comment",
            Self::CustomIndicator => "custom_indicator",
            Self::NoteKind => "note_kind",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Create,
    Update,
    Delete,
    StatusChange,
}

impl Op {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::StatusChange => "status_change",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChangeHistoryRecord {
    pub actor: Actor,
    pub target_kind: TargetKind,
    pub target_id: Uuid,
    pub op: Op,
    pub diff: Value,
    pub summary: Option<String>,
}

#[derive(Debug, Error)]
pub enum ChangeHistoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait ChangeHistoryPort: Send + Sync {
    async fn record(
        &self,
        transaction: &UnitOfWorkTransaction,
        record: ChangeHistoryRecord,
    ) -> Result<(), ChangeHistoryError>;
}

pub type SharedChangeHistoryPort = std::sync::Arc<dyn ChangeHistoryPort + Send + Sync>;

#[cfg(feature = "test-support")]
pub use fake::{FakeChangeHistory, FakeChangeHistoryEntry};
