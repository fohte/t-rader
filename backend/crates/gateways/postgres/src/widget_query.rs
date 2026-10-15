use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::widget::{
    WidgetAggregation, WidgetBy, WidgetQuery, WidgetQueryError, WidgetQueryRow, WidgetQuerySource,
    WidgetRowKey,
};
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};

use crate::DatabaseHandle;
use crate::persistence::persistence_error;

const RESOLVE_AS_OF_SQL: &str = r#"
    SELECT MAX((timestamp AT TIME ZONE 'UTC')::date) AS as_of
    FROM bars
    WHERE timeframe = '1d'
      AND ($1::date IS NULL
           OR timestamp < (($1::date + 1)::timestamp AT TIME ZONE 'UTC'))
"#;

const QUERY_WIDGET_SQL: &str = r#"
    WITH stock_values AS (
        SELECT stock.id,
               stock.name,
               prices.latest_close,
               prices.period_close,
               valuations.mkt_cap,
               CASE $3::text
                   WHEN 'change' THEN
                       prices.latest_close / NULLIF(prices.period_close, 0) - 1
                   WHEN 'mkt_cap' THEN valuations.mkt_cap
               END AS value
        FROM stock
        LEFT JOIN LATERAL (
            SELECT MAX(recent.close) FILTER (WHERE recent.ordinal = 1) AS latest_close,
                   MAX(recent.close) FILTER (WHERE recent.ordinal = $2::bigint + 1)
                       AS period_close
            FROM (
                SELECT bars.close,
                       ROW_NUMBER() OVER (ORDER BY bars.timestamp DESC) AS ordinal
                FROM bars
                WHERE bars.instrument_id = stock.id
                  AND bars.timeframe = '1d'
                  AND bars.timestamp < (($1::date + 1)::timestamp AT TIME ZONE 'UTC')
                ORDER BY bars.timestamp DESC
                LIMIT 61
            ) AS recent
        ) AS prices ON TRUE
        LEFT JOIN LATERAL (
            SELECT valuation.mkt_cap
            FROM valuation
            WHERE LEFT(valuation.code, 4) = stock.id
              AND valuation.date <= $1::date
              AND valuation.mkt_cap IS NOT NULL
            ORDER BY valuation.date DESC, valuation.code DESC
            LIMIT 1
        ) AS valuations ON TRUE
        WHERE (cardinality($9::text[]) = 0 OR stock.id = ANY($9::text[]))
          AND (
              $7::text IS NULL
              OR EXISTS (
                  SELECT 1
                  FROM stock_group_member AS universe_member
                  JOIN stock_group AS universe_group
                    ON universe_group.id = universe_member.group_id
                  JOIN group_axis AS universe_axis
                    ON universe_axis.id = universe_group.axis_id
                  WHERE universe_member.stock_id = stock.id
                    AND universe_axis.key = $7::text
                    AND universe_group.key = $8::text
              )
          )
    ),
    grouped_values AS (
        SELECT CASE $4::text
                   WHEN 'market' THEN 'market'
                   WHEN 'stock' THEN stock_values.id
                   WHEN 'group' THEN query_group.key
               END AS row_key,
               CASE $4::text
                   WHEN 'market' THEN '全体'
                   WHEN 'stock' THEN stock_values.name
                   WHEN 'group' THEN query_group.name
               END AS row_name,
               stock_values.value,
               stock_values.mkt_cap
        FROM stock_values
        LEFT JOIN group_axis AS query_axis
          ON query_axis.key = $6::text
         AND $4::text = 'group'
        LEFT JOIN stock_group_member AS query_member
          ON query_member.stock_id = stock_values.id
         AND $4::text = 'group'
        LEFT JOIN stock_group AS query_group
          ON query_group.id = query_member.group_id
         AND query_group.axis_id = query_axis.id
    ),
    aggregated_values AS (
        SELECT row_key,
               row_name,
               CASE
                   WHEN $4::text = 'stock' THEN MAX(value)
                   WHEN $5::text = 'mean' THEN AVG(value)
                   WHEN $5::text = 'sum' THEN SUM(value)
                   WHEN $5::text = 'count' THEN COUNT(value)::numeric
                   WHEN $5::text = 'mkt_cap_weighted' THEN
                       SUM(value * mkt_cap)
                           / NULLIF(SUM(mkt_cap) FILTER (WHERE value IS NOT NULL), 0)
               END AS value
        FROM grouped_values
        GROUP BY row_key, row_name
    )
    SELECT row_key, row_name, value
    FROM aggregated_values
    WHERE row_key IS NOT NULL
      AND value IS NOT NULL
    ORDER BY row_key
"#;

#[derive(Debug, FromQueryResult)]
struct AsOfRow {
    as_of: Option<NaiveDate>,
}

