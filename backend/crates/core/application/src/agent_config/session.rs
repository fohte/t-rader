use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use super::graph::MAX_TIMEOUT_MINUTES;

const MAX_MODEL_NAME_LENGTH: usize = 128;
const MAX_INSTRUCTIONS_LENGTH: usize = 100_000;
const MAX_TOOL_NAME_LENGTH: usize = 128;
const MAX_REASONING_EFFORT_LENGTH: usize = 32;
const MAX_CONCURRENT_SUBAGENTS: u32 = 32;
const MAX_TOTAL_TOKENS: u32 = 100_000_000;
const MAX_WEB_SEARCHES: u32 = 10_000;
const MAX_TOOL_CALLS: u32 = 100_000;
const MAX_PER_TOOL_CALLS: u32 = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSessionConfig {
    pub model: String,
    #[serde(default)]
    pub timeout_minutes: Option<u32>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub web_search: Option<WebSearchMode>,
    #[serde(default)]
    pub max_concurrent_subagents: Option<u32>,
    #[serde(default)]
    pub required_tools: Vec<String>,
    #[serde(default)]
    pub max_total_tokens: Option<u32>,
    #[serde(default)]
    pub max_web_searches: Option<u32>,
    #[serde(default)]
    pub max_tool_calls: Option<u32>,
    #[serde(default)]
    pub tool_limits: BTreeMap<String, u32>,
    #[serde(default)]
    pub tool_models: BTreeMap<String, String>,
    #[serde(default)]
    pub tool_reasoning_efforts: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WebSearchMode {
    Live,
    Cached,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AgentSessionError {
    #[error("agent_session is not valid YAML: {0}")]
    InvalidYaml(String),
    #[error("{field} must not be empty")]
    EmptyValue { field: String },
    #[error("{field} must not exceed {max} characters")]
    ValueTooLong { field: String, max: usize },
    #[error("{field} must be between {min} and {max}, got {value}")]
    ValueOutOfRange {
        field: String,
        min: u32,
        max: u32,
        value: u32,
    },
    #[error("{field} contains duplicate value {value:?}")]
    DuplicateValue { field: String, value: String },
    #[error("required tool {0:?} is not included in tools")]
    RequiredToolNotDeclared(String),
}

/// 空文字列は未設定として扱い、それ以外は YAML の形式と各項目を検証する。
pub fn parse_agent_session(yaml: &str) -> Result<Option<AgentSessionConfig>, AgentSessionError> {
    if yaml.trim().is_empty() {
        return Ok(None);
    }
    let config: AgentSessionConfig = serde_yaml_ng::from_str(yaml)
        .map_err(|error| AgentSessionError::InvalidYaml(error.to_string()))?;
    validate(&config)?;
    Ok(Some(config))
}

fn validate(config: &AgentSessionConfig) -> Result<(), AgentSessionError> {
    validate_text("model", &config.model, MAX_MODEL_NAME_LENGTH)?;
    if let Some(effort) = &config.reasoning_effort {
        validate_text("reasoning_effort", effort, MAX_REASONING_EFFORT_LENGTH)?;
    }
    if let Some(instructions) = &config.instructions
        && instructions.len() > MAX_INSTRUCTIONS_LENGTH
    {
        return Err(AgentSessionError::ValueTooLong {
            field: "instructions".to_string(),
            max: MAX_INSTRUCTIONS_LENGTH,
        });
    }
    if let Some(timeout) = config.timeout_minutes {
        validate_range("timeout_minutes", timeout, 1, MAX_TIMEOUT_MINUTES)?;
    }
    if let Some(max_concurrent_subagents) = config.max_concurrent_subagents {
        validate_range(
            "max_concurrent_subagents",
            max_concurrent_subagents,
            1,
            MAX_CONCURRENT_SUBAGENTS,
        )?;
    }
    if let Some(max_total_tokens) = config.max_total_tokens {
        validate_range("max_total_tokens", max_total_tokens, 1, MAX_TOTAL_TOKENS)?;
    }
    if let Some(max_web_searches) = config.max_web_searches {
        validate_range("max_web_searches", max_web_searches, 0, MAX_WEB_SEARCHES)?;
    }
    if let Some(max_tool_calls) = config.max_tool_calls {
        validate_range("max_tool_calls", max_tool_calls, 1, MAX_TOOL_CALLS)?;
    }

    validate_unique_tools("tools", &config.tools)?;
    validate_unique_tools("required_tools", &config.required_tools)?;
    let declared_tools: BTreeSet<&str> = config.tools.iter().map(String::as_str).collect();
    if let Some(tool) = config
        .required_tools
        .iter()
        .find(|required| !declared_tools.contains(required.as_str()))
    {
        return Err(AgentSessionError::RequiredToolNotDeclared(tool.clone()));
    }
    for (tool, limit) in &config.tool_limits {
        validate_text("tool_limits key", tool, MAX_TOOL_NAME_LENGTH)?;
        validate_range("tool_limits value", *limit, 0, MAX_PER_TOOL_CALLS)?;
    }
    for (tool, model) in &config.tool_models {
        validate_text("tool_models key", tool, MAX_TOOL_NAME_LENGTH)?;
        validate_text("tool_models value", model, MAX_MODEL_NAME_LENGTH)?;
    }
    for (tool, effort) in &config.tool_reasoning_efforts {
        validate_text("tool_reasoning_efforts key", tool, MAX_TOOL_NAME_LENGTH)?;
        validate_text(
            "tool_reasoning_efforts value",
            effort,
            MAX_REASONING_EFFORT_LENGTH,
        )?;
    }
    Ok(())
}

fn validate_unique_tools(field: &str, tools: &[String]) -> Result<(), AgentSessionError> {
    let mut seen = BTreeSet::new();
    for tool in tools {
        validate_text(field, tool, MAX_TOOL_NAME_LENGTH)?;
        if !seen.insert(tool) {
            return Err(AgentSessionError::DuplicateValue {
                field: field.to_string(),
                value: tool.clone(),
            });
        }
    }
    Ok(())
}

fn validate_text(field: &str, value: &str, max: usize) -> Result<(), AgentSessionError> {
    if value.trim().is_empty() {
        return Err(AgentSessionError::EmptyValue {
            field: field.to_string(),
        });
    }
    if value.len() > max {
        return Err(AgentSessionError::ValueTooLong {
            field: field.to_string(),
            max,
        });
    }
    Ok(())
}

fn validate_range(field: &str, value: u32, min: u32, max: u32) -> Result<(), AgentSessionError> {
    if !(min..=max).contains(&value) {
        return Err(AgentSessionError::ValueOutOfRange {
            field: field.to_string(),
            min,
            max,
            value,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{AgentSessionError, parse_agent_session};

    #[rstest]
    #[case::empty("")]
    #[case::whitespace(" \n")]
    fn parse_empty_session_as_unset(#[case] yaml: &str) {
        assert_eq!(parse_agent_session(yaml), Ok(None));
    }

    #[rstest]
    #[case::unknown_field(
        "model: demo-model\nphases: []",
        "agent_session is not valid YAML: unknown field `phases`, expected one of `model`, `timeout_minutes`, `reasoning_effort`, `instructions`, `tools`, `web_search`, `max_concurrent_subagents`, `required_tools`, `max_total_tokens`, `max_web_searches`, `max_tool_calls`, `tool_limits`, `tool_models`, `tool_reasoning_efforts` at line 2 column 1"
    )]
    #[case::timeout_too_high(
        "model: demo-model\ntimeout_minutes: 361",
        "timeout_minutes must be between 1 and 360, got 361"
    )]
    #[case::unsupported_search(
        "model: demo-model\nweb_search: offline",
        "agent_session is not valid YAML: web_search: unknown variant `offline`, expected one of `live`, `cached`, `disabled` at line 2 column 13"
    )]
    fn rejects_invalid_session(#[case] yaml: &str, #[case] message: &str) {
        assert_eq!(
            parse_agent_session(yaml),
            Err(if message.starts_with("agent_session") {
                AgentSessionError::InvalidYaml(
                    message
                        .trim_start_matches("agent_session is not valid YAML: ")
                        .to_string(),
                )
            } else {
                AgentSessionError::ValueOutOfRange {
                    field: "timeout_minutes".to_string(),
                    min: 1,
                    max: 360,
                    value: 361,
                }
            })
        );
    }

    #[test]
    fn rejects_required_tool_not_in_tools() {
        assert_eq!(
            parse_agent_session(indoc::indoc! {"
                    model: demo-model
                    tools: [read_demo]
                    required_tools: [write_demo]
                "}),
            Err(AgentSessionError::RequiredToolNotDeclared(
                "write_demo".to_string()
            )),
        );
    }

    #[rstest]
    #[case::tools(
        indoc::indoc! {"
            model: demo-model
            tools: [read_demo, read_demo]
        "},
        "tools"
    )]
    #[case::required_tools(
        indoc::indoc! {"
            model: demo-model
            tools: [read_demo]
            required_tools: [read_demo, read_demo]
        "},
        "required_tools"
    )]
    fn rejects_duplicate_tools(#[case] yaml: &str, #[case] field: &str) {
        assert_eq!(
            parse_agent_session(yaml),
            Err(AgentSessionError::DuplicateValue {
                field: field.to_string(),
                value: "read_demo".to_string(),
            }),
        );
    }

    #[rstest]
    #[case::zero_timeout("model: demo-model\ntimeout_minutes: 0", "timeout_minutes", 1, 360, 0)]
    #[case::too_many_subagents(
        "model: demo-model\nmax_concurrent_subagents: 33",
        "max_concurrent_subagents",
        1,
        32,
        33
    )]
    #[case::too_many_tokens(
        "model: demo-model\nmax_total_tokens: 100000001",
        "max_total_tokens",
        1,
        100000000,
        100000001
    )]
    #[case::too_many_searches(
        "model: demo-model\nmax_web_searches: 10001",
        "max_web_searches",
        0,
        10000,
        10001
    )]
    #[case::too_many_tool_calls(
        "model: demo-model\nmax_tool_calls: 100001",
        "max_tool_calls",
        1,
        100000,
        100001
    )]
    #[case::too_many_calls_per_tool(
        "model: demo-model\ntool_limits: { demo_read_tool: 100001 }",
        "tool_limits value",
        0,
        100000,
        100001
    )]
    fn rejects_out_of_range_limits(
        #[case] yaml: &str,
        #[case] field: &str,
        #[case] min: u32,
        #[case] max: u32,
        #[case] value: u32,
    ) {
        assert_eq!(
            parse_agent_session(yaml),
            Err(AgentSessionError::ValueOutOfRange {
                field: field.to_string(),
                min,
                max,
                value,
            }),
        );
    }
}
