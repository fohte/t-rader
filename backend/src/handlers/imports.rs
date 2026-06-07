//! 取引履歴の外部ソース取込ハンドラ。
//!
//! 設計: 2 段階モデル。
//! 1. `POST /api/imports/sbi/preview` — CSV を raw bytes で受け取り、パース結果と重複判定を返す
//! 2. `POST /api/imports/sbi/commit`  — preview 結果をユーザが確認・戦略割当した上で実 INSERT

use std::collections::HashSet;

use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, TransactionTrait};
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::entities::{stock, strategy, trade};
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonBody;
use crate::models::{
    SbiCommitRequest, SbiCommitResponse, SbiCommitRow, SbiPreviewIssue, SbiPreviewResponse,
    SbiPreviewRow,
};
use crate::services::change_history::{self, Op, TargetKind};
use crate::services::import::sbi;

const ALLOWED_SIDE: [&str; 2] = ["buy", "sell"];

/// SBI 国内株式 CSV プレビュー。
#[utoipa::path(
    post,
    path = "/api/imports/sbi/preview",
    tag = "imports",
    request_body(
        content = String,
        description = "SBI 取引履歴 CSV (Shift_JIS or UTF-8)",
        content_type = "text/csv",
    ),
    responses(
        (status = 200, body = SbiPreviewResponse),
        (status = 400, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn sbi_preview(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<SbiPreviewResponse>, AppError> {
    let parsed = sbi::parse_bytes(&body).map_err(|e| AppError::Validation(e.to_string()))?;

    let mut rows = Vec::with_capacity(parsed.rows.len());
    for r in parsed.rows {
        let is_duplicate =
            trade_exists(&state.db, r.date, &r.symbol, &r.side, r.qty, r.price).await?;
        rows.push(SbiPreviewRow {
            row_index: r.row_index,
            date: r.date,
            symbol: r.symbol,
            stock_name: r.stock_name,
            side: r.side,
            qty: r.qty,
            price: r.price,
            fee: r.fee,
            is_duplicate,
        });
    }
    let issues = parsed
        .issues
        .into_iter()
        .map(|i| SbiPreviewIssue {
            row_index: i.row_index,
            message: i.message,
        })
        .collect();
    Ok(Json(SbiPreviewResponse { rows, issues }))
}

/// プレビュー結果を確認・割当した上で実 INSERT する。
/// 各行に対して重複検知 (同日・同銘柄・同売買・同数量・同単価) を行い skip カウントを返す。
#[utoipa::path(
    post,
    path = "/api/imports/sbi/commit",
    tag = "imports",
    request_body = SbiCommitRequest,
    responses(
        (status = 200, body = SbiCommitResponse),
        (status = 400, body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn sbi_commit(
    State(state): State<AppState>,
    JsonBody(p): JsonBody<SbiCommitRequest>,
) -> Result<Json<SbiCommitResponse>, AppError> {
    for r in &p.rows {
        validate_commit_row(r)?;
    }
    ensure_strategies_exist(&state.db, &p.rows).await?;

    let txn = state.db.begin().await?;
    let mut imported = 0usize;
    let mut skipped = 0usize;
    // 同一 commit リクエスト内で重複行が来た場合、txn 内 SELECT は未 commit の先行 insert を
    // 見れないので別の手段で弾く必要がある (SBI CSV は実際に同条件 2 約定が出現する)
    let mut seen: HashSet<(NaiveDate, String, String, Decimal, Decimal)> = HashSet::new();

    for r in &p.rows {
        let key = (r.date, r.symbol.clone(), r.side.clone(), r.qty, r.price);
        if !seen.insert(key)
            || trade_exists(&txn, r.date, &r.symbol, &r.side, r.qty, r.price).await?
        {
            skipped += 1;
            continue;
        }

        ensure_stock(&txn, &r.symbol, &r.stock_name).await?;

        let id = Uuid::new_v4();
        let fee = r.fee.unwrap_or(Decimal::ZERO);
        trade::Entity::insert(trade::ActiveModel {
            id: Set(id),
            strategy_id: Set(r.strategy_id),
            symbol: Set(r.symbol.clone()),
            side: Set(r.side.clone()),
            qty: Set(r.qty),
            price: Set(r.price),
            fee: Set(fee),
            date: Set(r.date),
            source: Set("csv".into()),
            note: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
        })
        .exec(&txn)
        .await?;

        change_history::record(
            &txn,
            TargetKind::Trade,
            id,
            Op::Create,
            json!({
                "strategy_id": r.strategy_id,
                "symbol": r.symbol,
                "side": r.side,
                "qty": r.qty,
                "price": r.price,
                "source": "csv",
                "origin": "sbi_csv_import",
            }),
            None,
        )
        .await?;

        imported += 1;
    }

    txn.commit().await?;

    Ok(Json(SbiCommitResponse {
        imported_count: imported,
        skipped_count: skipped,
    }))
}

fn validate_commit_row(r: &SbiCommitRow) -> Result<(), AppError> {
    if r.symbol.trim().is_empty() {
        return Err(AppError::Validation("symbol must not be empty".into()));
    }
    if !ALLOWED_SIDE.contains(&r.side.as_str()) {
        return Err(AppError::Validation(format!("invalid side: {}", r.side)));
    }
    if r.qty <= Decimal::ZERO {
        return Err(AppError::Validation("qty must be positive".into()));
    }
    if r.price < Decimal::ZERO {
        return Err(AppError::Validation("price must be non-negative".into()));
    }
    Ok(())
}

/// 既存 stock の name は更新しない (CSV の銘柄名は SBI 由来の表記で、マスタ側を上書きしたくない)。
async fn ensure_stock<C: ConnectionTrait>(
    conn: &C,
    symbol: &str,
    name: &str,
) -> Result<(), AppError> {
    if stock::Entity::find_by_id(symbol.to_string())
        .one(conn)
        .await?
        .is_some()
    {
        return Ok(());
    }
    // 空文字 / 空白のみは銘柄名として無意味なので symbol を fallback とする
    let trimmed = name.trim();
    let resolved_name = if trimmed.is_empty() { symbol } else { trimmed }.to_string();
    let model = stock::ActiveModel {
        id: Set(symbol.to_string()),
        name: Set(resolved_name),
        market: Set(None),
        sector_id: Set(None),
        created_at: NotSet,
        updated_at: NotSet,
    };
    // 並列 import で race した場合に備えて unique 違反は無視する
    if let Err(e) = stock::Entity::insert(model).exec(conn).await {
        if matches!(
            e.sql_err(),
            Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
        ) {
            return Ok(());
        }
        return Err(AppError::Database(e));
    }
    Ok(())
}

/// 重複検知の単一基準: 同日・同銘柄・同売買・同数量・同単価。
async fn trade_exists<C: ConnectionTrait>(
    conn: &C,
    date: NaiveDate,
    symbol: &str,
    side: &str,
    qty: Decimal,
    price: Decimal,
) -> Result<bool, AppError> {
    Ok(trade::Entity::find()
        .filter(trade::Column::Date.eq(date))
        .filter(trade::Column::Symbol.eq(symbol))
        .filter(trade::Column::Side.eq(side))
        .filter(trade::Column::Qty.eq(qty))
        .filter(trade::Column::Price.eq(price))
        .one(conn)
        .await?
        .is_some())
}

/// commit リクエスト中の strategy_id が全て DB に存在することを検証する。
/// 存在しない UUID が混じった場合は FK 違反で 500 になる前に 400 で返す。
async fn ensure_strategies_exist<C: ConnectionTrait>(
    conn: &C,
    rows: &[SbiCommitRow],
) -> Result<(), AppError> {
    let unique: HashSet<Uuid> = rows.iter().map(|r| r.strategy_id).collect();
    if unique.is_empty() {
        return Ok(());
    }
    let ids: Vec<Uuid> = unique.iter().copied().collect();
    let found: HashSet<Uuid> = strategy::Entity::find()
        .filter(strategy::Column::Id.is_in(ids))
        .all(conn)
        .await?
        .into_iter()
        .map(|s| s.id)
        .collect();
    if let Some(missing) = unique.difference(&found).next() {
        return Err(AppError::Validation(format!(
            "strategy {missing} does not exist"
        )));
    }
    Ok(())
}
