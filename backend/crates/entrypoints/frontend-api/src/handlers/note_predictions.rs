use axum::Json;
use axum::extract::State;
use core_application::prediction::{PredictionRepositoryError, PredictionUseCaseError};
use core_application::unit_of_work::UnitOfWorkError;
use uuid::Uuid;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonPath;
use crate::models::PredictionResponse;

/// ノートに紐づく予測一覧 (記録順)。
#[utoipa::path(
    get,
    path = "/api/notes/{id}/predictions",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    responses(
        (status = 200, body = Vec<PredictionResponse>),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_note_predictions(
    State(state): State<FrontendApiState>,
    JsonPath(note_id): JsonPath<Uuid>,
) -> Result<Json<Vec<PredictionResponse>>, AppError> {
    let rows = state
        .prediction_use_cases
        .list_by_note(note_id)
        .await
        .map_err(map_prediction_error)?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

fn map_prediction_error(error: PredictionUseCaseError) -> AppError {
    match error {
        PredictionUseCaseError::Validation(message) => AppError::Validation(message),
        PredictionUseCaseError::NoteNotFound(note_id) => {
            AppError::NotFound(format!("note {note_id} not found"))
        }
        PredictionUseCaseError::Repository(PredictionRepositoryError::Database(error)) => {
            error.into()
        }
        PredictionUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | PredictionUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error)) => error.into(),
        PredictionUseCaseError::Repository(PredictionRepositoryError::InvalidTransaction)
        | PredictionUseCaseError::UnitOfWork(UnitOfWorkError::InvalidTransaction) => {
            AppError::Internal("prediction transaction has an unexpected type".into())
        }
    }
}
