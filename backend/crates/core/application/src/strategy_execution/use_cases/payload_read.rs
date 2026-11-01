use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::strategy_execution::{
    AuthorizedExecution, MAX_PAYLOAD_PAGE_BYTES, MAX_PAYLOAD_PAGE_ITEMS, PayloadChunk,
    PayloadManifest, PayloadPage, PayloadRef, PayloadStatus, ReadPayloadQuery,
    StrategyExecutionRepository, StrategyExecutionRepositoryError, StrategyExecutionUseCaseError,
};

use super::StrategyExecutionUseCases;

impl StrategyExecutionUseCases {
    pub async fn read_payload(
        &self,
        access: &AuthorizedExecution,
        query: ReadPayloadQuery,
    ) -> Result<PayloadPage, StrategyExecutionUseCaseError> {
        self.read_payload_by_task_id(access.task_id, query).await
    }

    pub(super) async fn read_payload_by_task_id(
        &self,
        task_id: Uuid,
        query: ReadPayloadQuery,
    ) -> Result<PayloadPage, StrategyExecutionUseCaseError> {
        validate_page_limit(query.limit)?;
        let manifest = self
            .repository
            .find_payload(task_id, query.payload_id)
            .await?
            .ok_or(StrategyExecutionUseCaseError::PayloadNotFound)?;
        if manifest.status != PayloadStatus::Sealed {
            return Err(StrategyExecutionUseCaseError::PayloadNotSealed);
        }
        self.read_sealed_payload_page(task_id, &manifest, query)
            .await
    }

    // Use the already fetched manifest so the page and its digest use the same sealed record.
    pub(super) async fn read_sealed_payload_page(
        &self,
        task_id: Uuid,
        manifest: &PayloadManifest,
        query: ReadPayloadQuery,
    ) -> Result<PayloadPage, StrategyExecutionUseCaseError> {
        if manifest.id != query.payload_id {
            return Err(StrategyExecutionUseCaseError::PayloadNotFound);
        }
        let (data, next_cursor, total_count) = if manifest.root_is_array {
            if query.pointer.is_empty() {
                read_array_page(
                    self.repository.as_ref(),
                    task_id,
                    manifest,
                    query.cursor,
                    query.limit,
                )
                .await?
            } else {
                let (root_index, nested_pointer) = parse_array_pointer(&query.pointer)?;
                if root_index >= manifest.item_count {
                    return Err(StrategyExecutionUseCaseError::InvalidPayloadPointer);
                }
                let chunks = self
                    .repository
                    .list_payload_chunks_for_range(task_id, query.payload_id, root_index, 1)
                    .await?;
                let items = array_items_in_range(&chunks, root_index, 1)?;
                let root_item = items
                    .into_iter()
                    .next()
                    .ok_or(StrategyExecutionUseCaseError::InvalidPayloadPointer)?;
                let selected = if nested_pointer.is_empty() {
                    root_item
                } else {
                    root_item
                        .pointer(&nested_pointer)
                        .cloned()
                        .ok_or(StrategyExecutionUseCaseError::InvalidPayloadPointer)?
                };
                page_value(selected, query.cursor, query.limit)?
            }
        } else {
            let chunks = self
                .repository
                .list_payload_chunks(task_id, query.payload_id)
                .await?;
            let root = assemble_payload(manifest, &chunks)?;
            let selected = if query.pointer.is_empty() {
                root
            } else {
                root.pointer(&query.pointer)
                    .cloned()
                    .ok_or(StrategyExecutionUseCaseError::InvalidPayloadPointer)?
            };
            page_value(selected, query.cursor, query.limit)?
        };
        if serde_json::to_vec(&data)
            .map_err(|_| StrategyExecutionUseCaseError::Serialization)?
            .len()
            > MAX_PAYLOAD_PAGE_BYTES
        {
            return Err(StrategyExecutionUseCaseError::PayloadPageTooLarge);
        }
        Ok(PayloadPage {
            payload_id: query.payload_id,
            pointer: query.pointer,
            data,
            cursor: query.cursor,
            next_cursor,
            total_count,
            digest: manifest
                .digest
                .clone()
                .ok_or(StrategyExecutionUseCaseError::PayloadNotSealed)?,
        })
    }
}