#[derive(Debug, FromQueryResult)]
struct WidgetQueryResultRow {
    row_key: String,
    row_name: String,
    value: Decimal,
}

#[derive(Clone)]
pub struct PostgresWidgetQuerySource {
    db: DatabaseHandle,
}

impl PostgresWidgetQuerySource {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl WidgetQuerySource for PostgresWidgetQuerySource {
    async fn resolve_as_of(
        &self,
        requested: Option<NaiveDate>,
    ) -> Result<Option<NaiveDate>, WidgetQueryError> {
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                RESOLVE_AS_OF_SQL,
                [requested.into()],
            ))
            .await
            .map_err(query_error)?;

        rows.first()
            .map(|row| AsOfRow::from_query_result(row, "").map(|value| value.as_of))
            .transpose()
            .map(|as_of| as_of.flatten())
            .map_err(query_error)
    }

    async fn query(
        &self,
        as_of: NaiveDate,
        definition: &WidgetQuery,
    ) -> Result<Vec<WidgetQueryRow>, WidgetQueryError> {
        let (universe_axis, universe_key) = definition
            .universe
            .as_ref()
            .and_then(|universe| universe.group_parts())
            .map(|(axis, key)| (Some(axis.to_string()), Some(key.to_string())))
            .unwrap_or((None, None));
        let stock_ids = definition
            .universe
            .as_ref()
            .and_then(|universe| universe.stock_ids())
            .map(<[String]>::to_vec)
            .unwrap_or_default();
        let aggregation = definition.agg.map(WidgetAggregation::as_str);
        let axis = definition.axis.clone();
        let period = definition
            .period
            .map(|period| period.trading_days())
            .unwrap_or(1);

        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                QUERY_WIDGET_SQL,
                [
                    as_of.into(),
                    period.into(),
                    definition.metric.as_str().into(),
                    query_by_sql(definition.by).into(),
                    aggregation.map(str::to_string).into(),
                    axis.into(),
                    universe_axis.into(),
                    universe_key.into(),
                    stock_ids.into(),
                ],
            ))
            .await
            .map_err(query_error)?;

        rows.iter()
            .map(|row| {
                let result =
                    WidgetQueryResultRow::from_query_result(row, "").map_err(query_error)?;
                let key = match definition.by {
                    WidgetBy::None => WidgetRowKey::Market,
                    WidgetBy::Stock => WidgetRowKey::Stock(result.row_key),
                    WidgetBy::Group => WidgetRowKey::Group {
                        axis: definition.axis.clone().unwrap_or_default(),
                        key: result.row_key,
                    },
                };
                Ok(WidgetQueryRow {
                    key,
                    name: result.row_name,
                    value: result.value,
                })
            })
            .collect()
    }
}

fn query_by_sql(by: WidgetBy) -> &'static str {
    match by {
        WidgetBy::None => "market",
        WidgetBy::Stock => "stock",
        WidgetBy::Group => "group",
    }
}

