//! 戦略実行 MCP の `read_macro_indicator` tool。
//!
//! マクロ指標の日次観測値 (`indicator_observation`、取り込み元は
//! `backend/src/services/fred_ingest.rs`) を id + 期間で読む。indicator は戦略に属さない
//! 市場データのため、`search_refs` / `search_news` 同様 `x-strategy-id` を検索条件には
//! 使わない。

use rmcp::ErrorData as McpError;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::entities::indicator_observation;

use super::dto::{IndicatorObservationDto, ReadMacroIndicatorParams, ReadMacroIndicatorResult};
use super::{StrategyServer, db_error, decimal_to_f64, invalid_params};

impl StrategyServer {
    pub(crate) async fn read_macro_indicator_inner(
        &self,
        _session_strategy_id: Uuid,
        params: ReadMacroIndicatorParams,
    ) -> Result<ReadMacroIndicatorResult, McpError> {
        let indicator_id = params.indicator_id.trim().to_string();
        if indicator_id.is_empty() {
            return Err(invalid_params("indicator_id must not be empty"));
        }
        if params.from > params.to {
            return Err(invalid_params("from must be on or before to"));
        }

        let rows = indicator_observation::Entity::find()
            .filter(indicator_observation::Column::IndicatorId.eq(indicator_id.clone()))
            .filter(indicator_observation::Column::Date.gte(params.from))
            .filter(indicator_observation::Column::Date.lte(params.to))
            .order_by_asc(indicator_observation::Column::Date)
            .all(&self.db)
            .await
            .map_err(db_error)?;

        Ok(ReadMacroIndicatorResult {
            indicator_id,
            observations: rows
                .into_iter()
                .map(|row| IndicatorObservationDto {
                    date: row.date,
                    value: decimal_to_f64(row.value),
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;

    use uuid::Uuid;

    use crate::entities::{indicator, indicator_observation};
    use crate::testing::create_test_db;

    use super::super::dto::{IndicatorObservationDto, ReadMacroIndicatorParams};
    use super::super::tests_common::build_server;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    async fn seed_indicator(db: &impl sea_orm::ConnectionTrait, id: &str) {
        indicator::ActiveModel {
            id: Set(id.to_string()),
            name: Set(id.to_string()),
            kind: Set("fx".to_string()),
        }
        .insert(db)
        .await
        .expect("seed indicator");
    }

    async fn seed_observation(
        db: &impl sea_orm::ConnectionTrait,
        id: &str,
        date: NaiveDate,
        value: &str,
    ) {
        indicator_observation::ActiveModel {
            indicator_id: Set(id.to_string()),
            date: Set(date),
            value: Set(value.parse().expect("valid decimal")),
        }
        .insert(db)
        .await
        .expect("seed observation");
    }

    #[backend_test_macros::database_test]
    async fn returns_observations_in_date_range_oldest_first(pool: sqlx::PgPool) {
        let db = create_test_db(pool).await;
        seed_indicator(&db, "USDJPY").await;
        seed_observation(&db, "USDJPY", ymd(2026, 9, 1), "147.50").await;
        seed_observation(&db, "USDJPY", ymd(2026, 9, 3), "148.20").await;
        seed_observation(&db, "USDJPY", ymd(2026, 9, 10), "150.00").await;

        let result = build_server(db)
            .read_macro_indicator_inner(
                Uuid::new_v4(),
                ReadMacroIndicatorParams {
                    indicator_id: "USDJPY".to_string(),
                    from: ymd(2026, 9, 1),
                    to: ymd(2026, 9, 5),
                },
            )
            .await
            .expect("read_macro_indicator");

        assert_eq!(
            result.observations,
            vec![
                IndicatorObservationDto {
                    date: ymd(2026, 9, 1),
                    value: 147.50,
                },
                IndicatorObservationDto {
                    date: ymd(2026, 9, 3),
                    value: 148.20,
                },
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn returns_empty_when_no_observation_in_range(pool: sqlx::PgPool) {
        let db = create_test_db(pool).await;
        seed_indicator(&db, "VIX").await;
        seed_observation(&db, "VIX", ymd(2026, 1, 1), "15.0").await;

        let result = build_server(db)
            .read_macro_indicator_inner(
                Uuid::new_v4(),
                ReadMacroIndicatorParams {
                    indicator_id: "VIX".to_string(),
                    from: ymd(2026, 9, 1),
                    to: ymd(2026, 9, 30),
                },
            )
            .await
            .expect("read_macro_indicator");

        assert_eq!(
            result,
            super::ReadMacroIndicatorResult {
                indicator_id: "VIX".to_string(),
                observations: vec![],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn rejects_empty_indicator_id(pool: sqlx::PgPool) {
        let db = create_test_db(pool).await;

        let err = build_server(db)
            .read_macro_indicator_inner(
                Uuid::new_v4(),
                ReadMacroIndicatorParams {
                    indicator_id: "   ".to_string(),
                    from: ymd(2026, 1, 1),
                    to: ymd(2026, 1, 31),
                },
            )
            .await
            .expect_err("empty indicator_id should be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn rejects_from_after_to(pool: sqlx::PgPool) {
        let db = create_test_db(pool).await;

        let err = build_server(db)
            .read_macro_indicator_inner(
                Uuid::new_v4(),
                ReadMacroIndicatorParams {
                    indicator_id: "USDJPY".to_string(),
                    from: ymd(2026, 9, 10),
                    to: ymd(2026, 9, 1),
                },
            )
            .await
            .expect_err("from after to should be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}
