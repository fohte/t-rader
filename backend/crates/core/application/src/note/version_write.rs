use std::collections::{BTreeMap, HashSet};

use chrono::Utc;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::change_history::{Actor, Op};
use crate::note::types::{NewNoteLink, NewNoteVersion, Note, NoteVersion, NoteVersionUpdate};
use crate::note::{INITIAL_NOTE_STATUS, NoteUseCaseError, NoteUseCases};
use crate::strategy_task_step_evidence::{
    StrategyTaskStepEvidence, StrategyTaskStepEvidenceUseCaseError,
};
use crate::unit_of_work::UnitOfWorkTransaction;

const APPROVED_NOTE_STATUS: &str = "approved";
const HUMAN_CREATED_BY_KIND: &str = "human";

pub(super) struct AppendVersionCommand {
    pub title: String,
    pub body_md: String,
    pub frontmatter_json: Value,
    pub graphs_json: Value,
    pub created_by_kind: String,
    pub execution_id: Option<String>,
    pub change_reason: Option<String>,
    pub change_diff: Option<Value>,
    pub actor: Actor,
}

impl NoteUseCases {
    pub(super) async fn append_version(
        &self,
        transaction: &UnitOfWorkTransaction,
        note: &Note,
        content: AppendVersionCommand,
    ) -> Result<NoteVersion, NoteUseCaseError> {
        let current = self
            .repository
            .find_current_version(transaction, note.id)
            .await?;
        let latest = self
            .repository
            .find_latest_version(transaction, note.id)
            .await?;
        let previous_content = current.as_ref().or(latest.as_ref());
        let requires_approval = if content.created_by_kind == HUMAN_CREATED_BY_KIND {
            false
        } else if let Some(kind_key) = note.kind.as_deref() {
            self.repository
                .find_note_kind_requires_approval(transaction, kind_key)
                .await?
                .ok_or_else(|| NoteUseCaseError::ReferencedNoteKindNotFound(kind_key.to_string()))?
        } else {
            false
        };
        let max_version_no = latest.as_ref().map_or(0, |version| version.version_no);
        if requires_approval
            && max_version_no > 0
            && content
                .change_reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
        {
            return Err(NoteUseCaseError::Validation(
                "change_reason is required for updates to this note kind".into(),
            ));
        }

        let references =
            core_domain::note_price_reference::extract_note_price_references(&content.body_md)
                .map_err(|errors| {
                    NoteUseCaseError::Validation(
                        core_domain::note_reference::format_note_token_errors(&errors),
                    )
                })?;
        let body_changed = previous_content
            .as_ref()
            .is_none_or(|previous| previous.body_md != content.body_md);
        let graphs_changed = previous_content
            .as_ref()
            .is_none_or(|previous| previous.graphs_json != content.graphs_json);
        if requires_approval && (body_changed || graphs_changed) {
            let _ = collect_note_references(
                &content.body_md,
                &content.graphs_json,
                core_domain::note_reference::BodyTokenPolicy::Validate,
            )?;
        }
        let resolved_price_references_json = if references.is_empty() {
            Value::Object(Map::new())
        } else if content.execution_id.is_none()
            && !body_changed
            && previous_content.is_some_and(|previous| {
                has_resolutions(&previous.resolved_price_references_json, &references)
            })
        {
            previous_content
                .map(|previous| previous.resolved_price_references_json.clone())
                .unwrap_or_else(|| Value::Object(Map::new()))
        } else {
            let stored_execution_id = content
                .execution_id
                .as_deref()
                .or_else(|| previous_content.and_then(|previous| previous.execution_id.as_deref()));
            let evidence_execution_id = if let Some(execution_id) = stored_execution_id {
                Some(execution_id.to_string())
            } else {
                self.infer_price_reference_execution_id(transaction, previous_content)
                    .await?
            };
            self.resolve_price_references(
                transaction,
                &references,
                evidence_execution_id.as_deref(),
            )
            .await?
        };

        let becomes_current = !requires_approval;
        let status = if content.created_by_kind == HUMAN_CREATED_BY_KIND {
            APPROVED_NOTE_STATUS
        } else {
            INITIAL_NOTE_STATUS
        };
        let version_no = max_version_no
            .checked_add(1)
            .ok_or_else(|| NoteUseCaseError::Validation("note version number overflow".into()))?;

        if becomes_current && let Some(previous) = current.as_ref() {
            self.repository
                .update_version(
                    transaction,
                    NoteVersionUpdate {
                        id: previous.id,
                        is_current: Some(false),
                        status: None,
                        reviewed_at: None,
                    },
                )
                .await?;
        }

        let summary = content.change_reason.clone();
        let version = self
            .repository
            .insert_version(
                transaction,
                NewNoteVersion {
                    id: Uuid::new_v4(),
                    note_id: note.id,
                    version_no,
                    title: content.title,
                    body_md: content.body_md,
                    frontmatter_json: content.frontmatter_json,
                    graphs_json: content.graphs_json,
                    resolved_price_references_json,
                    status: status.to_string(),
                    is_current: becomes_current,
                    change_reason: content.change_reason,
                    created_by_kind: content.created_by_kind,
                    execution_id: content.execution_id,
                },
            )
            .await?;

        self.repository
            .update_note_timestamp(transaction, note.id, Utc::now().fixed_offset())
            .await?;

        if body_changed {
            if becomes_current {
                self.sync_note_references(
                    transaction,
                    note.id,
                    &version.body_md,
                    &version.graphs_json,
                    core_domain::note_reference::BodyTokenPolicy::Validate,
                )
                .await?;
            }
            self.sync_note_links(transaction, &version).await?;
        } else {
            if becomes_current && graphs_changed {
                self.sync_note_references(
                    transaction,
                    note.id,
                    &version.body_md,
                    &version.graphs_json,
                    core_domain::note_reference::BodyTokenPolicy::AllowLegacyBodyTokens,
                )
                .await?;
            }
            if let Some(previous) = previous_content {
                self.repository
                    .copy_note_links(transaction, previous.id, version.id)
                    .await?;
            }
        }

        let mut diff = json!({
            "from_version_id": previous_content.map(|previous| previous.id),
            "to_version_id": version.id,
            "version_no": version.version_no,
        });
        if let Some(change_diff) = content.change_diff
            && let Some(changes) = change_diff.as_object()
        {
            for (key, value) in changes {
                diff[key] = value.clone();
            }
        }
        self.record_history(
            transaction,
            content.actor,
            note.id,
            if max_version_no > 0 {
                Op::Update
            } else {
                Op::Create
            },
            diff,
            summary,
        )
        .await?;

        Ok(version)
    }

