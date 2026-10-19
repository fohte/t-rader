use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use core_domain::bar::{Bar, Timeframe};
use core_domain::note_price_reference::{
    NotePriceReference, NotePriceReferenceToken, PriceReferenceField, ValuationReferenceField,
};
use core_domain::valuation::Valuation;
use rust_decimal::Decimal;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::bars::BarsQuery;
use crate::custom_indicator_evaluation::CustomIndicatorEvaluation;
use crate::indicator_observation::IndicatorObservationQuery;
use crate::note::{NoteUseCaseError, NoteUseCases};

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
    pub(in crate::note) async fn resolve_market_data_references(
        &self,
        body_md: &str,
        allow_indicator_changes: bool,
    ) -> Result<Map<String, Value>, NoteUseCaseError> {
        let references = if allow_indicator_changes {
            core_domain::note_price_reference::extract_note_price_references_with_indicator_changes(
                body_md,
            )
            .map_err(|errors| {
                NoteUseCaseError::Validation(
                    core_domain::note_reference::format_note_token_errors_with_indicator_changes(
                        &errors,
                    ),
                )
            })?
        } else {
            core_domain::note_price_reference::extract_note_price_references(body_md).map_err(
                |errors| {
                    NoteUseCaseError::Validation(
                        core_domain::note_reference::format_note_token_errors(&errors),
                    )
                },
            )?
        };
        self.resolve_market_data_reference_tokens(&references).await
    }

    async fn resolve_market_data_reference_tokens(
        &self,
        references: &[NotePriceReferenceToken],
    ) -> Result<Map<String, Value>, NoteUseCaseError> {
        let mut valuations_by_symbol_date =
            BTreeMap::<(String, NaiveDate), Option<Valuation>>::new();
        let mut observations_by_id_date = BTreeMap::<(String, NaiveDate), Option<Decimal>>::new();
        let mut evaluations_by_id = BTreeMap::<Uuid, Option<CustomIndicatorEvaluation>>::new();
        let mut daily_bars_by_instrument_date = BTreeMap::<(String, NaiveDate), Option<Bar>>::new();
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
                        &mut valuations_by_symbol_date,
                        &mut values,
                    )
                    .await?;
                }
                NotePriceReference::Indicator { indicator_id, date } => {
                    self.resolve_indicator_reference(
                        token,
                        indicator_id,
                        *date,
                        &mut observations_by_id_date,
                        &mut values,
                    )
                    .await?;
                }
                NotePriceReference::IndicatorChange {
                    indicator_id,
                    start,
                    end,
                } => {
                    self.resolve_indicator_change_reference(
                        token,
                        indicator_id,
                        (*start, *end),
                        &mut observations_by_id_date,
                        &mut values,
                    )
                    .await?;
                }
                NotePriceReference::CustomIndicatorEvaluation {
                    evaluation_id,
                    output_name,
                } => {
                    self.resolve_custom_indicator_evaluation_reference(
                        token,
                        *evaluation_id,
                        output_name,
                        &mut evaluations_by_id,
                        &mut values,
                    )
                    .await?;
                }
                NotePriceReference::Price {
                    instrument_id,
                    date,
                    field,
                } => {
                    self.resolve_price_reference(
                        token,
                        instrument_id,
                        *date,
                        *field,
                        &mut daily_bars_by_instrument_date,
                        &mut values,
                    )
                    .await?;
                }
                NotePriceReference::Change {
                    instrument_id,
                    start,
                    end,
                    field,
                } => {
                    self.resolve_change_reference(
                        token,
                        instrument_id,
                        (*start, *end),
                        *field,
                        &mut daily_bars_by_instrument_date,
                        &mut values,
                    )
                    .await?;
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
        cache: &mut BTreeMap<(String, NaiveDate), Option<Valuation>>,
        values: &mut Map<String, Value>,
    ) -> Result<(), NoteUseCaseError> {
        let key = (symbol.to_string(), date);
        if !cache.contains_key(&key) {
            let valuation = self
                .valuations
                .find_for_symbol(symbol, date, date)
                .await?
                .into_iter()
                .find(|valuation| valuation.date == date);
            cache.insert(key.clone(), valuation);
        }
        let valuation = cache.get(&key).and_then(Option::as_ref).ok_or_else(|| {
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
        cache: &mut BTreeMap<(String, NaiveDate), Option<Decimal>>,
        values: &mut Map<String, Value>,
    ) -> Result<(), NoteUseCaseError> {
        let value = self
            .find_indicator_observation(indicator_id, date, cache)
            .await?
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

    async fn resolve_indicator_change_reference(
        &self,
        token: &NotePriceReferenceToken,
        indicator_id: &str,
        date_range: (NaiveDate, NaiveDate),
        cache: &mut BTreeMap<(String, NaiveDate), Option<Decimal>>,
        values: &mut Map<String, Value>,
    ) -> Result<(), NoteUseCaseError> {
        let (start, end) = date_range;
        let start_value = self
            .find_indicator_observation(indicator_id, start, cache)
            .await?
            .ok_or_else(|| unresolved_indicator_change_reference(token))?;
        let end_value = self
            .find_indicator_observation(indicator_id, end, cache)
            .await?
            .ok_or_else(|| unresolved_indicator_change_reference(token))?;
        if start_value == Decimal::ZERO {
            return Err(NoteUseCaseError::Validation(format!(
                "指標変化参照 {} の開始値は 0 です",
                token.token
            )));
        }
        let value = percentage_change(start_value, end_value)
            .ok_or_else(|| unresolved_indicator_change_reference(token))?;
        values.insert(
            token.token.clone(),
            json!({ "value": serde_json::to_value(value)? }),
        );
        Ok(())
    }

    async fn find_indicator_observation(
        &self,
        indicator_id: &str,
        date: NaiveDate,
        cache: &mut BTreeMap<(String, NaiveDate), Option<Decimal>>,
    ) -> Result<Option<Decimal>, NoteUseCaseError> {
        let key = (indicator_id.to_string(), date);
        if !cache.contains_key(&key) {
            let observation = self
                .indicator_observations
                .read(IndicatorObservationQuery {
                    indicator_id: indicator_id.to_string(),
                    from: Some(date),
                    to: Some(date),
                })
                .await?
                .observations
                .into_iter()
                .find(|observation| observation.date == date)
                .map(|observation| observation.value);
            cache.insert(key.clone(), observation);
        }
        Ok(cache.get(&key).and_then(|value| *value))
    }

    async fn resolve_custom_indicator_evaluation_reference(
        &self,
        token: &NotePriceReferenceToken,
        evaluation_id: Uuid,
        output_name: &str,
        cache: &mut BTreeMap<Uuid, Option<CustomIndicatorEvaluation>>,
        values: &mut Map<String, Value>,
    ) -> Result<(), NoteUseCaseError> {
        if let std::collections::btree_map::Entry::Vacant(entry) = cache.entry(evaluation_id) {
            let evaluation = self
                .custom_indicator_evaluations
                .find_by_id(evaluation_id)
                .await?;
            entry.insert(evaluation);
        }
        let evaluation = cache
            .get(&evaluation_id)
            .and_then(Option::as_ref)
            .ok_or_else(|| {
                NoteUseCaseError::Validation(format!(
                    "eval 参照 {0} の評価記録が見つかりません",
                    token.token
                ))
            })?;
        let value = evaluation
            .output
            .as_object()
            .and_then(|output| output.get(output_name))
            .ok_or_else(|| {
                NoteUseCaseError::Validation(format!(
                    "eval 参照 {0} の出力名を解決できません",
                    token.token
                ))
            })?;
        if !value.is_number() {
            return Err(NoteUseCaseError::Validation(format!(
                "eval 参照 {} の出力は数値ではありません",
                token.token
            )));
        }
        values.insert(token.token.clone(), json!({ "value": value }));
        Ok(())
    }

    async fn resolve_price_reference(
        &self,
        token: &NotePriceReferenceToken,
        instrument_id: &str,
        date: NaiveDate,
        field: PriceReferenceField,
        cache: &mut BTreeMap<(String, NaiveDate), Option<Bar>>,
        values: &mut Map<String, Value>,
    ) -> Result<(), NoteUseCaseError> {
        let bar = self
            .find_daily_bar(token, instrument_id, date, cache)
            .await?
            .ok_or_else(|| unresolved_price_reference(token))?;
        values.insert(
            token.token.clone(),
            json!({ "value": serde_json::to_value(bar_field_value(&bar, field))? }),
        );
        Ok(())
    }

    async fn resolve_change_reference(
        &self,
        token: &NotePriceReferenceToken,
        instrument_id: &str,
        date_range: (NaiveDate, NaiveDate),
        field: PriceReferenceField,
        cache: &mut BTreeMap<(String, NaiveDate), Option<Bar>>,
        values: &mut Map<String, Value>,
    ) -> Result<(), NoteUseCaseError> {
        let (start, end) = date_range;
        let start_bar = self
            .find_daily_bar(token, instrument_id, start, cache)
            .await?
            .ok_or_else(|| unresolved_price_reference(token))?;
        let end_bar = self
            .find_daily_bar(token, instrument_id, end, cache)
            .await?
            .ok_or_else(|| unresolved_price_reference(token))?;
        let start_value = bar_field_value(&start_bar, field);
        if start_value == Decimal::ZERO {
            return Err(NoteUseCaseError::Validation(format!(
                "価格参照 {} の開始値は 0 です",
                token.token
            )));
        }
        let value = percentage_change(start_value, bar_field_value(&end_bar, field))
            .ok_or_else(|| unresolved_price_reference(token))?;
        values.insert(
            token.token.clone(),
            json!({ "value": serde_json::to_value(value)? }),
        );
        Ok(())
    }

    async fn find_daily_bar(
        &self,
        token: &NotePriceReferenceToken,
        instrument_id: &str,
        date: NaiveDate,
        cache: &mut BTreeMap<(String, NaiveDate), Option<Bar>>,
    ) -> Result<Option<Bar>, NoteUseCaseError> {
        let key = (instrument_id.to_string(), date);
        if !cache.contains_key(&key) {
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
                .find_bars(BarsQuery {
                    instrument_id: instrument_id.to_string(),
                    timeframe: Timeframe::Daily.to_string(),
                    from: Some(from),
                    to: Some(to),
                })
                .await?
                .into_iter()
                .find(|bar| bar.timestamp.date_naive() == date);
            cache.insert(key.clone(), bar);
        }
        Ok(cache.get(&key).cloned().flatten())
    }
}

fn unresolved_price_reference(token: &NotePriceReferenceToken) -> NoteUseCaseError {
    NoteUseCaseError::Validation(format!(
        "価格参照 {} を指定日の bars から解決できません",
        token.token
    ))
}

fn unresolved_indicator_change_reference(token: &NotePriceReferenceToken) -> NoteUseCaseError {
    NoteUseCaseError::Validation(format!(
        "指標変化参照 {} を指定日の観測値から解決できません",
        token.token
    ))
}

fn percentage_change(start_value: Decimal, end_value: Decimal) -> Option<Decimal> {
    end_value
        .checked_sub(start_value)
        .and_then(|difference| difference.checked_div(start_value))
        .and_then(|ratio| ratio.checked_mul(Decimal::from(100)))
        .map(|value| value.normalize())
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
