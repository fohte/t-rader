use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use core_domain::bar::{Bar, Timeframe};
use core_domain::note_price_reference::{
    NotePriceReference, NotePriceReferenceToken, PriceReferenceField, ValuationReferenceField,
};
use core_domain::valuation::Valuation;
use rust_decimal::Decimal;
use serde_json::{Map, Value, json};

use crate::bars::BarsQuery;
use crate::indicator_observation::IndicatorObservationQuery;
use crate::note::{NoteUseCaseError, NoteUseCases};
use crate::unit_of_work::UnitOfWorkTransaction;

struct ReferenceLookup<'a> {
    transaction: &'a UnitOfWorkTransaction,
    valuations: BTreeMap<(String, NaiveDate), Option<Valuation>>,
    observations: BTreeMap<(String, NaiveDate), Option<Decimal>>,
    daily_bars: BTreeMap<(String, NaiveDate), Option<Bar>>,
}

pub(super) fn has_resolutions(resolved: &Value, references: &[NotePriceReferenceToken]) -> bool {
    let Some(resolved) = resolved.as_object() else {
        return false;
    };
    let expected_tokens: HashSet<_> = references
        .iter()
        .map(|reference| reference.token.as_str())
        .collect();
    resolved.len() == expected_tokens.len()
        && references.iter().all(|reference| {
            resolved
                .get(&reference.token)
                .and_then(Value::as_object)
                .is_some_and(|entry| {
                    entry.get("value").is_some_and(|value| !value.is_null()) && entry.len() == 1
                })
        })
}

impl NoteUseCases {
    pub(super) async fn resolve_market_data_references(
        &self,
        transaction: &UnitOfWorkTransaction,
        references: &[NotePriceReferenceToken],
    ) -> Result<Map<String, Value>, NoteUseCaseError> {
        let mut lookup = ReferenceLookup {
            transaction,
            valuations: BTreeMap::new(),
            observations: BTreeMap::new(),
            daily_bars: BTreeMap::new(),
        };
        let mut values = Map::new();

        for token in references {
            match &token.reference {
                NotePriceReference::Valuation {
                    symbol,
                    date,
                    field,
                } => {
                    self.resolve_valuation_reference(
                        token,
                        symbol,
                        *date,
                        *field,
                        &mut lookup,
                        &mut values,
                    )
                    .await?;
                }
                NotePriceReference::Indicator { indicator_id, date } => {
                    self.resolve_indicator_reference(
                        token,
                        indicator_id,
                        *date,
                        &mut lookup,
                        &mut values,
                    )
                    .await?;
                }
                NotePriceReference::Price {
                    instrument_id,
                    date,
                    field,
                } => {
                    let value = self
                        .resolve_price_reference(token, instrument_id, *date, *field, &mut lookup)
                        .await?;
                    values.insert(
                        token.token.clone(),
                        json!({ "value": serde_json::to_value(value)? }),
                    );
                }
                NotePriceReference::Change {
                    instrument_id,
                    start,
                    end,
                    field,
                } => {
                    let value = self
                        .resolve_change_reference(
                            token,
                            instrument_id,
                            *start,
                            *end,
                            *field,
                            &mut lookup,
                        )
                        .await?;
                    values.insert(
                        token.token.clone(),
                        json!({ "value": serde_json::to_value(value)? }),
                    );
                }
            }
        }

        Ok(values)
    }

    async fn resolve_valuation_reference(
        &self,
        token: &NotePriceReferenceToken,
        symbol: &str,
        date: NaiveDate,
        field: ValuationReferenceField,
        lookup: &mut ReferenceLookup<'_>,
        values: &mut Map<String, Value>,
    ) -> Result<(), NoteUseCaseError> {
        let key = (symbol.to_string(), date);
        if !lookup.valuations.contains_key(&key) {
            let valuation = self
                .valuations
                .find_for_symbol_in_transaction(lookup.transaction, symbol, date, date)
                .await?
                .into_iter()
                .find(|valuation| valuation.date == date);
            lookup.valuations.insert(key.clone(), valuation);
        }
        let valuation = lookup
            .valuations
            .get(&key)
            .and_then(Option::as_ref)
            .ok_or_else(|| {
                NoteUseCaseError::Validation(format!(
                    "valuation 参照 {0} を指定日のデータから解決できません",
                    token.token
                ))
            })?;
        let value = valuation_field_value(valuation, field).ok_or_else(|| {
            NoteUseCaseError::Validation(format!(
                "valuation 参照 {0} の項目は記録されていません",
                token.token
            ))
        })?;
        values.insert(
            token.token.clone(),
            json!({ "value": serde_json::to_value(value)? }),
        );
        Ok(())
    }