    async fn resolve_price_references(
        &self,
        transaction: &UnitOfWorkTransaction,
        references: &[core_domain::note_price_reference::NotePriceReferenceToken],
        execution_id: Option<&str>,
    ) -> Result<Value, NoteUseCaseError> {
        if references.is_empty() {
            return Ok(Value::Object(Map::new()));
        }

        let execution_step_id = execution_id
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(|| {
                NoteUseCaseError::Validation(
                    "価格参照の解決には実行ステップの query_data が必要です".into(),
                )
            })?;
        let mut evidence_by_instrument = BTreeMap::new();
        let mut values = Map::new();
        for token in references {
            let instrument_id = reference_instrument_id(&token.reference);
            if !evidence_by_instrument.contains_key(instrument_id) {
                let evidence = self
                    .strategy_task_step_evidence
                    .find_query_data(transaction, execution_step_id, instrument_id)
                    .await
                    .map_err(StrategyTaskStepEvidenceUseCaseError::from)?;
                evidence_by_instrument.insert(instrument_id.to_string(), evidence);
            }
            let evidence = &evidence_by_instrument[instrument_id];
            let (value, evidence_id) = resolve_reference_value(&token.reference, evidence)
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

        Ok(Value::Object(values))
    }

    async fn infer_price_reference_execution_id(
        &self,
        transaction: &UnitOfWorkTransaction,
        previous_version: Option<&NoteVersion>,
    ) -> Result<Option<String>, NoteUseCaseError> {
        let evidence_id = previous_version.and_then(|version| {
            version
                .resolved_price_references_json
                .as_object()?
                .values()
                .find_map(|value| value.get("evidence_id")?.as_str()?.parse::<Uuid>().ok())
        });
        let Some(evidence_id) = evidence_id else {
            return Ok(None);
        };
        let evidence = self
            .strategy_task_step_evidence
            .find_query_data_by_evidence_id(transaction, evidence_id)
            .await
            .map_err(StrategyTaskStepEvidenceUseCaseError::from)?;
        Ok(evidence.map(|evidence| evidence.execution_step_id.to_string()))
    }

    pub(super) async fn sync_note_references(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
        body_md: &str,
        graphs_json: &Value,
        body_token_policy: core_domain::note_reference::BodyTokenPolicy,
    ) -> Result<(), NoteUseCaseError> {
        let references = collect_note_references(body_md, graphs_json, body_token_policy)?;
        self.repository
            .replace_references(transaction, note_id, references)
            .await?;
        Ok(())
    }

    async fn sync_note_links(
        &self,
        transaction: &UnitOfWorkTransaction,
        source_version: &NoteVersion,
    ) -> Result<(), NoteUseCaseError> {
        let mut target_policies = BTreeMap::new();
        let mut conflicted_targets = HashSet::new();
        for token in core_domain::note_reference::extract_note_link_tokens(&source_version.body_md)
        {
            if let Some(previous) = target_policies.insert(token.note_id, token.follows_current)
                && previous != token.follows_current
            {
                conflicted_targets.insert(token.note_id);
            }
        }

        if !conflicted_targets.is_empty() {
            return Err(NoteUseCaseError::Validation(
                "同じノートへのリンクでは固定指定と @current 指定を混在できません".into(),
            ));
        }

        let target_ids: Vec<Uuid> = target_policies
            .keys()
            .copied()
            .filter(|id| !conflicted_targets.contains(id))
            .collect();
        if target_ids.is_empty() {
            return Ok(());
        }

        let targets = self
            .repository
            .find_note_link_targets(transaction, &target_ids)
            .await?;
        let targets_by_id = targets
            .into_iter()
            .map(|target| (target.id, target))
            .collect::<BTreeMap<_, _>>();
        let mut links = Vec::with_capacity(target_ids.len());
        for target_id in target_ids {
            let follows_current = target_policies[&target_id];
            let Some(target) = targets_by_id.get(&target_id) else {
                return Err(NoteUseCaseError::Validation(format!(
                    "参照先のノート {target_id} が存在しません"
                )));
            };
            let to_version_id = if follows_current {
                None
            } else if let Some(version_id) = target.current_version_id {
                Some(version_id)
            } else {
                return Err(NoteUseCaseError::Validation(format!(
                    "参照先のノート {target_id} に現行バージョンがありません"
                )));
            };
            links.push(NewNoteLink {
                from_version_id: source_version.id,
                to_note_id: target_id,
                to_version_id,
            });
        }
        self.repository
            .insert_note_links(transaction, links)
            .await?;
        Ok(())
    }
}

fn collect_note_references(
    body_md: &str,
    graphs_json: &Value,
    body_token_policy: core_domain::note_reference::BodyTokenPolicy,
) -> Result<Vec<(String, String)>, NoteUseCaseError> {
    let graphs: Vec<core_domain::note_graph::GraphDef> =
        serde_json::from_value(graphs_json.clone()).map_err(|error| {
            NoteUseCaseError::Validation(format!("invalid graphs_json: {error}"))
        })?;
    let mut references = core_domain::note_reference::collect_note_refs_with_policy(
        body_md,
        &graphs,
        body_token_policy,
    )
    .map_err(|errors| {
        NoteUseCaseError::Validation(core_domain::note_reference::format_note_token_errors(
            &errors,
        ))
    })?;
    references.sort();
    references.dedup();
    Ok(references)
}

fn has_resolutions(
    resolved: &Value,
    references: &[core_domain::note_price_reference::NotePriceReferenceToken],
) -> bool {
    let Some(resolved) = resolved.as_object() else {
        return false;
    };
    let expected_tokens: HashSet<_> = references
        .iter()
        .map(|reference| reference.token.as_str())
        .collect();
    resolved.len() == expected_tokens.len()
        && expected_tokens.iter().all(|token| {
            resolved
                .get(*token)
                .and_then(Value::as_object)
                .is_some_and(|entry| {
                    entry.get("value").is_some_and(|value| !value.is_null())
                        && entry
                            .get("evidence_id")
                            .and_then(Value::as_str)
                            .and_then(|id| Uuid::parse_str(id).ok())
                            .is_some()
                })
        })
}

fn reference_instrument_id(
    reference: &core_domain::note_price_reference::NotePriceReference,
) -> &str {
    match reference {
        core_domain::note_price_reference::NotePriceReference::Price { instrument_id, .. }
        | core_domain::note_price_reference::NotePriceReference::Change { instrument_id, .. } => {
            instrument_id
        }
    }
}

fn resolve_reference_value(
    reference: &core_domain::note_price_reference::NotePriceReference,
    evidence: &[StrategyTaskStepEvidence],
) -> Option<(Value, Uuid)> {
    use core_domain::note_price_reference::NotePriceReference;

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
    }
}
