use std::sync::LazyLock;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::widget::{
    WidgetAggregation, WidgetBy, WidgetQuery, WidgetQueryError, WidgetQueryRow, WidgetQuerySource,
    WidgetRowKey,
};
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};

use crate::DatabaseHandle;
use crate::group_query_sql::with_shared_market_ctes;
use crate::persistence::persistence_error;

const RESOLVE_AS_OF_SQL: &str = r#"
    SELECT (MAX(latest_timestamp) AT TIME ZONE 'UTC')::date AS as_of
    FROM bars_timestamp_coverage
    WHERE timeframe = '1d'
"#;

const RESOLVE_REQUESTED_AS_OF_SQL: &str = r#"
    SELECT (MAX(timestamp) AT TIME ZONE 'UTC')::date AS as_of
    FROM bars
    WHERE timeframe = '1d'
      AND timestamp < (($1::date + 1)::timestamp AT TIME ZONE 'UTC')
"#;

static QUERY_WIDGET_SQL: LazyLock<String> = LazyLock::new(|| {
    with_shared_market_ctes(
        r#"
            SELECT $6::text AS axis_key WHERE $6::text IS NOT NULL
            UNION
            SELECT $7::text AS axis_key WHERE $7::text IS NOT NULL
        "#,
        r#"
            candidate_stocks AS (
                SELECT stock.id, stock.name
                FROM stock
                WHERE (cardinality($9::text[]) = 0 OR stock.id = ANY($9::text[]))
                  AND (
                      $7::text IS NULL
                      OR EXISTS (
                          SELECT 1
                          FROM group_members AS universe_member
                          WHERE universe_member.stock_id = stock.id
                            AND universe_member.axis_key = $7::text
                            AND universe_member.group_key = $8::text
                      )
                  )
                  AND (
                      $4::text <> 'group'
                      OR EXISTS (
                          SELECT 1
                          FROM group_members AS axis_member
                          WHERE axis_member.stock_id = stock.id
                            AND axis_member.axis_key = $6::text
                      )
                  )
            )
        "#,
        r#"
            stock_changes AS (
                SELECT candidate_stocks.id AS stock_id,
                       MAX(period_bars.close) FILTER (
                           WHERE period_bars.bar_number = 1
                       ) / NULLIF(
                           MAX(period_bars.close) FILTER (
                               WHERE period_bars.bar_number = $2::bigint + 1
                           ),
                           0
                       ) - 1 AS change_rate
                FROM candidate_stocks
                JOIN LATERAL (
                    SELECT bars.close,
                           ROW_NUMBER() OVER (
                               ORDER BY bars.timestamp DESC
                           ) AS bar_number
                    FROM bars
                    WHERE bars.instrument_id = candidate_stocks.id
                      AND bars.timeframe = '1d'
                      AND bars.timestamp >= (
                          (($1::date + 1)::timestamp AT TIME ZONE 'UTC')
                          - (($2::integer * 2 + 14) * INTERVAL '1 day')
                      )
                      AND bars.timestamp < (($1::date + 1)::timestamp AT TIME ZONE 'UTC')
                    ORDER BY bars.timestamp DESC
                    LIMIT $2::integer + 1
                ) AS period_bars ON TRUE
                GROUP BY candidate_stocks.id
                HAVING COUNT(*) FILTER (WHERE period_bars.bar_number = 1) > 0
                   AND COUNT(*) FILTER (
                       WHERE period_bars.bar_number = $2::bigint + 1
                   ) > 0
            )
        "#,
        r#"
            stock_values AS (
                SELECT candidate_stocks.id,
                       candidate_stocks.name,
                       valuations.mkt_cap,
                       CASE $3::text
                           WHEN 'change' THEN stock_changes.change_rate
                           WHEN 'mkt_cap' THEN valuations.mkt_cap
                       END AS value
                FROM candidate_stocks
                LEFT JOIN stock_changes
                  ON stock_changes.stock_id = candidate_stocks.id
                LEFT JOIN LATERAL (
                    SELECT valuation.mkt_cap
                    FROM valuation
                    WHERE LEFT(valuation.code, 4) = candidate_stocks.id
                      AND valuation.date <= $1::date
                      AND valuation.mkt_cap IS NOT NULL
                    ORDER BY valuation.date DESC, valuation.code DESC
                    LIMIT 1
                ) AS valuations ON TRUE
            ),
            grouped_values AS (
                SELECT CASE $4::text
                           WHEN 'market' THEN 'market'
                           WHEN 'stock' THEN stock_values.id
                           WHEN 'group' THEN query_group.group_key
                       END AS row_key,
                       CASE $4::text
                           WHEN 'market' THEN '全体'
                           WHEN 'stock' THEN stock_values.name
                           WHEN 'group' THEN query_group.group_name
                       END AS row_name,
                       stock_values.value,
                       stock_values.mkt_cap
                FROM stock_values
                LEFT JOIN group_members AS query_group
                  ON query_group.stock_id = stock_values.id
                 AND query_group.axis_key = $6::text
                 AND $4::text = 'group'
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
        "#,
    )
});

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
        let statement = match requested {
            Some(requested) => Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                RESOLVE_REQUESTED_AS_OF_SQL,
                [requested.into()],
            ),
            None => Statement::from_string(DatabaseBackend::Postgres, RESOLVE_AS_OF_SQL.to_owned()),
        };
        let rows = self
            .db
            .query_all_raw(statement)
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
                QUERY_WIDGET_SQL.as_str(),
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
        WidgetBy::Stock | WidgetBy::Group => by.as_str(),
    }
}

