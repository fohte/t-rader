#![cfg(feature = "test-support")]

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::ports::{Annotation, AnnotationRepository, AnnotationRepositoryError, NewAnnotation};
use crate::unit_of_work::{FakeTransaction, UnitOfWorkTransaction};

#[derive(Default)]
pub struct FakeAnnotationRepository {
    pub(super) annotations: Mutex<HashMap<Uuid, Annotation>>,
    commented: Mutex<HashSet<Uuid>>,
    note_strategy_ids: Mutex<HashMap<Uuid, Option<Uuid>>>,
    pub(super) transaction_ids: Mutex<Vec<Uuid>>,
    pub(super) update_calls: Mutex<usize>,
}

impl FakeAnnotationRepository {
    pub async fn insert_annotation(&self, annotation: Annotation) {
        self.annotations
            .lock()
            .await
            .insert(annotation.id, annotation);
    }

    pub async fn set_comment(&self, annotation_id: Uuid) {
        self.commented.lock().await.insert(annotation_id);
    }

    pub async fn set_note_strategy(&self, note_id: Uuid, strategy_id: Option<Uuid>) {
        self.note_strategy_ids
            .lock()
            .await
            .insert(note_id, strategy_id);
    }

    async fn record_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<(), AnnotationRepositoryError> {
        let transaction_id = transaction
            .downcast_ref::<FakeTransaction>()
            .map(|transaction| transaction.id)
            .ok_or(AnnotationRepositoryError::InvalidTransaction)?;
        self.transaction_ids.lock().await.push(transaction_id);
        Ok(())
    }
}

#[async_trait]
impl AnnotationRepository for FakeAnnotationRepository {
    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<Option<Annotation>, AnnotationRepositoryError> {
        self.record_transaction(transaction).await?;
        Ok(self.annotations.lock().await.get(&id).cloned())
    }

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        annotation: NewAnnotation,
    ) -> Result<Annotation, AnnotationRepositoryError> {
        self.record_transaction(transaction).await?;
        let now = Utc::now().fixed_offset();
        let created = Annotation {
            id: annotation.id,
            strategy_id: annotation.strategy_id,
            target_symbol: annotation.target_symbol,
            target_kind: annotation.target_kind,
            timestamp: annotation.timestamp,
            price: annotation.price,
            text: annotation.text,
            status: annotation.status,
            linked_note_id: annotation.linked_note_id,
            created_by_kind: annotation.created_by_kind,
            created_at: now,
            updated_at: now,
            execution_step_id: annotation.execution_step_id,
            execution_task_id: annotation.execution_task_id,
        };
        self.annotations
            .lock()
            .await
            .insert(created.id, created.clone());
        Ok(created)
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        annotation: Annotation,
    ) -> Result<Annotation, AnnotationRepositoryError> {
        self.record_transaction(transaction).await?;
        *self.update_calls.lock().await += 1;
        self.annotations
            .lock()
            .await
            .insert(annotation.id, annotation.clone());
        Ok(annotation)
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<bool, AnnotationRepositoryError> {
        self.record_transaction(transaction).await?;
        Ok(self.annotations.lock().await.remove(&id).is_some())
    }

    async fn find_stale_unread_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
        execution_step_id: Uuid,
        current_execution_task_id: &str,
    ) -> Result<Vec<Uuid>, AnnotationRepositoryError> {
        self.record_transaction(transaction).await?;
        let mut stale: Vec<_> = self
            .annotations
            .lock()
            .await
            .values()
            .filter(|annotation| {
                annotation.strategy_id == Some(strategy_id)
                    && annotation.execution_step_id == Some(execution_step_id)
                    && annotation.status == "unread"
                    && annotation.execution_task_id.as_deref() != Some(current_execution_task_id)
            })
            .map(|annotation| annotation.id)
            .collect();
        stale.sort();
        Ok(stale)
    }

    async fn commented_annotation_ids_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        annotation_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>, AnnotationRepositoryError> {
        self.record_transaction(transaction).await?;
        let commented = self.commented.lock().await;
        Ok(annotation_ids
            .iter()
            .filter(|id| commented.contains(*id))
            .copied()
            .collect())
    }

    async fn note_strategy_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<Option<Option<Uuid>>, AnnotationRepositoryError> {
        self.record_transaction(transaction).await?;
        Ok(self.note_strategy_ids.lock().await.get(&note_id).copied())
    }
}
