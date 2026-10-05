use super::super::graph_dto::GraphDef;
use core_application::note::{
    NoteListQuery, NoteReadQueryError, NoteReadUseCaseError, NoteSnapshot, frontmatter_tags,
};
use core_domain::note_reference::{ALLOWED_REF_KINDS, is_valid_ref_id_format};
use rmcp::ErrorData as McpError;

use super::super::dto::{ListNotesParams, ListNotesResult, NoteDto, NoteLinkDto, ReadNoteParams};
use super::super::{StrategyServer, clamp_limit, internal_error, invalid_params};

/// note_version.status の CHECK 制約と一致させる。
const ALLOWED_NOTE_STATUS: [&str; 3] = ["approved", "unread", "rejected"];
const PENDING_LIST_SCAN_PAGE_SIZE: u64 = super::super::MAX_LIST_LIMIT;

fn parse_note_ref(value: &str) -> Result<(String, String), McpError> {
    let Some((kind, id)) = value.split_once(':') else {
        return Err(invalid_params("ref must use the kind:id format"));
    };
    let kind = kind.trim();
    if !ALLOWED_REF_KINDS.contains(&kind) {
        return Err(invalid_params(format!("invalid ref kind: {kind}")));
    }
    let id = id.trim();
    if id.is_empty() {
        return Err(invalid_params("ref id must not be empty"));
    }
    if !is_valid_ref_id_format(kind, id) {
        return Err(invalid_params(
            "group ref id must use axis-key/group-key format",
        ));
    }
    Ok((kind.to_string(), id.to_string()))
}

fn note_to_dto(snapshot: NoteSnapshot, include_body: bool) -> Result<NoteDto, McpError> {
    let graphs: Vec<GraphDef> =
        serde_json::from_value(snapshot.version.graphs_json).map_err(|error| {
            internal_error(format!(
                "failed to deserialize note_version.graphs_json: {error}"
            ))
        })?;
    let frontmatter_json = snapshot
        .version
        .frontmatter_json
        .as_object()
        .cloned()
        .ok_or_else(|| internal_error("note_version.frontmatter_json is not a JSON object"))?;
    let tags = frontmatter_tags(&snapshot.version.frontmatter_json);
    Ok(NoteDto {
        note_id: snapshot.note.id,
        version_id: snapshot.version.id,
        version_no: snapshot.version.version_no,
        title: snapshot.version.title,
        body_md: include_body.then_some(snapshot.version.body_md),
        frontmatter_json,
        tags,
        kind: snapshot.note.kind,
        status: snapshot.version.status,
        created_by_kind: snapshot.created_by_kind,
        created_at: snapshot.note.created_at,
        updated_at: snapshot.note.updated_at,
        graphs,
        links: None,
    })
}

pub(crate) fn note_read_error_to_mcp(error: NoteReadUseCaseError) -> McpError {
    match error {
        NoteReadUseCaseError::NotFound(_) => McpError::resource_not_found("note not found", None),
        NoteReadUseCaseError::VersionDoesNotBelong {
            note_id,
            version_id,
        } => invalid_params(format!(
            "version_id {version_id} does not belong to note {note_id}"
        )),
        NoteReadUseCaseError::NoVersion(note_id) => {
            internal_error(format!("note {note_id} has no version"))
        }
        NoteReadUseCaseError::InitialVersionNotFound(note_id) => {
            internal_error(format!("note {note_id} has no initial version"))
        }
        NoteReadUseCaseError::VersionNumberNotFound { .. } => {
            McpError::resource_not_found("note version not found", None)
        }
        NoteReadUseCaseError::NoteVersionNotFound => {
            McpError::resource_not_found("note version not found", None)
        }
        NoteReadUseCaseError::Query(NoteReadQueryError::Database(error)) => {
            super::super::persistence_error_to_mcp(error)
        }
        NoteReadUseCaseError::Query(NoteReadQueryError::InvalidData(message)) => {
            internal_error(message)
        }
    }
}

impl StrategyServer {
    pub(crate) async fn read_note_inner(
        &self,
        params: ReadNoteParams,
    ) -> Result<NoteDto, McpError> {
        let snapshot = self
            .dependencies
            .note_reads
            .get_note(params.note_id, params.version_id, true)
            .await
            .map_err(note_read_error_to_mcp)?;
        let links = self
            .dependencies
            .note_reads
            .find_links_from_version(snapshot.version.id)
            .await
            .map_err(note_read_error_to_mcp)?
            .into_iter()
            .map(|link| NoteLinkDto {
                to_note_id: link.to_note_id,
                to_version_id: link.to_version_id,
            })
            .collect();
        let mut dto = note_to_dto(snapshot, true)?;
        dto.links = Some(links);
        Ok(dto)
    }

