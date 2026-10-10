use std::collections::HashMap;

use chrono::{DateTime, FixedOffset, NaiveDate};
use core_domain::bar::Bar;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::bars::BarsQuery;

use super::{
    PaperOrder, PaperOrderRejectReason, PaperOrderResult, PaperOrderSide, PaperTradeFillStats,
    PaperTradeUseCaseError, PaperTradeUseCases,
    adjustment::{SplitCursor, adjusted_quantity, utc_midnight},
};

#[derive(Clone)]
struct FillCandidate {
    order: PaperOrder,
    fill_date: NaiveDate,
    fill_price: Decimal,
}

enum FillEvent {
    Recorded {
        order: PaperOrder,
        fill_date: NaiveDate,
        fill_price: Decimal,
    },
    Candidate(FillCandidate),
}

impl FillEvent {
    fn order(&self) -> &PaperOrder {
        match self {
            Self::Recorded { order, .. } => order,
            Self::Candidate(candidate) => &candidate.order,
        }
    }

    fn fill_date(&self) -> NaiveDate {
        match self {
            Self::Candidate(candidate) => candidate.fill_date,
            Self::Recorded { fill_date, .. } => *fill_date,
        }
    }
}

#[derive(Default)]
struct Position {
    cash: Decimal,
    shares: HashMap<String, i64>,
}

impl Position {
    fn new(initial_cash_jpy: Decimal) -> Self {
        Self {
            cash: initial_cash_jpy,
            shares: HashMap::new(),
        }
    }

    fn apply_recorded_fill(
        &mut self,
        order: &PaperOrder,
        fill_price: Decimal,
    ) -> Result<(), PaperTradeUseCaseError> {
        let cost = order_cost(order, fill_price)?;
        self.apply_fill(order, cost)
    }

    fn apply_fill(
        &mut self,
        order: &PaperOrder,
        cost: Decimal,
    ) -> Result<(), PaperTradeUseCaseError> {
        match order.side {
            PaperOrderSide::Buy => {
                self.cash = checked_sub(self.cash, cost)?;
                self.add_shares(&order.stock_id, order.qty)?;
            }
            PaperOrderSide::Sell => {
                self.cash = checked_add(self.cash, cost)?;
                self.add_shares(&order.stock_id, -order.qty)?;
            }
        }
        Ok(())
    }

    fn apply_split(
        &mut self,
        stock_id: &str,
        adjustment_factor: Decimal,
    ) -> Result<(), PaperTradeUseCaseError> {
        if let Some(shares) = self.shares.get_mut(stock_id) {
            *shares = adjusted_quantity(*shares, adjustment_factor)?;
        }
        Ok(())
    }

    fn decide(
        &mut self,
        candidate: &FillCandidate,
        decided_at: DateTime<FixedOffset>,
    ) -> Result<PaperOrderResult, PaperTradeUseCaseError> {
        let order = &candidate.order;
        let cost = order_cost(order, candidate.fill_price)?;
        let rejection = match order.side {
            PaperOrderSide::Buy if self.cash < cost => {
                Some(PaperOrderRejectReason::InsufficientCash)
            }
            PaperOrderSide::Sell
                if self
                    .shares
                    .get(&order.stock_id)
                    .copied()
                    .unwrap_or_default()
                    < order.qty =>
            {
                Some(PaperOrderRejectReason::InsufficientShares)
            }
            _ => None,
        };
        if let Some(reject_reason) = rejection {
            return Ok(PaperOrderResult::Rejected {
                order_id: order.id,
                reject_reason,
                decided_at,
            });
        }

        self.apply_fill(order, cost)?;

        Ok(PaperOrderResult::Filled {
            order_id: order.id,
            fill_date: candidate.fill_date,
            fill_price: candidate.fill_price,
            decided_at,
        })
    }

    fn add_shares(&mut self, stock_id: &str, quantity: i64) -> Result<(), PaperTradeUseCaseError> {
        let shares = self.shares.entry(stock_id.to_string()).or_default();
        *shares = shares.checked_add(quantity).ok_or_else(|| {
            PaperTradeUseCaseError::Validation("paper position quantity overflowed".into())
        })?;
        Ok(())
    }
}

