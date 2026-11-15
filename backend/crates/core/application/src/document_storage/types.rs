use chrono::{DateTime, FixedOffset};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentMetadata {
    pub source: String,
    pub source_document_id: String,
    pub stock_id: Option<String>,
    pub source_issuer_code: String,
    pub title: String,
    pub published_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub id: Uuid,
    pub metadata: DocumentMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentVersion {
    pub id: Uuid,
    pub document_id: Uuid,
    pub content_hash: String,
    pub retrieved_at: DateTime<FixedOffset>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentChunkInput {
    pub sequence: i32,
    pub start_offset: i32,
    pub end_offset: i32,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentChunk {
    pub id: Uuid,
    pub version_id: Uuid,
    pub sequence: i32,
    pub start_offset: i32,
    pub end_offset: i32,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSnapshot {
    pub document: Document,
    pub version: DocumentVersion,
    pub chunks: Vec<DocumentChunk>,
}
