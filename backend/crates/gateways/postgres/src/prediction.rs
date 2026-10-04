use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::prediction::{
    GradedPrediction, NewPrediction, NewPredictionGrade, NoteOwner, Prediction, PredictionGrade,
    PredictionListQuery, PredictionRepository, PredictionRepositoryError,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use core_domain::bar::Bar;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::{note, prediction, prediction_grade, stock};
use crate::persistence::persistence_error;
use crate::repositories::bars::find_latest_bar_on_or_before;
use crate::transaction::transaction_ref;

#[derive(Clone)]
pub struct PostgresPredictionRepository {
    db: DatabaseHandle,
}

impl PostgresPredictionRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl PredictionRepository for PostgresPredictionRepository {
    async fn find_note_owner(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<Option<NoteOwner>, PredictionRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(PredictionRepositoryError::InvalidTransaction)?;
        note::Entity::find_by_id(note_id)
            .one(transaction)
            .await
            .map(|row| {
                row.map(|note| NoteOwner {
                    strategy_id: note.strategy_id,
                })
            })
            .map_err(repository_error)
    }

    async fn stock_exists(
        &self,
        transaction: &UnitOfWorkTransaction,
        stock_id: &str,
    ) -> Result<bool, PredictionRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(PredictionRepositoryError::InvalidTransaction)?;
        stock::Entity::find_by_id(stock_id.to_string())
            .one(transaction)
            .await
            .map(|row| row.is_some())
            .map_err(repository_error)
    }

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        value: NewPrediction,
    ) -> Result<Prediction, PredictionRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(PredictionRepositoryError::InvalidTransaction)?;
        prediction::Entity::insert(prediction::ActiveModel {
            prediction_id: Set(value.id),
            strategy_id: Set(value.strategy_id),
            note_id: Set(value.note_id),
            target_stock_id: Set(value.target_stock_id),
            benchmark_stock_id: Set(value.benchmark_stock_id),
            direction: Set(value.direction),
            probability: Set(value.probability),
            base_date: Set(value.base_date),
            due_date: Set(value.due_date),
            created_at: NotSet,
        })
        .exec_with_returning(transaction)
        .await
        .map(to_prediction)
        .map_err(repository_error)
    }

    async fn list_by_strategy(
        &self,
        strategy_id: Uuid,
        query: PredictionListQuery,
    ) -> Result<Vec<Prediction>, PredictionRepositoryError> {
        let mut select =
            prediction::Entity::find().filter(prediction::Column::StrategyId.eq(strategy_id));
        if let Some(due_after) = query.due_after {
            select = select.filter(prediction::Column::DueDate.gte(due_after));
        }
        if let Some(due_before) = query.due_before {
            select = select.filter(prediction::Column::DueDate.lte(due_before));
        }
        select
            .order_by_desc(prediction::Column::CreatedAt)
            .limit(query.limit)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_prediction).collect())
            .map_err(repository_error)
    }

    async fn list_by_note(
        &self,
        note_id: Uuid,
    ) -> Result<Option<Vec<Prediction>>, PredictionRepositoryError> {
        let note_exists = note::Entity::find_by_id(note_id)
            .one(&self.db)
            .await
            .map_err(repository_error)?
            .is_some();
        if !note_exists {
            return Ok(None);
        }
        prediction::Entity::find()
            .filter(prediction::Column::NoteId.eq(note_id))
            .order_by_asc(prediction::Column::CreatedAt)
            .all(&self.db)
            .await
            .map(|rows| Some(rows.into_iter().map(to_prediction).collect()))
            .map_err(repository_error)
    }

    async fn list_graded_by_strategy(
        &self,
        strategy_id: Uuid,
    ) -> Result<Vec<GradedPrediction>, PredictionRepositoryError> {
        prediction::Entity::find()
            .filter(prediction::Column::StrategyId.eq(strategy_id))
            .find_also_related(prediction_grade::Entity)
            .all(&self.db)
            .await
            .map(|rows| {
                rows.into_iter()
                    .filter_map(|(prediction, grade)| {
                        grade.map(|grade| GradedPrediction {
                            prediction: to_prediction(prediction),
                            grade: to_grade(grade),
                        })
                    })
                    .collect()
            })
            .map_err(repository_error)
    }

    async fn list_ungraded_due(
        &self,
        due_on_or_before: NaiveDate,
    ) -> Result<Vec<Prediction>, PredictionRepositoryError> {
        prediction::Entity::find()
            .filter(prediction::Column::DueDate.lte(due_on_or_before))
            .find_also_related(prediction_grade::Entity)
            .all(&self.db)
            .await
            .map(|rows| {
                rows.into_iter()
                    .filter_map(|(prediction, grade)| {
                        grade.is_none().then(|| to_prediction(prediction))
                    })
                    .collect()
            })
            .map_err(repository_error)
    }

    async fn find_latest_daily_bar_on_or_before(
        &self,
        stock_id: &str,
        date: NaiveDate,
    ) -> Result<Option<Bar>, PredictionRepositoryError> {
        find_latest_bar_on_or_before(&self.db, stock_id, "1d", date)
            .await
            .map(|bar| bar.map(Into::into))
            .map_err(repository_error)
    }

    async fn insert_grade(
        &self,
        grade: NewPredictionGrade,
    ) -> Result<(), PredictionRepositoryError> {
        prediction_grade::Entity::insert(prediction_grade::ActiveModel {
            prediction_id: Set(grade.prediction_id),
            target_base_close: Set(grade.target_base_close),
            target_due_close: Set(grade.target_due_close),
            benchmark_base_close: Set(grade.benchmark_base_close),
            benchmark_due_close: Set(grade.benchmark_due_close),
            target_return: Set(grade.target_return),
            benchmark_return: Set(grade.benchmark_return),
            correct: Set(grade.correct),
            graded_at: NotSet,
        })
        .exec_without_returning(&self.db)
        .await
        .map(|_| ())
        .map_err(repository_error)
    }
}

fn repository_error(error: sea_orm::DbErr) -> PredictionRepositoryError {
    PredictionRepositoryError::Database(persistence_error(error))
}

fn to_prediction(model: prediction::Model) -> Prediction {
    Prediction {
        id: model.prediction_id,
        strategy_id: model.strategy_id,
        note_id: model.note_id,
        target_stock_id: model.target_stock_id,
        benchmark_stock_id: model.benchmark_stock_id,
        direction: model.direction,
        probability: model.probability,
        base_date: model.base_date,
        due_date: model.due_date,
        created_at: model.created_at,
    }
}

fn to_grade(model: prediction_grade::Model) -> PredictionGrade {
    PredictionGrade {
        prediction_id: model.prediction_id,
        target_base_close: model.target_base_close,
        target_due_close: model.target_due_close,
        benchmark_base_close: model.benchmark_base_close,
        benchmark_due_close: model.benchmark_due_close,
        target_return: model.target_return,
        benchmark_return: model.benchmark_return,
        correct: model.correct,
        graded_at: model.graded_at,
    }
}
