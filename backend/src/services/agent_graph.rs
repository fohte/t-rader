use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// 戦略ごとの多段フェーズ実行設定。
///
/// 各フェーズの `output` (JSON Schema) の中身はセマンティック分類を含みうるユーザー由来の
/// 語彙なので、ここではキー/値ともに検証以上のことはせず素通しする。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentGraphConfig {
    pub phases: Vec<AgentGraphPhase>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentGraphPhase {
    pub key: String,
    pub label: String,
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub runs: Option<String>,
    #[serde(default)]
    pub for_each: Option<String>,
    #[serde(default)]
    pub label_field: Option<String>,
    #[serde(default)]
    pub max_parallel: Option<u32>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub output: serde_json::Map<String, JsonValue>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AgentGraphError {
    #[error("agent_graph is not valid YAML: {0}")]
    InvalidYaml(String),
    #[error("phase key {0:?} is duplicated")]
    DuplicatePhaseKey(String),
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
    let mut seen = BTreeSet::new();
    for (i, phase) in config.phases.iter().enumerate() {
        if !seen.insert(phase.key.as_str()) {
            return Err(AgentGraphError::DuplicatePhaseKey(phase.key.clone()));
        }
        if let Some(for_each) = &phase.for_each {
            validate_for_each(phase, for_each, &config.phases[..i])?;
        }
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
        assert_eq!(parse_agent_graph(""), Ok(None));
        assert_eq!(parse_agent_graph("   \n"), Ok(None));
    }

    #[rstest]
    fn parse_valid_graph() {
        let yaml = indoc! {"
            phases:
              - key: plan
                label: 調査計画
                model: claude-opus-4
                runs: once
                prompt: 仮説を立てよ
                output:
                  hypotheses:
                    type: array
                    description: 検証すべき仮説
                    items:
                      title: { type: string }
              - key: investigate
                label: 仮説の調査
                model: deepseek-v4-flash
                for_each: plan.hypotheses
                label_field: title
                max_parallel: 4
                prompt: 割り当てられた仮説を検証せよ
                tools: [query_data, write_note]
        "};

        assert_eq!(
            parse_agent_graph(yaml),
            Ok(Some(AgentGraphConfig {
                phases: vec![
                    AgentGraphPhase {
                        key: "plan".to_string(),
                        label: "調査計画".to_string(),
                        model: "claude-opus-4".to_string(),
                        prompt: "仮説を立てよ".to_string(),
                        runs: Some("once".to_string()),
                        for_each: None,
                        label_field: None,
                        max_parallel: None,
                        skills: vec![],
                        tools: vec![],
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
                        model: "deepseek-v4-flash".to_string(),
                        prompt: "割り当てられた仮説を検証せよ".to_string(),
                        runs: None,
                        for_each: Some("plan.hypotheses".to_string()),
                        label_field: Some("title".to_string()),
                        max_parallel: Some(4),
                        skills: vec![],
                        tools: vec!["query_data".to_string(), "write_note".to_string()],
                        output: serde_json::Map::new(),
                    },
                ],
            })),
        );
    }

    #[rstest]
    fn parse_rejects_invalid_yaml() {
        let err = parse_agent_graph("phases: [").unwrap_err();
        assert!(matches!(err, AgentGraphError::InvalidYaml(_)));
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
        let err = parse_agent_graph(yaml).unwrap_err();
        assert!(matches!(err, AgentGraphError::InvalidYaml(_)));
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