fn query_error(error: sea_orm::DbErr) -> WidgetQueryError {
    WidgetQueryError::Database(persistence_error(error))
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone};
    use core_application::widget::{
        WidgetAggregation, WidgetBy, WidgetMetric, WidgetPeriod, WidgetQuery, WidgetQuerySource,
        WidgetRowKey, WidgetUniverse,
    };
    use rust_decimal::Decimal;
    use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
    use uuid::Uuid;

    use super::PostgresWidgetQuerySource;
    use crate::DatabaseHandle;
    use crate::entities::{bars, group_axis, instruments, stock, stock_group, stock_group_member};

    fn date(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2098, 1, day).expect("valid test date")
    }

    async fn insert_stock(db: &DatabaseHandle, id: &str, name: &str) {
        stock::ActiveModel {
            id: Set(id.to_string()),
            name: Set(name.to_string()),
            market: Set(Some("OTHER".to_string())),
            product_category: Set(None),
            created_at: Default::default(),
            updated_at: Default::default(),
        }
        .insert(db)
        .await
        .expect("insert widget stock");
        instruments::Entity::insert(instruments::ActiveModel {
            id: Set(id.to_string()),
            name: Set(name.to_string()),
            market: Set("OTHER".to_string()),
            sector: Set(None),
        })
        .exec_without_returning(db)
        .await
        .expect("insert widget instrument");
    }

    async fn insert_bar(db: &DatabaseHandle, id: &str, day: u32, close: i64) {
        let timestamp = date(day)
            .and_hms_opt(0, 0, 0)
            .map(|timestamp| chrono::Utc.from_utc_datetime(&timestamp).fixed_offset())
            .expect("valid timestamp");
        let close = Decimal::from(close);
        bars::Entity::insert(bars::ActiveModel {
            instrument_id: Set(id.to_string()),
            timeframe: Set("1d".to_string()),
            timestamp: Set(timestamp),
            open: Set(close),
            high: Set(close),
            low: Set(close),
            close: Set(close),
            volume: Set(100),
            adjustment_factor: Set(Decimal::ONE),
        })
        .exec_without_returning(db)
        .await
        .expect("insert widget bar");
    }

    async fn insert_valuation(db: &DatabaseHandle, id: &str, mkt_cap: i64) {
        crate::entities::valuation::ActiveModel {
            code: Set(format!("{id}0")),
            date: Set(date(2)),
            eps: Set(None),
            fwd_eps: Set(None),
            bps: Set(None),
            roe: Set(None),
            fwd_roe: Set(None),
            per: Set(None),
            fwd_per: Set(None),
            pbr: Set(None),
            mkt_cap: Set(Some(Decimal::from(mkt_cap))),
        }
        .insert(db)
        .await
        .expect("insert widget valuation");
    }

    #[backend_test_macros::database_test]
    async fn weighted_change_uses_latest_market_cap_for_each_group(db: DatabaseHandle) {
        insert_stock(&db, "D001", "Stock Alpha").await;
        insert_stock(&db, "D002", "Stock Beta").await;
        insert_stock(&db, "D003", "Stock Gamma").await;
        for id in ["D001", "D002", "D003"] {
            insert_bar(&db, id, 1, 100).await;
        }
        insert_bar(&db, "D001", 2, 110).await;
        insert_bar(&db, "D002", 2, 90).await;
        insert_bar(&db, "D003", 2, 120).await;
        insert_valuation(&db, "D001", 1_000).await;
        insert_valuation(&db, "D002", 3_000).await;
        insert_valuation(&db, "D003", 1_000).await;

        let axis_id = Uuid::new_v4();
        group_axis::ActiveModel {
            id: Set(axis_id),
            key: Set("demo-axis".to_string()),
            name: Set("Demo Axis".to_string()),
            description: Set("Demo group axis".to_string()),
            derive_from: Set(None),
        }
        .insert(&db)
        .await
        .expect("insert widget group axis");
        let group_id = Uuid::new_v4();
        stock_group::ActiveModel {
            id: Set(group_id),
            axis_id: Set(axis_id),
            key: Set("group-alpha".to_string()),
            name: Set("Group Alpha".to_string()),
            description: Set(None),
            code: Set(None),
        }
        .insert(&db)
        .await
        .expect("insert widget group");
        for id in ["D001", "D002"] {
            stock_group_member::ActiveModel {
                stock_id: Set(id.to_string()),
                group_id: Set(group_id),
                created_at: Default::default(),
            }
            .insert(&db)
            .await
            .expect("insert widget group member");
        }

        let query = WidgetQuery {
            metric: WidgetMetric::Change,
            period: Some(WidgetPeriod::OneDay),
            by: WidgetBy::Group,
            axis: Some("demo-axis".to_string()),
            indicator: None,
            universe: None,
            agg: Some(WidgetAggregation::MarketCapWeighted),
        };
        let result = PostgresWidgetQuerySource::new(db)
            .query(date(2), &query)
            .await
            .expect("query weighted group change");

        assert_eq!(
            result,
            vec![core_application::widget::WidgetQueryRow {
                key: WidgetRowKey::Group {
                    axis: "demo-axis".to_string(),
                    key: "group-alpha".to_string(),
                },
                name: "Group Alpha".to_string(),
                value: Decimal::new(-5, 2),
            }],
        );
    }

    #[backend_test_macros::database_test]
    async fn universe_filters_stocks_before_aggregation(db: DatabaseHandle) {
        insert_stock(&db, "D001", "Stock Alpha").await;
        insert_stock(&db, "D002", "Stock Beta").await;
        for id in ["D001", "D002"] {
            insert_bar(&db, id, 1, 100).await;
        }
        insert_bar(&db, "D001", 2, 110).await;
        insert_bar(&db, "D002", 2, 90).await;

        let query = WidgetQuery {
            metric: WidgetMetric::Change,
            period: Some(WidgetPeriod::OneDay),
            by: WidgetBy::None,
            axis: None,
            indicator: None,
            universe: Some(WidgetUniverse::Stocks {
                stocks: vec!["D001".to_string()],
            }),
            agg: Some(WidgetAggregation::Mean),
        };
        let result = PostgresWidgetQuerySource::new(db)
            .query(date(2), &query)
            .await
            .expect("query filtered market change");

        assert_eq!(
            result,
            vec![core_application::widget::WidgetQueryRow {
                key: WidgetRowKey::Market,
                name: "全体".to_string(),
                value: Decimal::new(1, 1),
            }],
        );
    }
}
