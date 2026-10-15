use core_application::saved_screen::SavedScreen;
use core_application::screener::{
    ScreenerFilters, ScreenerRange, ScreenerSort, ScreenerSortDirection, ScreenerSortField,
};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct SavedScreenDto {
    pub id: Uuid,
    pub name: String,
    pub filters: SavedScreenFiltersDto,
    pub sort: SavedScreenSortDto,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct SavedScreenFiltersDto {
    pub markets: Vec<String>,
    pub sectors: Vec<String>,
    pub per: Option<SavedScreenRangeDto>,
    pub pbr: Option<SavedScreenRangeDto>,
    pub roe: Option<SavedScreenRangeDto>,
    pub dividend_yield: Option<SavedScreenRangeDto>,
    pub market_cap: Option<SavedScreenRangeDto>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct SavedScreenRangeDto {
    #[schemars(with = "Option<f64>")]
    pub min: Option<rust_decimal::Decimal>,
    #[schemars(with = "Option<f64>")]
    pub max: Option<rust_decimal::Decimal>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct SavedScreenSortDto {
    pub field: SavedScreenSortField,
    pub direction: SavedScreenSortDirection,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SavedScreenSortField {
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

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SavedScreenSortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ListSavedScreensResult {
    pub screens: Vec<SavedScreenDto>,
}

impl From<SavedScreen> for SavedScreenDto {
    fn from(screen: SavedScreen) -> Self {
        Self {
            id: screen.id,
            name: screen.name,
            filters: screen.filters.into(),
            sort: screen.sort.into(),
        }
    }
}

impl From<ScreenerFilters> for SavedScreenFiltersDto {
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

impl From<ScreenerRange> for SavedScreenRangeDto {
    fn from(range: ScreenerRange) -> Self {
        Self {
            min: range.min,
            max: range.max,
        }
    }
}

impl From<ScreenerSort> for SavedScreenSortDto {
    fn from(sort: ScreenerSort) -> Self {
        Self {
            field: match sort.field {
                ScreenerSortField::Id => SavedScreenSortField::Id,
                ScreenerSortField::Name => SavedScreenSortField::Name,
                ScreenerSortField::Market => SavedScreenSortField::Market,
                ScreenerSortField::Sector => SavedScreenSortField::Sector,
                ScreenerSortField::Close => SavedScreenSortField::Close,
                ScreenerSortField::Per => SavedScreenSortField::Per,
                ScreenerSortField::Pbr => SavedScreenSortField::Pbr,
                ScreenerSortField::Roe => SavedScreenSortField::Roe,
                ScreenerSortField::DividendYield => SavedScreenSortField::DividendYield,
                ScreenerSortField::MarketCap => SavedScreenSortField::MarketCap,
            },
            direction: match sort.direction {
                ScreenerSortDirection::Asc => SavedScreenSortDirection::Asc,
                ScreenerSortDirection::Desc => SavedScreenSortDirection::Desc,
            },
        }
    }
}
