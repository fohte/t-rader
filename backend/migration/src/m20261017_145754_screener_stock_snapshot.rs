use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261017_145754_screener_stock_snapshot"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            r#"
            CREATE MATERIALIZED VIEW screener_stock_snapshot AS
            WITH latest_any_valuation AS MATERIALIZED (
                SELECT MAX(valuation.date) AS date
                FROM valuation
                JOIN stock ON stock.id = LEFT(valuation.code, 4)
            ),
            latest_daily_timestamp AS (
                SELECT (
                    SELECT bars.timestamp
                    FROM bars
                    JOIN stock ON stock.id = bars.instrument_id
                    WHERE bars.timeframe = '1d'
                    ORDER BY bars.timestamp DESC
                    LIMIT 1
                ) AS timestamp
            ),
            latest_data AS (
                SELECT latest_daily_timestamp.timestamp,
                       COALESCE(
                           latest_daily_timestamp.timestamp::date,
                           (SELECT date FROM latest_any_valuation)
                       ) AS latest_date,
                       30 AS lookback_days
                FROM latest_daily_timestamp
            ),
            latest_valuations AS (
                SELECT DISTINCT ON (LEFT(valuation.code, 4))
                    LEFT(valuation.code, 4) AS stock_id,
                    valuation.date AS valuation_date,
                    valuation.per,
                    valuation.pbr,
                    valuation.roe * 100 AS roe,
                    valuation.mkt_cap / 100 AS market_cap
                FROM valuation
                JOIN stock ON stock.id = LEFT(valuation.code, 4)
                CROSS JOIN latest_data
                WHERE valuation.date >= latest_data.latest_date - latest_data.lookback_days
                ORDER BY LEFT(valuation.code, 4), valuation.date DESC, valuation.code DESC
            ),
            latest_bars AS (
                SELECT DISTINCT ON (bars.instrument_id)
                    bars.instrument_id,
                    bars.close
                FROM bars
                JOIN stock ON stock.id = bars.instrument_id
                WHERE bars.timeframe = '1d'
                  AND bars.timestamp >= (
                      SELECT latest_data.timestamp
                          - latest_data.lookback_days * INTERVAL '1 day'
                      FROM latest_data
                  )
                ORDER BY bars.instrument_id, bars.timestamp DESC
            ),
            latest_valuation_date AS (
                SELECT COALESCE(
                    (SELECT MAX(valuation_date) FROM latest_valuations),
                    (SELECT date FROM latest_any_valuation)
                ) AS as_of
            ),
            latest_financial_summaries AS (
                SELECT DISTINCT ON (LEFT(code, 4))
                    LEFT(code, 4) AS stock_id,
                    CASE
                        WHEN current_period_type = 'FY' THEN dividend_annual_forecast_next
                        ELSE dividend_annual_forecast
                    END AS dividend_annual_forecast
                FROM financial_summary
                WHERE CASE
                    WHEN current_period_type = 'FY' THEN dividend_annual_forecast_next
                    ELSE dividend_annual_forecast
                END IS NOT NULL
                ORDER BY LEFT(code, 4), disclosure_date DESC, disclosure_no::bigint DESC
            ),
            sectors AS (
                SELECT DISTINCT ON (member.stock_id)
                    member.stock_id, stock_group.key AS sector_key, stock_group.name AS sector
                FROM stock_group_member AS member
                JOIN stock_group ON stock_group.id = member.group_id
                JOIN group_axis ON group_axis.id = stock_group.axis_id
                WHERE group_axis.derive_from = 'tse_sector33'
                ORDER BY member.stock_id, stock_group.key
            )
            SELECT stock.id,
                   stock.name,
                   stock.market,
                   sectors.sector_key,
                   sectors.sector,
                   latest_bars.close,
                   latest_valuations.per,
                   latest_valuations.pbr,
                   latest_valuations.roe,
                   (latest_financial_summaries.dividend_annual_forecast::numeric
                       / NULLIF(latest_bars.close, 0)) * 100 AS dividend_yield,
                   latest_valuations.market_cap,
                   latest_valuation_date.as_of
            FROM stock
            LEFT JOIN latest_valuations ON latest_valuations.stock_id = stock.id
            LEFT JOIN latest_bars ON latest_bars.instrument_id = stock.id
            LEFT JOIN latest_financial_summaries ON latest_financial_summaries.stock_id = stock.id
            LEFT JOIN sectors ON sectors.stock_id = stock.id
            CROSS JOIN latest_valuation_date
            WITH DATA
            "#,
        )
        .await?;
        db.execute_unprepared(
            "CREATE UNIQUE INDEX screener_stock_snapshot_stock_id_idx ON screener_stock_snapshot (id)",
        )
        .await?;
        db.execute_unprepared(
            "COMMENT ON MATERIALIZED VIEW screener_stock_snapshot IS 'スクリーナーの銘柄別最新値を保持する。'",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP MATERIALIZED VIEW screener_stock_snapshot")
            .await?;
        Ok(())
    }
}
