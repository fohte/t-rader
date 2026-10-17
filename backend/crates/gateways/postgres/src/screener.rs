use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::screener::{
    SCREENER_PAGE_SIZE, ScreenerPage, ScreenerQuery, ScreenerQueryError, ScreenerQuerySource,
    ScreenerSector, ScreenerSortDirection, ScreenerSortField, ScreenerStock,
};
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};

use crate::DatabaseHandle;
use crate::persistence::persistence_error;

const SCREEN_STOCKS_SQL: &str = r#"
    WITH filtered AS (
        SELECT *
        FROM screener_stock_snapshot
        WHERE (cardinality($1::text[]) = 0 OR market = ANY($1::text[]))
          AND (cardinality($2::text[]) = 0 OR sector_key = ANY($2::text[]))
          AND ($3::numeric IS NULL OR per >= $3::numeric)
          AND ($4::numeric IS NULL OR per <= $4::numeric)
          AND ($5::numeric IS NULL OR pbr >= $5::numeric)
          AND ($6::numeric IS NULL OR pbr <= $6::numeric)
          AND ($7::numeric IS NULL OR roe >= $7::numeric)
          AND ($8::numeric IS NULL OR roe <= $8::numeric)
          AND ($9::numeric IS NULL OR dividend_yield >= $9::numeric)
          AND ($10::numeric IS NULL OR dividend_yield <= $10::numeric)
          AND ($11::numeric IS NULL OR market_cap >= $11::numeric)
          AND ($12::numeric IS NULL OR market_cap <= $12::numeric)
    ),
    metadata AS (
        SELECT COUNT(*) AS total_count,
               (SELECT MAX(as_of) FROM screener_stock_snapshot) AS as_of
        FROM filtered
    ),
    paged AS (
        SELECT *
        FROM filtered
        ORDER BY {sort_column} {sort_direction} NULLS LAST, id ASC
        LIMIT $13 OFFSET $14
    )
    SELECT metadata.total_count, metadata.as_of,
           paged.id, paged.name, paged.market, paged.sector, paged.close,
           paged.per, paged.pbr, paged.roe, paged.dividend_yield, paged.market_cap
    FROM metadata
    LEFT JOIN paged ON TRUE
    ORDER BY paged.{sort_column} {sort_direction} NULLS LAST, paged.id ASC
"#;

const LIST_SCREENER_SECTORS_SQL: &str = r#"
    SELECT stock_group.key, stock_group.name
    FROM stock_group
    JOIN group_axis ON group_axis.id = stock_group.axis_id
    WHERE group_axis.derive_from = 'tse_sector33'
    GROUP BY stock_group.key, stock_group.name
    ORDER BY MIN(stock_group.code) NULLS LAST, stock_group.key, stock_group.name
"#;

#[derive(Debug, FromQueryResult)]
struct ScreenerRow {
    total_count: i64,
    as_of: Option<NaiveDate>,
    id: Option<String>,
    name: Option<String>,
    market: Option<String>,
    sector: Option<String>,
    close: Option<Decimal>,
    per: Option<Decimal>,
    pbr: Option<Decimal>,
    roe: Option<Decimal>,
    dividend_yield: Option<Decimal>,
    market_cap: Option<Decimal>,
}

#[derive(Clone)]
pub struct PostgresScreenerQuerySource {
    db: DatabaseHandle,
}

impl PostgresScreenerQuerySource {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ScreenerQuerySource for PostgresScreenerQuerySource {
    async fn screen(&self, query: ScreenerQuery) -> Result<ScreenerPage, ScreenerQueryError> {
        let offset = i64::from(query.page.saturating_sub(1)) * i64::from(SCREENER_PAGE_SIZE);
        let sql = SCREEN_STOCKS_SQL
            .replace("{sort_column}", sort_column(query.sort.field))
            .replace("{sort_direction}", sort_direction(query.sort.direction));
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                sql,
                [
                    query.filters.markets.into(),
                    query.filters.sectors.into(),
                    range_min(query.filters.per.as_ref()).into(),
                    range_max(query.filters.per.as_ref()).into(),
                    range_min(query.filters.pbr.as_ref()).into(),
                    range_max(query.filters.pbr.as_ref()).into(),
                    range_min(query.filters.roe.as_ref()).into(),
                    range_max(query.filters.roe.as_ref()).into(),
                    range_min(query.filters.dividend_yield.as_ref()).into(),
                    range_max(query.filters.dividend_yield.as_ref()).into(),
                    range_min(query.filters.market_cap.as_ref()).into(),
                    range_max(query.filters.market_cap.as_ref()).into(),
                    i64::from(SCREENER_PAGE_SIZE).into(),
                    offset.into(),
                ],
            ))
            .await
            .map_err(query_error)?;

        let mut total_count = 0;
        let mut as_of = None;
        let mut items = Vec::with_capacity(SCREENER_PAGE_SIZE as usize);
        for row in &rows {
            let row = ScreenerRow::from_query_result(row, "").map_err(query_error)?;
            total_count = row.total_count;
            as_of = row.as_of;
            if let Some(id) = row.id {
                let name = row.name.ok_or_else(|| {
                    ScreenerQueryError::Database(
                        core_application::persistence::PersistenceError::Database(
                            "screener row has no stock name".to_string(),
                        ),
                    )
                })?;
                items.push(ScreenerStock {
                    id,
                    name,
                    market: row.market,
                    sector: row.sector,
                    close: row.close,
                    per: row.per,
                    pbr: row.pbr,
                    roe: row.roe,
                    dividend_yield: row.dividend_yield,
                    market_cap: row.market_cap,
                });
            }
        }

        Ok(ScreenerPage {
            items,
            total_count,
            as_of,
            page: query.page,
        })
    }

    async fn list_sectors(&self) -> Result<Vec<ScreenerSector>, ScreenerQueryError> {
        #[derive(Debug, FromQueryResult)]
        struct SectorRow {
            key: String,
            name: String,
        }

        self.db
            .query_all_raw(Statement::from_string(
                DatabaseBackend::Postgres,
                LIST_SCREENER_SECTORS_SQL.to_string(),
            ))
            .await
            .map_err(query_error)?
            .iter()
            .map(|row| SectorRow::from_query_result(row, "").map_err(query_error))
            .map(|result| {
                result.map(|sector| ScreenerSector {
                    key: sector.key,
                    name: sector.name,
                })
            })
            .collect()
    }
}

fn range_min(range: Option<&core_application::screener::ScreenerRange>) -> Option<Decimal> {
    range.and_then(|range| range.min)
}

fn range_max(range: Option<&core_application::screener::ScreenerRange>) -> Option<Decimal> {
    range.and_then(|range| range.max)
}

fn sort_column(field: ScreenerSortField) -> &'static str {
    match field {
        ScreenerSortField::Id => "id",
        ScreenerSortField::Name => "name",
        ScreenerSortField::Market => "market",
        ScreenerSortField::Sector => "sector",
        ScreenerSortField::Close => "close",
        ScreenerSortField::Per => "per",
        ScreenerSortField::Pbr => "pbr",
        ScreenerSortField::Roe => "roe",
        ScreenerSortField::DividendYield => "dividend_yield",
        ScreenerSortField::MarketCap => "market_cap",
    }
}

fn sort_direction(direction: ScreenerSortDirection) -> &'static str {
    match direction {
        ScreenerSortDirection::Asc => "ASC",
        ScreenerSortDirection::Desc => "DESC",
    }
}

fn query_error(error: sea_orm::DbErr) -> ScreenerQueryError {
    ScreenerQueryError::Database(persistence_error(error))
}
