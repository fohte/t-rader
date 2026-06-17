//! 価格データ取得の inner method 実装。
//!
//! DataProvider 抽象 (`super::data_provider`) 経由でバーデータを取得し、
//! MCP の wire 表現 ([`BarDto`]) に変換する。

use rmcp::ErrorData as McpError;
use uuid::Uuid;

use crate::data_provider::{DataProvider, DateRange};

use super::dto::{BarDto, QueryDataParams, QueryDataResult};
use super::{
    StrategyServer, data_provider_error, decimal_to_f64, ensure_strategy_match, internal_error,
    invalid_params,
};

impl StrategyServer {
    pub(crate) async fn query_data_inner(
        &self,
        session_strategy_id: Uuid,
        params: QueryDataParams,
    ) -> Result<QueryDataResult, McpError> {
        ensure_strategy_match(session_strategy_id, params.strategy_id)?;

        let instrument_id = params.instrument_id.trim().to_string();
        if instrument_id.is_empty() {
            return Err(invalid_params("instrument_id must not be empty"));
        }
        if params.from > params.to {
            return Err(invalid_params("from must be on or before to"));
        }

        let provider = self
            .data_provider
            .as_deref()
            .ok_or_else(|| internal_error("data provider is not configured"))?;

        let bars = provider
            .fetch_daily_bars(
                &instrument_id,
                &DateRange {
                    from: params.from,
                    to: params.to,
                },
            )
            .await
            .map_err(data_provider_error)?;

        let bars = bars
            .into_iter()
            .map(|b| BarDto {
                timestamp: b.timestamp.fixed_offset(),
                open: decimal_to_f64(b.open),
                high: decimal_to_f64(b.high),
                low: decimal_to_f64(b.low),
                close: decimal_to_f64(b.close),
                volume: b.volume,
            })
            .collect();
        Ok(QueryDataResult {
            instrument_id,
            bars,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::NaiveDate;
    use sqlx::PgPool;

    use crate::data_provider::DataProviderKind;
    use crate::data_provider::ibkr::mock::{IbkrMockServer, MockHistoryBar};
    use crate::testing::create_test_db;

    use super::super::StrategyServer;
    use super::super::dto::QueryDataParams;
    use super::super::tests_common::{build_server, insert_strategy};

    #[sqlx::test(migrations = false)]
    async fn query_data_returns_bars_from_mock_ibkr(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;

        let ibkr = IbkrMockServer::start().await;
        ibkr.stocks().ok().await;
        ibkr.history()
            .bars(vec![
                MockHistoryBar {
                    t: 1_736_121_600_000,
                    o: 100.0,
                    h: 110.0,
                    l: 90.0,
                    c: 105.0,
                    v: 1_000.0,
                },
                MockHistoryBar {
                    t: 1_736_208_000_000,
                    o: 105.0,
                    h: 115.0,
                    l: 95.0,
                    c: 110.0,
                    v: 1_500.0,
                },
            ])
            .ok()
            .await;
        let client = ibkr.client().expect("client");
        let provider = Arc::new(DataProviderKind::Ibkr(client));

        let server = StrategyServer::new(db, Some(provider));

        let result = server
            .query_data_inner(
                strategy_id,
                QueryDataParams {
                    strategy_id,
                    instrument_id: "7203".into(),
                    from: NaiveDate::from_ymd_opt(2025, 1, 6).expect("from"),
                    to: NaiveDate::from_ymd_opt(2025, 1, 7).expect("to"),
                },
            )
            .await
            .expect("query");
        let bars: Vec<(f64, f64, f64, f64, i64)> = result
            .bars
            .iter()
            .map(|b| (b.open, b.high, b.low, b.close, b.volume))
            .collect();
        assert_eq!(
            (result.instrument_id.as_str(), bars),
            (
                "7203",
                vec![
                    (100.0, 110.0, 90.0, 105.0, 1_000),
                    (105.0, 115.0, 95.0, 110.0, 1_500),
                ],
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn query_data_rejects_session_mismatch(pool: PgPool) {
        let db = create_test_db(pool).await;
        let session = insert_strategy(&db, "a").await;
        let other = insert_strategy(&db, "b").await;
        let server = build_server(db);
        let err = server
            .query_data_inner(
                session,
                QueryDataParams {
                    strategy_id: other,
                    instrument_id: "7203".into(),
                    from: NaiveDate::from_ymd_opt(2025, 1, 6).expect("from"),
                    to: NaiveDate::from_ymd_opt(2025, 1, 7).expect("to"),
                },
            )
            .await
            .expect_err("boundary violation expected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}
