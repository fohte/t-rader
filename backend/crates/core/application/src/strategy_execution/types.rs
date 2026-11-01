use std::sync::Arc;

use crate::custom_indicator::CustomIndicator;
use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::strategy_scope::StrategyScope;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::error::StrategyExecutionUseCaseError;

pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_PAYLOAD_CHUNK_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PAYLOAD_PAGE_BYTES: usize = 1024 * 1024;
pub const MAX_PAYLOAD_PAGE_ITEMS: usize = 1_000;
pub const DEFAULT_PAYLOAD_PAGE_ITEMS: usize = 100;
pub const MAX_PAYLOAD_MANIFEST_PAGE_ITEMS: usize = 100;
pub const DEFAULT_PAYLOAD_MANIFEST_PAGE_ITEMS: usize = 50;
pub const MAX_DATASET_PAGE_SIZE: usize = 1_000;
pub const MAX_PAYLOAD_PAGES: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedExecution {
    pub(crate) task_id: Uuid,
    pub(crate) strategy_id: Uuid,
    pub(crate) a2a_task_id: String,
    pub(crate) execution_step_id: Option<Uuid>,
}

impl AuthorizedExecution {
    pub fn task_id(&self) -> Uuid {
        self.task_id
    }

    pub fn strategy_id(&self) -> Uuid {
        self.strategy_id
    }

    pub fn execution_step_id(&self) -> Option<Uuid> {
        self.execution_step_id
    }

    pub fn a2a_task_id(&self) -> &str {
        &self.a2a_task_id
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DefinitionSnapshot {
    pub task_id: Uuid,
    pub definition_key: String,
    pub definition: JsonValue,
    pub digest: String,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedCustomIndicator {
    pub definition: CustomIndicator,
    pub digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadKind {
    Input,
    Output,
}

impl PayloadKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadStatus {
    Staging,
    Sealed,
    Incomplete,
}

impl PayloadStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Staging => "staging",
            Self::Sealed => "sealed",
            Self::Incomplete => "incomplete",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PayloadManifest {
    pub id: Uuid,
    pub task_id: Uuid,
    pub execution_step_id: Option<Uuid>,
    pub kind: PayloadKind,
    pub source: String,
    pub source_revision: Option<String>,
    pub source_locator: Option<JsonValue>,
    pub source_metadata: JsonValue,
    pub schema: JsonValue,
    pub capture_metadata: Option<PayloadCaptureMetadata>,
    pub root_is_array: bool,
    pub chunk_count: u64,
    pub item_count: u64,
    pub byte_count: u64,
    pub digest: Option<String>,
    pub status: PayloadStatus,
    pub incomplete_reason: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub sealed_at: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadManifestCursor {
    pub created_at: DateTime<FixedOffset>,
    pub id: Uuid,
}

impl PayloadManifestCursor {
    pub fn from_token(token: &str) -> Option<Self> {
        let (created_at, id) = token.split_once('|')?;
        Some(Self {
            created_at: DateTime::parse_from_rfc3339(created_at).ok()?,
            id: Uuid::parse_str(id).ok()?,
        })
    }

    pub fn to_token(&self) -> String {
        format!("{}|{}", self.created_at.to_rfc3339(), self.id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PayloadManifestPage {
    pub cursor: Option<PayloadManifestCursor>,
    pub next_cursor: Option<PayloadManifestCursor>,
    pub total_count: u64,
    pub manifests: Vec<PayloadManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPayloadManifestsQuery {
    pub cursor: Option<PayloadManifestCursor>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadCaptureMetadata {
    pub captured_at: DateTime<FixedOffset>,
    pub expected_count: Option<u64>,
    /// 対象 revision から受信した総件数。保存を拒否した page の件数も含む。
    pub received_count: u64,
    pub missing_count: Option<u64>,
    pub unretrieved_count: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PayloadDefinition {
    pub kind: PayloadKind,
    pub source: String,
    pub source_revision: Option<String>,
    pub source_locator: Option<JsonValue>,
    pub source_metadata: JsonValue,
    pub schema: JsonValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PayloadSeal {
    pub task_id: Uuid,
    pub payload_id: Uuid,
    pub digest: String,
    pub item_count: u64,
    pub byte_count: u64,
    pub sealed_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PayloadChunk {
    pub payload_id: Uuid,
    pub sequence: u64,
    pub start_item: u64,
    pub data: JsonValue,
    pub byte_count: u64,
    pub item_count: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PayloadRef {
    pub id: Uuid,
    pub task_id: Uuid,
    pub kind: PayloadKind,
    pub digest: String,
    pub item_count: u64,
    pub byte_count: u64,
    pub source_revision: Option<String>,
    pub source_locator: Option<JsonValue>,
    pub source_metadata: JsonValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PayloadPage {
    pub payload_id: Uuid,
    pub pointer: String,
    pub data: JsonValue,
    pub cursor: u64,
    pub next_cursor: Option<u64>,
    pub total_count: u64,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionPayloadRead {
    pub manifest: PayloadManifest,
    pub page: Option<PayloadPage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadPayloadQuery {
    pub payload_id: Uuid,
    pub pointer: String,
    pub cursor: u64,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DatasetSnapshot {
    pub source_revision: String,
    pub source_locator: Option<JsonValue>,
    pub source_metadata: JsonValue,
    pub schema: JsonValue,
    /// provider が固定した取得対象の時点。
    pub captured_at: DateTime<FixedOffset>,
    /// provider が開始時に把握している総件数。把握できない場合は `None`。
    pub expected_count: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DatasetPage {
    pub source_revision: String,
    pub items: Vec<JsonValue>,
    pub next_cursor: Option<String>,
    pub completion: DatasetPageCompletion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetCaptureReport {
    /// 完了時に provider が把握している総件数。把握できない場合は `None`。
    pub expected_count: Option<u64>,
    /// 対象 revision から provider が返した総件数。保存 item 数との一致が必要。
    pub received_count: u64,
    /// 対象に含まれるが取得元に存在しなかった件数。把握できない場合は `None`。
    pub missing_count: Option<u64>,
    /// provider が取得を終えられず未取得となった件数。把握できない場合は `None`。
    pub unretrieved_count: Option<u64>,
}

/// `Complete` のみ seal でき、`Incomplete` は返却済み item を保存して未完了にする。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatasetPageCompletion {
    /// 続きの page があり、`next_cursor` が必要。
    More,
    /// 全 page の取得を完了した。`next_cursor` はなく、未取得件数は `Some(0)` が必要。
    Complete(DatasetCaptureReport),
    /// provider が取得の打切りを報告した。`next_cursor` の有無に関わらず seal しない。
    Incomplete(DatasetCaptureReport),
}

#[async_trait]
pub trait DatasetProvider: Send + Sync {
    async fn open(
        &self,
        scope: StrategyScope,
        request: JsonValue,
    ) -> Result<DatasetSnapshot, crate::strategy_execution::DatasetProviderError>;

    async fn read_page(
        &self,
        snapshot: &DatasetSnapshot,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<DatasetPage, crate::strategy_execution::DatasetProviderError>;
}

pub type SharedDatasetProvider = Arc<dyn DatasetProvider + Send + Sync>;

#[async_trait]
pub trait TaskStartSnapshotter: Send + Sync {
    async fn capture_task_start(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        purpose: &str,
    ) -> Result<(), StrategyExecutionUseCaseError>;
}

pub type SharedTaskStartSnapshotter = Arc<dyn TaskStartSnapshotter + Send + Sync>;
