//! 一級参照型 (stock / indicator / group) の検索・詳細・リンク解決

use axum::Json;
use axum::extract::State;
use core_application::refs::{RefRepositoryError, RefUseCaseError};
use core_application::unit_of_work::UnitOfWorkError;
use serde::Deserialize;
use utoipa::IntoParams;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonPath, JsonQuery};
use crate::models::{IndicatorResponse, RefResolution, StockResponse};
use core_domain::note_reference::ALLOWED_REF_KINDS;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchQuery {
    /// 部分一致クエリ。空のときは先頭から最大 50 件返す
    #[serde(default)]
    pub q: Option<String>,
}

/// stock 検索
#[utoipa::path(
    get,
    path = "/api/refs/stocks",
    tag = "refs",
    params(SearchQuery),
    responses(
        (status = 200, body = Vec<StockResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_stocks(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<SearchQuery>,
) -> Result<Json<Vec<StockResponse>>, AppError> {
    let items = state
        .ref_use_cases
        .list_stocks(params.q.as_deref())
        .await
        .map_err(map_ref_error)?
        .into_iter()
        .map(StockResponse::from)
        .collect();
    Ok(Json(items))
}

/// stock 詳細
#[utoipa::path(
    get,
    path = "/api/refs/stocks/{id}",
    tag = "refs",
    params(("id" = String, Path, description = "銘柄コード")),
    responses(
        (status = 200, body = StockResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_stock(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<String>,
) -> Result<Json<StockResponse>, AppError> {
    let m = state
        .ref_use_cases
        .get_stock(&id)
        .await
        .map_err(map_ref_error)?
        .ok_or_else(|| AppError::NotFound(format!("stock {id} not found")))?;
    Ok(Json(StockResponse::from(m)))
}

/// indicator 検索
#[utoipa::path(
    get,
    path = "/api/refs/indicators",
    tag = "refs",
    params(SearchQuery),
    responses(
        (status = 200, body = Vec<IndicatorResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_indicators(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<SearchQuery>,
) -> Result<Json<Vec<IndicatorResponse>>, AppError> {
    let items = state
        .ref_use_cases
        .list_indicators(params.q.as_deref())
        .await
        .map_err(map_ref_error)?
        .into_iter()
        .map(IndicatorResponse::from)
        .collect();
    Ok(Json(items))
}

/// indicator 詳細
#[utoipa::path(
    get,
    path = "/api/refs/indicators/{id}",
    tag = "refs",
    operation_id = "get_ref_indicator",
    params(("id" = String, Path, description = "指標 ID")),
    responses(
        (status = 200, body = IndicatorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_indicator(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<String>,
) -> Result<Json<IndicatorResponse>, AppError> {
    let m = state
        .ref_use_cases
        .get_indicator(&id)
        .await
        .map_err(map_ref_error)?
        .ok_or_else(|| AppError::NotFound(format!("indicator {id} not found")))?;
    Ok(Json(IndicatorResponse::from(m)))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ResolveQuery {
    /// `[[kind:id]]` 形式のリンクテキスト、またはカンマ区切りで複数指定。group の id は `axis-key/group-key`。
    pub link: String,
}

/// `[[kind:id]]` の参照解決。リンクテキストから表示名を引く。
///
/// `link=stock:demo-code,indicator:demo-index,group:demo-axis/demo-group` のようにカンマ区切りで複数渡せる。
/// id が master と一致しない場合、`ref_term` の別名が一意に一致すれば正規の
/// id と name を返す (レスポンスの id が入力と異なることがある)。
/// どちらにも一致しないものは name = null、id は入力のまま返す。
#[utoipa::path(
    get,
    path = "/api/refs/resolve",
    tag = "refs",
    params(ResolveQuery),
    responses(
        (status = 200, body = Vec<RefResolution>),
        (status = 400, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn resolve_refs(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<ResolveQuery>,
) -> Result<Json<Vec<RefResolution>>, AppError> {
    const MAX_LINKS: usize = 200;

    let mut requested: Vec<(String, String)> = Vec::new();
    for raw in params.link.split(',') {
        let Some((kind, id)) = raw.split_once(':') else {
            return Err(AppError::Validation(format!(
                "invalid link format: {raw} (expected kind:id)"
            )));
        };
        let kind = kind.trim();
        let id = id.trim();
        if kind.is_empty() || id.is_empty() {
            return Err(AppError::Validation(format!("invalid link: {raw}")));
        }
        if !ALLOWED_REF_KINDS.contains(&kind) {
            return Err(AppError::Validation(format!(
                "unknown ref kind: {kind} (allowed: {})",
                ALLOWED_REF_KINDS.join(", ")
            )));
        }
        requested.push((kind.to_string(), id.to_string()));
        if requested.len() > MAX_LINKS {
            return Err(AppError::Validation(format!(
                "too many links (max {MAX_LINKS})"
            )));
        }
    }

    let out = state
        .ref_use_cases
        .resolve(&requested)
        .await
        .map_err(map_ref_error)?
        .into_iter()
        .map(RefResolution::from)
        .collect();
    Ok(Json(out))
}

fn map_ref_error(error: RefUseCaseError) -> AppError {
    match error {
        RefUseCaseError::Validation(message) => AppError::Validation(message),
        RefUseCaseError::Repository(RefRepositoryError::Database(error))
        | RefUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | RefUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error)) => error.into(),
        RefUseCaseError::Repository(RefRepositoryError::InvalidTransaction)
        | RefUseCaseError::UnitOfWork(UnitOfWorkError::InvalidTransaction) => {
            AppError::Internal("reference transaction has an unexpected type".into())
        }
    }
}
