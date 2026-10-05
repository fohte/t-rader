use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use core_domain::note_price_reference::PriceReferenceField;
use rust_decimal::Decimal;
use thiserror::Error;
use uuid::Uuid;

use crate::change_history::Actor;
use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub id: Uuid,
    pub target_symbol: String,
    pub target_kind: String,
    pub timestamp: DateTime<FixedOffset>,
    pub price: Option<Decimal>,
    pub text: String,
    pub status: String,
    pub linked_note_id: Option<Uuid>,
    pub created_by_kind: String,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub execution_step_id: Option<Uuid>,
    pub execution_task_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewAnnotation {
    pub id: Uuid,
    pub target_symbol: String,
    pub target_kind: String,
    pub timestamp: DateTime<FixedOffset>,
    pub price: Option<Decimal>,
    pub text: String,
    pub status: String,
    pub linked_note_id: Option<Uuid>,
    pub created_by_kind: String,
    pub execution_step_id: Option<Uuid>,
    pub execution_task_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateAnnotationCommand {
    pub actor: Actor,
    pub target_symbol: String,
    pub target_kind: String,
    pub timestamp: DateTime<FixedOffset>,
    pub price: Option<AnnotationPriceInput>,
    pub text: String,
    pub status: String,
    pub linked_note_id: Option<Uuid>,
    pub created_by_kind: String,
    pub execution_step_id: Option<Uuid>,
    pub execution_task_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnnotationPriceInput {
    Value(Decimal),
    Field(PriceReferenceField),
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateAnnotationCommand {
    pub actor: Actor,
    pub id: Uuid,
    pub target_symbol: Option<String>,
    pub target_kind: Option<String>,
    pub timestamp: Option<DateTime<FixedOffset>>,
    pub price: Option<Decimal>,
    pub text: Option<String>,
    pub linked_note_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChangeAnnotationStatusCommand {
    pub actor: Actor,
    pub id: Uuid,
    pub status: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeleteAnnotationCommand {
    pub actor: Actor,
    pub id: Uuid,
}

#[derive(Debug, Error)]
pub enum AnnotationRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait AnnotationRepository: Send + Sync {
    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<Option<Annotation>, AnnotationRepositoryError>;

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        annotation: NewAnnotation,
    ) -> Result<Annotation, AnnotationRepositoryError>;

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        annotation: Annotation,
    ) -> Result<Annotation, AnnotationRepositoryError>;

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<bool, AnnotationRepositoryError>;

    async fn find_stale_unread_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        execution_step_id: Uuid,
        current_execution_task_id: &str,
    ) -> Result<Vec<Uuid>, AnnotationRepositoryError>;

    async fn commented_annotation_ids_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        annotation_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>, AnnotationRepositoryError>;

    async fn note_exists_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<bool, AnnotationRepositoryError>;
}

pub type SharedAnnotationRepository = Arc<dyn AnnotationRepository + Send + Sync>;
