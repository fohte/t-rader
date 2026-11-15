use chrono::{DateTime, FixedOffset};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchDocumentsParams {
    /// 全文検索する語句。各語は正規化後に 2 文字以上である必要がある。
    pub query: String,
    /// 公開時刻と取得済みバージョンを制限する基準時刻。
    pub as_of: Option<DateTime<FixedOffset>>,
    /// 発行企業の銘柄 ID。
    pub stock_id: Option<String>,
    /// 返す文書片の最大件数。省略時は 20。
    pub limit: Option<u32>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct SearchDocumentsResult {
    pub items: Vec<SearchDocumentItemDto>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct SearchDocumentItemDto {
    pub document: DocumentDto,
    pub version: DocumentVersionReferenceDto,
    pub chunk: DocumentChunkDto,
    pub mentions: Vec<DocumentMentionDto>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct DocumentDto {
    pub id: Uuid,
    pub source: String,
    pub source_document_id: String,
    /// 文書の発行企業。本文中で言及された銘柄とは別の値。
    pub stock_id: Option<String>,
    pub source_issuer_code: String,
    pub title: String,
    pub published_at: DateTime<FixedOffset>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct DocumentVersionReferenceDto {
    pub id: Uuid,
    pub content_hash: String,
    pub retrieved_at: DateTime<FixedOffset>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct DocumentVersionDto {
    pub id: Uuid,
    pub content_hash: String,
    pub retrieved_at: DateTime<FixedOffset>,
    pub body: String,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct DocumentChunkDto {
    pub id: Uuid,
    pub sequence: i32,
    pub start_offset: i32,
    pub end_offset: i32,
    pub body: String,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct DocumentMentionDto {
    pub stock_id: String,
    pub start_offset: i32,
    pub end_offset: i32,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetDocumentChunkParams {
    /// `search_documents` が返す文書片 ID。
    pub id: Uuid,
    /// 公開時刻と取得済みバージョンを制限する基準時刻。
    pub as_of: Option<DateTime<FixedOffset>>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct GetDocumentChunkResult {
    pub document: DocumentDto,
    pub version: DocumentVersionDto,
    pub previous_chunk: Option<DocumentChunkDto>,
    pub chunk: DocumentChunkDto,
    pub next_chunk: Option<DocumentChunkDto>,
    pub mentions: Vec<DocumentMentionDto>,
}
