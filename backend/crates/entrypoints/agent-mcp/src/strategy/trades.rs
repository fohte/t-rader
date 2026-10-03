//! 戦略実行 MCP の `read_trades` tool。
//!
//! `read_portfolio` の account スコープ (全戦略横断) と同じ規則で対象を決める
//! (`super` の doc comment にある例外参照)。戦略境界の検査は行わず、
//! `TradeDto::strategy_id` でどの戦略の約定かを判別できるようにする。

use core_application::strategy_scope::StrategyScope;
use core_application::trade::{Trade, TradeNoteReference, TradeOrder, TradeQuery};
use rmcp::ErrorData as McpError;

use super::dto::{ReadTradesParams, ReadTradesResult, TradeDto, TradeNoteReferenceDto};
use super::{StrategyServer, clamp_limit, decimal_to_f64, trade_error};

fn trade_to_dto(m: Trade, note_references: Vec<TradeNoteReference>) -> TradeDto {
    TradeDto {
        trade_id: m.id,
        strategy_id: m.strategy_id,
        date: m.date,
        symbol: m.symbol,
        side: m.side,
        qty: decimal_to_f64(m.qty),
        price: decimal_to_f64(m.price),
        notes: note_references
            .into_iter()
            .map(|reference| TradeNoteReferenceDto {
                note_id: reference.note_id,
                note_version_id: reference.note_version_id,
            })
            .collect(),
    }
}

impl StrategyServer {
    pub(crate) async fn read_trades_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: ReadTradesParams,
    ) -> Result<ReadTradesResult, McpError> {
        let _scope = scope.into();
        let symbol = params
            .symbol
            .as_deref()
            .map(str::trim)
            .filter(|symbol| !symbol.is_empty())
            .map(ToOwned::to_owned);
        let rows = self
            .dependencies
            .trades
            .list(TradeQuery {
                strategy_id: None,
                symbol,
                date_from: params.date_from,
                limit: Some(clamp_limit(params.limit)),
                order: TradeOrder::DateDescending,
                include_note_count: false,
                include_note_references: true,
            })
            .await
            .map_err(trade_error)?;
        Ok(ReadTradesResult {
            trades: rows
                .into_iter()
                .map(|row| trade_to_dto(row.trade, row.note_references))
                .collect(),
        })
    }
}
