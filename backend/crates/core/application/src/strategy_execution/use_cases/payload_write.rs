use std::collections::HashSet;

use chrono::Utc;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::strategy_execution::{
    AuthorizedExecution, DatasetPage, DatasetProvider, DatasetSnapshot, MAX_DATASET_PAGE_SIZE,
    MAX_PAYLOAD_BYTES, MAX_PAYLOAD_CHUNK_BYTES, MAX_PAYLOAD_PAGES, PayloadChunk, PayloadDefinition,
    PayloadKind, PayloadManifest, PayloadRef, PayloadSeal, PayloadStatus,
    StrategyExecutionRepositoryError, StrategyExecutionUseCaseError,
};
use crate::strategy_scope::StrategyScope;

use super::{
    StrategyExecutionUseCases, digest_bytes,
    payload_read::{assemble_payload, payload_ref, validate_instance},
};

impl StrategyExecutionUseCases {
    pub async fn begin_payload(
        &self,
        access: &AuthorizedExecution,
        definition: PayloadDefinition,
        root_is_array: bool,
    ) -> Result<Uuid, StrategyExecutionUseCaseError> {
        validate_schema(&definition.schema)?;
        let manifest = PayloadManifest {
            id: Uuid::new_v4(),
            task_id: access.task_id,
            execution_step_id: access.execution_step_id,
            kind: definition.kind,
            source: definition.source,
            source_revision: definition.source_revision,
            source_locator: definition.source_locator,
            source_metadata: definition.source_metadata,
            schema: definition.schema,
            root_is_array,
            chunk_count: 0,
            item_count: 0,
            byte_count: 0,
            digest: None,
            status: PayloadStatus::Staging,
            incomplete_reason: None,
            created_at: Utc::now().fixed_offset(),
            sealed_at: None,
        };
        let id = manifest.id;
        let transaction = self.unit_of_work.begin().await?;
        self.repository
            .insert_payload(&transaction, manifest)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(id)
    }