pub(super) fn validate_page_limit(limit: usize) -> Result<(), StrategyExecutionUseCaseError> {
    if !(1..=MAX_PAYLOAD_PAGE_ITEMS).contains(&limit) {
        return Err(StrategyExecutionUseCaseError::InvalidPageLimit);
    }
    Ok(())
}

async fn read_array_page(
    repository: &dyn StrategyExecutionRepository,
    task_id: Uuid,
    manifest: &PayloadManifest,
    cursor: u64,
    limit: usize,
) -> Result<(JsonValue, Option<u64>, u64), StrategyExecutionUseCaseError> {
    if cursor > manifest.item_count {
        return Err(StrategyExecutionUseCaseError::InvalidPayloadPointer);
    }
    let end = cursor.saturating_add(limit as u64).min(manifest.item_count);
    let requested_count = usize::try_from(end - cursor)
        .map_err(|_| StrategyExecutionUseCaseError::InvalidPayloadPointer)?;
    let chunks = repository
        .list_payload_chunks_for_range(task_id, manifest.id, cursor, requested_count)
        .await?;
    let items = array_items_in_range(&chunks, cursor, requested_count)?;
    if items.len() != requested_count {
        return Err(StrategyExecutionUseCaseError::Repository(
            StrategyExecutionRepositoryError::InconsistentPayload,
        ));
    }
    Ok((
        JsonValue::Array(items),
        (end < manifest.item_count).then_some(end),
        manifest.item_count,
    ))
}

fn array_items_in_range(
    chunks: &[PayloadChunk],
    start: u64,
    item_count: usize,
) -> Result<Vec<JsonValue>, StrategyExecutionUseCaseError> {
    if item_count == 0 {
        return Ok(Vec::new());
    }
    let end = start.saturating_add(item_count as u64);
    let mut expected_item = start;
    let mut items = Vec::with_capacity(item_count);
    for chunk in chunks {
        let values = chunk.data.as_array().ok_or({
            StrategyExecutionUseCaseError::Repository(
                StrategyExecutionRepositoryError::InconsistentPayload,
            )
        })?;
        if values.len() as u64 != chunk.item_count {
            return Err(StrategyExecutionUseCaseError::Repository(
                StrategyExecutionRepositoryError::InconsistentPayload,
            ));
        }
        let chunk_end = chunk
            .start_item
            .checked_add(chunk.item_count)
            .ok_or(StrategyExecutionUseCaseError::Serialization)?;
        let overlap_start = start.max(chunk.start_item);
        let overlap_end = end.min(chunk_end);
        if overlap_start < overlap_end {
            if overlap_start != expected_item {
                return Err(StrategyExecutionUseCaseError::Repository(
                    StrategyExecutionRepositoryError::InconsistentPayload,
                ));
            }
            let from = usize::try_from(overlap_start - chunk.start_item)
                .map_err(|_| StrategyExecutionUseCaseError::Serialization)?;
            let to = usize::try_from(overlap_end - chunk.start_item)
                .map_err(|_| StrategyExecutionUseCaseError::Serialization)?;
            items.extend(values[from..to].iter().cloned());
            expected_item = overlap_end;
        }
    }
    if expected_item != end {
        return Err(StrategyExecutionUseCaseError::Repository(
            StrategyExecutionRepositoryError::InconsistentPayload,
        ));
    }
    Ok(items)
}

fn parse_array_pointer(pointer: &str) -> Result<(u64, String), StrategyExecutionUseCaseError> {
    let rest = pointer
        .strip_prefix('/')
        .ok_or(StrategyExecutionUseCaseError::InvalidPayloadPointer)?;
    let (index, nested_pointer) = match rest.split_once('/') {
        Some((index, nested)) => (index, format!("/{nested}")),
        None => (rest, String::new()),
    };
    let index = index
        .parse::<u64>()
        .map_err(|_| StrategyExecutionUseCaseError::InvalidPayloadPointer)?;
    if index.to_string() != rest.split('/').next().unwrap_or_default() {
        return Err(StrategyExecutionUseCaseError::InvalidPayloadPointer);
    }
    Ok((index, nested_pointer))
}

