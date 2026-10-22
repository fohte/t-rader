use chrono::{DateTime, FixedOffset};
use core_application::strategy_execution::{
    DefinitionSnapshot, ExecutionPayloadRead, PayloadCaptureMetadata, PayloadManifest,
    PayloadManifestCursor, PayloadManifestPage, PayloadPage,
};
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct ExecutionArtifactsResponse {
    pub task_id: Uuid,
    pub definition_snapshots: Vec<ExecutionDefinitionSnapshotResponse>,
}

impl ExecutionArtifactsResponse {
    pub fn new(task_id: Uuid, snapshots: Vec<DefinitionSnapshot>) -> Self {
        Self {
            task_id,
            definition_snapshots: snapshots
                .into_iter()
                .map(ExecutionDefinitionSnapshotResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExecutionPayloadManifestPageResponse {
    pub task_id: Uuid,
    pub cursor: Option<String>,
    pub next_cursor: Option<String>,
    /// 取得時点の現在総数。追加分は cursor なしで先頭ページを再読すると確認できる。
    pub total_count: u64,
    pub items: Vec<ExecutionPayloadManifestResponse>,
}

impl ExecutionPayloadManifestPageResponse {
    pub fn new(task_id: Uuid, page: PayloadManifestPage) -> Self {
        Self {
            task_id,
            cursor: page.cursor.as_ref().map(PayloadManifestCursor::to_token),
            next_cursor: page
                .next_cursor
                .as_ref()
                .map(PayloadManifestCursor::to_token),
            total_count: page.total_count,
            items: page
                .manifests
                .into_iter()
                .map(ExecutionPayloadManifestResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExecutionDefinitionSnapshotResponse {
    pub task_id: Uuid,
    pub definition_key: String,
    #[schema(value_type = serde_json::Value)]
    pub definition: Value,
    pub digest: String,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: DateTime<FixedOffset>,
}

impl From<DefinitionSnapshot> for ExecutionDefinitionSnapshotResponse {
    fn from(snapshot: DefinitionSnapshot) -> Self {
        Self {
            task_id: snapshot.task_id,
            definition_key: snapshot.definition_key,
            definition: snapshot.definition,
            digest: snapshot.digest,
            created_at: snapshot.created_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExecutionPayloadResponse {
    pub manifest: ExecutionPayloadManifestResponse,
    pub page: Option<ExecutionPayloadPageResponse>,
}

impl From<ExecutionPayloadRead> for ExecutionPayloadResponse {
    fn from(read: ExecutionPayloadRead) -> Self {
        Self {
            manifest: ExecutionPayloadManifestResponse::from(read.manifest),
            page: read.page.map(ExecutionPayloadPageResponse::from),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExecutionPayloadManifestResponse {
    pub id: Uuid,
    pub task_id: Uuid,
    pub execution_step_id: Option<Uuid>,
    pub kind: String,
    pub source: String,
    pub source_revision: Option<String>,
    #[schema(value_type = Option<serde_json::Value>)]
    pub source_locator: Option<Value>,
    #[schema(value_type = serde_json::Value)]
    pub source_metadata: Value,
    #[schema(value_type = serde_json::Value)]
    pub schema: Value,
    pub capture_metadata: Option<ExecutionPayloadCaptureMetadataResponse>,
    pub root_is_array: bool,
    pub chunk_count: u64,
    pub item_count: u64,
    pub byte_count: u64,
    pub digest: Option<String>,
    pub status: String,
    pub incomplete_reason: Option<String>,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: DateTime<FixedOffset>,
    #[schema(value_type = Option<chrono::DateTime<chrono::Utc>>)]
    pub sealed_at: Option<DateTime<FixedOffset>>,
}

impl From<PayloadManifest> for ExecutionPayloadManifestResponse {
    fn from(manifest: PayloadManifest) -> Self {
        Self {
            id: manifest.id,
            task_id: manifest.task_id,
            execution_step_id: manifest.execution_step_id,
            kind: manifest.kind.as_str().to_string(),
            source: manifest.source,
            source_revision: manifest.source_revision,
            source_locator: manifest.source_locator,
            source_metadata: manifest.source_metadata,
            schema: manifest.schema,
            capture_metadata: manifest
                .capture_metadata
                .map(ExecutionPayloadCaptureMetadataResponse::from),
            root_is_array: manifest.root_is_array,
            chunk_count: manifest.chunk_count,
            item_count: manifest.item_count,
            byte_count: manifest.byte_count,
            digest: manifest.digest,
            status: manifest.status.as_str().to_string(),
            incomplete_reason: manifest.incomplete_reason,
            created_at: manifest.created_at,
            sealed_at: manifest.sealed_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExecutionPayloadCaptureMetadataResponse {
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub captured_at: DateTime<FixedOffset>,
    pub expected_count: Option<u64>,
    pub received_count: u64,
    pub missing_count: Option<u64>,
    pub unretrieved_count: Option<u64>,
}

impl From<PayloadCaptureMetadata> for ExecutionPayloadCaptureMetadataResponse {
    fn from(metadata: PayloadCaptureMetadata) -> Self {
        Self {
            captured_at: metadata.captured_at,
            expected_count: metadata.expected_count,
            received_count: metadata.received_count,
            missing_count: metadata.missing_count,
            unretrieved_count: metadata.unretrieved_count,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExecutionPayloadPageResponse {
    pub payload_id: Uuid,
    pub pointer: String,
    #[schema(value_type = serde_json::Value)]
    pub data: Value,
    pub cursor: u64,
    pub next_cursor: Option<u64>,
    pub total_count: u64,
    pub digest: String,
}

impl From<PayloadPage> for ExecutionPayloadPageResponse {
    fn from(page: PayloadPage) -> Self {
        Self {
            payload_id: page.payload_id,
            pointer: page.pointer,
            data: page.data,
            cursor: page.cursor,
            next_cursor: page.next_cursor,
            total_count: page.total_count,
            digest: page.digest,
        }
    }
}
