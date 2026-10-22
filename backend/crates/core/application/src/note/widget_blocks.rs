use core_domain::note_reference::fenced_code_blocks;

use crate::note::NoteUseCaseError;
use crate::widget::WidgetDefinition;

pub(super) fn validate_note_widget_blocks(body_md: &str) -> Result<(), NoteUseCaseError> {
    let mut block_number = 0;

    for block in fenced_code_blocks(body_md)
        .into_iter()
        .filter(|block| block.info == "widget")
    {
        block_number += 1;
        let definition = WidgetDefinition::from_yaml(block.content).map_err(|error| {
            NoteUseCaseError::Validation(format!("widget block {block_number}: {error}"))
        })?;
        definition.validate().map_err(|error| {
            NoteUseCaseError::Validation(format!("widget block {block_number}: {error}"))
        })?;
        if definition.time.as_of.is_none() {
            return Err(NoteUseCaseError::Validation(format!(
                "widget block {block_number}: time.as_of is required"
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use rstest::rstest;

    use super::validate_note_widget_blocks;
    use crate::note::NoteUseCaseError;

    #[rstest]
    #[case::fixed_as_of(indoc! {"
        ```widget
        queries:
          price:
            metric: close
            by: stock
            universe:
              stocks: [demo-code]
        viz:
          type: line
          series: [price]
        time:
          as_of: 2098-01-06
          range: 2098-01-05..2098-01-06
        ```
    "}, Ok(()))]
    #[case::missing_as_of(indoc! {"
        ```widget
        queries:
          value:
            metric: indicator
            by: none
            indicator: demo-indicator
        viz:
          type: line
          series: [value]
        time:
          range: 1y
        ```
    "}, Err("widget block 1: time.as_of is required".to_string()))]
    #[case::range_after_as_of(indoc! {"
        ```widget
        queries:
          price:
            metric: close
            by: stock
            universe:
              stocks: [demo-code]
        viz:
          type: line
          series: [price]
        time:
          as_of: 2098-01-05
          range: 2098-01-05..2098-01-06
        ```
    "}, Err("widget block 1: time.range: range end must not be after time.as_of".to_string()))]
    fn validates_line_widget_blocks_for_note_storage(
        #[case] body: &str,
        #[case] expected: Result<(), String>,
    ) {
        let actual = validate_note_widget_blocks(body).map_err(|error| match error {
            NoteUseCaseError::Validation(message) => message,
            other => other.to_string(),
        });

        assert_eq!(actual, expected);
    }
}
