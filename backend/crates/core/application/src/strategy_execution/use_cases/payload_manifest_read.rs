use uuid::Uuid;

use crate::strategy_scope::StrategyScope;

use super::StrategyExecutionUseCases;
use crate::strategy_execution::{
    ListPayloadManifestsQuery, MAX_PAYLOAD_MANIFEST_PAGE_ITEMS, PayloadManifestCursor,
    PayloadManifestPage, StrategyExecutionUseCaseError,
};

impl StrategyExecutionUseCases {
    pub async fn list_task_execution_payload_manifests(
        &self,
        scope: StrategyScope,
        task_id: Uuid,
        query: ListPayloadManifestsQuery,
    ) -> Result<PayloadManifestPage, StrategyExecutionUseCaseError> {
        self.verify_task_scope(scope, task_id).await?;
        if !(1..=MAX_PAYLOAD_MANIFEST_PAGE_ITEMS).contains(&query.limit) {
            return Err(StrategyExecutionUseCaseError::InvalidPayloadManifestPageLimit);
        }

        let total_count = self
            .manifest_query_repository
            .count_payload_manifests(task_id)
            .await?;
        let mut manifests = self
            .manifest_query_repository
            .list_payload_manifests(task_id, query.cursor.clone(), query.limit + 1)
            .await?;
        let has_more = manifests.len() > query.limit;
        manifests.truncate(query.limit);
        let next_cursor = if has_more {
            manifests.last().map(|manifest| PayloadManifestCursor {
                created_at: manifest.created_at,
                id: manifest.id,
            })
        } else {
            None
        };

        Ok(PayloadManifestPage {
            cursor: query.cursor,
            next_cursor,
            total_count,
            manifests,
        })
    }
}