    async fn resolve_indicator_reference(
        &self,
        token: &NotePriceReferenceToken,
        indicator_id: &str,
        date: NaiveDate,
        lookup: &mut ReferenceLookup<'_>,
        values: &mut Map<String, Value>,
    ) -> Result<(), NoteUseCaseError> {
        let key = (indicator_id.to_string(), date);
        if !lookup.observations.contains_key(&key) {
            let observation = self
                .indicator_observations
                .read_in_transaction(
                    lookup.transaction,
                    IndicatorObservationQuery {
                        indicator_id: indicator_id.to_string(),
                        from: Some(date),
                        to: Some(date),
                    },
                )
                .await?
                .observations
                .into_iter()
                .find(|observation| observation.date == date)
                .map(|observation| observation.value);
            lookup.observations.insert(key.clone(), observation);
        }
        let value = lookup
            .observations
            .get(&key)
            .and_then(|value| *value)
            .ok_or_else(|| {
                NoteUseCaseError::Validation(format!(
                    "indicator 参照 {0} を指定日のデータから解決できません",
                    token.token
                ))
            })?;
        values.insert(
            token.token.clone(),
            json!({ "value": serde_json::to_value(value)? }),
        );
        Ok(())
    }

    async fn resolve_price_reference(
        &self,
        token: &NotePriceReferenceToken,
        instrument_id: &str,
        date: NaiveDate,
        field: PriceReferenceField,
        lookup: &mut ReferenceLookup<'_>,
    ) -> Result<Decimal, NoteUseCaseError> {
        let bar = self
            .find_daily_bar(token, instrument_id, date, lookup)
            .await?
            .ok_or_else(|| unresolved_price_reference(token))?;
        Ok(bar_field_value(&bar, field))
    }

    async fn resolve_change_reference(
        &self,
        token: &NotePriceReferenceToken,
        instrument_id: &str,
        start_date: NaiveDate,
        end_date: NaiveDate,
        field: PriceReferenceField,
        lookup: &mut ReferenceLookup<'_>,
    ) -> Result<Decimal, NoteUseCaseError> {
        let start_bar = self
            .find_daily_bar(token, instrument_id, start_date, lookup)
            .await?
            .ok_or_else(|| unresolved_price_reference(token))?;
        let end_bar = self
            .find_daily_bar(token, instrument_id, end_date, lookup)
            .await?
            .ok_or_else(|| unresolved_price_reference(token))?;
        let start = bar_field_value(&start_bar, field);
        if start == Decimal::ZERO {
            return Err(NoteUseCaseError::Validation(format!(
                "価格参照 {} の開始値は 0 です",
                token.token
            )));
        }
        bar_field_value(&end_bar, field)
            .checked_sub(start)
            .and_then(|difference| difference.checked_div(start))
            .and_then(|ratio| ratio.checked_mul(Decimal::from(100)))
            .map(|value| value.normalize())
            .ok_or_else(|| unresolved_price_reference(token))
    }

    async fn find_daily_bar(
        &self,
        token: &NotePriceReferenceToken,
        instrument_id: &str,
        date: NaiveDate,
        lookup: &mut ReferenceLookup<'_>,
    ) -> Result<Option<Bar>, NoteUseCaseError> {
        let key = (instrument_id.to_string(), date);
        if !lookup.daily_bars.contains_key(&key) {
            let next_date = date
                .succ_opt()
                .ok_or_else(|| unresolved_price_reference(token))?;
            let from =
                DateTime::<Utc>::from_naive_utc_and_offset(date.and_time(NaiveTime::MIN), Utc)
                    .fixed_offset();
            let to =
                DateTime::<Utc>::from_naive_utc_and_offset(next_date.and_time(NaiveTime::MIN), Utc)
                    .fixed_offset();
            let bar = self
                .bars
                .find_bars_in_transaction(
                    lookup.transaction,
                    BarsQuery {
                        instrument_id: instrument_id.to_string(),
                        timeframe: Timeframe::Daily.to_string(),
                        from: Some(from),
                        to: Some(to),
                    },
                )
                .await?
                .into_iter()
                .find(|bar| bar.timestamp.date_naive() == date);
            lookup.daily_bars.insert(key.clone(), bar);
        }
        Ok(lookup.daily_bars.get(&key).cloned().flatten())
    }
}

fn unresolved_price_reference(token: &NotePriceReferenceToken) -> NoteUseCaseError {
    NoteUseCaseError::Validation(format!(
        "価格参照 {} を指定日の bars から解決できません",
        token.token
    ))
}

fn bar_field_value(bar: &Bar, field: PriceReferenceField) -> Decimal {
    match field {
        PriceReferenceField::Open => bar.open,
        PriceReferenceField::High => bar.high,
        PriceReferenceField::Low => bar.low,
        PriceReferenceField::Close => bar.close,
        PriceReferenceField::Volume => Decimal::from(bar.volume),
    }
}

fn valuation_field_value(valuation: &Valuation, field: ValuationReferenceField) -> Option<Decimal> {
    match field {
        ValuationReferenceField::Per => valuation.per,
        ValuationReferenceField::FwdPer => valuation.fwd_per,
        ValuationReferenceField::Pbr => valuation.pbr,
        ValuationReferenceField::Roe => valuation.roe,
        ValuationReferenceField::FwdRoe => valuation.fwd_roe,
        ValuationReferenceField::MktCap => valuation.mkt_cap,
        ValuationReferenceField::Eps => valuation.eps,
        ValuationReferenceField::FwdEps => valuation.fwd_eps,
        ValuationReferenceField::Bps => valuation.bps,
    }
}
