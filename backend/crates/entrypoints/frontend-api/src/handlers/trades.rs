use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::change_history::ChangeHistoryError;
use core_application::note::NoteRepositoryError;
use core_application::trade::{
    CreateTradeCommand, PerformanceSummary as TradePerformanceSummary, TradeOrder, TradeQuery,
    TradeRepositoryError, TradeUpdateCommand, TradeUseCaseError,
};
use core_application::unit_of_work::UnitOfWorkError;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath, JsonQuery};
use crate::models::{
    CreateTradeRequest, PerformanceSummary, TradeListItem, TradeResponse, UpdateTradeRequest,
};

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListTradesQuery {
    pub strategy_id: Option<Uuid>,
    pub symbol: Option<String>,
}

/// 取引履歴一覧
#[utoipa::path(
    get,
    path = "/api/trades",
    tag = "trades",
    params(ListTradesQuery),
    responses(
        (status = 200, body = Vec<TradeListItem>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_trades(
    State(state): State<AppState>,
    JsonQuery(p): JsonQuery<ListTradesQuery>,
) -> Result<Json<Vec<TradeListItem>>, AppError> {
    let rows = state
        .trade_use_cases
        .list(TradeQuery {
            strategy_id: p.strategy_id,
            symbol: p.symbol.filter(|symbol| !symbol.is_empty()),
            date_from: None,
            limit: None,
            order: TradeOrder::DateAscending,
            include_note_count: true,
            include_note_references: false,
        })
        .await
        .map_err(map_trade_error)?;
    Ok(Json(
        rows.into_iter()
            .map(|row| TradeListItem {
                trade: row.trade.into(),
                note_count: row.note_count,
            })
            .collect(),
    ))
}

/// 取引取得
#[utoipa::path(
    get,
    path = "/api/trades/{id}",
    tag = "trades",
    params(("id" = Uuid, Path, description = "取引 ID")),
    responses(
        (status = 200, body = TradeResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_trade(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<TradeResponse>, AppError> {
    let trade = state
        .trade_use_cases
        .get(id)
        .await
        .map_err(map_trade_error)?;
    Ok(Json(trade.into()))
}

/// 取引作成
#[utoipa::path(
    post,
    path = "/api/trades",
    tag = "trades",
    request_body = CreateTradeRequest,
    responses(
        (status = 201, body = TradeResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_trade(
    State(state): State<AppState>,
    JsonBody(p): JsonBody<CreateTradeRequest>,
) -> Result<(StatusCode, Json<TradeResponse>), AppError> {
    let created = state
        .trade_use_cases
        .create(CreateTradeCommand {
            strategy_id: p.strategy_id,
            symbol: p.symbol,
            side: p.side,
            qty: p.qty,
            price: p.price,
            fee: p.fee,
            date: p.date,
            source: p.source,
            note: p.note,
        })
        .await
        .map_err(map_trade_error)?;
    Ok((StatusCode::CREATED, Json(created.into())))
}

/// 取引更新
#[utoipa::path(
    patch,
    path = "/api/trades/{id}",
    tag = "trades",
    params(("id" = Uuid, Path, description = "取引 ID")),
    request_body = UpdateTradeRequest,
    responses(
        (status = 200, body = TradeResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_trade(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(p): JsonBody<UpdateTradeRequest>,
) -> Result<Json<TradeResponse>, AppError> {
    let updated = state
        .trade_use_cases
        .update(
            id,
            TradeUpdateCommand {
                strategy_id: p.strategy_id,
                symbol: p.symbol,
                side: p.side,
                qty: p.qty,
                price: p.price,
                fee: p.fee,
                date: p.date,
                source: p.source,
                note: p.note,
            },
        )
        .await
        .map_err(map_trade_error)?;
    Ok(Json(updated.into()))
}

/// 取引削除
#[utoipa::path(
    delete,
    path = "/api/trades/{id}",
    tag = "trades",
    params(("id" = Uuid, Path, description = "取引 ID")),
    responses(
        (status = 204),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_trade(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    state
        .trade_use_cases
        .delete(id)
        .await
        .map_err(map_trade_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SummaryQuery {
    /// 指定すると戦略単位の集計、未指定だと全戦略横断 (ポートフォリオ全体)
    pub strategy_id: Option<Uuid>,
}

/// 損益サマリ (FIFO ベース)。`strategy_id` 未指定なら全体ポートフォリオ。
#[utoipa::path(
    get,
    path = "/api/trades/summary",
    tag = "trades",
    params(SummaryQuery),
    responses(
        (status = 200, body = PerformanceSummary),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn trades_summary(
    State(state): State<AppState>,
    JsonQuery(p): JsonQuery<SummaryQuery>,
) -> Result<Json<PerformanceSummary>, AppError> {
    let summary = state
        .trade_use_cases
        .summary(p.strategy_id)
        .await
        .map_err(map_trade_error)?;
    Ok(Json(summary_response(summary)))
}

pub(super) fn map_trade_error(error: TradeUseCaseError) -> AppError {
    match error {
        TradeUseCaseError::Validation(message) => AppError::Validation(message),
        TradeUseCaseError::NotFound(id) => AppError::NotFound(format!("trade {id} not found")),
        TradeUseCaseError::ResourceNotFound(message) => AppError::NotFound(message),
        TradeUseCaseError::NoteRead(error) => super::notes::map_note_read_error(error),
        TradeUseCaseError::Repository(TradeRepositoryError::Database(error))
        | TradeUseCaseError::NoteRepository(NoteRepositoryError::Database(error))
        | TradeUseCaseError::ChangeHistory(ChangeHistoryError::Database(error))
        | TradeUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | TradeUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error))
        | TradeUseCaseError::StrategyExistence(
            core_application::strategy_existence::StrategyExistenceError::Database(error),
        ) => error.into(),
        other => AppError::Internal(other.to_string()),
    }
}

fn summary_response(summary: TradePerformanceSummary) -> PerformanceSummary {
    PerformanceSummary {
        strategy_id: summary.strategy_id,
        trade_count: summary.trade_count,
        realized_pnl: summary.realized_pnl,
        positions: summary
            .positions
            .into_iter()
            .map(|position| crate::models::PositionSummary {
                symbol: position.symbol,
                qty: position.qty,
                avg_cost: position.avg_cost,
                cost_basis: position.cost_basis,
                realized_pnl: position.realized_pnl,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use core_application::persistence::PersistenceError;
    use rstest::rstest;
    use serde_json::{Value, json};

    #[rstest]
    #[case::missing_reference(
        PersistenceError::MissingReference("foreign key violation".into()),
        StatusCode::BAD_REQUEST,
        "referenced resource does not exist",
    )]
    #[case::conflict(
        PersistenceError::Conflict("unique violation".into()),
        StatusCode::CONFLICT,
        "resource already exists",
    )]
    #[case::constraint_violation(
        PersistenceError::ConstraintViolation("check violation".into()),
        StatusCode::BAD_REQUEST,
        "value violates database constraint",
    )]
    #[case::record_not_updated(
        PersistenceError::RecordNotUpdated("record not updated".into()),
        StatusCode::NOT_FOUND,
        "resource not found",
    )]
    #[case::database_error(
        PersistenceError::Database("database unavailable".into()),
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal server error",
    )]
    #[tokio::test]
    async fn persistence_errors_keep_http_response_classification(
        #[case] error: PersistenceError,
        #[case] expected_status: StatusCode,
        #[case] expected_message: &str,
    ) {
        let response = crate::error::AppError::from(error).into_response();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let body: Value = serde_json::from_slice(&bytes).expect("parse response body");

        assert_eq!(
            (status, body),
            (expected_status, json!({ "error": expected_message }),),
        );
    }
}
