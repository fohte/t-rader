use chrono::{DateTime, FixedOffset};
use core_application::saved_screen::{SavedScreen, SavedScreenData};
use core_application::screener::{ScreenerFilters, ScreenerRange, ScreenerSort};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::handlers::screener::{ScreenerSortBy, ScreenerSortOrder};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ScreenerFiltersDto {
    /// 市場区分。
    pub markets: Vec<String>,
    /// 東証 33 業種グループの key。
    pub sectors: Vec<String>,
    pub per: Option<ScreenerRangeDto>,
    pub pbr: Option<ScreenerRangeDto>,
    /// パーセント単位。
    pub roe: Option<ScreenerRangeDto>,
    /// パーセント単位。
    pub dividend_yield: Option<ScreenerRangeDto>,
    /// 億円単位。
    pub market_cap: Option<ScreenerRangeDto>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ScreenerRangeDto {
    pub min: Option<Decimal>,
    pub max: Option<Decimal>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ScreenerSortDto {
    pub field: ScreenerSortBy,
    pub direction: ScreenerSortOrder,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SaveScreenRequest {
    /// 保存条件の名前。
    pub name: String,
    /// 銘柄の絞り込み条件。
    pub filters: ScreenerFiltersDto,
    /// 銘柄の並び順。
    pub sort: ScreenerSortDto,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = SavedScreen)]
pub struct SavedScreenResponse {
    pub id: Uuid,
    pub name: String,
    pub filters: ScreenerFiltersDto,
    pub sort: ScreenerSortDto,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: DateTime<FixedOffset>,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub updated_at: DateTime<FixedOffset>,
}

impl From<ScreenerFiltersDto> for ScreenerFilters {
    fn from(filters: ScreenerFiltersDto) -> Self {
        Self {
            markets: filters.markets,
            sectors: filters.sectors,
            per: filters.per.map(Into::into),
            pbr: filters.pbr.map(Into::into),
            roe: filters.roe.map(Into::into),
            dividend_yield: filters.dividend_yield.map(Into::into),
            market_cap: filters.market_cap.map(Into::into),
        }
    }
}

impl From<ScreenerFilters> for ScreenerFiltersDto {
    fn from(filters: ScreenerFilters) -> Self {
        Self {
            markets: filters.markets,
            sectors: filters.sectors,
            per: filters.per.map(Into::into),
            pbr: filters.pbr.map(Into::into),
            roe: filters.roe.map(Into::into),
            dividend_yield: filters.dividend_yield.map(Into::into),
            market_cap: filters.market_cap.map(Into::into),
        }
    }
}

impl From<ScreenerRangeDto> for ScreenerRange {
    fn from(range: ScreenerRangeDto) -> Self {
        Self {
            min: range.min,
            max: range.max,
        }
    }
}

impl From<ScreenerRange> for ScreenerRangeDto {
    fn from(range: ScreenerRange) -> Self {
        Self {
            min: range.min,
            max: range.max,
        }
    }
}

impl From<ScreenerSortDto> for ScreenerSort {
    fn from(sort: ScreenerSortDto) -> Self {
        Self {
            field: sort.field.into(),
            direction: sort.direction.into(),
        }
    }
}

impl From<ScreenerSort> for ScreenerSortDto {
    fn from(sort: ScreenerSort) -> Self {
        Self {
            field: sort.field.into(),
            direction: sort.direction.into(),
        }
    }
}

impl From<SaveScreenRequest> for SavedScreenData {
    fn from(request: SaveScreenRequest) -> Self {
        Self {
            name: request.name,
            filters: request.filters.into(),
            sort: request.sort.into(),
        }
    }
}

impl From<SavedScreen> for SavedScreenResponse {
    fn from(screen: SavedScreen) -> Self {
        Self {
            id: screen.id,
            name: screen.name,
            filters: screen.filters.into(),
            sort: screen.sort.into(),
            created_at: screen.created_at,
            updated_at: screen.updated_at,
        }
    }
}