impl PaperTradeUseCases {
    pub async fn fill_pending_orders(
        &self,
        decided_at: DateTime<FixedOffset>,
    ) -> Result<PaperTradeFillStats, PaperTradeUseCaseError> {
        let as_of_date = japan_date(decided_at)?;
        let transaction = self.unit_of_work.begin().await?;
        let accounts = self
            .repository
            .lock_accounts_for_filling(&transaction)
            .await?;
        let orders = self
            .repository
            .list_orders_for_filling(&transaction)
            .await?;
        let pending = orders
            .iter()
            .filter(|item| item.result.is_none())
            .collect::<Vec<_>>();
        let order_dates = pending
            .iter()
            .map(|item| japan_date(item.order.ordered_at))
            .collect::<Result<Vec<_>, _>>()?;
        let Some(first_order_date) = order_dates.iter().min().copied() else {
            self.unit_of_work.commit(transaction).await?;
            return Ok(PaperTradeFillStats::default());
        };
        let ingested_dates = self
            .bars
            .find_ingested_dates(first_order_date)
            .await?
            .into_iter()
            .filter(|date| *date <= as_of_date)
            .collect::<Vec<_>>();
        let bars_by_stock = self
            .load_bars_by_stock(&pending, &orders, as_of_date)
            .await?;

        let mut decisions = HashMap::<Uuid, Result<FillCandidate, PaperOrderRejectReason>>::new();
        for (item, order_date) in pending.iter().zip(order_dates.iter().copied()) {
            if let Some(decision) = decide_fill(
                &item.order,
                order_date,
                as_of_date,
                &ingested_dates,
                bars_by_stock
                    .get(&item.order.stock_id)
                    .map_or(&[], Vec::as_slice),
            ) {
                decisions.insert(item.order.id, decision);
            }
        }

        let results_to_insert =
            plan_results(&accounts, &orders, &decisions, &bars_by_stock, decided_at)?;

        let mut stats = PaperTradeFillStats::default();
        for result in results_to_insert {
            match result {
                PaperOrderResult::Filled { .. } => stats.filled += 1,
                PaperOrderResult::Rejected { .. } => stats.rejected += 1,
            }
            self.repository.insert_result(&transaction, result).await?;
        }
        self.unit_of_work.commit(transaction).await?;
        Ok(stats)
    }

