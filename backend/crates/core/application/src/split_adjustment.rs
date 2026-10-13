use chrono::NaiveDate;
use core_domain::bar::Bar;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SplitAdjustment {
    pub instrument_id: String,
    pub date: NaiveDate,
    pub factor: Decimal,
}

impl SplitAdjustment {
    pub(crate) fn from_bar(bar: &Bar) -> Self {
        Self {
            instrument_id: bar.instrument_id.clone(),
            date: bar.timestamp.date_naive(),
            factor: bar.adjustment_factor,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum SplitAdjustmentError {
    #[error("split adjustment factor must be positive")]
    NonPositiveFactor,
    #[error("split-adjusted quantity overflowed")]
    QuantityOverflow,
    #[error("split-adjusted price overflowed")]
    PriceOverflow,
}

pub(crate) struct SplitCursor {
    adjustments: Vec<SplitAdjustment>,
    next: usize,
}

impl SplitCursor {
    pub(crate) fn new(
        adjustments: impl IntoIterator<Item = SplitAdjustment>,
    ) -> Result<Self, SplitAdjustmentError> {
        let mut adjustments = adjustments
            .into_iter()
            .filter(|adjustment| adjustment.factor != Decimal::ONE)
            .collect::<Vec<_>>();
        if adjustments
            .iter()
            .any(|adjustment| adjustment.factor <= Decimal::ZERO)
        {
            return Err(SplitAdjustmentError::NonPositiveFactor);
        }
        adjustments.sort_by(|left, right| {
            left.date
                .cmp(&right.date)
                .then_with(|| left.instrument_id.cmp(&right.instrument_id))
        });
        Ok(Self {
            adjustments,
            next: 0,
        })
    }

    pub(crate) fn apply_through<E>(
        &mut self,
        date: NaiveDate,
        mut apply: impl FnMut(&SplitAdjustment) -> Result<(), E>,
    ) -> Result<(), E> {
        // 分割日の始値は分割後基準なので、同日の取引より先に既存 lot を補正する。
        while self
            .adjustments
            .get(self.next)
            .is_some_and(|adjustment| adjustment.date <= date)
        {
            apply(&self.adjustments[self.next])?;
            self.next += 1;
        }
        Ok(())
    }

    pub(crate) fn apply_remaining<E>(
        &mut self,
        mut apply: impl FnMut(&SplitAdjustment) -> Result<(), E>,
    ) -> Result<(), E> {
        while let Some(adjustment) = self.adjustments.get(self.next) {
            apply(adjustment)?;
            self.next += 1;
        }
        Ok(())
    }
}

pub(crate) fn adjust_quantity(
    quantity: Decimal,
    factor: Decimal,
) -> Result<Decimal, SplitAdjustmentError> {
    if factor <= Decimal::ZERO {
        return Err(SplitAdjustmentError::NonPositiveFactor);
    }
    quantity
        .checked_div(factor)
        .map(|adjusted| adjusted.round_dp(quantity.scale()))
        .ok_or(SplitAdjustmentError::QuantityOverflow)
}

pub(crate) fn adjust_whole_quantity(
    quantity: i64,
    factor: Decimal,
) -> Result<i64, SplitAdjustmentError> {
    adjust_quantity(Decimal::from(quantity), factor)?
        .to_i64()
        .ok_or(SplitAdjustmentError::QuantityOverflow)
}

pub(crate) fn adjust_lot(
    quantity: Decimal,
    price: Decimal,
    factor: Decimal,
) -> Result<(Decimal, Decimal), SplitAdjustmentError> {
    let quantity = adjust_quantity(quantity, factor)?;
    let price = price
        .checked_mul(factor)
        .ok_or(SplitAdjustmentError::PriceOverflow)?;
    Ok((quantity, price))
}

pub(crate) fn adjust_integer_lot(
    quantity: i64,
    price: Decimal,
    factor: Decimal,
) -> Result<(i64, Decimal), SplitAdjustmentError> {
    let quantity = adjust_whole_quantity(quantity, factor)?;
    let price = price
        .checked_mul(factor)
        .ok_or(SplitAdjustmentError::PriceOverflow)?;
    Ok((quantity, price))
}
