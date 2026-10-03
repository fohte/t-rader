use serde_json::Value;

use crate::note::NoteUseCaseError;

pub fn frontmatter_tags(value: &Value) -> Vec<String> {
    value
        .get("tags")
        .and_then(Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

pub fn frontmatter_has_tag(value: &Value, tag: &str) -> bool {
    value
        .get("tags")
        .and_then(Value::as_array)
        .is_some_and(|tags| tags.iter().any(|value| value.as_str() == Some(tag)))
}

pub(super) fn ensure_frontmatter_tags_are_strings(value: &Value) -> Result<(), NoteUseCaseError> {
    let Some(tags) = value.get("tags") else {
        return Ok(());
    };

    if tags
        .as_array()
        .is_some_and(|tags| tags.iter().all(Value::is_string))
    {
        Ok(())
    } else {
        Err(NoteUseCaseError::Validation(
            "frontmatter_json.tags must be an array of strings".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::{Value, json};

    use super::{ensure_frontmatter_tags_are_strings, frontmatter_has_tag, frontmatter_tags};

    #[rstest]
    #[case::missing(json!({}), None)]
    #[case::string_array(json!({"tags": ["sample-label", "another-label"]}), None)]
    #[case::empty_array(json!({"tags": []}), None)]
    #[case::string(json!({"tags": "sample-label"}), Some("frontmatter_json.tags must be an array of strings"))]
    #[case::mixed_array(json!({"tags": ["sample-label", 7]}), Some("frontmatter_json.tags must be an array of strings"))]
    #[case::null_element(json!({"tags": [null]}), Some("frontmatter_json.tags must be an array of strings"))]
    fn validates_frontmatter_tags(#[case] value: Value, #[case] expected_error: Option<&str>) {
        assert_eq!(
            ensure_frontmatter_tags_are_strings(&value)
                .err()
                .map(|error| error.to_string())
                .as_deref(),
            expected_error,
        );
    }

    #[test]
    fn reads_string_tags_from_frontmatter() {
        assert_eq!(
            (
                frontmatter_tags(&json!({ "tags": ["sample-label", 7] })),
                frontmatter_has_tag(&json!({ "tags": ["sample-label", 7] }), "sample-label"),
                frontmatter_has_tag(&json!({ "tags": ["sample-label", 7] }), "another-label"),
            ),
            (vec!["sample-label".to_owned()], true, false),
        );
    }
}
