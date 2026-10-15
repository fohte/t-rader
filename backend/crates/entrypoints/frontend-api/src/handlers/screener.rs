use axum::Json;
use axum::extract::State;
use core_application::screener::{
    ScreenerFilters, ScreenerQuery, ScreenerRange, ScreenerSort, ScreenerSortDirection,
    ScreenerSortField, ScreenerUseCaseError,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonQuery;
use crate::models::{ScreenerResponse, ScreenerSectorResponse};

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ScreenerQueryParams {
    /// 絞り込む市場。複数指定はカンマ区切り
    pub markets: Option<String>,
    /// 東証 33 業種グループの key。複数指定はカンマ区切り
    pub sectors: Option<String>,
    /// PER の下限
    pub min_per: Option<Decimal>,
    /// PER の上限
    pub max_per: Option<Decimal>,
    /// PBR の下限
    pub min_pbr: Option<Decimal>,
    /// PBR の上限
    pub max_pbr: Option<Decimal>,
    /// ROE (パーセント) の下限
    pub min_roe: Option<Decimal>,
    /// ROE (パーセント) の上限
    pub max_roe: Option<Decimal>,
    /// 予想配当利回り (パーセント) の下限
    pub min_dividend_yield: Option<Decimal>,
    /// 予想配当利回り (パーセント) の上限
    pub max_dividend_yield: Option<Decimal>,
    /// 時価総額 (億円) の下限
    pub min_market_cap: Option<Decimal>,
    /// 時価総額 (億円) の上限
    pub max_market_cap: Option<Decimal>,
    /// 並び替える列。省略時は market_cap
    pub sort_by: Option<ScreenerSortBy>,
    /// 並び順。省略時は desc
    pub sort_order: Option<ScreenerSortOrder>,
    /// 1 始まりのページ番号
    #[param(minimum = 1)]
    pub page: Option<u32>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScreenerSortBy {
    Id,
    Name,
    Market,
    Sector,
    Close,
    Per,
    Pbr,
    Roe,
    DividendYield,
    MarketCap,
}

impl From<ScreenerSortBy> for ScreenerSortField {
    fn from(value: ScreenerSortBy) -> Self {
        match value {
            ScreenerSortBy::Id => Self::Id,
            ScreenerSortBy::Name => Self::Name,
            ScreenerSortBy::Market => Self::Market,
            ScreenerSortBy::Sector => Self::Sector,
            ScreenerSortBy::Close => Self::Close,
            ScreenerSortBy::Per => Self::Per,
            ScreenerSortBy::Pbr => Self::Pbr,
            ScreenerSortBy::Roe => Self::Roe,
            ScreenerSortBy::DividendYield => Self::DividendYield,
            ScreenerSortBy::MarketCap => Self::MarketCap,
        }
    }
}

impl From<ScreenerSortField> for ScreenerSortBy {
    fn from(value: ScreenerSortField) -> Self {
        match value {
            ScreenerSortField::Id => Self::Id,
            ScreenerSortField::Name => Self::Name,
            ScreenerSortField::Market => Self::Market,
            ScreenerSortField::Sector => Self::Sector,
            ScreenerSortField::Close => Self::Close,
            ScreenerSortField::Per => Self::Per,
            ScreenerSortField::Pbr => Self::Pbr,
            ScreenerSortField::Roe => Self::Roe,
            ScreenerSortField::DividendYield => Self::DividendYield,
            ScreenerSortField::MarketCap => Self::MarketCap,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScreenerSortOrder {
    Asc,
    Desc,
}

impl From<ScreenerSortOrder> for ScreenerSortDirection {
    fn from(value: ScreenerSortOrder) -> Self {
        match value {
            ScreenerSortOrder::Asc => Self::Asc,
            ScreenerSortOrder::Desc => Self::Desc,
        }
    }
}

impl From<ScreenerSortDirection> for ScreenerSortOrder {
    fn from(value: ScreenerSortDirection) -> Self {
        match value {
            ScreenerSortDirection::Asc => Self::Asc,
            ScreenerSortDirection::Desc => Self::Desc,
        }
    }
}

/// 条件に一致する銘柄を一覧する
#[utoipa::path(
    get,
    path = "/api/screener",
    tag = "screener",
    params(ScreenerQueryParams),
    responses(
        (status = 200, description = "条件に一致する銘柄", body = ScreenerResponse),
        (status = 400, description = "検索条件が不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn screen_stocks(
    State(state): State<FrontendApiState>,
    JsonQuery(params): JsonQuery<ScreenerQueryParams>,
) -> Result<Json<ScreenerResponse>, AppError> {
    let filters = ScreenerFilters {
        markets: split_list(params.markets),
        sectors: split_list(params.sectors),
        per: range(params.min_per, params.max_per),
        pbr: range(params.min_pbr, params.max_pbr),
        roe: range(params.min_roe, params.max_roe),
        dividend_yield: range(params.min_dividend_yield, params.max_dividend_yield),
        market_cap: range(params.min_market_cap, params.max_market_cap),
    };
    let default_query = ScreenerQuery::default();
    let query = ScreenerQuery {
        filters,
        sort: ScreenerSort {
            field: params
                .sort_by
                .map(Into::into)
                .unwrap_or(default_query.sort.field),
            direction: params
                .sort_order
                .map(Into::into)
                .unwrap_or(default_query.sort.direction),
        },
        page: params.page.unwrap_or(default_query.page),
    };

    let result = state
        .screener_use_cases
        .screen(query)
        .await
        .map_err(map_screener_error)?;

    Ok(Json(result.into()))
}

/// スクリーナーで選択できる東証 33 業種を一覧する
#[utoipa::path(
    get,
    path = "/api/screener/sectors",
    tag = "screener",
    responses(
        (status = 200, description = "東証 33 業種の key と名前", body = Vec<ScreenerSectorResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_sector_options(
    State(state): State<FrontendApiState>,
) -> Result<Json<Vec<ScreenerSectorResponse>>, AppError> {
    let sectors = state
        .screener_use_cases
        .list_sectors()
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;

    Ok(Json(sectors.into_iter().map(Into::into).collect()))
}

fn range(min: Option<Decimal>, max: Option<Decimal>) -> Option<ScreenerRange> {
    (min.is_some() || max.is_some()).then_some(ScreenerRange { min, max })
}

fn split_list(value: Option<String>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn map_screener_error(error: ScreenerUseCaseError) -> AppError {
    match error {
        error @ (ScreenerUseCaseError::InvalidFilters(_) | ScreenerUseCaseError::InvalidPage) => {
            AppError::Validation(error.to_string())
        }
        ScreenerUseCaseError::Query(error) => AppError::Internal(error.to_string()),
    }
}
