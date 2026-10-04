use chrono::Utc;
use core_domain::stock_id::has_country_prefix;
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
        if has_country_prefix(&target_stock_id) || has_country_prefix(&benchmark_stock_id) {
            return Err(PredictionUseCaseError::Validation(
                "predictions only support Japanese stocks".into(),
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
            let note_exists = self
                .repository
                .find_note_owner(&transaction, note_id)
                .await?
                .is_some();
            if !note_exists {
                return Err(PredictionUseCaseError::NoteNotFound(note_id));
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

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::{DateTime, NaiveDate, Utc};
    use core_domain::bar::Bar;
    use rstest::rstest;
    use rust_decimal::Decimal;
    use tokio::sync::Mutex;

    use super::*;
    use crate::prediction::repository::{PredictionRepository, PredictionRepositoryError};
    use crate::prediction::types::{GradedPrediction, NewPredictionGrade, NoteOwner, Prediction};
    use crate::unit_of_work::FakeUnitOfWork;

    const NOTE_ID: Uuid = Uuid::from_u128(1);
    const MISSING_NOTE_ID: Uuid = Uuid::from_u128(5);
    const RUNNING_STRATEGY_ID: Uuid = Uuid::from_u128(2);
    const OWNER_STRATEGY_ID: Uuid = Uuid::from_u128(3);
    const NORMALIZED_PREDICTION_ID: Uuid = Uuid::from_u128(4);

    #[derive(Default)]
    struct FakePredictionRepository {
        owner_strategy_id: Option<Uuid>,
        inserted: Mutex<Vec<NewPrediction>>,
    }

    #[async_trait]
    impl PredictionRepository for FakePredictionRepository {
        async fn find_note_owner(
            &self,
            _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
            note_id: Uuid,
        ) -> Result<Option<NoteOwner>, PredictionRepositoryError> {
            Ok((note_id == NOTE_ID).then_some(NoteOwner {
                strategy_id: self.owner_strategy_id,
            }))
        }

        async fn stock_exists(
            &self,
            _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
            _stock_id: &str,
        ) -> Result<bool, PredictionRepositoryError> {
            Ok(true)
        }

        async fn insert(
            &self,
            _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
            prediction: NewPrediction,
        ) -> Result<Prediction, PredictionRepositoryError> {
            self.inserted.lock().await.push(prediction.clone());
            Ok(Prediction {
                id: prediction.id,
                strategy_id: prediction.strategy_id,
                note_id: prediction.note_id,
                target_stock_id: prediction.target_stock_id,
                benchmark_stock_id: prediction.benchmark_stock_id,
                direction: prediction.direction,
                probability: prediction.probability,
                base_date: prediction.base_date,
                due_date: prediction.due_date,
                created_at: DateTime::<Utc>::UNIX_EPOCH.fixed_offset(),
            })
        }

        async fn list_by_strategy(
            &self,
            _strategy_id: Uuid,
            _query: PredictionListQuery,
        ) -> Result<Vec<Prediction>, PredictionRepositoryError> {
            Ok(Vec::new())
        }

        async fn list_by_note(
            &self,
            _note_id: Uuid,
        ) -> Result<Option<Vec<Prediction>>, PredictionRepositoryError> {
            Ok(Some(Vec::new()))
        }

        async fn list_graded_by_strategy(
            &self,
            _strategy_id: Uuid,
        ) -> Result<Vec<GradedPrediction>, PredictionRepositoryError> {
            Ok(Vec::new())
        }

        async fn list_ungraded_due(
            &self,
            _due_on_or_before: NaiveDate,
        ) -> Result<Vec<Prediction>, PredictionRepositoryError> {
            Ok(Vec::new())
        }

        async fn find_latest_daily_bar_on_or_before(
            &self,
            _stock_id: &str,
            _date: NaiveDate,
        ) -> Result<Option<Bar>, PredictionRepositoryError> {
            Ok(None)
        }

        async fn insert_grade(
            &self,
            _grade: NewPredictionGrade,
        ) -> Result<(), PredictionRepositoryError> {
            Ok(())
        }
    }

    #[rstest]
    #[case::another_strategy(Some(OWNER_STRATEGY_ID))]
    #[case::unassigned(None)]
    #[tokio::test]
    async fn record_accepts_a_note_owned_by_any_strategy(#[case] owner_strategy_id: Option<Uuid>) {
        let repository = Arc::new(FakePredictionRepository {
            owner_strategy_id,
            inserted: Mutex::new(Vec::new()),
        });
        let use_cases =
            PredictionUseCases::new(Arc::new(FakeUnitOfWork::new()), repository.clone());
        let command = RecordPredictionCommand {
            note_id: Some(NOTE_ID),
            target_stock_id: "FICTIONAL-ASSET-A".into(),
            benchmark_stock_id: "FICTIONAL-ASSET-B".into(),
            direction: "outperform".into(),
            probability: Decimal::new(70, 2),
            base_date: NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid date"),
            due_date: NaiveDate::from_ymd_opt(2025, 2, 1).expect("valid date"),
        };

        let prediction = use_cases
            .record(RUNNING_STRATEGY_ID.into(), command)
            .await
            .expect("prediction can refer to a note from another strategy");
        let inserted = repository
            .inserted
            .lock()
            .await
            .iter()
            .map(|prediction| NewPrediction {
                id: NORMALIZED_PREDICTION_ID,
                ..prediction.clone()
            })
            .collect::<Vec<_>>();

        assert_eq!(
            (prediction.strategy_id, prediction.note_id, inserted,),
            (
                RUNNING_STRATEGY_ID,
                Some(NOTE_ID),
                vec![NewPrediction {
                    id: NORMALIZED_PREDICTION_ID,
                    strategy_id: RUNNING_STRATEGY_ID,
                    note_id: Some(NOTE_ID),
                    target_stock_id: "FICTIONAL-ASSET-A".into(),
                    benchmark_stock_id: "FICTIONAL-ASSET-B".into(),
                    direction: "outperform".into(),
                    probability: Decimal::new(70, 2),
                    base_date: NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid date"),
                    due_date: NaiveDate::from_ymd_opt(2025, 2, 1).expect("valid date"),
                }],
            ),
        );
    }

    #[rstest::rstest]
    #[case::foreign_target("US:QZ-7", "FICTIONAL-ASSET-B")]
    #[case::foreign_benchmark("FICTIONAL-ASSET-A", "KR:QZ9012")]
    #[tokio::test]
    async fn record_rejects_foreign_stock_ids(
        #[case] target_stock_id: &str,
        #[case] benchmark_stock_id: &str,
    ) {
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakePredictionRepository {
            inserted: Mutex::new(Vec::new()),
            ..Default::default()
        });
        let use_cases = PredictionUseCases::new(unit_of_work.clone(), repository.clone());
        let result = use_cases
            .record(
                RUNNING_STRATEGY_ID.into(),
                RecordPredictionCommand {
                    note_id: None,
                    target_stock_id: target_stock_id.into(),
                    benchmark_stock_id: benchmark_stock_id.into(),
                    direction: "outperform".into(),
                    probability: Decimal::new(70, 2),
                    base_date: NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid date"),
                    due_date: NaiveDate::from_ymd_opt(2025, 2, 1).expect("valid date"),
                },
            )
            .await
            .map_err(|error| error.to_string());
        let inserted_count = repository.inserted.lock().await.len();
        let begun_count = unit_of_work.begun.lock().await.len();

        assert_eq!(
            (result, inserted_count, begun_count),
            (Err("predictions only support Japanese stocks".into()), 0, 0,),
        );
    }

    #[tokio::test]
    async fn record_rejects_a_missing_note() {
        let repository = Arc::new(FakePredictionRepository::default());
        let use_cases =
            PredictionUseCases::new(Arc::new(FakeUnitOfWork::new()), repository.clone());
        let result = use_cases
            .record(
                RUNNING_STRATEGY_ID.into(),
                RecordPredictionCommand {
                    note_id: Some(MISSING_NOTE_ID),
                    target_stock_id: "FICTIONAL-ASSET-A".into(),
                    benchmark_stock_id: "FICTIONAL-ASSET-B".into(),
                    direction: "outperform".into(),
                    probability: Decimal::new(70, 2),
                    base_date: NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid date"),
                    due_date: NaiveDate::from_ymd_opt(2025, 2, 1).expect("valid date"),
                },
            )
            .await
            .map(|_| ())
            .map_err(|error| error.to_string());
        let inserted = repository.inserted.lock().await.clone();

        assert_eq!(
            (result, inserted),
            (Err(format!("note {MISSING_NOTE_ID} not found")), Vec::new(),),
        );
    }
}
