use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::change_history::ChangeHistoryError;
use core_application::persistence::PersistenceError;
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

fn map_trade_error(error: TradeUseCaseError) -> AppError {
    match error {
        TradeUseCaseError::Validation(message) => AppError::Validation(message),
        TradeUseCaseError::NotFound(id) => AppError::NotFound(format!("trade {id} not found")),
        TradeUseCaseError::Repository(TradeRepositoryError::Database(error))
        | TradeUseCaseError::ChangeHistory(ChangeHistoryError::Database(error))
        | TradeUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | TradeUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error)) => {
            map_persistence_error(error)
        }
        other => AppError::Database(sea_orm::DbErr::Custom(other.to_string())),
    }
}

fn map_persistence_error(error: PersistenceError) -> AppError {
    match error {
        PersistenceError::Database(message) => AppError::Database(sea_orm::DbErr::Custom(message)),
        PersistenceError::MissingReference(_) => {
            AppError::Validation("referenced resource does not exist".into())
        }
        PersistenceError::Conflict(_) => AppError::Conflict("resource already exists".into()),
        PersistenceError::ConstraintViolation(_) => {
            AppError::Validation("value violates database constraint".into())
        }
        PersistenceError::RecordNotUpdated(_) => AppError::NotFound("resource not found".into()),
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
    use rstest::rstest;
    use rust_decimal::Decimal;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use serde_json::{Value, json};
    use uuid::Uuid;

    use super::PersistenceError;
    use crate::testing::{create_test_server_with_db, insert_test_strategy};
    use gateway_postgres::entities::{change_history, trade};

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
        let response = super::map_persistence_error(error).into_response();
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

    fn normalize_trade(mut value: Value) -> Value {
        value["id"] = json!("<dyn>");
        value["strategy_id"] = json!("<dyn>");
        value["created_at"] = json!("<dyn>");
        value["updated_at"] = json!("<dyn>");
        value
    }

    struct SeedTrade {
        strategy_id: Uuid,
        trade_id: Uuid,
        side: &'static str,
        qty: i64,
        price: i64,
        fee: i64,
        day: u32,
    }

    async fn seed_trade(db: &impl sea_orm::ConnectionTrait, seed: SeedTrade) {
        trade::ActiveModel {
            id: Set(seed.trade_id),
            strategy_id: Set(seed.strategy_id),
            symbol: Set("FICTIONAL-SYMBOL".into()),
            side: Set(seed.side.into()),
            qty: Set(Decimal::from(seed.qty)),
            price: Set(Decimal::from(seed.price)),
            fee: Set(Decimal::from(seed.fee)),
            date: Set(chrono::NaiveDate::from_ymd_opt(2026, 6, seed.day).expect("valid date")),
            source: Set("manual".into()),
            note: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert trade");
    }

    async fn history_snapshot(
        db: &impl sea_orm::ConnectionTrait,
        trade_id: Uuid,
    ) -> (String, Uuid, String, String, String, Value) {
        let history = change_history::Entity::find()
            .filter(change_history::Column::TargetId.eq(trade_id))
            .one(db)
            .await
            .expect("query change history")
            .expect("change history exists");
        (
            history.target_kind,
            history.target_id,
            history.actor_kind,
            history.actor_label,
            history.op,
            history.diff_json,
        )
    }

    #[backend_test_macros::database_test]
    async fn list_returns_flattened_trade_with_note_count(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "fictional-strategy").await;
        let trade_id = Uuid::new_v4();
        trade::ActiveModel {
            id: Set(trade_id),
            strategy_id: Set(strategy_id),
            symbol: Set("fictional-symbol-123".into()),
            side: Set("buy".into()),
            qty: Set(Decimal::from(1200)),
            price: Set(Decimal::from(275)),
            fee: Set(Decimal::from(1)),
            date: Set(chrono::NaiveDate::from_ymd_opt(2026, 2, 3).expect("valid date")),
            source: Set("manual".into()),
            note: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(&db)
        .await
        .expect("insert trade");

        let response = server
            .get(&format!("/api/trades?strategy_id={strategy_id}"))
            .await;
        response.assert_status_ok();
        let actual = response
            .json::<Vec<Value>>()
            .into_iter()
            .map(normalize_trade)
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            vec![json!({
                "id": "<dyn>",
                "strategy_id": "<dyn>",
                "symbol": "fictional-symbol-123",
                "side": "buy",
                "qty": 1200,
                "price": 275,
                "fee": 1,
                "date": "2026-02-03",
                "source": "manual",
                "note": null,
                "created_at": "<dyn>",
                "updated_at": "<dyn>",
                "note_count": 0,
            })],
        );
    }

    #[backend_test_macros::database_test]
    async fn create_persists_trade_and_change_history(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "fictional-strategy").await;
        let response = server
            .post("/api/trades")
            .json(&json!({
                    "strategy_id": strategy_id,
                    "symbol": "  FICTIONAL-SYMBOL  ",
                    "side": "buy",
                    "qty": 3,
                    "price": 25,
                    "date": "2026-06-01",
                    "source": "manual",
            }))
            .await;
        let response_status = response.status_code();
        let response_body = response.json::<Value>();
        let trade_id = Uuid::parse_str(response_body["id"].as_str().expect("trade id"))
            .expect("valid trade id");
        let actual = normalize_trade(response_body);
        let history = history_snapshot(&db, trade_id).await;

        assert_eq!(
            (response_status, actual, history),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<dyn>",
                    "strategy_id": "<dyn>",
                    "symbol": "FICTIONAL-SYMBOL",
                    "side": "buy",
                    "qty": 3,
                    "price": 25,
                    "fee": 0,
                    "date": "2026-06-01",
                    "source": "manual",
                    "note": null,
                    "created_at": "<dyn>",
                    "updated_at": "<dyn>",
                }),
                (
                    "trade".to_string(),
                    trade_id,
                    "human".to_string(),
                    "user".to_string(),
                    "create".to_string(),
                    json!({
                        "strategy_id": strategy_id,
                        "symbol": "FICTIONAL-SYMBOL",
                        "side": "buy",
                        "qty": 3,
                        "price": 25,
                    }),
                ),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_records_the_changed_fields(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "fictional-strategy").await;
        let trade_id = Uuid::new_v4();
        seed_trade(
            &db,
            SeedTrade {
                strategy_id,
                trade_id,
                side: "buy",
                qty: 3,
                price: 25,
                fee: 0,
                day: 1,
            },
        )
        .await;

        let response = server
            .patch(&format!("/api/trades/{trade_id}"))
            .json(&json!({ "side": "sell", "qty": 5 }))
            .await;
        let response_status = response.status_code();
        let actual = normalize_trade(response.json::<Value>());
        let history = history_snapshot(&db, trade_id).await;

        assert_eq!(
            (response_status, actual, history),
            (
                StatusCode::OK,
                json!({
                    "id": "<dyn>",
                    "strategy_id": "<dyn>",
                    "symbol": "FICTIONAL-SYMBOL",
                    "side": "sell",
                    "qty": 5,
                    "price": 25,
                    "fee": 0,
                    "date": "2026-06-01",
                    "source": "manual",
                    "note": null,
                    "created_at": "<dyn>",
                    "updated_at": "<dyn>",
                }),
                (
                    "trade".to_string(),
                    trade_id,
                    "human".to_string(),
                    "user".to_string(),
                    "update".to_string(),
                    json!({
                        "side": { "from": "buy", "to": "sell" },
                        "qty": { "from": 3, "to": 5 },
                    }),
                ),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn summary_uses_fifo_cost_and_realized_profit(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "fictional-strategy").await;
        seed_trade(
            &db,
            SeedTrade {
                strategy_id,
                trade_id: Uuid::new_v4(),
                side: "buy",
                qty: 100,
                price: 100,
                fee: 10,
                day: 1,
            },
        )
        .await;
        seed_trade(
            &db,
            SeedTrade {
                strategy_id,
                trade_id: Uuid::new_v4(),
                side: "buy",
                qty: 100,
                price: 120,
                fee: 0,
                day: 2,
            },
        )
        .await;
        seed_trade(
            &db,
            SeedTrade {
                strategy_id,
                trade_id: Uuid::new_v4(),
                side: "sell",
                qty: 150,
                price: 130,
                fee: 20,
                day: 3,
            },
        )
        .await;

        let response = server
            .get(&format!("/api/trades/summary?strategy_id={strategy_id}"))
            .await;

        assert_eq!(
            response.json::<Value>(),
            json!({
                "strategy_id": strategy_id,
                "trade_count": 3,
                "realized_pnl": 3470,
                "positions": [{
                    "symbol": "FICTIONAL-SYMBOL",
                    "qty": 50,
                    "avg_cost": 120,
                    "cost_basis": 6000,
                    "realized_pnl": 3470,
                }],
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_removes_trade_and_records_change_history(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "fictional-strategy").await;
        let trade_id = Uuid::new_v4();
        seed_trade(
            &db,
            SeedTrade {
                strategy_id,
                trade_id,
                side: "buy",
                qty: 3,
                price: 25,
                fee: 0,
                day: 1,
            },
        )
        .await;

        let response = server.delete(&format!("/api/trades/{trade_id}")).await;
        let response_status = response.status_code();
        let trade_exists = trade::Entity::find_by_id(trade_id)
            .one(&db)
            .await
            .expect("query trade")
            .is_some();
        let history = history_snapshot(&db, trade_id).await;

        assert_eq!(
            (response_status, trade_exists, history),
            (
                StatusCode::NO_CONTENT,
                false,
                (
                    "trade".to_string(),
                    trade_id,
                    "human".to_string(),
                    "user".to_string(),
                    "delete".to_string(),
                    json!({}),
                ),
            ),
        );
    }
}
