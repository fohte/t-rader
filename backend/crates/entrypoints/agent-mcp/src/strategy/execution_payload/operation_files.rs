use std::collections::HashSet;

use core_application::{
    kata_exec::ExecFile,
    strategy_execution::{AuthorizedExecution, ReadPayloadQuery},
};
use rmcp::ErrorData as McpError;
use serde::Serialize;
use serde_json::json;

use super::super::StrategyServer;
use super::super::dto::OperationPayloadInput;
use super::super::{internal_error, invalid_params};
use super::strategy_execution_error;

pub(super) const MAX_OPERATION_FILES: usize = 128;
pub(super) const MAX_OPERATION_FILE_BYTES: usize = 8 * 1024 * 1024;
const MAX_OPERATION_TOTAL_BYTES: usize = 16 * 1024 * 1024;
const OPERATION_PAYLOAD_PAGE_LIMIT: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct MaterializedPayloadInput {
    field: String,
    payload_id: uuid::Uuid,
    pointer: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u32>,
    digest: String,
    path: String,
}

impl StrategyServer {
    pub(super) async fn materialize_operation_inputs(
        &self,
        access: &AuthorizedExecution,
        mut args: serde_json::Value,
        bindings: &[OperationPayloadInput],
    ) -> Result<
        (
            serde_json::Value,
            Option<Vec<ExecFile>>,
            Vec<MaterializedPayloadInput>,
        ),
        McpError,
    > {
        if bindings.is_empty() {
            return Ok((args, None, Vec::new()));
        }
        if bindings.len() > MAX_OPERATION_FILES {
            return Err(invalid_params(format!(
                "payload_inputs exceeds {MAX_OPERATION_FILES} files"
            )));
        }
        if !args.is_object() {
            return Err(invalid_params(
                "args must be an object when payload_inputs are used",
            ));
        }
        let mut fields = HashSet::with_capacity(bindings.len());
        let mut files = Vec::with_capacity(bindings.len());
        let mut materialized = Vec::with_capacity(bindings.len());
        let mut total_bytes = 0usize;

        for (index, binding) in bindings.iter().enumerate() {
            if binding.field.trim().is_empty() {
                return Err(invalid_params("payload input field must not be empty"));
            }
            if !fields.insert(binding.field.as_str()) {
                return Err(invalid_params(format!(
                    "payload input field '{}' is duplicated",
                    binding.field
                )));
            }
            if args.get(&binding.field).is_some() {
                return Err(invalid_params(format!(
                    "args already contains payload input field '{}'",
                    binding.field
                )));
            }

            let (bytes, digest) = self.read_payload_selection_as_file(access, binding).await?;
            total_bytes = total_bytes
                .checked_add(bytes.len())
                .ok_or_else(|| invalid_params("payload input size overflow"))?;
            if total_bytes > MAX_OPERATION_TOTAL_BYTES {
                return Err(invalid_params(format!(
                    "payload inputs exceed {MAX_OPERATION_TOTAL_BYTES} total bytes"
                )));
            }

            let path = format!("inputs/input-{index}.json");
            args.as_object_mut()
                .ok_or_else(|| internal_error("payload input args stopped being an object"))?
                .insert(
                    binding.field.clone(),
                    json!({
                        "path": path,
                        "payload_id": binding.payload_id,
                        "pointer": binding.pointer,
                        "digest": digest,
                    }),
                );
            files.push(ExecFile {
                path: path.clone(),
                bytes,
            });
            materialized.push(MaterializedPayloadInput {
                field: binding.field.clone(),
                payload_id: binding.payload_id,
                pointer: binding.pointer.clone(),
                cursor: binding.cursor,
                limit: binding.limit,
                digest,
                path,
            });
        }

        Ok((args, Some(files), materialized))
    }

