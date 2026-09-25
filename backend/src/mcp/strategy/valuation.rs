//! 戦略実行 MCP の `read_valuation` tool。

use chrono::NaiveDate;
use rmcp::ErrorData as McpError;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use uuid::Uuid;

use super::dto::{ReadValuationParams, ReadValuationResult, ValuationDto};
use super::{StrategyServer, db_error};

const READ_VALUATION_SQL: &str = indoc::indoc! {"
    SELECT
        date,
        eps::double precision AS eps,
        fwd_eps::double precision AS fwd_eps,
        bps::double precision AS bps,
        roe::double precision AS roe,
        fwd_roe::double precision AS fwd_roe,
        per::double precision AS per,
        fwd_per::double precision AS fwd_per,
        pbr::double precision AS pbr,
        mkt_cap::double precision AS mkt_cap
    FROM valuation
    WHERE LEFT(code, 4) = $1
      AND date >= $2
      AND date <= $3
    ORDER BY date DESC
"};

#[derive(Debug, FromQueryResult)]
struct ValuationRow {
    date: NaiveDate,
    eps: Option<f64>,
    fwd_eps: Option<f64>,
    bps: Option<f64>,
    roe: Option<f64>,
    fwd_roe: Option<f64>,
    per: Option<f64>,
    fwd_per: Option<f64>,
    pbr: Option<f64>,
    mkt_cap: Option<f64>,
}

impl From<ValuationRow> for ValuationDto {
    fn from(row: ValuationRow) -> Self {
        Self {
            date: row.date,
            eps: row.eps,
            fwd_eps: row.fwd_eps,
            bps: row.bps,
            roe: row.roe,
            fwd_roe: row.fwd_roe,
            per: row.per,
            fwd_per: row.fwd_per,
            pbr: row.pbr,
            mkt_cap: row.mkt_cap,
        }
    }
}

impl StrategyServer {
    pub(crate) async fn read_valuation_inner(
        &self,
        // valuation は会社単位の市場データであり戦略に属さないため検索条件に使わない。
        _session_strategy_id: Uuid,
        params: ReadValuationParams,
    ) -> Result<ReadValuationResult, McpError> {
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                READ_VALUATION_SQL,
                [
                    params.symbol.clone().into(),
                    params.from.into(),
                    params.to.into(),
                ],
            ))
            .await
            .map_err(db_error)?;

        let items = rows
            .iter()
            .map(|row| ValuationRow::from_query_result(row, ""))
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_error)?
            .into_iter()
            .map(ValuationDto::from)
            .collect();

        Ok(ReadValuationResult {
            symbol: params.symbol,
            items,
        })
    }
}