    async fn load_bars_by_stock(
        &self,
        pending: &[&super::PaperOrderWithResult],
        orders: &[super::PaperOrderWithResult],
        as_of_date: NaiveDate,
    ) -> Result<HashMap<String, Vec<Bar>>, PaperTradeUseCaseError> {
        let pending_stock_ids = pending
            .iter()
            .map(|item| item.order.stock_id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let mut earliest_order_date_by_stock = HashMap::<String, NaiveDate>::new();
        for item in orders
            .iter()
            .filter(|item| pending_stock_ids.contains(item.order.stock_id.as_str()))
        {
            let date = japan_date(item.order.ordered_at)?;
            earliest_order_date_by_stock
                .entry(item.order.stock_id.clone())
                .and_modify(|earliest| *earliest = (*earliest).min(date))
                .or_insert(date);
        }
        let mut bars_by_stock = HashMap::<String, Vec<Bar>>::new();
        for (stock_id, from_date) in earliest_order_date_by_stock {
            if from_date > as_of_date {
                bars_by_stock.insert(stock_id, Vec::new());
                continue;
            }
            let bars = self
                .bars
                .find_bars(BarsQuery {
                    instrument_id: stock_id.clone(),
                    timeframe: "1d".to_string(),
                    from: Some(utc_midnight(from_date)),
                    to: Some(utc_midnight(as_of_date)),
                })
                .await?;
            bars_by_stock.insert(stock_id, bars);
        }
        Ok(bars_by_stock)
    }
}

fn decide_fill(
    order: &PaperOrder,
    order_date: NaiveDate,
    as_of_date: NaiveDate,
    ingested_dates: &[NaiveDate],
    bars: &[Bar],
) -> Option<Result<FillCandidate, PaperOrderRejectReason>> {
    if order_date >= as_of_date {
        return None;
    }

    let mut sessions = ingested_dates
        .iter()
        .copied()
        .filter(|date| *date > order_date)
        .collect::<Vec<_>>();
    sessions.sort_unstable();
    let expiry_date = sessions.get(4).copied();
    let first_bar = bars
        .iter()
        .filter(|bar| bar.timestamp.date_naive() > order_date)
        .min_by_key(|bar| bar.timestamp.date_naive());

    match (first_bar, expiry_date) {
        (Some(bar), Some(expiry_date)) if bar.timestamp.date_naive() > expiry_date => {
            Some(Err(PaperOrderRejectReason::DailyBarUnavailable))
        }
        (Some(bar), _) => Some(Ok(FillCandidate {
            order: order.clone(),
            fill_date: bar.timestamp.date_naive(),
            fill_price: bar.open,
        })),
        (None, Some(_)) => Some(Err(PaperOrderRejectReason::DailyBarUnavailable)),
        (None, None) => None,
    }
}

fn plan_results(
    accounts: &[super::PaperAccount],
    orders: &[super::PaperOrderWithResult],
    decisions: &HashMap<Uuid, Result<FillCandidate, PaperOrderRejectReason>>,
    bars_by_stock: &HashMap<String, Vec<Bar>>,
    decided_at: DateTime<FixedOffset>,
) -> Result<Vec<PaperOrderResult>, PaperTradeUseCaseError> {
    let accounts_by_id = accounts
        .iter()
        .map(|account| (account.id, account))
        .collect::<HashMap<_, _>>();
    let mut events_by_account = HashMap::<Uuid, Vec<FillEvent>>::new();
    let mut results = Vec::new();
    for item in orders {
        match item.result.as_ref() {
            Some(PaperOrderResult::Filled {
                fill_date,
                fill_price,
                ..
            }) => events_by_account
                .entry(item.order.account_id)
                .or_default()
                .push(FillEvent::Recorded {
                    order: item.order.clone(),
                    fill_date: *fill_date,
                    fill_price: *fill_price,
                }),
            Some(PaperOrderResult::Rejected { .. }) => {}
            None => match decisions.get(&item.order.id) {
                Some(Ok(candidate)) => events_by_account
                    .entry(item.order.account_id)
                    .or_default()
                    .push(FillEvent::Candidate(candidate.clone())),
                Some(Err(reject_reason)) => results.push(PaperOrderResult::Rejected {
                    order_id: item.order.id,
                    reject_reason: *reject_reason,
                    decided_at,
                }),
                None => {}
            },
        }
    }

    for (account_id, events) in &mut events_by_account {
        let account = accounts_by_id
            .get(account_id)
            .copied()
            .ok_or(PaperTradeUseCaseError::AccountNotFound(*account_id))?;
        results.extend(replay_account(account, events, bars_by_stock, decided_at)?);
    }
    Ok(results)
}

fn replay_account(
    account: &super::PaperAccount,
    events: &mut [FillEvent],
    bars_by_stock: &HashMap<String, Vec<Bar>>,
    decided_at: DateTime<FixedOffset>,
) -> Result<Vec<PaperOrderResult>, PaperTradeUseCaseError> {
    events.sort_by(|left, right| {
        left.fill_date()
            .cmp(&right.fill_date())
            .then_with(|| left.order().ordered_at.cmp(&right.order().ordered_at))
            .then_with(|| left.order().id.cmp(&right.order().id))
    });
    let mut split_cursor = SplitCursor::new(bars_by_stock.values().flatten())?;
    let mut position = Position::new(account.initial_cash_jpy);
    let mut results = Vec::new();
    for event in events {
        let fill_date = event.fill_date();
        split_cursor.apply_through(fill_date, |bar| {
            position.apply_split(&bar.instrument_id, bar.adjustment_factor)
        })?;
        match event {
            FillEvent::Recorded {
                order, fill_price, ..
            } => position.apply_recorded_fill(order, *fill_price)?,
            FillEvent::Candidate(candidate) => {
                results.push(position.decide(candidate, decided_at)?);
            }
        }
    }
    Ok(results)
}

fn japan_date(datetime: DateTime<FixedOffset>) -> Result<NaiveDate, PaperTradeUseCaseError> {
    let offset = FixedOffset::east_opt(9 * 60 * 60).ok_or_else(|| {
        PaperTradeUseCaseError::Validation("Japan timezone offset is invalid".into())
    })?;
    Ok(datetime.with_timezone(&offset).date_naive())
}

fn order_cost(order: &PaperOrder, fill_price: Decimal) -> Result<Decimal, PaperTradeUseCaseError> {
    fill_price
        .checked_mul(Decimal::from(order.qty))
        .ok_or_else(|| PaperTradeUseCaseError::Validation("paper order value overflowed".into()))
}

fn checked_add(left: Decimal, right: Decimal) -> Result<Decimal, PaperTradeUseCaseError> {
    left.checked_add(right)
        .ok_or_else(|| PaperTradeUseCaseError::Validation("paper cash balance overflowed".into()))
}

fn checked_sub(left: Decimal, right: Decimal) -> Result<Decimal, PaperTradeUseCaseError> {
    left.checked_sub(right)
        .ok_or_else(|| PaperTradeUseCaseError::Validation("paper cash balance overflowed".into()))
}

#[cfg(all(test, feature = "test-support"))]
mod tests;
