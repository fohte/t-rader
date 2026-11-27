use chrono::{DateTime, FixedOffset};
use uuid::Uuid;

use crate::document_storage::{Document, DocumentChunk};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSearchCriteria {
    pub query: String,
    pub as_of: Option<DateTime<FixedOffset>>,
    pub stock_id: Option<String>,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSearchResult {
    pub document: Document,
    pub version: DocumentSearchVersion,
    pub chunk: DocumentChunk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSearchVersion {
    pub id: Uuid,
    pub content_hash: String,
    pub retrieved_at: DateTime<FixedOffset>,
}