fn page_value(
    selected: JsonValue,
    cursor: u64,
    limit: usize,
) -> Result<(JsonValue, Option<u64>, u64), StrategyExecutionUseCaseError> {
    match selected {
        JsonValue::Array(items) => {
            let start = usize::try_from(cursor)
                .map_err(|_| StrategyExecutionUseCaseError::InvalidPayloadPointer)?;
            if start > items.len() {
                return Err(StrategyExecutionUseCaseError::InvalidPayloadPointer);
            }
            let end = start.saturating_add(limit).min(items.len());
            Ok((
                JsonValue::Array(items[start..end].to_vec()),
                (end < items.len()).then_some(end as u64),
                items.len() as u64,
            ))
        }
        value if cursor == 0 => Ok((value, None, 1)),
        _ => Err(StrategyExecutionUseCaseError::InvalidPayloadPointer),
    }
}

pub(super) fn validate_instance(
    schema: &JsonValue,
    instance: &JsonValue,
) -> Result<(), StrategyExecutionUseCaseError> {
    match crate::json_schema::validate_instance(schema, instance) {
        Ok(()) => Ok(()),
        Err(crate::json_schema::JsonSchemaError::InvalidSchema(message)) => {
            Err(StrategyExecutionUseCaseError::InvalidPayloadSchema(message))
        }
        Err(crate::json_schema::JsonSchemaError::Mismatch(message)) => Err(
            StrategyExecutionUseCaseError::PayloadSchemaMismatch(message),
        ),
    }
}

pub(super) fn assemble_payload(
    manifest: &PayloadManifest,
    chunks: &[PayloadChunk],
) -> Result<JsonValue, StrategyExecutionUseCaseError> {
    if chunks.len() as u64 != manifest.chunk_count {
        return Err(StrategyExecutionUseCaseError::Repository(
            StrategyExecutionRepositoryError::InconsistentPayload,
        ));
    }
    if manifest.root_is_array {
        let mut items = Vec::with_capacity(manifest.item_count as usize);
        let mut expected_start_item = 0;
        for (expected_sequence, chunk) in chunks.iter().enumerate() {
            if chunk.sequence != expected_sequence as u64 || chunk.start_item != expected_start_item
            {
                return Err(StrategyExecutionUseCaseError::Repository(
                    StrategyExecutionRepositoryError::InconsistentPayload,
                ));
            }
            let values = chunk.data.as_array().ok_or({
                StrategyExecutionUseCaseError::Repository(
                    StrategyExecutionRepositoryError::InconsistentPayload,
                )
            })?;
            if values.len() as u64 != chunk.item_count {
                return Err(StrategyExecutionUseCaseError::Repository(
                    StrategyExecutionRepositoryError::InconsistentPayload,
                ));
            }
            items.extend(values.iter().cloned());
            expected_start_item = expected_start_item
                .checked_add(chunk.item_count)
                .ok_or(StrategyExecutionUseCaseError::Serialization)?;
        }
        if expected_start_item != manifest.item_count {
            return Err(StrategyExecutionUseCaseError::Repository(
                StrategyExecutionRepositoryError::InconsistentPayload,
            ));
        }
        Ok(JsonValue::Array(items))
    } else {
        match chunks {
            [chunk] if chunk.sequence == 0 && chunk.start_item == 0 && chunk.item_count == 1 => {
                Ok(chunk.data.clone())
            }
            _ => Err(StrategyExecutionUseCaseError::Repository(
                StrategyExecutionRepositoryError::InconsistentPayload,
            )),
        }
    }
}

pub(super) fn payload_ref(
    manifest: PayloadManifest,
) -> Result<PayloadRef, StrategyExecutionUseCaseError> {
    if manifest.status != PayloadStatus::Sealed {
        return Err(StrategyExecutionUseCaseError::PayloadNotSealed);
    }
    Ok(PayloadRef {
        id: manifest.id,
        task_id: manifest.task_id,
        kind: manifest.kind,
        digest: manifest
            .digest
            .ok_or(StrategyExecutionUseCaseError::PayloadNotSealed)?,
        item_count: manifest.item_count,
        byte_count: manifest.byte_count,
        source_revision: manifest.source_revision,
        source_locator: manifest.source_locator,
        source_metadata: manifest.source_metadata,
    })
}
