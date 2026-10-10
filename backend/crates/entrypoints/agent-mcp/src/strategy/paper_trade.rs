use core_application::paper_trade::{
    NewPaperOrder, PaperAccount, PaperOrderResult, PaperOrderSide, PaperTradeRepositoryError,
    PaperTradeUseCaseError,
};
use core_application::strategy_scope::StrategyScope;
use core_application::strategy_task::GetTaskError;
use rmcp::ErrorData as McpError;
use uuid::Uuid;

use super::dto::{
    PaperTradeAccountStatsDto, PaperTradeOrderDto, PaperTradePositionDto, PlacePaperOrderParams,
    PlacePaperOrderResult, ReadPaperPortfolioParams, ReadPaperPortfolioResult,
    ReadPaperStatsResult,
};
use super::{StrategyServer, decimal_to_f64, internal_error, invalid_params};

impl StrategyServer {
    pub(crate) async fn place_paper_order_inner(
        &self,
        scope: impl Into<StrategyScope>,
        task_id: Option<String>,
        params: PlacePaperOrderParams,
    ) -> Result<PlacePaperOrderResult, McpError> {
        let scope = scope.into();
        let account = self
            .paper_account_for_execution(scope.id(), task_id.as_deref())
            .await?;
        let side = PaperOrderSide::parse(&params.side)
            .ok_or_else(|| invalid_params("side must be 'buy' or 'sell'"))?;
        let order = self
            .dependencies
            .paper_trades
            .record_order(NewPaperOrder {
                account_id: account.id,
                stock_id: params.stock_id,
                side,
                qty: params.qty,
                note_version_id: params.note_version_id,
            })
            .await
            .map_err(paper_trade_error_to_mcp)?;

        Ok(PlacePaperOrderResult {
            order_id: order.id,
            stock_id: order.stock_id,
            side: order.side.as_str().to_string(),
            qty: order.qty,
            note_version_id: order.note_version_id,
            ordered_at: order.ordered_at,
        })
    }

    pub(crate) async fn read_paper_portfolio_inner(
        &self,
        scope: impl Into<StrategyScope>,
        task_id: Option<String>,
        params: ReadPaperPortfolioParams,
    ) -> Result<ReadPaperPortfolioResult, McpError> {
        let scope = scope.into();
        let account_id = match params.account_id {
            Some(account_id) => account_id,
            None => {
                self.paper_account_for_execution(scope.id(), task_id.as_deref())
                    .await?
                    .id
            }
        };
        let portfolio = self
            .dependencies
            .paper_trades
            .portfolio(account_id)
            .await
            .map_err(paper_trade_error_to_mcp)?;

        Ok(ReadPaperPortfolioResult {
            account_id: portfolio.account.id,
            account_name: portfolio.account.name,
            strategy_id: portfolio.account.strategy_id,
            purpose: portfolio.account.purpose,
            started_on: portfolio.account.started_on,
            as_of: portfolio.as_of,
            initial_cash_jpy: decimal_to_f64(portfolio.account.initial_cash_jpy),
            cash_jpy: decimal_to_f64(portfolio.cash_jpy),
            positions: portfolio
                .positions
                .into_iter()
                .map(|position| PaperTradePositionDto {
                    stock_id: position.stock_id,
                    qty: position.qty,
                    avg_cost_jpy: decimal_to_f64(position.avg_cost_jpy),
                    current_price_jpy: decimal_to_f64(position.current_price_jpy),
                    market_value_jpy: decimal_to_f64(position.market_value_jpy),
                    unrealized_pnl_jpy: decimal_to_f64(position.unrealized_pnl_jpy),
                })
                .collect(),
            orders: portfolio
                .orders
                .into_iter()
                .map(paper_order_to_dto)
                .collect(),
        })
    }

