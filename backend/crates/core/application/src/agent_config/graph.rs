use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::strategy_execution::MAX_PAYLOAD_PAGE_ITEMS;

/// 戦略ごとの多段フェーズ実行設定。
///
/// 各フェーズの `output` (JSON Schema) の中身はセマンティック分類を含みうるユーザー由来の
/// 語彙なので、ここではキー/値ともに検証以上のことはせず素通しする。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentGraphConfig {
    #[serde(default)]
    pub timeout_minutes: Option<u32>,
    #[serde(default)]
    pub execution_limits: Option<AgentGraphExecutionLimits>,
    #[serde(default)]
    pub tool_limits: BTreeMap<String, u32>,
    #[serde(default)]
    pub tool_models: BTreeMap<String, String>,
    pub phases: Vec<AgentGraphPhase>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentGraphExecutionLimits {
    #[serde(default)]
    pub max_model_calls: Option<u32>,
    #[serde(default)]
    pub max_tool_calls: Option<u32>,
    #[serde(default)]
    pub max_subagent_runs: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentGraphPhase {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub kind: AgentGraphPhaseKind,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub timeout_minutes: Option<u32>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub operation: Option<String>,
    #[serde(default)]
    pub code_id: Option<String>,
    #[serde(default)]
    pub for_each: Option<String>,
    #[serde(default)]
    pub label_field: Option<String>,
    #[serde(default)]
    pub max_parallel: Option<u32>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub subagents: Vec<String>,
    #[serde(default)]
    pub max_subagent_runs: Option<u32>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub input: BTreeMap<String, InputBinding>,
    #[serde(default)]
    pub output: serde_json::Map<String, JsonValue>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentGraphPhaseKind {
    #[default]
    Llm,
    Operation,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum InputBinding {
    Literal(LiteralInputBinding),
    PriorOutput(PriorOutputInputBinding),
    Dataset(DatasetInputBinding),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiteralInputBinding {
    pub literal: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PriorOutputInputBinding {
    pub prior_output: PriorOutputBinding,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DatasetInputBinding {
    pub dataset: DatasetBinding,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PriorOutputBinding {
    pub phase: String,
    pub pointer: String,
    #[serde(default)]
    pub cursor: Option<u64>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DatasetBinding {
    pub provider: String,
    #[serde(default)]
    pub params: BTreeMap<String, JsonValue>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AgentGraphError {
    #[error("agent_graph is not valid YAML: {0}")]
    InvalidYaml(String),
    #[error("phase key {0:?} is duplicated")]
    DuplicatePhaseKey(String),
    #[error("phase {phase:?}: max_subagent_runs must be set when subagents are configured")]
    MissingMaxSubagentRuns { phase: String },
    #[error("phase {phase:?}: max_parallel must be greater than zero")]
    InvalidMaxParallel { phase: String },
    #[error("phase {phase:?}: model and prompt are required for llm phases")]
    MissingLlmFields { phase: String },
    #[error("phase {phase:?}: operation must be `evaluate_saved_code` with a code_id")]
    InvalidOperation { phase: String },
    #[error("phase {phase:?}: subagents are only supported by llm phases")]
    OperationHasSubagents { phase: String },
    #[error("phase {phase:?}: for_each is only supported by llm phases")]
    OperationHasForEach { phase: String },
    #[error("phase {phase:?}: input binding references unknown phase {referenced:?}")]
    InputUnknownPhase { phase: String, referenced: String },
    #[error("phase {phase:?}: prior_output must reference an earlier operation phase")]
    PriorOutputRequiresOperation { phase: String },
    #[error("phase {phase:?}: dataset bindings are only supported by operation phases")]
    DatasetRequiresOperation { phase: String },
    #[error("phase {phase:?}: dataset provider must not be empty")]
    EmptyDatasetProvider { phase: String },
    #[error("phase {phase:?}: prior_output limit must be greater than zero")]
    InvalidPageLimit { phase: String },
    #[error("phase {phase:?}: prior_output pointer must be a JSON Pointer")]
    InvalidJsonPointer { phase: String },
    #[error(
        "{scope}: timeout_minutes must be between 1 and {max}, got {value}",
        max = MAX_TIMEOUT_MINUTES
    )]
    TimeoutMinutesOutOfRange { scope: String, value: u32 },
    #[error("phase {phase:?}: for_each must be in the form \"<phase_key>.<field>\", got {value:?}")]
    InvalidForEachFormat { phase: String, value: String },
    #[error(
        "phase {phase:?}: for_each references unknown phase {referenced:?} (must be an earlier phase)"
    )]
    ForEachUnknownPhase { phase: String, referenced: String },
    #[error(
        "phase {phase:?}: for_each references field {field:?} which is not defined in phase {referenced:?}'s output"
    )]
    ForEachUnknownField {
        phase: String,
        referenced: String,
        field: String,
    },
    #[error(
        "phase {phase:?}: for_each references field {field:?} in phase {referenced:?}, which is not an array"
    )]
    ForEachNotArray {
        phase: String,
        referenced: String,
        field: String,
    },
}

pub const MAX_TIMEOUT_MINUTES: u32 = 360;

/// `agent_graph` の YAML テキストをパース・検証する。
///
/// 空文字列 (未設定) は `Ok(None)` を返す。
pub fn parse_agent_graph(yaml: &str) -> Result<Option<AgentGraphConfig>, AgentGraphError> {
    if yaml.trim().is_empty() {
        return Ok(None);
    }
    let config: AgentGraphConfig =
        serde_yaml_ng::from_str(yaml).map_err(|e| AgentGraphError::InvalidYaml(e.to_string()))?;
    validate(&config)?;
    Ok(Some(config))
}

fn validate(config: &AgentGraphConfig) -> Result<(), AgentGraphError> {
    if let Some(timeout_minutes) = config.timeout_minutes {
        validate_timeout_minutes("agent_graph", timeout_minutes)?;
    }
    let mut seen = BTreeSet::new();
    for (i, phase) in config.phases.iter().enumerate() {
        if !seen.insert(phase.key.as_str()) {
            return Err(AgentGraphError::DuplicatePhaseKey(phase.key.clone()));
        }
        if !phase.subagents.is_empty() && phase.max_subagent_runs.is_none() {
            return Err(AgentGraphError::MissingMaxSubagentRuns {
                phase: phase.key.clone(),
            });
        }
        if phase.max_parallel == Some(0) {
            return Err(AgentGraphError::InvalidMaxParallel {
                phase: phase.key.clone(),
            });
        }
        match phase.kind {
            AgentGraphPhaseKind::Llm => {
                if phase.model.is_none() || phase.prompt.is_none() {
                    return Err(AgentGraphError::MissingLlmFields {
                        phase: phase.key.clone(),
                    });
                }
            }
            AgentGraphPhaseKind::Operation => {
                if phase.operation.as_deref() != Some("evaluate_saved_code")
                    || phase.code_id.as_deref().is_none_or(str::is_empty)
                {
                    return Err(AgentGraphError::InvalidOperation {
                        phase: phase.key.clone(),
                    });
                }
                if phase.model.is_some()
                    || phase.prompt.is_some()
                    || phase.reasoning_effort.is_some()
                    || !phase.skills.is_empty()
                    || !phase.tools.is_empty()
                    || phase.max_subagent_runs.is_some()
                    || phase.max_parallel.is_some()
                {
                    return Err(AgentGraphError::InvalidOperation {
                        phase: phase.key.clone(),
                    });
                }
                if !phase.subagents.is_empty() {
                    return Err(AgentGraphError::OperationHasSubagents {
                        phase: phase.key.clone(),
                    });
                }
                if phase.for_each.is_some() {
                    return Err(AgentGraphError::OperationHasForEach {
                        phase: phase.key.clone(),
                    });
                }
            }
        }
        if let Some(timeout_minutes) = phase.timeout_minutes {
            validate_timeout_minutes(&format!("phase {:?}", phase.key), timeout_minutes)?;
        }
        if let Some(for_each) = &phase.for_each {
            validate_for_each(phase, for_each, &config.phases[..i])?;
        }
        validate_input_bindings(phase, &config.phases[..i])?;
    }
    Ok(())
}

fn validate_input_bindings(
    phase: &AgentGraphPhase,
    earlier_phases: &[AgentGraphPhase],
) -> Result<(), AgentGraphError> {
    for binding in phase.input.values() {
        match binding {
            InputBinding::Literal(_) => {}
            InputBinding::PriorOutput(input) => {
                let prior_output = &input.prior_output;
                let source_phase = earlier_phases
                    .iter()
                    .find(|candidate| candidate.key == prior_output.phase)
                    .ok_or_else(|| AgentGraphError::InputUnknownPhase {
                        phase: phase.key.clone(),
                        referenced: prior_output.phase.clone(),
                    })?;
                if source_phase.kind != AgentGraphPhaseKind::Operation {
                    return Err(AgentGraphError::PriorOutputRequiresOperation {
                        phase: phase.key.clone(),
                    });
                }
                if !prior_output.pointer.is_empty() && !prior_output.pointer.starts_with('/') {
                    return Err(AgentGraphError::InvalidJsonPointer {
                        phase: phase.key.clone(),
                    });
                }
                if prior_output
                    .limit
                    .is_some_and(|limit| limit == 0 || limit as usize > MAX_PAYLOAD_PAGE_ITEMS)
                {
                    return Err(AgentGraphError::InvalidPageLimit {
                        phase: phase.key.clone(),
                    });
                }
            }
            InputBinding::Dataset(input) => {
                if phase.kind != AgentGraphPhaseKind::Operation {
                    return Err(AgentGraphError::DatasetRequiresOperation {
                        phase: phase.key.clone(),
                    });
                }
                if input.dataset.provider.trim().is_empty() {
                    return Err(AgentGraphError::EmptyDatasetProvider {
                        phase: phase.key.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_timeout_minutes(scope: &str, value: u32) -> Result<(), AgentGraphError> {
    if !(1..=MAX_TIMEOUT_MINUTES).contains(&value) {
        return Err(AgentGraphError::TimeoutMinutesOutOfRange {
            scope: scope.to_string(),
            value,
        });
    }
    Ok(())
}

fn validate_for_each(
    phase: &AgentGraphPhase,
    for_each: &str,
    earlier_phases: &[AgentGraphPhase],
) -> Result<(), AgentGraphError> {
    let Some((ref_key, field)) = for_each.split_once('.') else {
        return Err(AgentGraphError::InvalidForEachFormat {
            phase: phase.key.clone(),
            value: for_each.to_string(),
        });
    };
    let referenced = earlier_phases
        .iter()
        .find(|p| p.key == ref_key)
        .ok_or_else(|| AgentGraphError::ForEachUnknownPhase {
            phase: phase.key.clone(),
            referenced: ref_key.to_string(),
        })?;
    let field_schema =
        referenced
            .output
            .get(field)
            .ok_or_else(|| AgentGraphError::ForEachUnknownField {
                phase: phase.key.clone(),
                referenced: ref_key.to_string(),
                field: field.to_string(),
            })?;
    let is_array = field_schema.get("type").and_then(JsonValue::as_str) == Some("array");
    if !is_array {
        return Err(AgentGraphError::ForEachNotArray {
            phase: phase.key.clone(),
            referenced: ref_key.to_string(),
            field: field.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use rstest::rstest;

    use super::*;

    #[rstest]
    fn parse_empty_string_is_unset() {
        assert_eq!(
            (parse_agent_graph(""), parse_agent_graph("   \n")),
            (Ok(None), Ok(None)),
        );
    }

    #[rstest]
    fn parse_valid_graph() {
        let yaml = indoc! {"
            tool_limits:
              search_web: 4
            tool_models:
              query_youtube: example-model-youtube
            phases:
              - key: plan
                label: 調査計画
                model: sample-model-plan
                reasoning_effort: high
                prompt: 仮説を立てよ
                output:
                  hypotheses:
                    type: array
                    description: 検証すべき仮説
                    items:
                      title: { type: string }
              - key: investigate
                label: 仮説の調査
                model: sample-model-investigator
                for_each: plan.hypotheses
                label_field: title
                max_parallel: 4
                subagents: [researcher, reviewer]
                max_subagent_runs: 3
                prompt: 割り当てられた仮説を検証せよ
                tools: [query_data, write_note]
        "};

        assert_eq!(
            parse_agent_graph(yaml),
            Ok(Some(AgentGraphConfig {
                timeout_minutes: None,
                execution_limits: None,
                tool_limits: BTreeMap::from([("search_web".to_string(), 4)]),
                tool_models: BTreeMap::from([(
                    "query_youtube".to_string(),
                    "example-model-youtube".to_string()
                ),]),
                phases: vec![
                    AgentGraphPhase {
                        key: "plan".to_string(),
                        label: "調査計画".to_string(),
                        kind: AgentGraphPhaseKind::Llm,
                        model: Some("sample-model-plan".to_string()),
                        timeout_minutes: None,
                        reasoning_effort: Some("high".to_string()),
                        prompt: Some("仮説を立てよ".to_string()),
                        operation: None,
                        code_id: None,
                        for_each: None,
                        label_field: None,
                        max_parallel: None,
                        skills: vec![],
                        subagents: vec![],
                        max_subagent_runs: None,
                        tools: vec![],
                        input: BTreeMap::new(),
                        output: serde_json::json!({
                            "hypotheses": {
                                "type": "array",
                                "description": "検証すべき仮説",
                                "items": { "title": { "type": "string" } },
                            },
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    },
                    AgentGraphPhase {
                        key: "investigate".to_string(),
                        label: "仮説の調査".to_string(),
                        kind: AgentGraphPhaseKind::Llm,
                        model: Some("sample-model-investigator".to_string()),
                        timeout_minutes: None,
                        reasoning_effort: None,
                        prompt: Some("割り当てられた仮説を検証せよ".to_string()),
                        operation: None,
                        code_id: None,
                        for_each: Some("plan.hypotheses".to_string()),
                        label_field: Some("title".to_string()),
                        max_parallel: Some(4),
                        skills: vec![],
                        subagents: vec!["researcher".to_string(), "reviewer".to_string()],
                        max_subagent_runs: Some(3),
                        tools: vec!["query_data".to_string(), "write_note".to_string()],
                        input: BTreeMap::new(),
                        output: serde_json::Map::new(),
                    },
                ],
            })),
        );
    }

    #[rstest]
    fn parse_graph_defaults_tool_models_when_omitted() {
        let yaml = indoc! {"
            phases:
              - key: plan
                label: Plan
                model: example-model-plan
                prompt: Make a plan
        "};

        assert_eq!(
            parse_agent_graph(yaml),
            Ok(Some(AgentGraphConfig {
                timeout_minutes: None,
                execution_limits: None,
                tool_limits: BTreeMap::new(),
                tool_models: BTreeMap::new(),
                phases: vec![AgentGraphPhase {
                    key: "plan".to_string(),
                    label: "Plan".to_string(),
                    kind: AgentGraphPhaseKind::Llm,
                    model: Some("example-model-plan".to_string()),
                    timeout_minutes: None,
                    reasoning_effort: None,
                    prompt: Some("Make a plan".to_string()),
                    operation: None,
                    code_id: None,
                    for_each: None,
                    label_field: None,
                    max_parallel: None,
                    skills: vec![],
                    subagents: vec![],
                    max_subagent_runs: None,
                    tools: vec![],
                    input: BTreeMap::new(),
                    output: serde_json::Map::new(),
                }],
            })),
        );
    }

    #[rstest]
    fn parse_task_execution_limits_without_changing_legacy_defaults() {
        let yaml = indoc! {"
            execution_limits:
              max_model_calls: 9
              max_tool_calls: 13
            phases:
              - key: plan
                label: Plan
                model: example-model-plan
                prompt: Make a plan
        "};

        assert_eq!(
            parse_agent_graph(yaml),
            Ok(Some(AgentGraphConfig {
                timeout_minutes: None,
                execution_limits: Some(AgentGraphExecutionLimits {
                    max_model_calls: Some(9),
                    max_tool_calls: Some(13),
                    max_subagent_runs: None,
                }),
                tool_limits: BTreeMap::new(),
                tool_models: BTreeMap::new(),
                phases: vec![AgentGraphPhase {
                    key: "plan".to_string(),
                    label: "Plan".to_string(),
                    kind: AgentGraphPhaseKind::Llm,
                    model: Some("example-model-plan".to_string()),
                    timeout_minutes: None,
                    reasoning_effort: None,
                    prompt: Some("Make a plan".to_string()),
                    operation: None,
                    code_id: None,
                    for_each: None,
                    label_field: None,
                    max_parallel: None,
                    skills: vec![],
                    subagents: vec![],
                    max_subagent_runs: None,
                    tools: vec![],
                    input: BTreeMap::new(),
                    output: serde_json::Map::new(),
                }],
            })),
        );
    }

    #[rstest]
    fn parse_operation_graph_and_input_bindings() {
        let yaml = indoc! {"
            phases:
              - key: prepare
                label: Prepare
                kind: operation
                operation: evaluate_saved_code
                code_id: demo-transform
                input:
                  count:
                    literal: 3
                  source:
                    dataset:
                      provider: demo-source
                      params:
                        collection: demo-set
                output:
                  items:
                    type: array
                    items:
                      type: string
              - key: summarize
                label: Summarize
                model: sample-model
                prompt: Summarize the prepared values
                input:
                  items:
                    prior_output:
                      phase: prepare
                      pointer: /items
                      cursor: 0
                      limit: 50
        "};

        assert_eq!(
            parse_agent_graph(yaml),
            Ok(Some(AgentGraphConfig {
                timeout_minutes: None,
                execution_limits: None,
                tool_limits: BTreeMap::new(),
                tool_models: BTreeMap::new(),
                phases: vec![
                    AgentGraphPhase {
                        key: "prepare".to_string(),
                        label: "Prepare".to_string(),
                        kind: AgentGraphPhaseKind::Operation,
                        model: None,
                        timeout_minutes: None,
                        reasoning_effort: None,
                        prompt: None,
                        operation: Some("evaluate_saved_code".to_string()),
                        code_id: Some("demo-transform".to_string()),
                        for_each: None,
                        label_field: None,
                        max_parallel: None,
                        skills: vec![],
                        subagents: vec![],
                        max_subagent_runs: None,
                        tools: vec![],
                        input: BTreeMap::from([
                            (
                                "count".to_string(),
                                InputBinding::Literal(LiteralInputBinding {
                                    literal: JsonValue::from(3),
                                }),
                            ),
                            (
                                "source".to_string(),
                                InputBinding::Dataset(DatasetInputBinding {
                                    dataset: DatasetBinding {
                                        provider: "demo-source".to_string(),
                                        params: BTreeMap::from([(
                                            "collection".to_string(),
                                            JsonValue::String("demo-set".to_string()),
                                        )]),
                                    },
                                }),
                            ),
                        ]),
                        output: serde_json::json!({
                            "items": {
                                "type": "array",
                                "items": { "type": "string" },
                            },
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    },
                    AgentGraphPhase {
                        key: "summarize".to_string(),
                        label: "Summarize".to_string(),
                        kind: AgentGraphPhaseKind::Llm,
                        model: Some("sample-model".to_string()),
                        timeout_minutes: None,
                        reasoning_effort: None,
                        prompt: Some("Summarize the prepared values".to_string()),
                        operation: None,
                        code_id: None,
                        for_each: None,
                        label_field: None,
                        max_parallel: None,
                        skills: vec![],
                        subagents: vec![],
                        max_subagent_runs: None,
                        tools: vec![],
                        input: BTreeMap::from([(
                            "items".to_string(),
                            InputBinding::PriorOutput(PriorOutputInputBinding {
                                prior_output: PriorOutputBinding {
                                    phase: "prepare".to_string(),
                                    pointer: "/items".to_string(),
                                    cursor: Some(0),
                                    limit: Some(50),
                                },
                            }),
                        )]),
                        output: serde_json::Map::new(),
                    },
                ],
            })),
        );
    }

    #[rstest]
    #[case::unsupported_operation("other", "demo-code", AgentGraphError::InvalidOperation { phase: "run".to_string() })]
    #[case::empty_code("evaluate_saved_code", "", AgentGraphError::InvalidOperation { phase: "run".to_string() })]
    fn parse_rejects_invalid_operation(
        #[case] operation: &str,
        #[case] code_id: &str,
        #[case] expected: AgentGraphError,
    ) {
        let yaml = format!(
            indoc! {"
                phases:
                  - key: run
                    label: Run
                    kind: operation
                    operation: {}
                    code_id: {}
            "},
            operation, code_id,
        );

        assert_eq!(parse_agent_graph(&yaml), Err(expected));
    }

    #[rstest]
    fn parse_rejects_operation_for_each_without_an_item_binding() {
        let yaml = indoc! {"
            phases:
              - key: prepare
                label: Prepare
                model: sample-model
                prompt: Prepare values
                output:
                  items:
                    type: array
              - key: run
                label: Run
                kind: operation
                operation: evaluate_saved_code
                code_id: demo-transform
                for_each: prepare.items
        "};

        assert_eq!(
            parse_agent_graph(yaml),
            Err(AgentGraphError::OperationHasForEach {
                phase: "run".to_string(),
            }),
        );
    }

    #[rstest]
    #[case::zero_max_parallel(
        "0",
        "prepare",
        "",
        AgentGraphError::InvalidMaxParallel { phase: "run".to_string() }
    )]
    #[case::forward_reference(
        "1",
        "later",
        "",
        AgentGraphError::InputUnknownPhase { phase: "run".to_string(), referenced: "later".to_string() }
    )]
    #[case::zero_page_limit(
        "1",
        "prepare",
        "limit: 0",
        AgentGraphError::InvalidPageLimit { phase: "run".to_string() }
    )]
    fn parse_rejects_invalid_operation_runtime_fields(
        #[case] max_parallel: &str,
        #[case] prior_phase: &str,
        #[case] limit: &str,
        #[case] expected: AgentGraphError,
    ) {
        let yaml = format!(
            indoc! {"
                phases:
                  - key: prepare
                    label: Prepare
                    kind: operation
                    operation: evaluate_saved_code
                    code_id: demo-transform
                  - key: run
                    label: Run
                    model: sample-model
                    prompt: Run
                    max_parallel: {}
                    input:
                      items:
                        prior_output:
                          phase: {}
                          pointer: /items
                          {}
            "},
            max_parallel, prior_phase, limit,
        );

        assert_eq!(parse_agent_graph(&yaml), Err(expected));
    }

    #[rstest]
    #[case::prior_llm_output(
        "prior_output:\n      phase: prepare\n      pointer: /items",
        AgentGraphError::PriorOutputRequiresOperation { phase: "run".to_string() }
    )]
    #[case::dataset_on_llm(
        "dataset:\n      provider: demo-source",
        AgentGraphError::DatasetRequiresOperation { phase: "run".to_string() }
    )]
    fn parse_rejects_unfixed_input_sources(
        #[case] binding: &str,
        #[case] expected: AgentGraphError,
    ) {
        let yaml = format!(
            indoc! {"
                phases:
                  - key: prepare
                    label: Prepare
                    model: sample-model
                    prompt: Prepare values
                  - key: run
                    label: Run
                    model: sample-model
                    prompt: Run
                    input:
                      value:
                        {}
            "},
            binding.replace('\n', "\n        "),
        );

        assert_eq!(parse_agent_graph(&yaml), Err(expected));
    }

    #[rstest]
    fn parse_rejects_subagents_without_max_subagent_runs() {
        let yaml = indoc! {"
            phases:
              - key: inspect
                label: Inspect
                model: example-model-check
                prompt: Inspect a question
                subagents: [helper]
        "};

        assert_eq!(
            parse_agent_graph(yaml),
            Err(AgentGraphError::MissingMaxSubagentRuns {
                phase: "inspect".to_string(),
            }),
        );
    }

    #[rstest]
    #[case::minimum(1, 360)]
    #[case::maximum(360, 1)]
    fn parse_accepts_timeout_boundaries(
        #[case] timeout_minutes: u32,
        #[case] phase_timeout_minutes: u32,
    ) {
        let yaml = format!(
            indoc! {"
                timeout_minutes: {}
                phases:
                  - key: plan
                    label: Plan
                    model: example-model-plan
                    timeout_minutes: {}
                    prompt: Make a plan
            "},
            timeout_minutes, phase_timeout_minutes,
        );

        assert_eq!(
            parse_agent_graph(&yaml),
            Ok(Some(AgentGraphConfig {
                timeout_minutes: Some(timeout_minutes),
                execution_limits: None,
                tool_limits: BTreeMap::new(),
                tool_models: BTreeMap::new(),
                phases: vec![AgentGraphPhase {
                    key: "plan".to_string(),
                    label: "Plan".to_string(),
                    kind: AgentGraphPhaseKind::Llm,
                    model: Some("example-model-plan".to_string()),
                    timeout_minutes: Some(phase_timeout_minutes),
                    reasoning_effort: None,
                    prompt: Some("Make a plan".to_string()),
                    operation: None,
                    code_id: None,
                    for_each: None,
                    label_field: None,
                    max_parallel: None,
                    skills: vec![],
                    subagents: vec![],
                    max_subagent_runs: None,
                    tools: vec![],
                    input: BTreeMap::new(),
                    output: serde_json::Map::new(),
                }],
            })),
        );
    }

    #[rstest]
    #[case::top_level_below_range("agent_graph", 0)]
    #[case::top_level_above_range("agent_graph", 361)]
    #[case::phase_below_range("phase \"plan\"", 0)]
    #[case::phase_above_range("phase \"plan\"", 361)]
    fn parse_rejects_timeout_outside_range(#[case] scope: &str, #[case] value: u32) {
        let yaml = if scope == "agent_graph" {
            format!(
                indoc! {"
                    timeout_minutes: {}
                    phases:
                      - key: plan
                        label: Plan
                        model: example-model-plan
                        prompt: Make a plan
                "},
                value,
            )
        } else {
            format!(
                indoc! {"
                    phases:
                      - key: plan
                        label: Plan
                        model: example-model-plan
                        timeout_minutes: {}
                        prompt: Make a plan
                "},
                value,
            )
        };

        assert_eq!(
            parse_agent_graph(&yaml),
            Err(AgentGraphError::TimeoutMinutesOutOfRange {
                scope: scope.to_string(),
                value,
            }),
        );
    }

    #[rstest]
    fn parse_rejects_invalid_yaml() {
        assert_eq!(
            parse_agent_graph("phases: ["),
            Err(AgentGraphError::InvalidYaml(
                "did not find expected node content at line 2 column 1, while parsing a flow node"
                    .to_string(),
            )),
        );
    }

    #[rstest]
    fn parse_rejects_unknown_field() {
        let yaml = indoc! {"
            phases:
              - key: plan
                label: l
                model: m
                prompt: p
                unknown_field: x
        "};
        assert_eq!(
            parse_agent_graph(yaml),
            Err(AgentGraphError::InvalidYaml(
                "phases[0]: unknown field `unknown_field`, expected one of `key`, `label`, `kind`, `model`, `timeout_minutes`, `reasoning_effort`, `prompt`, `operation`, `code_id`, `for_each`, `label_field`, `max_parallel`, `skills`, `subagents`, `max_subagent_runs`, `tools`, `input`, `output` at line 6 column 5".to_string(),
            )),
        );
    }

    #[rstest]
    fn parse_rejects_duplicate_phase_key() {
        let yaml = indoc! {"
            phases:
              - key: plan
                label: l1
                model: m
                prompt: p
              - key: plan
                label: l2
                model: m
                prompt: p
        "};
        assert_eq!(
            parse_agent_graph(yaml),
            Err(AgentGraphError::DuplicatePhaseKey("plan".to_string()))
        );
    }

    #[rstest]
    #[case::not_dotted(
        "plan",
        AgentGraphError::InvalidForEachFormat { phase: "investigate".to_string(), value: "plan".to_string() }
    )]
    #[case::unknown_phase(
        "missing.hypotheses",
        AgentGraphError::ForEachUnknownPhase { phase: "investigate".to_string(), referenced: "missing".to_string() }
    )]
    #[case::unknown_field(
        "plan.missing_field",
        AgentGraphError::ForEachUnknownField { phase: "investigate".to_string(), referenced: "plan".to_string(), field: "missing_field".to_string() }
    )]
    fn parse_rejects_invalid_for_each(#[case] for_each: &str, #[case] expected: AgentGraphError) {
        let yaml = format!(
            indoc! {"
                phases:
                  - key: plan
                    label: l
                    model: m
                    prompt: p
                    output:
                      hypotheses:
                        type: array
                  - key: investigate
                    label: l
                    model: m
                    prompt: p
                    for_each: {}
            "},
            for_each
        );
        assert_eq!(parse_agent_graph(&yaml), Err(expected));
    }

    #[rstest]
    fn parse_rejects_for_each_field_not_array() {
        let yaml = indoc! {"
            phases:
              - key: plan
                label: l
                model: m
                prompt: p
                output:
                  hypotheses:
                    type: string
              - key: investigate
                label: l
                model: m
                prompt: p
                for_each: plan.hypotheses
        "};
        assert_eq!(
            parse_agent_graph(yaml),
            Err(AgentGraphError::ForEachNotArray {
                phase: "investigate".to_string(),
                referenced: "plan".to_string(),
                field: "hypotheses".to_string(),
            })
        );
    }

    #[rstest]
    fn parse_rejects_for_each_referencing_later_phase() {
        let yaml = indoc! {"
            phases:
              - key: investigate
                label: l
                model: m
                prompt: p
                for_each: plan.hypotheses
              - key: plan
                label: l
                model: m
                prompt: p
                output:
                  hypotheses:
                    type: array
        "};
        assert_eq!(
            parse_agent_graph(yaml),
            Err(AgentGraphError::ForEachUnknownPhase {
                phase: "investigate".to_string(),
                referenced: "plan".to_string(),
            })
        );
    }
}
