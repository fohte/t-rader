//! 取引履歴の外部ソース取込ハンドラ。
//!
//! 設計: 2 段階モデル。
//! 1. `POST /api/imports/sbi/preview` — CSV を raw bytes で受け取り、パース結果と重複判定を返す
//! 2. `POST /api/imports/sbi/commit`  — preview 結果をユーザが確認・戦略割当した上で実 INSERT

use std::collections::HashMap;

use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use core_application::trade::{SbiImportRow, TradeMatchQuery};

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonBody;
use crate::models::{
    SbiCommitRequest, SbiCommitResponse, SbiPreviewIssue, SbiPreviewResponse, SbiPreviewRow,
};
use crate::services::import::sbi;

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
    State(state): State<FrontendApiState>,
    body: Bytes,
) -> Result<Json<SbiPreviewResponse>, AppError> {
    let parsed = sbi::parse_bytes(&body).map_err(|e| AppError::Validation(e.to_string()))?;
    let trades = state.trade_use_cases;

    let mut rows = Vec::with_capacity(parsed.rows.len());
    let mut csv_seen: HashMap<TradeMatchQuery, usize> = HashMap::new();
    for r in parsed.rows {
        let query = TradeMatchQuery {
            date: r.date,
            symbol: r.symbol.clone(),
            side: r.side.clone(),
            qty: r.qty,
            price: r.price,
        };
        let csv_index = {
            let count = csv_seen.entry(query.clone()).or_insert(0);
            *count += 1;
            *count
        };
        let db_count = trades
            .count_import_matches(&query)
            .await
            .map_err(super::trades::map_trade_error)?;
        // CSV 内 N 件目の出現を、DB 既存 N 件と突合する。分割約定など同条件の取引が複数回
        // 起こりうるので、単純な存在チェックだと正当な 2 件目以降が skip 扱いになる。
        let is_duplicate = csv_index <= db_count;
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
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn sbi_commit(
    State(state): State<FrontendApiState>,
    JsonBody(p): JsonBody<SbiCommitRequest>,
) -> Result<Json<SbiCommitResponse>, AppError> {
    let result = state
        .trade_use_cases
        .commit_sbi_import(
            p.rows
                .into_iter()
                .map(|row| SbiImportRow {
                    strategy_id: row.strategy_id,
                    date: row.date,
                    symbol: row.symbol,
                    stock_name: row.stock_name,
                    side: row.side,
                    qty: row.qty,
                    price: row.price,
                    fee: row.fee,
                })
                .collect(),
        )
        .await
        .map_err(super::trades::map_trade_error)?;
    Ok(Json(SbiCommitResponse {
        imported_count: result.imported_count,
        skipped_count: result.skipped_count,
    }))
}