    async fn list_notes_including_pending(
        &self,
        params: ListNotesParams,
        reference: Option<(String, String)>,
        page_size: u64,
    ) -> Result<ListNotesResult, McpError> {
        let limit = clamp_limit(params.limit) as usize;
        let include_body = params.include_body.unwrap_or(true);
        let mut cursor = None;
        let mut notes = Vec::with_capacity(limit);

        loop {
            let page = self
                .dependencies
                .note_reads
                .list_notes(NoteListQuery {
                    kind: params.kind.clone(),
                    status: params.status.clone(),
                    tag: params.tag.clone(),
                    reference: reference.clone(),
                    updated_after: params.updated_after,
                    include_pending: true,
                    cursor,
                    limit: Some(page_size),
                })
                .await
                .map_err(note_read_error_to_mcp)?;
            for snapshot in page.notes {
                notes.push(note_to_dto(snapshot, include_body)?);
                if notes.len() == limit {
                    return Ok(ListNotesResult { notes });
                }
            }
            if !page.has_more {
                break;
            }
            cursor = page.cursor;
        }

        Ok(ListNotesResult { notes })
    }

    pub(crate) async fn list_notes_inner(
        &self,
        params: ListNotesParams,
    ) -> Result<ListNotesResult, McpError> {
        if let Some(status) = params.status.as_deref()
            && !ALLOWED_NOTE_STATUS.contains(&status)
        {
            return Err(invalid_params(format!(
                "invalid status: {status} (expected one of {ALLOWED_NOTE_STATUS:?})"
            )));
        }
        let reference = params.r#ref.as_deref().map(parse_note_ref).transpose()?;

        if params.include_pending.unwrap_or(false) {
            return self
                .list_notes_including_pending(params, reference, PENDING_LIST_SCAN_PAGE_SIZE)
                .await;
        }

        let page = self
            .dependencies
            .note_reads
            .list_notes(NoteListQuery {
                kind: params.kind,
                reference,
                status: params.status,
                tag: params.tag,
                updated_after: params.updated_after,
                limit: Some(clamp_limit(params.limit)),
                ..NoteListQuery::default()
            })
            .await
            .map_err(note_read_error_to_mcp)?;
        let include_body = params.include_body.unwrap_or(true);
        let notes = page
            .notes
            .into_iter()
            .map(|snapshot| note_to_dto(snapshot, include_body))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ListNotesResult { notes })
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use core_application::note::{Note, NoteVersion};
    use serde_json::json;
    use uuid::Uuid;

    use super::{NoteDto, NoteSnapshot, note_to_dto};

    #[test]
    fn note_dto_maps_note_fields() {
        let id = Uuid::nil();
        let timestamp =
            DateTime::parse_from_rfc3339("2000-01-01T00:00:00+00:00").expect("valid timestamp");
        let actual = note_to_dto(
            NoteSnapshot {
                note: Note {
                    id,
                    kind: None,
                    trigger: None,
                    trigger_label: None,
                    created_at: timestamp,
                    updated_at: timestamp,
                    execution_id: None,
                },
                version: NoteVersion {
                    id,
                    note_id: id,
                    version_no: 1,
                    title: "Example note".into(),
                    body_md: "Example body".into(),
                    frontmatter_json: json!({ "tags": ["demo-focus"] }),
                    graphs_json: json!([]),
                    resolved_price_references_json: json!({}),
                    status: "approved".into(),
                    is_current: true,
                    change_reason: None,
                    created_by_kind: "human".into(),
                    execution_id: None,
                    created_at: timestamp,
                    reviewed_at: None,
                },
                created_by_kind: "human".into(),
            },
            true,
        )
        .expect("note converts to a DTO");

        assert_eq!(
            actual,
            NoteDto {
                note_id: id,
                version_id: id,
                version_no: 1,
                title: "Example note".into(),
                body_md: Some("Example body".into()),
                frontmatter_json: json!({ "tags": ["demo-focus"] })
                    .as_object()
                    .cloned()
                    .expect("object frontmatter"),
                tags: vec!["demo-focus".into()],
                kind: None,
                status: "approved".into(),
                created_by_kind: "human".into(),
                created_at: timestamp,
                updated_at: timestamp,
                graphs: Vec::new(),
                links: None,
            }
        );
    }
}
