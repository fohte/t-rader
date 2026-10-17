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
