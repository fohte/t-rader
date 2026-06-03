use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use rust_decimal::Decimal;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, TransactionTrait,
};
use serde::Deserialize;
use serde_json::json;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::AppState;
use crate::entities::trade;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath, JsonQuery};
use crate::models::{CreateTradeRequest, PerformanceSummary, UpdateTradeRequest};
use crate::services::change_history::{self, Op, TargetKind};
use crate::services::trades as trades_svc;

const ALLOWED_SIDE: [&str; 2] = ["buy", "sell"];
const ALLOWED_SOURCE: [&str; 3] = ["manual", "csv", "api"];

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
        (status = 200, body = Vec<trade::Model>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_trades(
    State(state): State<AppState>,
    JsonQuery(p): JsonQuery<ListTradesQuery>,
) -> Result<Json<Vec<trade::Model>>, AppError> {
    let mut q = trade::Entity::find()
        .order_by_asc(trade::Column::Date)
        .order_by_asc(trade::Column::CreatedAt);
    if let Some(sid) = p.strategy_id {
        q = q.filter(trade::Column::StrategyId.eq(sid));
    }
    if let Some(sym) = p.symbol.as_deref().filter(|s| !s.is_empty()) {
        q = q.filter(trade::Column::Symbol.eq(sym));
    }
    Ok(Json(q.all(&state.db).await?))
}

/// 取引取得
#[utoipa::path(
    get,
    path = "/api/trades/{id}",
    tag = "trades",
    params(("id" = Uuid, Path, description = "取引 ID")),
    responses(
        (status = 200, body = trade::Model),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_trade(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<trade::Model>, AppError> {
    let m = trade::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("trade {id} not found")))?;
    Ok(Json(m))
}

/// 取引作成
#[utoipa::path(
    post,
    path = "/api/trades",
    tag = "trades",
    request_body = CreateTradeRequest,
    responses(
        (status = 201, body = trade::Model),
        (status = 400, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_trade(
    State(state): State<AppState>,
    JsonBody(p): JsonBody<CreateTradeRequest>,
) -> Result<(StatusCode, Json<trade::Model>), AppError> {
    let symbol = p.symbol.trim().to_string();
    if symbol.is_empty() {
        return Err(AppError::Validation("symbol must not be empty".into()));
    }
    if !ALLOWED_SIDE.contains(&p.side.as_str()) {
        return Err(AppError::Validation(format!("invalid side: {}", p.side)));
    }
    if !ALLOWED_SOURCE.contains(&p.source.as_str()) {
        return Err(AppError::Validation(format!(
            "invalid source: {}",
            p.source
        )));
    }
    if p.qty <= Decimal::ZERO {
        return Err(AppError::Validation("qty must be positive".into()));
    }
    if p.price < Decimal::ZERO {
        return Err(AppError::Validation("price must be non-negative".into()));
    }

    let id = Uuid::new_v4();
    let model = trade::ActiveModel {
        id: Set(id),
        strategy_id: Set(p.strategy_id),
        symbol: Set(symbol.clone()),
        side: Set(p.side.clone()),
        qty: Set(p.qty),
        price: Set(p.price),
        fee: Set(p.fee.unwrap_or(Decimal::ZERO)),
        date: Set(p.date),
        source: Set(p.source.clone()),
        note: Set(p.note.clone()),
        created_at: NotSet,
        updated_at: NotSet,
    };
    let txn = state.db.begin().await?;
    let created = trade::Entity::insert(model)
        .exec_with_returning(&txn)
        .await?;

    change_history::record(
        &txn,
        TargetKind::Trade,
        id,
        Op::Create,
        json!({
            "strategy_id": p.strategy_id,
            "symbol": symbol,
            "side": p.side,
            "qty": p.qty,
            "price": p.price,
        }),
        None,
    )
    .await?;
    txn.commit().await?;

    Ok((StatusCode::CREATED, Json(created)))
}

/// 取引更新
#[utoipa::path(
    patch,
    path = "/api/trades/{id}",
    tag = "trades",
    params(("id" = Uuid, Path, description = "取引 ID")),
    request_body = UpdateTradeRequest,
    responses(
        (status = 200, body = trade::Model),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_trade(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(p): JsonBody<UpdateTradeRequest>,
) -> Result<Json<trade::Model>, AppError> {
    let current = trade::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("trade {id} not found")))?;
    let mut active = current.clone().into_active_model();
    let mut diff = serde_json::Map::new();

    if let Some(v) = p.symbol {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            return Err(AppError::Validation("symbol must not be empty".into()));
        }
        diff.insert(
            "symbol".into(),
            json!({ "from": current.symbol, "to": trimmed }),
        );
        active.symbol = Set(trimmed);
    }
    if let Some(v) = p.side {
        if !ALLOWED_SIDE.contains(&v.as_str()) {
            return Err(AppError::Validation(format!("invalid side: {v}")));
        }
        diff.insert("side".into(), json!({ "from": current.side, "to": v }));
        active.side = Set(v);
    }
    if let Some(v) = p.qty {
        if v <= Decimal::ZERO {
            return Err(AppError::Validation("qty must be positive".into()));
        }
        diff.insert("qty".into(), json!({ "from": current.qty, "to": v }));
        active.qty = Set(v);
    }
    if let Some(v) = p.price {
        diff.insert("price".into(), json!({ "from": current.price, "to": v }));
        active.price = Set(v);
    }
    if let Some(v) = p.fee {
        diff.insert("fee".into(), json!({ "from": current.fee, "to": v }));
        active.fee = Set(v);
    }
    if let Some(v) = p.date {
        diff.insert("date".into(), json!({ "from": current.date, "to": v }));
        active.date = Set(v);
    }
    if let Some(v) = p.source {
        if !ALLOWED_SOURCE.contains(&v.as_str()) {
            return Err(AppError::Validation(format!("invalid source: {v}")));
        }
        diff.insert("source".into(), json!({ "from": current.source, "to": v }));
        active.source = Set(v);
    }
    if let Some(v) = p.note {
        diff.insert("note".into(), json!({ "from": current.note, "to": v }));
        active.note = Set(Some(v));
    }
    active.updated_at = Set(chrono::Utc::now().fixed_offset());

    let txn = state.db.begin().await?;
    let updated = active.update(&txn).await?;
    if !diff.is_empty() {
        change_history::record(
            &txn,
            TargetKind::Trade,
            id,
            Op::Update,
            serde_json::Value::Object(diff),
            None,
        )
        .await?;
    }
    txn.commit().await?;
    Ok(Json(updated))
}

/// 取引削除
#[utoipa::path(
    delete,
    path = "/api/trades/{id}",
    tag = "trades",
    params(("id" = Uuid, Path, description = "取引 ID")),
    responses(
        (status = 204),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_trade(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    let txn = state.db.begin().await?;
    let res = trade::Entity::delete_by_id(id).exec(&txn).await?;
    if res.rows_affected == 0 {
        return Err(AppError::NotFound(format!("trade {id} not found")));
    }
    change_history::record(&txn, TargetKind::Trade, id, Op::Delete, json!({}), None).await?;
    txn.commit().await?;
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
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn trades_summary(
    State(state): State<AppState>,
    JsonQuery(p): JsonQuery<SummaryQuery>,
) -> Result<Json<PerformanceSummary>, AppError> {
    let mut q = trade::Entity::find()
        .order_by_asc(trade::Column::Date)
        .order_by_asc(trade::Column::CreatedAt);
    if let Some(sid) = p.strategy_id {
        q = q.filter(trade::Column::StrategyId.eq(sid));
    }
    let trades = q.all(&state.db).await?;
    Ok(Json(trades_svc::summarize(p.strategy_id, &trades)))
}
