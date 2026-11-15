use chrono::{DateTime, FixedOffset};

use crate::document_storage::{Document, DocumentChunk, DocumentVersion};

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
    pub version: DocumentVersion,
    pub chunk: DocumentChunk,
}
