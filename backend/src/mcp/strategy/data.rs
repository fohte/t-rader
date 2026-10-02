//! 価格データ取得の inner method 実装。
//!
//! Bars use case から複数銘柄分のバーデータをまとめて取得し、MCP の wire 表現
//! ([`InstrumentBarsDto`]) に変換する。日足は poll で定期的に取り込むため、ここでは
//! データプロバイダへの問い合わせを行わない。

use std::collections::HashMap;

use core_application::bars::BarsByInstrumentsQuery;
use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;
use uuid::Uuid;

use super::dto::{BarDto, InstrumentBarsDto, QueryDataParams, QueryDataResult};
use super::{StrategyServer, decimal_to_f64, internal_error, invalid_params};

/// 1 回の呼び出しで指定できる銘柄数の上限
const MAX_QUERY_DATA_INSTRUMENTS: usize = 100;

impl StrategyServer {
    pub(crate) async fn query_data_inner(
        &self,
        scope: impl Into<StrategyScope>,
        execution_step_id: Option<Uuid>,
        params: QueryDataParams,
    ) -> Result<QueryDataResult, McpError> {
        let _scope = scope.into();
        if params.instrument_ids.is_empty() {
            return Err(invalid_params("instrument_ids must not be empty"));
        }
        if params.instrument_ids.len() > MAX_QUERY_DATA_INSTRUMENTS {
            return Err(invalid_params(format!(
                "instrument_ids must not exceed {MAX_QUERY_DATA_INSTRUMENTS} entries"
            )));
        }
        if params.from > params.to {
            return Err(invalid_params("from must be on or before to"));
        }

        let instrument_ids: Vec<String> = params
            .instrument_ids
            .iter()
            .map(|id| id.trim().to_string())
            .collect();
        if instrument_ids.iter().any(String::is_empty) {
            return Err(invalid_params(
                "instrument_ids must not contain empty values",
            ));
        }
        {
            let mut seen = std::collections::HashSet::with_capacity(instrument_ids.len());
            if !instrument_ids.iter().all(|id| seen.insert(id)) {
                return Err(invalid_params("instrument_ids must not contain duplicates"));
            }
        }

        let from = params
            .from
            .and_hms_opt(0, 0, 0)
            .map(|dt| dt.and_utc().fixed_offset());
        let to = params
            .to
            .and_hms_opt(23, 59, 59)
            .map(|dt| dt.and_utc().fixed_offset());

        let rows = self
            .dependencies
            .bars
            .find_bars_by_instruments(BarsByInstrumentsQuery {
                instrument_ids: instrument_ids.clone(),
                timeframe: "1d".to_string(),
                from,
                to,
            })
            .await
            .map_err(|error| {
                tracing::error!(error = %error, "strategy mcp db error");
                internal_error(format!("database error: {error}"))
            })?;

        let mut bars_by_instrument: HashMap<String, Vec<BarDto>> = HashMap::new();
        for bar in rows {
            bars_by_instrument
                .entry(bar.instrument_id.clone())
                .or_default()
                .push(BarDto {
                    timestamp: bar.timestamp.fixed_offset(),
                    open: decimal_to_f64(bar.open),
                    high: decimal_to_f64(bar.high),
                    low: decimal_to_f64(bar.low),
                    close: decimal_to_f64(bar.close),
                    volume: bar.volume,
                });
        }

        let mut results = Vec::with_capacity(instrument_ids.len());
        let evidence_use_cases = &self.dependencies.strategy_task_step_evidence;
        for instrument_id in &instrument_ids {
            let bars = bars_by_instrument.remove(instrument_id).unwrap_or_default();

            if let Some(execution_step_id) = execution_step_id
                && let Err(err) = super::evidence::record_query_data(
                    evidence_use_cases,
                    execution_step_id,
                    instrument_id,
                    params.from,
                    params.to,
                    &bars,
                )
                .await
            {
                tracing::warn!(
                    error = %err,
                    %execution_step_id,
                    %instrument_id,
                    "failed to record query_data evidence",
                );
            }

            results.push(InstrumentBarsDto {
                instrument_id: instrument_id.clone(),
                bars,
            });
        }

        Ok(QueryDataResult { results })
    }
}

#[cfg(test)]
mod tests;
