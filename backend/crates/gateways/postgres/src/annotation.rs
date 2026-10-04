use std::collections::HashSet;

use async_trait::async_trait;
use core_application::annotation::{
    Annotation, AnnotationRepository, AnnotationRepositoryError, NewAnnotation,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
use uuid::Uuid;

use crate::entities::{annotation, comment, note};
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

#[derive(Clone, Copy, Default)]
pub struct PostgresAnnotationRepository;

#[async_trait]
impl AnnotationRepository for PostgresAnnotationRepository {
    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<Option<Annotation>, AnnotationRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        annotation::Entity::find_by_id(id)
            .one(transaction)
            .await
            .map(|row| row.map(to_domain))
            .map_err(repository_error)
    }

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        annotation: NewAnnotation,
    ) -> Result<Annotation, AnnotationRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = annotation::ActiveModel {
            id: Set(annotation.id),
            target_symbol: Set(annotation.target_symbol),
            target_kind: Set(annotation.target_kind),
            timestamp: Set(annotation.timestamp),
            price: Set(annotation.price),
            text: Set(annotation.text),
            status: Set(annotation.status),
            linked_note_id: Set(annotation.linked_note_id),
            created_by_kind: Set(annotation.created_by_kind),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
            execution_step_id: Set(annotation.execution_step_id),
            execution_task_id: Set(annotation.execution_task_id),
        };
        annotation::Entity::insert(model)
            .exec_with_returning(transaction)
            .await
            .map(to_domain)
            .map_err(repository_error)
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        annotation: Annotation,
    ) -> Result<Annotation, AnnotationRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = annotation::ActiveModel {
            id: sea_orm::ActiveValue::Unchanged(annotation.id),
            target_symbol: Set(annotation.target_symbol),
            target_kind: Set(annotation.target_kind),
            timestamp: Set(annotation.timestamp),
            price: Set(annotation.price),
            text: Set(annotation.text),
            status: Set(annotation.status),
            linked_note_id: Set(annotation.linked_note_id),
            created_by_kind: Set(annotation.created_by_kind),
            created_at: sea_orm::ActiveValue::Unchanged(annotation.created_at),
            updated_at: Set(annotation.updated_at),
            execution_step_id: Set(annotation.execution_step_id),
            execution_task_id: Set(annotation.execution_task_id),
        };
        model
            .update(transaction)
            .await
            .map(to_domain)
            .map_err(repository_error)
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<bool, AnnotationRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        annotation::Entity::delete_by_id(id)
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }

    async fn find_stale_unread_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        execution_step_id: Uuid,
        current_execution_task_id: &str,
    ) -> Result<Vec<Uuid>, AnnotationRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        annotation::Entity::find()
            .select_only()
            .column(annotation::Column::Id)
            .filter(annotation::Column::ExecutionStepId.eq(execution_step_id))
            .filter(annotation::Column::Status.eq("unread"))
            .filter(annotation::Column::ExecutionTaskId.ne(current_execution_task_id))
            .into_tuple::<Uuid>()
            .all(transaction)
            .await
            .map_err(repository_error)
    }

    async fn commented_annotation_ids_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        annotation_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>, AnnotationRepositoryError> {
        if annotation_ids.is_empty() {
            return Ok(HashSet::new());
        }
        let transaction = transaction_ref(transaction)?;
        comment::Entity::find()
            .select_only()
            .column(comment::Column::TargetId)
            .filter(comment::Column::TargetKind.eq("annotation"))
            .filter(comment::Column::TargetId.is_in(annotation_ids.iter().copied()))
            .into_tuple::<Uuid>()
            .all(transaction)
            .await
            .map(|ids| ids.into_iter().collect())
            .map_err(repository_error)
    }

    async fn note_exists_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<bool, AnnotationRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        note::Entity::find_by_id(note_id)
            .one(transaction)
            .await
            .map(|row| row.is_some())
            .map_err(repository_error)
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, AnnotationRepositoryError> {
    postgres_transaction_ref(transaction).ok_or(AnnotationRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> AnnotationRepositoryError {
    AnnotationRepositoryError::Database(persistence_error(error))
}

pub(super) fn to_domain(model: annotation::Model) -> Annotation {
    Annotation {
        id: model.id,
        target_symbol: model.target_symbol,
        target_kind: model.target_kind,
        timestamp: model.timestamp,
        price: model.price,
        text: model.text,
        status: model.status,
        linked_note_id: model.linked_note_id,
        created_by_kind: model.created_by_kind,
        created_at: model.created_at,
        updated_at: model.updated_at,
        execution_step_id: model.execution_step_id,
        execution_task_id: model.execution_task_id,
    }
}
