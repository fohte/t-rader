use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::bar::Bar;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::{
    GradedPrediction, NewPrediction, NewPredictionGrade, Prediction, PredictionListQuery,
};

#[derive(Debug, Error)]
pub enum PredictionRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait PredictionRepository: Send + Sync {
    async fn find_note_owner(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<Option<super::types::NoteOwner>, PredictionRepositoryError>;
    async fn stock_exists(
        &self,
        transaction: &UnitOfWorkTransaction,
        stock_id: &str,
    ) -> Result<bool, PredictionRepositoryError>;
    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        prediction: NewPrediction,
    ) -> Result<Prediction, PredictionRepositoryError>;
    async fn list_by_strategy(
        &self,
        strategy_id: Uuid,
        query: PredictionListQuery,
    ) -> Result<Vec<Prediction>, PredictionRepositoryError>;
    async fn list_by_note(
        &self,
        note_id: Uuid,
    ) -> Result<Option<Vec<Prediction>>, PredictionRepositoryError>;
    async fn list_graded_by_strategy(
        &self,
        strategy_id: Uuid,
    ) -> Result<Vec<GradedPrediction>, PredictionRepositoryError>;
    async fn list_ungraded_due(
        &self,
        due_on_or_before: NaiveDate,
    ) -> Result<Vec<Prediction>, PredictionRepositoryError>;
    async fn find_latest_daily_bar_on_or_before(
        &self,
        stock_id: &str,
        date: NaiveDate,
    ) -> Result<Option<Bar>, PredictionRepositoryError>;
    async fn insert_grade(
        &self,
        grade: NewPredictionGrade,
    ) -> Result<(), PredictionRepositoryError>;
}

pub type SharedPredictionRepository = Arc<dyn PredictionRepository + Send + Sync>;
