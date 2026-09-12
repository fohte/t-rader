//! 価格データ取得の inner method 実装。
//!
//! DataProvider 抽象 (`super::data_provider`) 経由でバーデータを取得し、
//! MCP の wire 表現 ([`BarDto`]) に変換する。

use rmcp::ErrorData as McpError;
use uuid::Uuid;

use crate::data_provider::{DataProvider, DateRange};

use super::dto::{BarDto, QueryDataParams, QueryDataResult};
use super::{StrategyServer, data_provider_error, decimal_to_f64, internal_error, invalid_params};

impl StrategyServer {
    pub(crate) async fn query_data_inner(
        &self,
        _session_strategy_id: Uuid,
        execution_step_id: Option<Uuid>,
        params: QueryDataParams,
    ) -> Result<QueryDataResult, McpError> {
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

        let bars: Vec<BarDto> = bars
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

        if let Some(execution_step_id) = execution_step_id
            && let Err(err) = super::evidence::record_query_data(
                &self.db,
                execution_step_id,
                &instrument_id,
                params.from,
                params.to,
                &bars,
            )
            .await
        {
            tracing::warn!(error = %err, %execution_step_id, "failed to record query_data evidence");
        }

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
    use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::data_provider::DataProviderKind;
    use crate::data_provider::ibkr::mock::{IbkrMockServer, MockHistoryBar};
    use crate::entities::strategy_task_step_evidence;
    use crate::testing::create_test_db;

    use super::super::StrategyServer;
    use super::super::dto::QueryDataParams;
    use super::super::tests_common::insert_strategy;

    /// 2 本のバーを返すモック IBKR provider 付きの `StrategyServer` を組み立てる。
    async fn setup_server_with_two_bars(
        pool: PgPool,
    ) -> (DatabaseConnection, StrategyServer, Uuid) {
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

        let server = StrategyServer::new(db.clone(), Some(provider));
        (db, server, strategy_id)
    }

    async fn fetch_evidence_by_step(
        db: &DatabaseConnection,
        execution_step_id: Uuid,
    ) -> Vec<strategy_task_step_evidence::Model> {
        strategy_task_step_evidence::Entity::find()
            .filter(strategy_task_step_evidence::Column::ExecutionStepId.eq(execution_step_id))
            .all(db)
            .await
            .expect("fetch evidence")
    }

    #[sqlx::test(migrations = false)]
    async fn query_data_returns_bars_from_mock_ibkr(pool: PgPool) {
        let (_db, server, strategy_id) = setup_server_with_two_bars(pool).await;

        let result = server
            .query_data_inner(
                strategy_id,
                None,
                QueryDataParams {
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
    async fn query_data_records_evidence_when_execution_step_id_present(pool: PgPool) {
        let (db, server, strategy_id) = setup_server_with_two_bars(pool).await;
        let execution_step_id = Uuid::new_v4();

        let before = chrono::Utc::now().fixed_offset();
        let result = server
            .query_data_inner(
                strategy_id,
                Some(execution_step_id),
                QueryDataParams {
                    instrument_id: "7203".into(),
                    from: NaiveDate::from_ymd_opt(2025, 1, 6).expect("from"),
                    to: NaiveDate::from_ymd_opt(2025, 1, 7).expect("to"),
                },
            )
            .await
            .expect("query");
        let after = chrono::Utc::now().fixed_offset();
        let last_ts = result.bars.last().map(|b| b.timestamp).expect("last bar");

        let expected_snapshot = serde_json::json!({
            "instrument_id": "7203",
            "from": "2025-01-06",
            "to": "2025-01-07",
            "bars": result.bars,
            "total_bars": 2,
            "truncated": false,
        });

        let rows = fetch_evidence_by_step(&db, execution_step_id).await;
        assert_eq!(rows.len(), 1);
        let row = rows.into_iter().next().expect("row");
        // observed_at は呼び出し時刻の動的な値なので、範囲だけ別途検証し、
        // 全体比較では実測値をそのまま期待値に採用する。
        assert!(row.observed_at >= before && row.observed_at <= after);
        let observed_at = row.observed_at;
        let id = row.id;

        assert_eq!(
            row,
            strategy_task_step_evidence::Model {
                id,
                execution_step_id,
                source: "query_data".to_string(),
                source_ref: "7203".to_string(),
                observed_at,
                published_at: Some(last_ts),
                effective_at: Some(last_ts),
                snapshot: expected_snapshot,
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn query_data_records_no_evidence_when_execution_step_id_absent(pool: PgPool) {
        let (db, server, strategy_id) = setup_server_with_two_bars(pool).await;

        server
            .query_data_inner(
                strategy_id,
                None,
                QueryDataParams {
                    instrument_id: "7203".into(),
                    from: NaiveDate::from_ymd_opt(2025, 1, 6).expect("from"),
                    to: NaiveDate::from_ymd_opt(2025, 1, 7).expect("to"),
                },
            )
            .await
            .expect("query");

        let rows = strategy_task_step_evidence::Entity::find()
            .all(&db)
            .await
            .expect("fetch evidence");
        assert_eq!(rows, Vec::new());
    }
}