    async fn read_payload_selection_as_file(
        &self,
        access: &AuthorizedExecution,
        binding: &OperationPayloadInput,
    ) -> Result<(Vec<u8>, String), McpError> {
        let start_cursor = binding.cursor.unwrap_or_default();
        if binding.limit == Some(0) {
            return Err(invalid_params(
                "payload input limit must be greater than zero",
            ));
        }
        let selection_limit = binding.limit.map(u64::from);
        let mut cursor = start_cursor;
        let mut digest: Option<String> = None;
        let mut expected_total = None;
        let mut selected_array: Option<Vec<serde_json::Value>> = None;
        let mut selected_value = None;
        let mut selected_count = 0u64;
        let mut selected_content_bytes = 2usize;

        loop {
            let page_limit = next_payload_page_limit(selection_limit, selected_count);
            if page_limit == 0 {
                break;
            }
            let page = self
                .dependencies
                .strategy_executions
                .read_payload(
                    access,
                    ReadPayloadQuery {
                        payload_id: binding.payload_id,
                        pointer: binding.pointer.clone(),
                        cursor,
                        limit: page_limit,
                    },
                )
                .await
                .map_err(strategy_execution_error)?;
            if page.cursor != cursor {
                return Err(internal_error("payload page cursor did not match request"));
            }
            if digest
                .as_ref()
                .is_some_and(|current| current != &page.digest)
            {
                return Err(internal_error("payload digest changed between pages"));
            }
            digest.get_or_insert_with(|| page.digest.clone());
            if expected_total.is_some_and(|total| total != page.total_count) {
                return Err(internal_error("payload item count changed between pages"));
            }
            expected_total.get_or_insert(page.total_count);

            if let Some(page_items) = page.data.as_array() {
                let items = selected_array.get_or_insert_with(Vec::new);
                for item in page_items {
                    let item_size = serde_json::to_vec(item)
                        .map_err(|_| internal_error("failed to serialize payload input"))?
                        .len();
                    selected_content_bytes = selected_content_bytes
                        .checked_add(item_size + usize::from(!items.is_empty()))
                        .ok_or_else(|| invalid_params("payload input size overflow"))?;
                    if selected_content_bytes > MAX_OPERATION_FILE_BYTES {
                        return Err(invalid_params(format!(
                            "payload input exceeds {MAX_OPERATION_FILE_BYTES} bytes"
                        )));
                    }
                    items.push(item.clone());
                }
                selected_count = selected_count
                    .checked_add(page_items.len() as u64)
                    .ok_or_else(|| invalid_params("payload input item count overflow"))?;
            } else if cursor == 0 && selected_value.is_none() {
                selected_value = Some(page.data.clone());
                selected_count = 1;
            } else {
                return Err(internal_error("payload page shape changed between pages"));
            }

            if let Some(next_cursor) = page.next_cursor {
                if selected_value.is_some() {
                    return Err(internal_error("non-array payload returned a next cursor"));
                }
                let page_item_count = page.data.as_array().map_or(0, |items| items.len() as u64);
                if next_cursor <= cursor
                    || next_cursor > page.total_count
                    || cursor.checked_add(page_item_count) != Some(next_cursor)
                {
                    return Err(internal_error("payload page skipped or repeated items"));
                }
            }

            let limit_reached = selection_limit.is_some_and(|limit| selected_count >= limit);
            match (page.next_cursor, limit_reached) {
                (Some(next_cursor), false) => cursor = next_cursor,
                (_, true) | (None, false) => break,
            }
        }

        let total_count =
            expected_total.ok_or_else(|| internal_error("payload page omitted item count"))?;
        let expected_selected_count =
            expected_payload_selection_count(total_count, start_cursor, selection_limit);
        let value = match selected_array {
            Some(items) if items.len() as u64 == expected_selected_count => {
                serde_json::Value::Array(items)
            }
            Some(_) => return Err(internal_error("payload ended before selected items")),
            None if start_cursor == 0 && total_count == 1 && selected_count == 1 => selected_value
                .ok_or_else(|| internal_error("payload page omitted selected value"))?,
            None if total_count == 0 && start_cursor == 0 => serde_json::Value::Array(Vec::new()),
            None => return Err(internal_error("payload page omitted selected array")),
        };
        let bytes = serde_json::to_vec(&value)
            .map_err(|_| internal_error("failed to serialize payload input"))?;
        if bytes.len() > MAX_OPERATION_FILE_BYTES {
            return Err(invalid_params(format!(
                "payload input exceeds {MAX_OPERATION_FILE_BYTES} bytes"
            )));
        }
        Ok((
            bytes,
            digest.ok_or_else(|| internal_error("payload digest is missing"))?,
        ))
    }
}

