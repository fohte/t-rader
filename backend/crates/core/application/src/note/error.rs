use thiserror::Error;

use crate::bars::BarsUseCaseError;
use crate::change_history::ChangeHistoryError;
use crate::custom_indicator_evaluation::CustomIndicatorEvaluationUseCaseError;
use crate::indicator_observation::IndicatorObservationUseCaseError;
use crate::note::repository::NoteRepositoryError;
use crate::unit_of_work::UnitOfWorkError;
use crate::valuation::ValuationUseCaseError;
use core_domain::note_body_warning::NoteBodyWarning;

#[derive(Debug, Error)]
pub enum NoteUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("note body contains unresolved warnings")]
    UnresolvedBodyWarnings { warnings: Vec<NoteBodyWarning> },
    #[error("unknown note kind: {0}")]
    UnknownNoteKind(String),
    #[error("{0}")]
    NotFound(String),
    #[error("note kind {0} not found")]
    ReferencedNoteKindNotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Repository(#[from] NoteRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    ChangeHistory(#[from] ChangeHistoryError),
    #[error(transparent)]
    Bars(#[from] BarsUseCaseError),
    #[error(transparent)]
    Valuation(#[from] ValuationUseCaseError),
    #[error(transparent)]
    IndicatorObservation(#[from] IndicatorObservationUseCaseError),
    #[error(transparent)]
    CustomIndicatorEvaluation(#[from] CustomIndicatorEvaluationUseCaseError),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}
