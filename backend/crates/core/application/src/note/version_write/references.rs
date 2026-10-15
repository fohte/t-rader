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
    pub(super) async fn resolve_price_references(
        &self,
        transaction: &UnitOfWorkTransaction,
        references: &[NotePriceReferenceToken],
        execution_id: Option<&str>,
    ) -> Result<Value, NoteUseCaseError> {
        if references.is_empty() {
            return Ok(Value::Object(Map::new()));
        }

        let execution_step_id = if requires_query_data(references) {
            Some(
                execution_id
                    .and_then(|value| Uuid::parse_str(value).ok())
                    .ok_or_else(|| {
                        NoteUseCaseError::Validation(
                            "価格参照の解決には実行ステップの query_data が必要です".into(),
                        )
                    })?,
            )
        } else {
            None
        };
        let mut evidence_by_instrument = BTreeMap::new();
        let mut valuations_by_symbol_date =
            BTreeMap::<(String, NaiveDate), Option<Valuation>>::new();
        let mut observations_by_id_date = BTreeMap::<(String, NaiveDate), Option<Decimal>>::new();
        let mut values = Map::new();
        for token in references {
            match &token.reference {
                NotePriceReference::Price { instrument_id, .. }
                | NotePriceReference::Change { instrument_id, .. } => {
                    let execution_step_id = execution_step_id.ok_or_else(|| {
                        NoteUseCaseError::Validation(
                            "価格参照の解決には実行ステップの query_data が必要です".into(),
                        )
                    })?;
                    if !evidence_by_instrument.contains_key(instrument_id) {
                        let evidence = self
                            .strategy_task_step_evidence
                            .find_query_data(transaction, execution_step_id, instrument_id)
                            .await
                            .map_err(StrategyTaskStepEvidenceUseCaseError::from)?;
                        evidence_by_instrument.insert(instrument_id.clone(), evidence);
                    }
                    let evidence = &evidence_by_instrument[instrument_id];
                    let (value, evidence_id) =
                        resolve_evidence_reference(&token.reference, evidence).ok_or_else(
                            || {
                                NoteUseCaseError::Validation(format!(
                                    "価格参照 {0} を実行ステップの query_data から解決できません",
                                    token.token
                                ))
                            },
                        )?;
                    values.insert(
                        token.token.clone(),
                        json!({
                            "value": value,
                            "evidence_id": evidence_id,
                        }),
                    );
                }
                NotePriceReference::Valuation {
                    symbol,
                    date,
                    field,
                } => {
                    let key = (symbol.clone(), *date);
                    if !valuations_by_symbol_date.contains_key(&key) {
                        let valuation = self
                            .valuations
                            .find_for_symbol(symbol, *date, *date)
                            .await?
                            .into_iter()
                            .find(|valuation| valuation.date == *date);
                        valuations_by_symbol_date.insert(key.clone(), valuation);
                    }
                    let valuation = valuations_by_symbol_date
                        .get(&key)
                        .and_then(Option::as_ref)
                        .ok_or_else(|| {
                            NoteUseCaseError::Validation(format!(
                                "valuation 参照 {0} を指定日のデータから解決できません",
                                token.token
                            ))
                        })?;
                    let value = valuation_field_value(valuation, *field).ok_or_else(|| {
                        NoteUseCaseError::Validation(format!(
                            "valuation 参照 {0} の項目は記録されていません",
                            token.token
                        ))
                    })?;
                    values.insert(
                        token.token.clone(),
                        json!({ "value": serde_json::to_value(value)? }),
                    );
                }
                NotePriceReference::Indicator { indicator_id, date } => {
                    let key = (indicator_id.clone(), *date);
                    if !observations_by_id_date.contains_key(&key) {
                        let observation = self
                            .indicator_observations
                            .read(IndicatorObservationQuery {
                                indicator_id: indicator_id.clone(),
                                from: Some(*date),
                                to: Some(*date),
                            })
                            .await?
                            .observations
                            .into_iter()
                            .find(|observation| observation.date == *date)
                            .map(|observation| observation.value);
                        observations_by_id_date.insert(key.clone(), observation);
                    }
                    let value = observations_by_id_date
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
                }
            }
        }

        Ok(Value::Object(values))
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
