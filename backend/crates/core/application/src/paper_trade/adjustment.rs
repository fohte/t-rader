use std::collections::HashMap;

use chrono::{NaiveDate, NaiveTime};
use core_domain::bar::Bar;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::bars::{BarsQuery, SharedBarsRepository};

use super::{PaperOrderResult, PaperOrderWithResult, PaperTradeUseCaseError};

pub(super) async fn load_split_bars_for_orders(
    bars: &SharedBarsRepository,
    orders: &[PaperOrderWithResult],
) -> Result<Vec<Bar>, PaperTradeUseCaseError> {
    let mut from_dates = HashMap::<String, NaiveDate>::new();
    for item in orders {
        let Some(PaperOrderResult::Filled { fill_date, .. }) = item.result.as_ref() else {
            continue;
        };
        from_dates
            .entry(item.order.stock_id.clone())
            .and_modify(|from| *from = (*from).min(*fill_date))
            .or_insert(*fill_date);
    }

    let mut split_bars = Vec::new();
    for (stock_id, from_date) in from_dates {
        let stock_bars = bars
            .find_bars(BarsQuery {
                instrument_id: stock_id,
                timeframe: "1d".to_string(),
                from: Some(utc_midnight(from_date)),
                to: None,
            })
            .await?;
        split_bars.extend(
            stock_bars
                .into_iter()
                .filter(|bar| bar.adjustment_factor != Decimal::ONE),
        );
    }
    split_bars.sort_by(|left, right| {
        left.timestamp
            .cmp(&right.timestamp)
            .then_with(|| left.instrument_id.cmp(&right.instrument_id))
    });
    Ok(split_bars)
}

pub(super) fn adjusted_quantity(
    quantity: i64,
    adjustment_factor: Decimal,
) -> Result<i64, PaperTradeUseCaseError> {
    if adjustment_factor <= Decimal::ZERO {
        return Err(PaperTradeUseCaseError::Validation(
            "split adjustment factor must be positive".into(),
        ));
    }
    Decimal::from(quantity)
        .checked_div(adjustment_factor)
        .map(|quantity| quantity.round_dp(0))
        .and_then(|quantity| quantity.to_i64())
        .ok_or_else(|| {
            PaperTradeUseCaseError::Validation("split-adjusted quantity overflowed".into())
        })
}

pub(super) fn utc_midnight(date: NaiveDate) -> chrono::DateTime<chrono::FixedOffset> {
    date.and_time(NaiveTime::MIN).and_utc().fixed_offset()
}
