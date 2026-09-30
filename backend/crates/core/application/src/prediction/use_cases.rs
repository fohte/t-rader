use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::strategy_scope::StrategyScope;
use crate::unit_of_work::SharedUnitOfWork;

use super::error::PredictionUseCaseError;
use super::repository::SharedPredictionRepository;
use super::types::{NewPrediction, Prediction, PredictionListQuery, RecordPredictionCommand};

pub(super) const DIRECTIONS: [&str; 2] = ["outperform", "underperform"];

pub(super) fn probability_steps() -> [Decimal; 8] {
    [55, 60, 65, 70, 75, 80, 85, 90].map(|value| Decimal::new(value, 2))
}

#[derive(Clone)]
pub struct PredictionUseCases {
    pub(super) unit_of_work: SharedUnitOfWork,
    pub(super) repository: SharedPredictionRepository,
}

impl PredictionUseCases {
    pub fn new(unit_of_work: SharedUnitOfWork, repository: SharedPredictionRepository) -> Self {
        Self {
            unit_of_work,
            repository,
        }
    }

    pub async fn record(
        &self,
        scope: StrategyScope,
        command: RecordPredictionCommand,
    ) -> Result<Prediction, PredictionUseCaseError> {
        let target_stock_id = command.target_stock_id.trim().to_string();
        let benchmark_stock_id = command.benchmark_stock_id.trim().to_string();
        if target_stock_id.is_empty() {
            return Err(PredictionUseCaseError::Validation(
                "target_stock_id must not be empty".into(),
            ));
        }
        if benchmark_stock_id.is_empty() {
            return Err(PredictionUseCaseError::Validation(
                "benchmark_stock_id must not be empty".into(),
            ));
        }
        if target_stock_id == benchmark_stock_id {
            return Err(PredictionUseCaseError::Validation(
                "target_stock_id and benchmark_stock_id must differ".into(),
            ));
        }
        let direction = command.direction.trim();
        if !DIRECTIONS.contains(&direction) {
            return Err(PredictionUseCaseError::Validation(format!(
                "invalid direction: {direction}"
            )));
        }
        if !probability_steps().contains(&command.probability) {
            return Err(PredictionUseCaseError::Validation(format!(
                "invalid probability: {}",
                command.probability
            )));
        }
        if command.due_date <= command.base_date {
            return Err(PredictionUseCaseError::Validation(
                "due_date must be after base_date".into(),
            ));
        }

        let transaction = self.unit_of_work.begin().await?;
        if let Some(note_id) = command.note_id {
            let Some(owner) = self
                .repository
                .find_note_owner(&transaction, note_id)
                .await?
            else {
                return Err(PredictionUseCaseError::NoteNotFound(note_id));
            };
            if owner.strategy_id != Some(scope.id()) {
                return Err(PredictionUseCaseError::Forbidden(note_id));
            }
        }
        for stock_id in [&target_stock_id, &benchmark_stock_id] {
            if !self.repository.stock_exists(&transaction, stock_id).await? {
                return Err(PredictionUseCaseError::Validation(format!(
                    "stock {stock_id} not found"
                )));
            }
        }

        let prediction = self
            .repository
            .insert(
                &transaction,
                NewPrediction {
                    id: Uuid::new_v4(),
                    strategy_id: scope.id(),
                    note_id: command.note_id,
                    target_stock_id,
                    benchmark_stock_id,
                    direction: direction.to_string(),
                    probability: command.probability,
                    base_date: command.base_date,
                    due_date: command.due_date,
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(prediction)
    }

    pub async fn list_by_strategy(
        &self,
        scope: StrategyScope,
        query: PredictionListQuery,
    ) -> Result<Vec<Prediction>, PredictionUseCaseError> {
        self.repository
            .list_by_strategy(scope.id(), query)
            .await
            .map_err(Into::into)
    }

    pub async fn list_by_note(
        &self,
        note_id: Uuid,
    ) -> Result<Vec<Prediction>, PredictionUseCaseError> {
        self.repository
            .list_by_note(note_id)
            .await?
            .ok_or(PredictionUseCaseError::NoteNotFound(note_id))
    }

    pub async fn grade_due(
        &self,
        today: chrono::NaiveDate,
    ) -> Result<super::types::GradingStats, PredictionUseCaseError> {
        self.grade_due_inner(today).await
    }

    pub async fn grade_due_today(
        &self,
    ) -> Result<super::types::GradingStats, PredictionUseCaseError> {
        self.grade_due(Utc::now().date_naive()).await
    }
}
