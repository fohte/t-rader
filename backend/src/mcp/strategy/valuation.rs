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

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use rust_decimal::Decimal;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sea_orm::DatabaseConnection;
    use sqlx::PgPool;
    use uuid::Uuid;

    use super::super::tests_common::build_server;
    use super::{ReadValuationParams, ReadValuationResult, ValuationDto};
    use crate::entities::valuation;
    use crate::testing::create_test_db;

    fn ymd(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    async fn seed(db: &DatabaseConnection, code: &str, date: NaiveDate, eps: Decimal) {
        valuation::ActiveModel {
            code: Set(code.to_string()),
            date: Set(date),
            eps: Set(Some(eps)),
            fwd_eps: Set(None),
            bps: Set(None),
            roe: Set(None),
            fwd_roe: Set(None),
            per: Set(None),
            fwd_per: Set(None),
            pbr: Set(None),
            mkt_cap: Set(None),
        }
        .insert(db)
        .await
        .expect("seed valuation");
    }

    #[sqlx::test(migrations = false)]
    async fn read_valuation_matches_code_prefix_and_date_range(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());
        seed(&db, "ZZZZ0", ymd(2099, 1, 3), Decimal::new(125, 1)).await;
        seed(&db, "ZZZZ1", ymd(2099, 1, 8), Decimal::new(185, 1)).await;
        seed(&db, "ZZZZ0", ymd(2099, 1, 1), Decimal::new(90, 1)).await;
        seed(&db, "YYYY0", ymd(2099, 1, 5), Decimal::new(150, 1)).await;

        let result = server
            .read_valuation_inner(
                Uuid::new_v4(),
                ReadValuationParams {
                    symbol: "ZZZZ".to_string(),
                    from: ymd(2099, 1, 2),
                    to: ymd(2099, 1, 10),
                },
            )
            .await
            .expect("read valuation");

        assert_eq!(
            result,
            ReadValuationResult {
                symbol: "ZZZZ".to_string(),
                items: vec![
                    ValuationDto {
                        date: ymd(2099, 1, 8),
                        eps: Some(18.5),
                        fwd_eps: None,
                        bps: None,
                        roe: None,
                        fwd_roe: None,
                        per: None,
                        fwd_per: None,
                        pbr: None,
                        mkt_cap: None,
                    },
                    ValuationDto {
                        date: ymd(2099, 1, 3),
                        eps: Some(12.5),
                        fwd_eps: None,
                        bps: None,
                        roe: None,
                        fwd_roe: None,
                        per: None,
                        fwd_per: None,
                        pbr: None,
                        mkt_cap: None,
                    },
                ],
            }
        );
    }
}
