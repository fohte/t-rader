use std::collections::HashMap;

use chrono::{DateTime, FixedOffset, NaiveDate};
use core_application::paper_trade::{
    NewPaperAccount, PaperAccount, PaperOrderResult, PaperOrderWithResult, PaperTradeAccountStats,
    PaperTradePortfolio, PaperTradePosition,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PaperAccount)]
pub struct PaperAccountResponse {
    pub id: Uuid,
    pub name: String,
    pub strategy_id: Uuid,
    pub purpose: String,
    pub initial_cash_jpy: Decimal,
    pub benchmark_stock_id: Option<String>,
    pub started_on: NaiveDate,
}

impl From<PaperAccount> for PaperAccountResponse {
    fn from(account: PaperAccount) -> Self {
        Self {
            id: account.id,
            name: account.name,
            strategy_id: account.strategy_id,
            purpose: account.purpose,
            initial_cash_jpy: account.initial_cash_jpy,
            benchmark_stock_id: account.benchmark_stock_id,
            started_on: account.started_on,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PaperAccountStats)]
pub struct PaperAccountStatsResponse {
    pub account_id: Uuid,
    pub account_name: String,
    pub strategy_id: Uuid,
    pub purpose: String,
    pub started_on: NaiveDate,
    pub initial_cash_jpy: Decimal,
    pub benchmark_stock_id: Option<String>,
    pub as_of: NaiveDate,
    pub total_assets_jpy: Decimal,
    pub return_since_start: Decimal,
    pub benchmark_return: Option<Decimal>,
    pub closed_trade_count: usize,
    pub win_rate: Option<Decimal>,
    pub average_win_excess_return: Option<Decimal>,
    pub average_loss_excess_return: Option<Decimal>,
    pub unrealized_pnl_jpy: Decimal,
}

impl From<PaperTradeAccountStats> for PaperAccountStatsResponse {
    fn from(stats: PaperTradeAccountStats) -> Self {
        Self {
            account_id: stats.account.id,
            account_name: stats.account.name,
            strategy_id: stats.account.strategy_id,
            purpose: stats.account.purpose,
            started_on: stats.account.started_on,
            initial_cash_jpy: stats.account.initial_cash_jpy,
            benchmark_stock_id: stats.account.benchmark_stock_id,
            as_of: stats.as_of,
            total_assets_jpy: stats.total_assets_jpy,
            return_since_start: stats.return_since_start,
            benchmark_return: stats.benchmark_return,
            closed_trade_count: stats.closed_trade_count,
            win_rate: stats.win_rate,
            average_win_excess_return: stats.average_win_excess_return,
            average_loss_excess_return: stats.average_loss_excess_return,
            unrealized_pnl_jpy: stats.unrealized_pnl_jpy,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PaperAccountPortfolio)]
pub struct PaperAccountPortfolioResponse {
    pub account_id: Uuid,
    pub account_name: String,
    pub strategy_id: Uuid,
    pub purpose: String,
    pub started_on: NaiveDate,
    pub as_of: NaiveDate,
    pub initial_cash_jpy: Decimal,
    pub cash_jpy: Decimal,
    pub positions: Vec<PaperTradePositionResponse>,
    pub orders: Vec<PaperTradeOrderResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PaperTradePositionResponse {
    pub stock_id: String,
    pub qty: i64,
    pub avg_cost_jpy: Decimal,
    pub current_price_jpy: Decimal,
    pub market_value_jpy: Decimal,
    pub unrealized_pnl_jpy: Decimal,
}

impl From<PaperTradePosition> for PaperTradePositionResponse {
    fn from(position: PaperTradePosition) -> Self {
        Self {
            stock_id: position.stock_id,
            qty: position.qty,
            avg_cost_jpy: position.avg_cost_jpy,
            current_price_jpy: position.current_price_jpy,
            market_value_jpy: position.market_value_jpy,
            unrealized_pnl_jpy: position.unrealized_pnl_jpy,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PaperTradeOrderResponse {
    pub order_id: Uuid,
    pub stock_id: String,
    pub side: String,
    pub qty: i64,
    /// 注文の根拠となったノート ID。
    pub note_id: Uuid,
    /// 注文の根拠となったノートバージョン ID。
    pub note_version_id: Uuid,
    pub ordered_at: DateTime<FixedOffset>,
    pub outcome: Option<String>,
    pub fill_date: Option<NaiveDate>,
    pub fill_price_jpy: Option<Decimal>,
    pub reject_reason: Option<String>,
}

impl From<(PaperOrderWithResult, Uuid)> for PaperTradeOrderResponse {
    fn from((item, note_id): (PaperOrderWithResult, Uuid)) -> Self {
        let (outcome, fill_date, fill_price_jpy, reject_reason) = match item.result {
            Some(PaperOrderResult::Filled {
                fill_date,
                fill_price,
                ..
            }) => (
                Some("filled".into()),
                Some(fill_date),
                Some(fill_price),
                None,
            ),
            Some(PaperOrderResult::Rejected { reject_reason, .. }) => (
                Some("rejected".into()),
                None,
                None,
                Some(reject_reason.as_str().into()),
            ),
            None => (None, None, None, None),
        };
        Self {
            order_id: item.order.id,
            stock_id: item.order.stock_id,
            side: item.order.side.as_str().into(),
            qty: item.order.qty,
            note_id,
            note_version_id: item.order.note_version_id,
            ordered_at: item.order.ordered_at,
            outcome,
            fill_date,
            fill_price_jpy,
            reject_reason,
        }
    }
}

impl PaperAccountPortfolioResponse {
    pub(crate) fn new(
        portfolio: PaperTradePortfolio,
        note_ids_by_version: &HashMap<Uuid, Uuid>,
    ) -> Result<Self, Uuid> {
        let orders = portfolio
            .orders
            .into_iter()
            .map(|item| {
                let version_id = item.order.note_version_id;
                note_ids_by_version
                    .get(&version_id)
                    .copied()
                    .map(|note_id| (item, note_id).into())
                    .ok_or(version_id)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            account_id: portfolio.account.id,
            account_name: portfolio.account.name,
            strategy_id: portfolio.account.strategy_id,
            purpose: portfolio.account.purpose,
            started_on: portfolio.account.started_on,
            as_of: portfolio.as_of,
            initial_cash_jpy: portfolio.account.initial_cash_jpy,
            cash_jpy: portfolio.cash_jpy,
            positions: portfolio.positions.into_iter().map(Into::into).collect(),
            orders,
        })
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreatePaperAccountRequest {
    #[schema(min_length = 1, pattern = r"\S")]
    pub name: String,
    pub strategy_id: Uuid,
    #[schema(min_length = 1, pattern = r"\S")]
    pub purpose: String,
    pub initial_cash_jpy: Decimal,
    #[serde(default)]
    pub benchmark_stock_id: Option<String>,
    pub started_on: NaiveDate,
}

impl From<CreatePaperAccountRequest> for NewPaperAccount {
    fn from(request: CreatePaperAccountRequest) -> Self {
        Self {
            name: request.name,
            strategy_id: request.strategy_id,
            purpose: request.purpose,
            initial_cash_jpy: request.initial_cash_jpy,
            benchmark_stock_id: request.benchmark_stock_id,
            started_on: request.started_on,
        }
    }
}
