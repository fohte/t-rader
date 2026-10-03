use serde_json::Value;

use crate::note::NoteUseCaseError;

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

    use super::ensure_frontmatter_tags_are_strings;

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
}
