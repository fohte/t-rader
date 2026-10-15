use std::collections::{BTreeMap, HashSet};

use chrono::NaiveDate;
use core_domain::note_price_reference::{
    NotePriceReference, NotePriceReferenceToken, ValuationReferenceField,
};
use core_domain::valuation::Valuation;
use rust_decimal::Decimal;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::indicator_observation::IndicatorObservationQuery;
use crate::note::{NoteUseCaseError, NoteUseCases};
use crate::strategy_task_step_evidence::{
    StrategyTaskStepEvidence, StrategyTaskStepEvidenceUseCaseError,
};
use crate::unit_of_work::UnitOfWorkTransaction;

pub(in crate::note) type ResolvedMarketDataReferences = Map<String, Value>;

pub(super) fn requires_query_data(references: &[NotePriceReferenceToken]) -> bool {
    references.iter().any(|reference| {
        matches!(
            &reference.reference,
            NotePriceReference::Price { .. } | NotePriceReference::Change { .. }
        )
    })
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
                    entry.get("value").is_some_and(Value::is_number)
                        && match &reference.reference {
                            NotePriceReference::Price { .. }
                            | NotePriceReference::Change { .. } => entry
                                .get("evidence_id")
                                .and_then(Value::as_str)
                                .and_then(|id| Uuid::parse_str(id).ok())
                                .is_some(),
                            NotePriceReference::Valuation { .. }
                            | NotePriceReference::Indicator { .. } => {
                                entry.len() == 1 && !entry.contains_key("evidence_id")
                            }
                        }
                })
        })
}

impl NoteUseCases {
    pub(in crate::note) async fn resolve_market_data_references(
        &self,
        body_md: &str,
    ) -> Result<ResolvedMarketDataReferences, NoteUseCaseError> {
        let references = core_domain::note_price_reference::extract_note_price_references(body_md)
            .map_err(|errors| {
                NoteUseCaseError::Validation(core_domain::note_reference::format_note_token_errors(
                    &errors,
                ))
            })?;
        let mut valuations_by_symbol_date =
            BTreeMap::<(String, NaiveDate), Option<Valuation>>::new();
        let mut observations_by_id_date = BTreeMap::<(String, NaiveDate), Option<Decimal>>::new();
        let mut values = Map::new();

        for token in &references {
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
                NotePriceReference::Price { .. } | NotePriceReference::Change { .. } => {}
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
        let value = cache.get(&key).and_then(|value| *value).ok_or_else(|| {
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

    pub(super) async fn resolve_price_references(
        &self,
        transaction: &UnitOfWorkTransaction,
        references: &[NotePriceReferenceToken],
        execution_id: Option<&str>,
        mut resolved_market_data: ResolvedMarketDataReferences,
    ) -> Result<Value, NoteUseCaseError> {
        if references.is_empty() {
            return Ok(Value::Object(Map::new()));
        }

        if requires_query_data(references) {
            let execution_step_id = execution_id
                .and_then(|value| Uuid::parse_str(value).ok())
                .ok_or_else(|| {
                    NoteUseCaseError::Validation(
                        "価格参照の解決には実行ステップの query_data が必要です".into(),
                    )
                })?;
            self.resolve_query_data_references(
                transaction,
                references,
                execution_step_id,
                &mut resolved_market_data,
            )
            .await?;
        }

        let resolved = Value::Object(resolved_market_data);
        if !has_resolutions(&resolved, references) {
            return Err(NoteUseCaseError::Validation(
                "価格参照をすべて解決できません".into(),
            ));
        }
        Ok(resolved)
    }

    async fn resolve_query_data_references(
        &self,
        transaction: &UnitOfWorkTransaction,
        references: &[NotePriceReferenceToken],
        execution_step_id: Uuid,
        values: &mut Map<String, Value>,
    ) -> Result<(), NoteUseCaseError> {
        let mut evidence_by_instrument = BTreeMap::new();
        for token in references {
            let instrument_id = match &token.reference {
                NotePriceReference::Price { instrument_id, .. }
                | NotePriceReference::Change { instrument_id, .. } => instrument_id,
                NotePriceReference::Valuation { .. } | NotePriceReference::Indicator { .. } => {
                    continue;
                }
            };
            if !evidence_by_instrument.contains_key(instrument_id) {
                let evidence = self
                    .strategy_task_step_evidence
                    .find_query_data(transaction, execution_step_id, instrument_id)
                    .await
                    .map_err(StrategyTaskStepEvidenceUseCaseError::from)?;
                evidence_by_instrument.insert(instrument_id.to_string(), evidence);
            }
            let evidence = evidence_by_instrument.get(instrument_id).ok_or_else(|| {
                NoteUseCaseError::Validation("query_data の取得結果がありません".into())
            })?;
            let (value, evidence_id) = resolve_evidence_reference(&token.reference, evidence)
                .ok_or_else(|| {
                    NoteUseCaseError::Validation(format!(
                        "価格参照 {0} を実行ステップの query_data から解決できません",
                        token.token
                    ))
                })?;
            values.insert(
                token.token.clone(),
                json!({
                    "value": value,
                    "evidence_id": evidence_id,
                }),
            );
        }
        Ok(())
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

fn resolve_evidence_reference(
    reference: &NotePriceReference,
    evidence: &[StrategyTaskStepEvidence],
) -> Option<(Value, Uuid)> {
    match reference {
        NotePriceReference::Price { date, field, .. } => {
            evidence.iter().rev().find_map(|snapshot| {
                snapshot
                    .query_data_bar_value(*date, *field)
                    .map(|value| (value, snapshot.id))
            })
        }
        NotePriceReference::Change {
            start, end, field, ..
        } => evidence.iter().rev().find_map(|snapshot| {
            let start = snapshot.query_data_bar_value(*start, *field)?.as_f64()?;
            let end = snapshot.query_data_bar_value(*end, *field)?.as_f64()?;
            if start == 0.0 {
                return None;
            }
            serde_json::Number::from_f64((end - start) / start * 100.0)
                .map(|value| (Value::Number(value), snapshot.id))
        }),
        NotePriceReference::Valuation { .. } | NotePriceReference::Indicator { .. } => None,
    }
}
