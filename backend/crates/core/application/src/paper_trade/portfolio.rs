use std::collections::HashMap;

use core_domain::bar::Bar;
use rust_decimal::Decimal;
use uuid::Uuid;

use super::{
    PaperTradePortfolio, PaperTradePosition, PaperTradeUseCaseError, PaperTradeUseCases,
    adjustment::load_split_bars_for_orders, ledger::replay_fills,
};

impl PaperTradeUseCases {
    pub async fn account_for_strategy_purpose(
        &self,
        strategy_id: Uuid,
        purpose: &str,
    ) -> Result<Option<super::PaperAccount>, PaperTradeUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let account = self
            .repository
            .account_for_strategy_purpose(&transaction, strategy_id, purpose)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(account)
    }

    pub async fn portfolio(
        &self,
        account_id: Uuid,
    ) -> Result<PaperTradePortfolio, PaperTradeUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let account = self
            .repository
            .find_account(&transaction, account_id)
            .await?;
        let Some(account) = account else {
            self.unit_of_work.commit(transaction).await?;
            return Err(PaperTradeUseCaseError::AccountNotFound(account_id));
        };
        let mut orders = self
            .repository
            .list_orders_with_results(&transaction, Some(account_id))
            .await?;
        self.unit_of_work.commit(transaction).await?;
        orders.sort_by(|left, right| {
            left.order
                .ordered_at
                .cmp(&right.order.ordered_at)
                .then_with(|| left.order.id.cmp(&right.order.id))
        });
        let split_bars = load_split_bars_for_orders(&self.bars, &orders).await?;
        let ledger = replay_fills(&account, &orders, &split_bars)?;
        let mut latest_bar_dates = HashMap::<String, chrono::NaiveDate>::new();
        let mut positions = Vec::new();

        for (stock_id, qty, cost_basis) in ledger.open_holdings() {
            let stock_id = stock_id.to_string();
            let bar = self
                .bars
                .find_latest_bar(&stock_id, "1d")
                .await?
                .ok_or_else(|| {
                    PaperTradeUseCaseError::LatestDailyBarUnavailable(stock_id.clone())
                })?;
            let bar_date = bar.timestamp.date_naive();
            latest_bar_dates.insert(stock_id.clone(), bar_date);
            positions.push(position(stock_id, qty, cost_basis, &bar));
        }
        positions.sort_by(|left, right| left.stock_id.cmp(&right.stock_id));

        let as_of = latest_bar_dates
            .values()
            .copied()
            .max()
            .unwrap_or(account.started_on);
        Ok(PaperTradePortfolio {
            account,
            as_of,
            cash_jpy: ledger.cash_jpy,
            positions,
            orders,
        })
    }
}

pub(super) fn position(
    stock_id: String,
    qty: i64,
    cost_basis_jpy: Decimal,
    latest_bar: &Bar,
) -> PaperTradePosition {
    let market_value_jpy = Decimal::from(qty) * latest_bar.close;
    PaperTradePosition {
        stock_id,
        qty,
        avg_cost_jpy: cost_basis_jpy / Decimal::from(qty),
        current_price_jpy: latest_bar.close,
        market_value_jpy,
        unrealized_pnl_jpy: market_value_jpy - cost_basis_jpy,
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests;
