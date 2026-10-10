use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::paper_trade::{PaperTradeRepositoryError, PaperTradeUseCaseError};
use core_application::unit_of_work::UnitOfWorkError;
use uuid::Uuid;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{
    CreatePaperAccountRequest, PaperAccountPortfolioResponse, PaperAccountResponse,
    PaperAccountStatsResponse,
};

/// 全ペーパートレード口座の成績
#[utoipa::path(
    get,
    path = "/api/paper-accounts/stats",
    tag = "paper_accounts",
    responses(
        (status = 200, description = "全口座の成績", body = Vec<PaperAccountStatsResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_paper_account_stats(
    State(state): State<FrontendApiState>,
) -> Result<Json<Vec<PaperAccountStatsResponse>>, AppError> {
    let accounts = state
        .paper_trade_use_cases
        .stats()
        .await
        .map_err(map_paper_trade_use_case_error)?;
    Ok(Json(accounts.into_iter().map(Into::into).collect()))
}

/// ペーパートレード口座のポートフォリオ
#[utoipa::path(
    get,
    path = "/api/paper-accounts/{id}/portfolio",
    tag = "paper_accounts",
    params(("id" = Uuid, Path, description = "ペーパートレード口座 ID")),
    responses(
        (status = 200, description = "口座の現金、保有、注文", body = PaperAccountPortfolioResponse),
        (status = 404, description = "口座が見つからない", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_paper_account_portfolio(
    State(state): State<FrontendApiState>,
    JsonPath(account_id): JsonPath<Uuid>,
) -> Result<Json<PaperAccountPortfolioResponse>, AppError> {
    let portfolio = state
        .paper_trade_use_cases
        .portfolio(account_id)
        .await
        .map_err(map_paper_trade_use_case_error)?;
    let note_version_ids = portfolio
        .orders
        .iter()
        .map(|item| item.order.note_version_id)
        .collect::<Vec<_>>();
    let note_ids_by_version = state
        .note_read_use_cases
        .note_ids_for_versions(&note_version_ids)
        .await
        .map_err(crate::handlers::notes::map_note_read_error)?;
    let response = PaperAccountPortfolioResponse::new(portfolio, &note_ids_by_version).map_err(
        |version_id| {
            AppError::Internal(format!(
                "paper order note version {version_id} was not found"
            ))
        },
    )?;
    Ok(Json(response))
}

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
        PaperTradeUseCaseError::AccountNotFound(account_id) => {
            AppError::NotFound(format!("paper account {account_id} not found"))
        }
        PaperTradeUseCaseError::Repository(PaperTradeRepositoryError::Database(error)) => {
            error.into()
        }
        PaperTradeUseCaseError::UnitOfWork(
            UnitOfWorkError::Begin(error) | UnitOfWorkError::Commit(error),
        ) => error.into(),
        other => AppError::Internal(other.to_string()),
    }
}
