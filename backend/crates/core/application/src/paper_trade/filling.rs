use std::collections::HashMap;

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime};
use core_domain::bar::Bar;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::bars::BarsQuery;

use super::{
    PaperOrder, PaperOrderRejectReason, PaperOrderResult, PaperOrderSide, PaperTradeFillStats,
    PaperTradeUseCaseError, PaperTradeUseCases,
};

#[derive(Clone)]
struct FillCandidate {
    order: PaperOrder,
    fill_date: NaiveDate,
    fill_price: Decimal,
}

enum FillEvent {
    Recorded(PaperOrder),
    Candidate(FillCandidate),
}

impl FillEvent {
    fn order(&self) -> &PaperOrder {
        match self {
            Self::Recorded(order) => order,
            Self::Candidate(candidate) => &candidate.order,
        }
    }

    fn fill_date(&self, records: &HashMap<Uuid, Option<PaperOrderResult>>) -> NaiveDate {
        match self {
            Self::Candidate(candidate) => candidate.fill_date,
            Self::Recorded(order) => match records.get(&order.id).and_then(Option::as_ref) {
                Some(PaperOrderResult::Filled { fill_date, .. }) => *fill_date,
                _ => order.ordered_at.date_naive(),
            },
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
        if pending.is_empty() {
            self.unit_of_work.commit(transaction).await?;
            return Ok(PaperTradeFillStats::default());
        }

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

        let mut earliest_order_date_by_stock = HashMap::<String, NaiveDate>::new();
        for (item, date) in pending.iter().zip(order_dates.iter().copied()) {
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

        let accounts_by_id = accounts
            .into_iter()
            .map(|account| (account.id, account))
            .collect::<HashMap<_, _>>();
        let records_by_id = orders
            .iter()
            .map(|item| (item.order.id, item.result.clone()))
            .collect::<HashMap<_, _>>();
        let mut events_by_account = HashMap::<Uuid, Vec<FillEvent>>::new();
        for item in &orders {
            if matches!(&item.result, Some(PaperOrderResult::Filled { .. })) {
                events_by_account
                    .entry(item.order.account_id)
                    .or_default()
                    .push(FillEvent::Recorded(item.order.clone()));
            } else if let Some(Ok(candidate)) = decisions.get(&item.order.id) {
                events_by_account
                    .entry(item.order.account_id)
                    .or_default()
                    .push(FillEvent::Candidate(candidate.clone()));
            }
        }

        let mut results_to_insert = Vec::new();
        for item in &orders {
            if item.result.is_some() {
                continue;
            }
            if let Some(Err(reject_reason)) = decisions.get(&item.order.id) {
                results_to_insert.push(PaperOrderResult::Rejected {
                    order_id: item.order.id,
                    reject_reason: *reject_reason,
                    decided_at,
                });
            }
        }

        for (account_id, events) in &mut events_by_account {
            let account = accounts_by_id
                .get(account_id)
                .ok_or(PaperTradeUseCaseError::AccountNotFound(*account_id))?;
            results_to_insert.extend(replay_account(account, events, &records_by_id, decided_at)?);
        }

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

fn replay_account(
    account: &super::PaperAccount,
    events: &mut [FillEvent],
    records: &HashMap<Uuid, Option<PaperOrderResult>>,
    decided_at: DateTime<FixedOffset>,
) -> Result<Vec<PaperOrderResult>, PaperTradeUseCaseError> {
    events.sort_by(|left, right| {
        left.fill_date(records)
            .cmp(&right.fill_date(records))
            .then_with(|| left.order().ordered_at.cmp(&right.order().ordered_at))
            .then_with(|| left.order().id.cmp(&right.order().id))
    });
    let mut position = Position::new(account.initial_cash_jpy);
    let mut results = Vec::new();
    for event in events {
        match event {
            FillEvent::Recorded(order) => {
                if let Some(PaperOrderResult::Filled { fill_price, .. }) =
                    records.get(&order.id).and_then(Option::as_ref)
                {
                    position.apply_recorded_fill(order, *fill_price)?;
                }
            }
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

fn utc_midnight(date: NaiveDate) -> DateTime<FixedOffset> {
    date.and_time(NaiveTime::MIN).and_utc().fixed_offset()
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