    pub(crate) async fn read_paper_stats_inner(&self) -> Result<ReadPaperStatsResult, McpError> {
        let accounts = self
            .dependencies
            .paper_trades
            .stats()
            .await
            .map_err(paper_trade_error_to_mcp)?;

        Ok(ReadPaperStatsResult {
            accounts: accounts
                .into_iter()
                .map(|stats| PaperTradeAccountStatsDto {
                    account_id: stats.account.id,
                    account_name: stats.account.name,
                    strategy_id: stats.account.strategy_id,
                    purpose: stats.account.purpose,
                    started_on: stats.account.started_on,
                    initial_cash_jpy: decimal_to_f64(stats.account.initial_cash_jpy),
                    benchmark_stock_id: stats.account.benchmark_stock_id,
                    as_of: stats.as_of,
                    total_assets_jpy: decimal_to_f64(stats.total_assets_jpy),
                    return_since_start: decimal_to_f64(stats.return_since_start),
                    benchmark_return: stats.benchmark_return.map(decimal_to_f64),
                    closed_trade_count: stats.closed_trade_count,
                    win_rate: stats.win_rate.map(decimal_to_f64),
                    average_win_excess_return: stats.average_win_excess_return.map(decimal_to_f64),
                    average_loss_excess_return: stats
                        .average_loss_excess_return
                        .map(decimal_to_f64),
                    unrealized_pnl_jpy: decimal_to_f64(stats.unrealized_pnl_jpy),
                })
                .collect(),
        })
    }

    async fn paper_account_for_execution(
        &self,
        strategy_id: Uuid,
        task_id: Option<&str>,
    ) -> Result<PaperAccount, McpError> {
        let task_id = task_id.ok_or_else(|| {
            invalid_params("paper trading requires an x-execution-id for the current task")
        })?;
        let task = self
            .dependencies
            .strategy_tasks
            .get_by_a2a_task_id(task_id)
            .await
            .map_err(strategy_task_error_to_mcp)?
            .ok_or_else(|| invalid_params("x-execution-id refers to an unknown strategy task"))?;
        if task.strategy_id != strategy_id {
            return Err(invalid_params(
                "x-execution-id refers to a task from another strategy",
            ));
        }
        let purpose = task
            .purpose
            .filter(|purpose| !purpose.trim().is_empty())
            .ok_or_else(|| invalid_params("strategy task has no execution purpose"))?;
        self.dependencies
            .paper_trades
            .account_for_strategy_purpose(strategy_id, &purpose)
            .await
            .map_err(paper_trade_error_to_mcp)?
            .ok_or_else(|| {
                invalid_params("no paper account is configured for this strategy and purpose")
            })
    }
}

fn paper_order_to_dto(
    item: core_application::paper_trade::PaperOrderWithResult,
) -> PaperTradeOrderDto {
    let (outcome, fill_date, fill_price, reject_reason) = match item.result {
        Some(PaperOrderResult::Filled {
            fill_date,
            fill_price,
            ..
        }) => (
            Some("filled".to_string()),
            Some(fill_date),
            Some(decimal_to_f64(fill_price)),
            None,
        ),
        Some(PaperOrderResult::Rejected { reject_reason, .. }) => (
            Some("rejected".to_string()),
            None,
            None,
            Some(reject_reason.as_str().to_string()),
        ),
        None => (None, None, None, None),
    };
    PaperTradeOrderDto {
        order_id: item.order.id,
        stock_id: item.order.stock_id,
        side: item.order.side.as_str().to_string(),
        qty: item.order.qty,
        note_version_id: item.order.note_version_id,
        ordered_at: item.order.ordered_at,
        outcome,
        fill_date,
        fill_price_jpy: fill_price,
        reject_reason,
    }
}

fn paper_trade_error_to_mcp(error: PaperTradeUseCaseError) -> McpError {
    match error {
        PaperTradeUseCaseError::Validation(message) => invalid_params(message),
        PaperTradeUseCaseError::AccountNotFound(_) => {
            invalid_params("paper account does not exist")
        }
        PaperTradeUseCaseError::Repository(PaperTradeRepositoryError::Database(error)) => {
            super::persistence_error_to_mcp(error)
        }
        error => {
            tracing::error!(error = %error, "strategy mcp paper trade failed");
            internal_error(format!("paper trade error: {error}"))
        }
    }
}

fn strategy_task_error_to_mcp(error: GetTaskError) -> McpError {
    tracing::error!(error = %error, "strategy mcp paper trade task lookup failed");
    internal_error(format!("strategy task lookup failed: {error}"))
}
