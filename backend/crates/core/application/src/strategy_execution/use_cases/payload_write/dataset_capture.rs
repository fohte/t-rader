use std::collections::HashSet;

use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::strategy_execution::{
    AuthorizedExecution, DatasetCaptureReport, DatasetPage, DatasetPageCompletion, DatasetProvider,
    DatasetSnapshot, MAX_DATASET_PAGE_SIZE, MAX_PAYLOAD_PAGES, PayloadCaptureMetadata,
    PayloadDefinition, PayloadKind, PayloadRef, StrategyExecutionUseCaseError,
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
        let mut capture_metadata = PayloadCaptureMetadata {
            captured_at: snapshot.captured_at,
            expected_count: snapshot.expected_count,
            received_count: 0,
            missing_count: None,
            unretrieved_count: None,
        };
        self.update_capture_metadata(access.task_id, payload_id, capture_metadata.clone())
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
                let page_item_count = u64::try_from(page.items.len())
                    .map_err(|_| StrategyExecutionUseCaseError::InvalidDatasetCompletion)?;
                capture_metadata.received_count = capture_metadata
                    .received_count
                    .checked_add(page_item_count)
                    .ok_or(StrategyExecutionUseCaseError::InvalidDatasetCompletion)?;
                self.update_capture_metadata(access.task_id, payload_id, capture_metadata.clone())
                    .await?;
                self.mark_incomplete(access.task_id, payload_id, "provider_page_size_exceeded")
                    .await?;
                return Err(StrategyExecutionUseCaseError::InvalidDatasetCursor);
            }
            let DatasetPage {
                items,
                next_cursor,
                completion,
                ..
            } = page;
            let page_item_count = u64::try_from(items.len())
                .map_err(|_| StrategyExecutionUseCaseError::InvalidDatasetCompletion)?;
            self.append_dataset_page(access, payload_id, &snapshot, items)
                .await?;
            capture_metadata.received_count = capture_metadata
                .received_count
                .checked_add(page_item_count)
                .ok_or(StrategyExecutionUseCaseError::InvalidDatasetCompletion)?;
            self.update_capture_metadata(access.task_id, payload_id, capture_metadata.clone())
                .await?;

            let next = match completion {
                DatasetPageCompletion::More => {
                    let Some(next) = next_cursor else {
                        self.mark_incomplete(
                            access.task_id,
                            payload_id,
                            "provider_completion_unconfirmed",
                        )
                        .await?;
                        return Err(StrategyExecutionUseCaseError::DatasetCaptureIncomplete);
                    };
                    next
                }
                DatasetPageCompletion::Complete(report) => {
                    if next_cursor.is_some() {
                        self.mark_incomplete(
                            access.task_id,
                            payload_id,
                            "invalid_provider_completion",
                        )
                        .await?;
                        return Err(StrategyExecutionUseCaseError::InvalidDatasetCursor);
                    }
                    let Some(final_metadata) =
                        completion_metadata(&snapshot, &capture_metadata, &report, true)
                    else {
                        self.mark_incomplete(
                            access.task_id,
                            payload_id,
                            "invalid_provider_completion",
                        )
                        .await?;
                        return Err(StrategyExecutionUseCaseError::InvalidDatasetCompletion);
                    };
                    self.update_capture_metadata(access.task_id, payload_id, final_metadata)
                        .await?;
                    return self.seal_payload(access, payload_id).await;
                }
                DatasetPageCompletion::Incomplete(report) => {
                    let Some(final_metadata) =
                        completion_metadata(&snapshot, &capture_metadata, &report, false)
                    else {
                        self.mark_incomplete(
                            access.task_id,
                            payload_id,
                            "invalid_provider_completion",
                        )
                        .await?;
                        return Err(StrategyExecutionUseCaseError::InvalidDatasetCompletion);
                    };
                    self.update_capture_metadata(access.task_id, payload_id, final_metadata)
                        .await?;
                    self.mark_incomplete(access.task_id, payload_id, "provider_incomplete")
                        .await?;
                    return Err(StrategyExecutionUseCaseError::DatasetCaptureIncomplete);
                }
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
        items: Vec<JsonValue>,
    ) -> Result<(), StrategyExecutionUseCaseError> {
        self.append_payload_chunk(
            access,
            payload_id,
            Some(&snapshot.source_revision),
            JsonValue::Array(items),
        )
        .await
    }

    async fn update_capture_metadata(
        &self,
        task_id: Uuid,
        payload_id: Uuid,
        metadata: PayloadCaptureMetadata,
    ) -> Result<(), StrategyExecutionUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        self.repository
            .update_payload_capture_metadata(&transaction, task_id, payload_id, metadata)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }
}

fn completion_metadata(
    snapshot: &DatasetSnapshot,
    current: &PayloadCaptureMetadata,
    report: &DatasetCaptureReport,
    complete: bool,
) -> Option<PayloadCaptureMetadata> {
    if report.received_count != current.received_count
        || snapshot
            .expected_count
            .zip(report.expected_count)
            .is_some_and(|(snapshot, report)| snapshot != report)
    {
        return None;
    }

    let expected_count = report.expected_count.or(snapshot.expected_count);
    if !is_consistent_capture_count(
        expected_count,
        report.received_count,
        report.missing_count,
        report.unretrieved_count,
        complete,
    ) {
        return None;
    }

    Some(PayloadCaptureMetadata {
        captured_at: snapshot.captured_at,
        expected_count,
        received_count: current.received_count,
        missing_count: report.missing_count,
        unretrieved_count: report.unretrieved_count,
    })
}

pub(super) fn is_consistent_capture_count(
    expected_count: Option<u64>,
    received_count: u64,
    missing_count: Option<u64>,
    unretrieved_count: Option<u64>,
    complete: bool,
) -> bool {
    if complete && unretrieved_count != Some(0) {
        return false;
    }
    let Some(expected_count) = expected_count else {
        return true;
    };
    let Some(received_and_missing) = missing_count
        .map(|missing| received_count.checked_add(missing))
        .unwrap_or(Some(received_count))
    else {
        return false;
    };
    let Some(total_count) = unretrieved_count
        .map(|unretrieved| received_and_missing.checked_add(unretrieved))
        .unwrap_or(Some(received_and_missing))
    else {
        return false;
    };
    if complete {
        let Some(missing_count) = missing_count else {
            return received_count == expected_count;
        };
        received_count
            .checked_add(missing_count)
            .is_some_and(|count| count == expected_count)
    } else if missing_count.is_some() && unretrieved_count.is_some() {
        total_count == expected_count
    } else {
        total_count <= expected_count
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::is_consistent_capture_count;

    #[rstest]
    #[case::unknown_total_can_complete(None, 2, None, Some(0), true, true)]
    #[case::known_total_with_missing_items(Some(4), 3, Some(1), Some(0), true, true)]
    #[case::known_total_must_reconcile(Some(4), 3, Some(0), Some(0), true, false)]
    #[case::reported_truncation_accounts_for_all_items(Some(5), 2, Some(1), Some(2), false, true)]
    #[case::inconsistent_truncation_counts(Some(5), 2, Some(1), Some(1), false, false)]
    fn capture_count_report_is_checked(
        #[case] expected_count: Option<u64>,
        #[case] received_count: u64,
        #[case] missing_count: Option<u64>,
        #[case] unretrieved_count: Option<u64>,
        #[case] complete: bool,
        #[case] expected: bool,
    ) {
        assert_eq!(
            is_consistent_capture_count(
                expected_count,
                received_count,
                missing_count,
                unretrieved_count,
                complete,
            ),
            expected,
        );
    }
}