fn query_error(error: sea_orm::DbErr) -> WidgetQueryError {
    WidgetQueryError::Database(persistence_error(error))
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone};
    use core_application::group_movers::{GroupMoversQuery, GroupMoversQuerySource};
    use core_application::widget::{
        WidgetAggregation, WidgetBy, WidgetMetric, WidgetPeriod, WidgetQuery, WidgetQueryRow,
        WidgetQuerySource, WidgetRowKey, WidgetUniverse,
    };
    use core_domain::bar::{Bar, Timeframe};
    use rust_decimal::Decimal;
    use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
    use uuid::Uuid;

    use super::PostgresWidgetQuerySource;
    use crate::DatabaseHandle;
    use crate::entities::{group_axis, instruments, stock, stock_group, stock_group_member};
    use crate::group_movers::PostgresGroupMoversQuerySource;

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
            .map(|timestamp| chrono::Utc.from_utc_datetime(&timestamp))
            .expect("valid timestamp");
        let close = Decimal::from(close);
        crate::repositories::bars::upsert_bars(
            db,
            vec![Bar {
                instrument_id: id.to_string(),
                timeframe: Timeframe::Daily,
                timestamp,
                open: close,
                high: close,
                low: close,
                close,
                volume: 100,
                adjustment_factor: Decimal::ONE,
            }],
        )
        .await
        .expect("upsert widget bar");
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

    async fn insert_market_aggregate_fixture(db: &DatabaseHandle) {
        insert_stock(db, "D007", "Stock Eta").await;
        insert_stock(db, "D008", "Stock Theta").await;
        for id in ["D007", "D008"] {
            insert_bar(db, id, 1, 100).await;
        }
        insert_bar(db, "D007", 2, 110).await;
        insert_bar(db, "D008", 2, 120).await;
        insert_valuation(db, "D007", 1_000).await;
        insert_valuation(db, "D008", 3_000).await;
    }

    #[backend_test_macros::database_test]
    async fn resolves_as_of_from_the_latest_daily_bar(db: DatabaseHandle) {
        insert_stock(&db, "D009", "Stock Iota").await;
        for day in [1, 3, 5] {
            insert_bar(&db, "D009", day, 100).await;
        }

        let source = PostgresWidgetQuerySource::new(db);
        let result = (
            source
                .resolve_as_of(None)
                .await
                .expect("resolve latest as-of"),
            source
                .resolve_as_of(Some(date(3)))
                .await
                .expect("resolve requested as-of"),
        );

        assert_eq!(result, (Some(date(5)), Some(date(3))));
    }

    #[backend_test_macros::database_test]
    async fn resolves_as_of_to_none_without_daily_bars(db: DatabaseHandle) {
        let result = PostgresWidgetQuerySource::new(db)
            .resolve_as_of(None)
            .await
            .expect("resolve missing as-of");

        assert_eq!(result, None);
    }

    fn market_query(
        metric: core_application::widget::WidgetMetric,
        period: Option<core_application::widget::WidgetPeriod>,
        agg: WidgetAggregation,
    ) -> WidgetQuery {
        WidgetQuery {
            metric,
            period,
            by: WidgetBy::None,
            axis: None,
            indicator: None,
            universe: None,
            agg: Some(agg),
        }
    }

    async fn insert_group_axis(db: &DatabaseHandle, key: &str, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        group_axis::ActiveModel {
            id: Set(id),
            key: Set(key.to_string()),
            name: Set(name.to_string()),
            description: Set("Demo group axis".to_string()),
            derive_from: Set(None),
        }
        .insert(db)
        .await
        .expect("insert widget group axis");
        id
    }

    async fn insert_group(db: &DatabaseHandle, axis_id: Uuid, key: &str, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        stock_group::ActiveModel {
            id: Set(id),
            axis_id: Set(axis_id),
            key: Set(key.to_string()),
            name: Set(name.to_string()),
            description: Set(None),
            code: Set(None),
        }
        .insert(db)
        .await
        .expect("insert widget group");
        id
    }

    async fn insert_group_member(db: &DatabaseHandle, stock_id: &str, group_id: Uuid) {
        stock_group_member::ActiveModel {
            stock_id: Set(stock_id.to_string()),
            group_id: Set(group_id),
            created_at: Default::default(),
        }
        .insert(db)
        .await
        .expect("insert widget group member");
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

        let axis_id = insert_group_axis(&db, "demo-axis", "Demo Axis").await;
        let group_id = insert_group(&db, axis_id, "group-alpha", "Group Alpha").await;
        for id in ["D001", "D002"] {
            insert_group_member(&db, id, group_id).await;
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
    async fn widget_change_matches_group_movers_for_the_same_date_window(db: DatabaseHandle) {
        insert_stock(&db, "D004", "Stock Delta").await;
        for (day, close) in [
            (1, 10_000),
            (2, 100),
            (3, 110),
            (4, 120),
            (5, 130),
            (6, 140),
            (7, 150),
            (8, 20_000),
        ] {
            insert_bar(&db, "D004", day, close).await;
        }

        let axis_id = insert_group_axis(&db, "demo-axis", "Demo Axis").await;
        let group_id = insert_group(&db, axis_id, "group-delta", "Group Delta").await;
        insert_group_member(&db, "D004", group_id).await;

        let widget_query = WidgetQuery {
            metric: WidgetMetric::Change,
            period: Some(WidgetPeriod::FiveDays),
            by: WidgetBy::Stock,
            axis: None,
            indicator: None,
            universe: None,
            agg: None,
        };
        let widget_result = PostgresWidgetQuerySource::new(db.clone())
            .query(date(7), &widget_query)
            .await
            .expect("query widget stock change");
        let group_result = PostgresGroupMoversQuerySource::new(db)
            .list_group_movers(GroupMoversQuery {
                axis_key: "demo-axis".to_string(),
                from: date(3),
                to: date(7),
                min_avg_turnover: Decimal::ZERO,
            })
            .await
            .expect("query group movers");

        assert_eq!(
            (widget_result, group_result),
            (
                vec![WidgetQueryRow {
                    key: WidgetRowKey::Stock("D004".to_string()),
                    name: "Stock Delta".to_string(),
                    value: Decimal::new(5, 1),
                }],
                vec![core_application::group_movers::GroupMover {
                    group_key: "group-delta".to_string(),
                    name: "Group Delta".to_string(),
                    constituent_count: 1,
                    median_change_rate: Some(Decimal::new(5, 1)),
                    turnover_weighted_change_rate: Some(Decimal::new(5, 1)),
                    total_avg_turnover: Decimal::from(13_000),
                    top_stocks: vec![core_application::group_movers::GroupMoverStock {
                        stock_id: "D004".to_string(),
                        name: "Stock Delta".to_string(),
                        change_rate: Decimal::new(5, 1),
                        avg_turnover: Decimal::from(13_000),
                    }],
                }],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn includes_lower_bound_bar_and_excludes_stocks_without_enough_bars(db: DatabaseHandle) {
        insert_stock(&db, "D011", "Stock Kappa").await;
        insert_stock(&db, "D012", "Stock Lambda").await;
        insert_stock(&db, "D013", "Stock Mu").await;
        insert_stock(&db, "D014", "Stock Nu").await;
        insert_bar(&db, "D011", 1, 100).await;
        insert_bar(&db, "D011", 2, 120).await;
        insert_bar(&db, "D012", 19, 100).await;
        insert_bar(&db, "D012", 20, 110).await;
        insert_bar(&db, "D013", 4, 100).await;
        insert_bar(&db, "D013", 20, 120).await;
        insert_bar(&db, "D014", 5, 100).await;
        insert_bar(&db, "D014", 20, 120).await;

        let query = WidgetQuery {
            metric: WidgetMetric::Change,
            period: Some(WidgetPeriod::OneDay),
            by: WidgetBy::Stock,
            axis: None,
            indicator: None,
            universe: None,
            agg: None,
        };
        let result = PostgresWidgetQuerySource::new(db)
            .query(date(20), &query)
            .await
            .expect("query widget stock changes");

        assert_eq!(
            result,
            vec![
                WidgetQueryRow {
                    key: WidgetRowKey::Stock("D012".to_string()),
                    name: "Stock Lambda".to_string(),
                    value: Decimal::new(1, 1),
                },
                WidgetQueryRow {
                    key: WidgetRowKey::Stock("D014".to_string()),
                    name: "Stock Nu".to_string(),
                    value: Decimal::new(2, 1),
                },
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn excludes_change_when_previous_close_is_zero(db: DatabaseHandle) {
        insert_stock(&db, "D015", "Stock Omicron").await;
        insert_bar(&db, "D015", 19, 0).await;
        insert_bar(&db, "D015", 20, 100).await;

        let query = WidgetQuery {
            metric: WidgetMetric::Change,
            period: Some(WidgetPeriod::OneDay),
            by: WidgetBy::Stock,
            axis: None,
            indicator: None,
            universe: None,
            agg: None,
        };
        let result = PostgresWidgetQuerySource::new(db)
            .query(date(20), &query)
            .await
            .expect("query widget stock change");

        assert_eq!(result, Vec::<WidgetQueryRow>::new());
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

    #[backend_test_macros::database_test]
    async fn sums_market_change_rates(db: DatabaseHandle) {
        insert_market_aggregate_fixture(&db).await;
        let result = PostgresWidgetQuerySource::new(db)
            .query(
                date(2),
                &market_query(
                    WidgetMetric::Change,
                    Some(WidgetPeriod::OneDay),
                    WidgetAggregation::Sum,
                ),
            )
            .await
            .expect("query sum of market changes");

        assert_eq!(
            result,
            vec![WidgetQueryRow {
                key: WidgetRowKey::Market,
                name: "全体".to_string(),
                value: Decimal::new(3, 1),
            }],
        );
    }

    #[backend_test_macros::database_test]
    async fn counts_stocks_with_market_change_rates(db: DatabaseHandle) {
        insert_market_aggregate_fixture(&db).await;
        let result = PostgresWidgetQuerySource::new(db)
            .query(
                date(2),
                &market_query(
                    WidgetMetric::Change,
                    Some(WidgetPeriod::OneDay),
                    WidgetAggregation::Count,
                ),
            )
            .await
            .expect("query count of market changes");

        assert_eq!(
            result,
            vec![WidgetQueryRow {
                key: WidgetRowKey::Market,
                name: "全体".to_string(),
                value: Decimal::from(2),
            }],
        );
    }

    #[backend_test_macros::database_test]
    async fn sums_market_capitalizations(db: DatabaseHandle) {
        insert_market_aggregate_fixture(&db).await;
        let result = PostgresWidgetQuerySource::new(db)
            .query(
                date(2),
                &market_query(WidgetMetric::MktCap, None, WidgetAggregation::Sum),
            )
            .await
            .expect("query total market capitalization");

        assert_eq!(
            result,
            vec![WidgetQueryRow {
                key: WidgetRowKey::Market,
                name: "全体".to_string(),
                value: Decimal::from(4_000),
            }],
        );
    }

    #[backend_test_macros::database_test]
    async fn group_universe_filters_stocks_before_group_aggregation(db: DatabaseHandle) {
        insert_stock(&db, "D005", "Stock Epsilon").await;
        insert_stock(&db, "D006", "Stock Zeta").await;
        for id in ["D005", "D006"] {
            insert_bar(&db, id, 1, 100).await;
        }
        insert_bar(&db, "D005", 2, 110).await;
        insert_bar(&db, "D006", 2, 90).await;

        let axis_id = insert_group_axis(&db, "demo-axis", "Demo Axis").await;
        let selected_group = insert_group(&db, axis_id, "group-epsilon", "Group Epsilon").await;
        let other_group = insert_group(&db, axis_id, "group-zeta", "Group Zeta").await;
        insert_group_member(&db, "D005", selected_group).await;
        insert_group_member(&db, "D006", other_group).await;

        let query = WidgetQuery {
            metric: WidgetMetric::Change,
            period: Some(WidgetPeriod::OneDay),
            by: WidgetBy::Group,
            axis: Some("demo-axis".to_string()),
            indicator: None,
            universe: Some(WidgetUniverse::Group {
                group: "demo-axis/group-epsilon".to_string(),
            }),
            agg: Some(WidgetAggregation::Mean),
        };
        let result = PostgresWidgetQuerySource::new(db)
            .query(date(2), &query)
            .await
            .expect("query selected group change");

        assert_eq!(
            result,
            vec![core_application::widget::WidgetQueryRow {
                key: WidgetRowKey::Group {
                    axis: "demo-axis".to_string(),
                    key: "group-epsilon".to_string(),
                },
                name: "Group Epsilon".to_string(),
                value: Decimal::new(1, 1),
            }],
        );
    }
}