fn next_payload_page_limit(selection_limit: Option<u64>, selected_count: u64) -> usize {
    selection_limit.map_or(OPERATION_PAYLOAD_PAGE_LIMIT as u64, |limit| {
        limit
            .saturating_sub(selected_count)
            .min(OPERATION_PAYLOAD_PAGE_LIMIT as u64)
    }) as usize
}

fn expected_payload_selection_count(
    total_count: u64,
    start_cursor: u64,
    selection_limit: Option<u64>,
) -> u64 {
    let remaining_items = total_count.saturating_sub(start_cursor);
    selection_limit.map_or(remaining_items, |limit| remaining_items.min(limit))
}

pub(super) fn validate_file_collection(files: &[ExecFile], label: &str) -> Result<(), McpError> {
    if files.len() > MAX_OPERATION_FILES {
        return Err(invalid_params(format!(
            "{label} file count exceeds {MAX_OPERATION_FILES}"
        )));
    }
    let mut paths = HashSet::with_capacity(files.len());
    let mut total_bytes = 0usize;
    for file in files {
        validate_relative_file_path(&file.path, label)?;
        if !paths.insert(file.path.as_str()) {
            return Err(invalid_params(format!("{label} file paths must be unique")));
        }
        if file.bytes.len() > MAX_OPERATION_FILE_BYTES {
            return Err(invalid_params(format!(
                "{label} file exceeds {MAX_OPERATION_FILE_BYTES} bytes"
            )));
        }
        total_bytes = total_bytes
            .checked_add(file.bytes.len())
            .ok_or_else(|| invalid_params(format!("{label} file size overflow")))?;
        if total_bytes > MAX_OPERATION_TOTAL_BYTES {
            return Err(invalid_params(format!(
                "{label} files exceed {MAX_OPERATION_TOTAL_BYTES} total bytes"
            )));
        }
    }
    for path in &paths {
        let mut parent = String::new();
        let parts: Vec<_> = path.split('/').collect();
        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            if !parent.is_empty() {
                parent.push('/');
            }
            parent.push_str(part);
            if paths.contains(parent.as_str()) {
                return Err(invalid_params(format!("{label} file paths conflict")));
            }
        }
    }
    Ok(())
}

fn validate_relative_file_path(path: &str, label: &str) -> Result<(), McpError> {
    let parts: Vec<_> = path.split('/').collect();
    if path.is_empty()
        || path.len() > 4096
        || path.contains('\\')
        || path.contains('\0')
        || parts
            .iter()
            .any(|part| part.is_empty() || *part == "." || *part == ".." || part.len() > 255)
    {
        return Err(invalid_params(format!(
            "{label} file path is not normalized"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{expected_payload_selection_count, next_payload_page_limit};

    #[rstest]
    #[case::unbounded(None, 50, 1_000)]
    #[case::first_finite_page(Some(1_200), 0, 1_000)]
    #[case::last_finite_page(Some(1_200), 1_000, 200)]
    #[case::complete(Some(1_200), 1_200, 0)]
    fn chooses_a_bounded_payload_page_size(
        #[case] selection_limit: Option<u64>,
        #[case] selected_count: u64,
        #[case] expected: usize,
    ) {
        assert_eq!(
            next_payload_page_limit(selection_limit, selected_count),
            expected,
        );
    }

    #[rstest]
    #[case::whole_array(20, 0, None, 20)]
    #[case::from_cursor(20, 5, None, 15)]
    #[case::finite_window(20, 5, Some(3), 3)]
    #[case::empty_after_end(5, 5, Some(3), 0)]
    fn counts_only_the_requested_payload_window(
        #[case] total_count: u64,
        #[case] start_cursor: u64,
        #[case] selection_limit: Option<u64>,
        #[case] expected: u64,
    ) {
        assert_eq!(
            expected_payload_selection_count(total_count, start_cursor, selection_limit),
            expected,
        );
    }
}