    pub async fn append_payload_chunk(
        &self,
        access: &AuthorizedExecution,
        payload_id: Uuid,
        source_revision: Option<&str>,
        data: JsonValue,
    ) -> Result<(), StrategyExecutionUseCaseError> {
        let manifest = self
            .repository
            .find_payload(access.task_id, payload_id)
            .await?
            .ok_or(StrategyExecutionUseCaseError::PayloadNotFound)?;
        if manifest.status != PayloadStatus::Staging {
            return Err(StrategyExecutionUseCaseError::Repository(
                StrategyExecutionRepositoryError::InvalidPayloadState,
            ));
        }
        if manifest.source_revision.as_deref() != source_revision {
            self.mark_incomplete(access.task_id, payload_id, "source_revision_changed")
                .await?;
            return Err(StrategyExecutionUseCaseError::SourceRevisionChanged);
        }

        let is_array = data.is_array();
        if is_array != manifest.root_is_array
            || (!manifest.root_is_array && manifest.chunk_count > 0)
        {
            self.mark_incomplete(access.task_id, payload_id, "invalid_chunk_shape")
                .await?;
            return Err(StrategyExecutionUseCaseError::Repository(
                StrategyExecutionRepositoryError::InconsistentPayload,
            ));
        }
        let serialized =
            serde_json::to_vec(&data).map_err(|_| StrategyExecutionUseCaseError::Serialization)?;
        if serialized.len() > MAX_PAYLOAD_CHUNK_BYTES {
            self.mark_incomplete(access.task_id, payload_id, "chunk_size_exceeded")
                .await?;
            return Err(StrategyExecutionUseCaseError::Repository(
                StrategyExecutionRepositoryError::PayloadTooLarge,
            ));
        }
        let item_count = if manifest.root_is_array {
            data.as_array()
                .map(|values| values.len() as u64)
                .ok_or(StrategyExecutionUseCaseError::Serialization)?
        } else {
            1
        };
        if manifest.byte_count.saturating_add(serialized.len() as u64) > MAX_PAYLOAD_BYTES as u64 {
            self.mark_incomplete(access.task_id, payload_id, "payload_size_exceeded")
                .await?;
            return Err(StrategyExecutionUseCaseError::Repository(
                StrategyExecutionRepositoryError::PayloadTooLarge,
            ));
        }
        let transaction = self.unit_of_work.begin().await?;
        self.repository
            .append_payload_chunk(
                &transaction,
                access.task_id,
                PayloadChunk {
                    payload_id,
                    sequence: manifest.chunk_count,
                    start_item: manifest.item_count,
                    data,
                    byte_count: serialized.len() as u64,
                    item_count,
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }

    pub async fn seal_payload(
        &self,
        access: &AuthorizedExecution,
        payload_id: Uuid,
    ) -> Result<PayloadRef, StrategyExecutionUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let manifest = self
            .repository
            .find_payload_for_update(&transaction, access.task_id, payload_id)
            .await?
            .ok_or(StrategyExecutionUseCaseError::PayloadNotFound)?;
        if manifest.status != PayloadStatus::Staging {
            return Err(StrategyExecutionUseCaseError::Repository(
                StrategyExecutionRepositoryError::InvalidPayloadState,
            ));
        }
        let chunks = self
            .repository
            .list_payload_chunks_for_update(&transaction, access.task_id, payload_id)
            .await?;
        let root = match assemble_payload(&manifest, &chunks) {
            Ok(root) => root,
            Err(error) => {
                self.repository
                    .mark_payload_incomplete(
                        &transaction,
                        access.task_id,
                        payload_id,
                        "inconsistent_chunks".to_string(),
                    )
                    .await?;
                self.unit_of_work.commit(transaction).await?;
                return Err(error);
            }
        };
        if let Err(error) = validate_instance(&manifest.schema, &root) {
            self.repository
                .mark_payload_incomplete(
                    &transaction,
                    access.task_id,
                    payload_id,
                    "schema_validation_failed".to_string(),
                )
                .await?;
            self.unit_of_work.commit(transaction).await?;
            return Err(error);
        }
        let serialized =
            serde_json::to_vec(&root).map_err(|_| StrategyExecutionUseCaseError::Serialization)?;
        if serialized.len() > MAX_PAYLOAD_BYTES {
            self.repository
                .mark_payload_incomplete(
                    &transaction,
                    access.task_id,
                    payload_id,
                    "payload_size_exceeded".to_string(),
                )
                .await?;
            self.unit_of_work.commit(transaction).await?;
            return Err(StrategyExecutionUseCaseError::Repository(
                StrategyExecutionRepositoryError::PayloadTooLarge,
            ));
        }
        let item_count = if manifest.root_is_array {
            root.as_array()
                .map(|items| items.len() as u64)
                .ok_or(StrategyExecutionUseCaseError::Serialization)?
        } else {
            1
        };
        let sealed = self
            .repository
            .mark_payload_sealed(
                &transaction,
                PayloadSeal {
                    task_id: access.task_id,
                    payload_id,
                    digest: digest_bytes(&serialized),
                    item_count,
                    byte_count: serialized.len() as u64,
                    sealed_at: Utc::now().fixed_offset(),
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        payload_ref(sealed)
    }

    pub async fn write_payload(
        &self,
        access: &AuthorizedExecution,
        definition: PayloadDefinition,
        data: JsonValue,
    ) -> Result<PayloadRef, StrategyExecutionUseCaseError> {
        let root_is_array = data.is_array();
        let source_revision = definition.source_revision.clone();
        let payload_id = self
            .begin_payload(access, definition, root_is_array)
            .await?;
        let chunks = match &data {
            JsonValue::Array(items) => match split_array_chunks(items) {
                Ok(chunks) => chunks,
                Err(error) => {
                    self.mark_incomplete(access.task_id, payload_id, "chunk_size_exceeded")
                        .await?;
                    return Err(error);
                }
            },
            _ => vec![data],
        };
        for chunk in chunks {
            self.append_payload_chunk(access, payload_id, source_revision.as_deref(), chunk)
                .await?;
        }
        self.seal_payload(access, payload_id).await
    }

    pub async fn capture_dataset(
        &self,
        scope: StrategyScope,
        access: &AuthorizedExecution,
        provider_name: String,
        provider: &dyn DatasetProvider,
        request: JsonValue,
        page_size: usize,
    ) -> Result<PayloadRef, StrategyExecutionUseCaseError> {
        if scope.id() != access.strategy_id {
            return Err(StrategyExecutionUseCaseError::StrategyMismatch);
        }
        if !(1..=MAX_DATASET_PAGE_SIZE).contains(&page_size) {
            return Err(StrategyExecutionUseCaseError::InvalidPageLimit);
        }
        let snapshot = provider.open(scope, request).await?;
        if snapshot.source_revision.trim().is_empty() {
            return Err(StrategyExecutionUseCaseError::InvalidDefinitionSnapshot);
        }
        let payload_id = self
            .begin_payload(
                access,
                PayloadDefinition {
                    kind: PayloadKind::Input,
                    source: provider_name,
                    source_revision: Some(snapshot.source_revision.clone()),
                    source_locator: snapshot.source_locator.clone(),
                    source_metadata: snapshot.source_metadata.clone(),
                    schema: snapshot.schema.clone(),
                },
                true,
            )
            .await?;
        let mut cursor: Option<String> = None;
        let mut seen_cursors = HashSet::new();
        for page_index in 0..MAX_PAYLOAD_PAGES {
            let page = match provider
                .read_page(&snapshot, cursor.as_deref(), page_size)
                .await
            {
                Ok(page) => page,
                Err(error) => {
                    self.mark_incomplete(access.task_id, payload_id, "provider_failed")
                        .await?;
                    return Err(error.into());
                }
            };
            if page.source_revision != snapshot.source_revision {
                self.mark_incomplete(access.task_id, payload_id, "source_revision_changed")
                    .await?;
                return Err(StrategyExecutionUseCaseError::SourceRevisionChanged);
            }
            if page.items.len() > page_size {
                self.mark_incomplete(access.task_id, payload_id, "provider_page_size_exceeded")
                    .await?;
                return Err(StrategyExecutionUseCaseError::InvalidDatasetCursor);
            }
            let next_cursor = page.next_cursor.clone();
            self.append_dataset_page(access, payload_id, &snapshot, page)
                .await?;
            let Some(next) = next_cursor else {
                return self.seal_payload(access, payload_id).await;
            };
            if next.trim().is_empty()
                || cursor.as_ref() == Some(&next)
                || !seen_cursors.insert(next.clone())
                || page_index + 1 == MAX_PAYLOAD_PAGES
            {
                self.mark_incomplete(access.task_id, payload_id, "invalid_provider_cursor")
                    .await?;
                return Err(StrategyExecutionUseCaseError::InvalidDatasetCursor);
            }
            cursor = Some(next);
        }
        self.mark_incomplete(access.task_id, payload_id, "page_limit_exceeded")
            .await?;
        Err(StrategyExecutionUseCaseError::InvalidDatasetCursor)
    }
    async fn append_dataset_page(
        &self,
        access: &AuthorizedExecution,
        payload_id: Uuid,
        snapshot: &DatasetSnapshot,
        page: DatasetPage,
    ) -> Result<(), StrategyExecutionUseCaseError> {
        self.append_payload_chunk(
            access,
            payload_id,
            Some(&snapshot.source_revision),
            JsonValue::Array(page.items),
        )
        .await
    }

    async fn mark_incomplete(
        &self,
        task_id: Uuid,
        payload_id: Uuid,
        reason: &str,
    ) -> Result<(), StrategyExecutionUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        self.repository
            .mark_payload_incomplete(&transaction, task_id, payload_id, reason.to_string())
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }
}

fn validate_schema(schema: &JsonValue) -> Result<(), StrategyExecutionUseCaseError> {
    crate::json_schema::validate_definition(schema)
        .map_err(StrategyExecutionUseCaseError::InvalidPayloadSchema)
}

fn split_array_chunks(
    items: &[JsonValue],
) -> Result<Vec<JsonValue>, StrategyExecutionUseCaseError> {
    if items.is_empty() {
        return Ok(vec![JsonValue::Array(Vec::new())]);
    }

    let mut chunks = Vec::new();
    let mut current = Vec::new();
    let mut current_bytes = 2;
    for item in items {
        let item_bytes = serde_json::to_vec(item)
            .map_err(|_| StrategyExecutionUseCaseError::Serialization)?
            .len();
        if item_bytes + 2 > MAX_PAYLOAD_CHUNK_BYTES {
            return Err(StrategyExecutionUseCaseError::Repository(
                StrategyExecutionRepositoryError::PayloadTooLarge,
            ));
        }
        let separator_bytes = usize::from(!current.is_empty());
        if !current.is_empty()
            && current_bytes + separator_bytes + item_bytes > MAX_PAYLOAD_CHUNK_BYTES
        {
            chunks.push(JsonValue::Array(std::mem::take(&mut current)));
            current_bytes = 2;
        }
        current_bytes += usize::from(!current.is_empty()) + item_bytes;
        current.push(item.clone());
    }
    if !current.is_empty() {
        chunks.push(JsonValue::Array(current));
    }
    Ok(chunks)
}
