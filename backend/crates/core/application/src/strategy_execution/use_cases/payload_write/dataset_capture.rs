use std::collections::HashSet;

use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::strategy_execution::{
    AuthorizedExecution, DatasetPage, DatasetProvider, DatasetSnapshot, MAX_DATASET_PAGE_SIZE,
    MAX_PAYLOAD_PAGES, PayloadDefinition, PayloadKind, PayloadRef, StrategyExecutionUseCaseError,
};
use crate::strategy_scope::StrategyScope;

use super::super::StrategyExecutionUseCases;

impl StrategyExecutionUseCases {
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
}
