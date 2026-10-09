use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::paper_trade::{PaperTradeRepositoryError, PaperTradeUseCaseError};
use core_application::unit_of_work::UnitOfWorkError;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonBody;
use crate::models::{CreatePaperAccountRequest, PaperAccountResponse};

/// ペーパートレード口座一覧
#[utoipa::path(
    get,
    path = "/api/paper-accounts",
    tag = "paper_accounts",
    responses(
        (status = 200, description = "ペーパートレード口座一覧", body = Vec<PaperAccountResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_paper_accounts(
    State(state): State<FrontendApiState>,
) -> Result<Json<Vec<PaperAccountResponse>>, AppError> {
    let accounts = state
        .paper_trade_use_cases
        .list_accounts()
        .await
        .map_err(map_paper_trade_use_case_error)?;
    Ok(Json(
        accounts
            .into_iter()
            .map(PaperAccountResponse::from)
            .collect(),
    ))
}

/// ペーパートレード口座を作成
#[utoipa::path(
    post,
    path = "/api/paper-accounts",
    tag = "paper_accounts",
    request_body = CreatePaperAccountRequest,
    responses(
        (status = 201, description = "作成されたペーパートレード口座", body = PaperAccountResponse),
        (status = 400, description = "リクエストまたは参照先が不正", body = ErrorResponse),
        (status = 409, description = "戦略と purpose の組み合わせが既に存在する", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_paper_account(
    State(state): State<FrontendApiState>,
    JsonBody(payload): JsonBody<CreatePaperAccountRequest>,
) -> Result<(StatusCode, Json<PaperAccountResponse>), AppError> {
    let account = state
        .paper_trade_use_cases
        .create_account(payload.into())
        .await
        .map_err(map_paper_trade_use_case_error)?;
    Ok((StatusCode::CREATED, Json(account.into())))
}

fn map_paper_trade_use_case_error(error: PaperTradeUseCaseError) -> AppError {
    match error {
        PaperTradeUseCaseError::Validation(message) => AppError::Validation(message),
        PaperTradeUseCaseError::Repository(PaperTradeRepositoryError::Database(error)) => {
            error.into()
        }
        PaperTradeUseCaseError::UnitOfWork(
            UnitOfWorkError::Begin(error) | UnitOfWorkError::Commit(error),
        ) => error.into(),
        other => AppError::Internal(other.to_string()),
    }
}
