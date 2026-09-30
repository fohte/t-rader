use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::strategy::Strategy;

pub(super) fn build_standard_context(strategy: &Strategy, now: DateTime<Utc>) -> Value {
    serde_json::json!({
        "now": now.to_rfc3339(),
        "strategy": {
            "id": strategy.id.to_string(),
            "name": strategy.name,
        },
    })
}

pub fn expand_template(template: &str, payload: &Value, context: &Value) -> String {
    let mut output = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        output.push_str(&rest[..open]);
        let after_open = &rest[open + 2..];
        match after_open.find("}}") {
            Some(close) => {
                let path = after_open[..close].trim();
                output.push_str(&resolve_path(path, payload, context));
                rest = &after_open[close + 2..];
            }
            None => {
                output.push_str("{{");
                rest = after_open;
                break;
            }
        }
    }
    output.push_str(rest);
    output
}

fn resolve_path(path: &str, payload: &Value, context: &Value) -> String {
    if path.is_empty() {
        return String::new();
    }
    let mut parts = path.split('.');
    let head = parts.next().unwrap_or("");
    let root = if head == "payload" { payload } else { context };
    let target = if head == "payload" {
        walk(root, parts)
    } else {
        walk(root, std::iter::once(head).chain(parts))
    };
    match target {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Null) | None => String::new(),
        Some(value) => value.to_string(),
    }
}

fn walk(root: &Value, parts: impl IntoIterator<Item = impl AsRef<str>>) -> Option<&Value> {
    let mut current = root;
    for part in parts {
        current = current.as_object()?.get(part.as_ref())?;
    }
    Some(current)
}

pub fn evaluate_event_match(event_match: Option<&Value>, payload: &Value) -> bool {
    let Some(spec) = event_match else {
        return true;
    };
    if spec.is_null() {
        return true;
    }
    let Some(map) = spec.as_object() else {
        return false;
    };
    for (path, condition) in map {
        let actual = walk(payload, path.split('.'));
        let Some(condition) = condition.as_object() else {
            return false;
        };
        if let Some(expected) = condition.get("eq")
            && !matches!(actual, Some(value) if value == expected)
        {
            return false;
        }
        if let Some(exists) = condition.get("exists").and_then(Value::as_bool) {
            let has_value = matches!(actual, Some(value) if !value.is_null());
            if has_value != exists {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::{Value, json};

    use super::{evaluate_event_match, expand_template};

    fn context() -> Value {
        json!({
            "now": "2026-01-01T00:00:00Z",
            "strategy": { "id": "strategy-id", "name": "テスト戦略" },
        })
    }

    #[rstest]
    #[case::literal("hello world", json!({}), "hello world")]
    #[case::payload("symbol={{payload.symbol}}", json!({"symbol": "SYM-X"}), "symbol=SYM-X")]
    #[case::nested_payload("v={{payload.a.b.c}}", json!({"a": {"b": {"c": "x"}}}), "v=x")]
    #[case::strategy_name("strategy={{strategy.name}}", json!({}), "strategy=テスト戦略")]
    #[case::now("at={{now}}", json!({}), "at=2026-01-01T00:00:00Z")]
    #[case::missing_path("v=[{{payload.missing}}]", json!({}), "v=[]")]
    #[case::number("n={{payload.n}}", json!({"n": 42}), "n=42")]
    #[case::trimmed_path("v={{ payload.x }}", json!({"x": "y"}), "v=y")]
    #[case::unclosed_placeholder("{ {{ open", json!({}), "{ {{ open")]
    fn expands_template_paths(
        #[case] template: &str,
        #[case] payload: Value,
        #[case] expected: &str,
    ) {
        assert_eq!(expand_template(template, &payload, &context()), expected);
    }

    #[rstest]
    #[case::no_spec(None, json!({"a": 1}), true)]
    #[case::null_spec(Some(json!(null)), json!({"a": 1}), true)]
    #[case::equal(Some(json!({"event": {"eq": "fired"}})), json!({"event": "fired"}), true)]
    #[case::not_equal(Some(json!({"event": {"eq": "fired"}})), json!({"event": "other"}), false)]
    #[case::exists(Some(json!({"symbol": {"exists": true}})), json!({"symbol": "SYM-X"}), true)]
    #[case::not_exists(Some(json!({"symbol": {"exists": false}})), json!({}), true)]
    #[case::nested_path(Some(json!({"a.b": {"eq": "x"}})), json!({"a": {"b": "x"}}), true)]
    #[case::all_conditions_match(
        Some(json!({"event": {"eq": "fired"}, "symbol": {"exists": true}})),
        json!({"event": "fired", "symbol": "SYM-X"}),
        true
    )]
    #[case::one_condition_fails(
        Some(json!({"event": {"eq": "fired"}, "symbol": {"exists": true}})),
        json!({"event": "fired"}),
        false
    )]
    fn matches_payload(
        #[case] spec: Option<Value>,
        #[case] payload: Value,
        #[case] expected: bool,
    ) {
        assert_eq!(evaluate_event_match(spec.as_ref(), &payload), expected);
    }
}
