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
                self.record_received_count(
                    access.task_id,
                    payload_id,
                    &mut capture_metadata,
                    page.items.len(),
                )
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
            let page_item_count = items.len();
            self.append_dataset_page(access, payload_id, &snapshot, items)
                .await?;
            self.record_received_count(
                access.task_id,
                payload_id,
                &mut capture_metadata,
                page_item_count,
            )
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
                        return self
                            .fail_invalid_provider_completion(
                                access.task_id,
                                payload_id,
                                StrategyExecutionUseCaseError::InvalidDatasetCursor,
                            )
                            .await;
                    }
                    let Some(final_metadata) =
                        completion_metadata(&snapshot, &capture_metadata, &report)
                    else {
                        return self
                            .fail_invalid_provider_completion(
                                access.task_id,
                                payload_id,
                                StrategyExecutionUseCaseError::InvalidDatasetCompletion,
                            )
                            .await;
                    };
                    self.update_capture_metadata(access.task_id, payload_id, final_metadata)
                        .await?;
                    return self.seal_payload(access, payload_id).await;
                }
                DatasetPageCompletion::Incomplete(report) => {
                    let Some(final_metadata) =
                        incomplete_metadata(&snapshot, &capture_metadata, &report)
                    else {
                        return self
                            .fail_invalid_provider_completion(
                                access.task_id,
                                payload_id,
                                StrategyExecutionUseCaseError::InvalidDatasetCompletion,
                            )
                            .await;
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

    async fn record_received_count(
        &self,
        task_id: Uuid,
        payload_id: Uuid,
        metadata: &mut PayloadCaptureMetadata,
        page_item_count: usize,
    ) -> Result<(), StrategyExecutionUseCaseError> {
        let page_item_count = u64::try_from(page_item_count)
            .map_err(|_| StrategyExecutionUseCaseError::InvalidDatasetCompletion)?;
        metadata.received_count = metadata
            .received_count
            .checked_add(page_item_count)
            .ok_or(StrategyExecutionUseCaseError::InvalidDatasetCompletion)?;
        self.update_capture_metadata(task_id, payload_id, metadata.clone())
            .await
    }

    async fn fail_invalid_provider_completion<T>(
        &self,
        task_id: Uuid,
        payload_id: Uuid,
        error: StrategyExecutionUseCaseError,
    ) -> Result<T, StrategyExecutionUseCaseError> {
        self.mark_incomplete(task_id, payload_id, "invalid_provider_completion")
            .await?;
        Err(error)
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
) -> Option<PayloadCaptureMetadata> {
    if !capture_report_matches_snapshot(snapshot, current, report)
        || !is_consistent_complete_counts(
            report.expected_count.or(snapshot.expected_count),
            report.received_count,
            report.missing_count,
            report.unretrieved_count,
        )
    {
        return None;
    }

    Some(metadata_from_report(snapshot, current, report))
}

fn incomplete_metadata(
    snapshot: &DatasetSnapshot,
    current: &PayloadCaptureMetadata,
    report: &DatasetCaptureReport,
) -> Option<PayloadCaptureMetadata> {
    if !capture_report_matches_snapshot(snapshot, current, report)
        || !is_consistent_incomplete_counts(
            report.expected_count.or(snapshot.expected_count),
            report.received_count,
            report.missing_count,
            report.unretrieved_count,
        )
    {
        return None;
    }

    Some(metadata_from_report(snapshot, current, report))
}

fn capture_report_matches_snapshot(
    snapshot: &DatasetSnapshot,
    current: &PayloadCaptureMetadata,
    report: &DatasetCaptureReport,
) -> bool {
    report.received_count == current.received_count
        && !snapshot
            .expected_count
            .zip(report.expected_count)
            .is_some_and(|(snapshot, report)| snapshot != report)
}

fn metadata_from_report(
    snapshot: &DatasetSnapshot,
    current: &PayloadCaptureMetadata,
    report: &DatasetCaptureReport,
) -> PayloadCaptureMetadata {
    PayloadCaptureMetadata {
        captured_at: snapshot.captured_at,
        expected_count: report.expected_count.or(snapshot.expected_count),
        received_count: current.received_count,
        missing_count: report.missing_count,
        unretrieved_count: report.unretrieved_count,
    }
}

pub(super) fn is_consistent_complete_counts(
    expected_count: Option<u64>,
    received_count: u64,
    missing_count: Option<u64>,
    unretrieved_count: Option<u64>,
) -> bool {
    if unretrieved_count != Some(0) {
        return false;
    }
    let Some(expected_count) = expected_count else {
        return true;
    };
    missing_count
        .map(|missing| received_count.checked_add(missing) == Some(expected_count))
        .unwrap_or(received_count == expected_count)
}

fn is_consistent_incomplete_counts(
    expected_count: Option<u64>,
    received_count: u64,
    missing_count: Option<u64>,
    unretrieved_count: Option<u64>,
) -> bool {
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
    if missing_count.is_some() && unretrieved_count.is_some() {
        total_count == expected_count
    } else {
        total_count <= expected_count
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{is_consistent_complete_counts, is_consistent_incomplete_counts};

    #[derive(Clone, Copy)]
    enum CaptureStatus {
        Complete,
        Incomplete,
    }

    #[rstest]
    #[case::unknown_total_can_complete(CaptureStatus::Complete, None, 2, None, Some(0), true)]
    #[case::known_total_with_missing_items(
        CaptureStatus::Complete,
        Some(4),
        3,
        Some(1),
        Some(0),
        true
    )]
    #[case::known_total_must_reconcile(
        CaptureStatus::Complete,
        Some(4),
        3,
        Some(0),
        Some(0),
        false
    )]
    #[case::reported_truncation_accounts_for_all_items(
        CaptureStatus::Incomplete,
        Some(5),
        2,
        Some(1),
        Some(2),
        true
    )]
    #[case::inconsistent_truncation_counts(
        CaptureStatus::Incomplete,
        Some(5),
        2,
        Some(1),
        Some(1),
        false
    )]
    fn capture_count_report_is_checked(
        #[case] status: CaptureStatus,
        #[case] expected_count: Option<u64>,
        #[case] received_count: u64,
        #[case] missing_count: Option<u64>,
        #[case] unretrieved_count: Option<u64>,
        #[case] expected: bool,
    ) {
        let actual = match status {
            CaptureStatus::Complete => is_consistent_complete_counts(
                expected_count,
                received_count,
                missing_count,
                unretrieved_count,
            ),
            CaptureStatus::Incomplete => is_consistent_incomplete_counts(
                expected_count,
                received_count,
                missing_count,
                unretrieved_count,
            ),
        };
        assert_eq!(actual, expected);
    }
}
