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
        .use_cases
        .refs()
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
        .use_cases
        .refs()
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
        .use_cases
        .refs()
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
        .use_cases
        .refs()
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
        .use_cases
        .refs()
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

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::testing::{create_test_server_with_db, insert_test_group, insert_test_stock};

    fn normalize_stock_timestamps(stocks: &mut Value) {
        for stock in stocks.as_array_mut().expect("stock list") {
            stock["created_at"] = json!("normalized timestamp");
            stock["updated_at"] = json!("normalized timestamp");
        }
    }

    #[backend_test_macros::database_test]
    async fn list_stocks_filters_by_query(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        insert_test_stock(&db, "MOCK_001", "Mock Alpha").await;
        insert_test_stock(&db, "MOCK_002", "Mock Beta").await;

        let response = server.get("/api/refs/stocks?q=Alpha").await;
        response.assert_status_ok();
        let mut actual = response.json::<Value>();
        normalize_stock_timestamps(&mut actual);

        assert_eq!(
            actual,
            json!([{
                "id": "MOCK_001",
                "name": "Mock Alpha",
                "market": null,
                "sector_id": null,
                "created_at": "normalized timestamp",
                "updated_at": "normalized timestamp",
                "product_category": null,
            }]),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_stock_returns_404_for_unknown_id(db: gateway_postgres::DatabaseHandle) {
        let (_db, server) = create_test_server_with_db(db).await;

        let response = server.get("/api/refs/stocks/UNKNOWN").await;

        response.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn resolve_group_link_returns_its_name(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        insert_test_group(&db, "demo-axis", "demo-group", "Sample Group").await;

        let response = server
            .get("/api/refs/resolve?link=group%3Ademo-axis%2Fdemo-group")
            .await;

        assert_eq!(
            (response.status_code(), response.json::<Value>()),
            (
                axum::http::StatusCode::OK,
                json!([{
                    "kind": "group",
                    "id": "demo-axis/demo-group",
                    "name": "Sample Group",
                }]),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn resolve_group_link_rejects_a_missing_axis_key(db: gateway_postgres::DatabaseHandle) {
        let (_db, server) = create_test_server_with_db(db).await;

        let response = server
            .get("/api/refs/resolve?link=group%3Ademo-group")
            .await;

        assert_eq!(response.status_code(), axum::http::StatusCode::BAD_REQUEST,);
    }
}
