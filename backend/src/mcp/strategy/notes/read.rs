use crate::services::graph::GraphDef;
use crate::services::note_links::find_links_from_version;
use crate::services::note_versions::{
    self, current_note_ids, current_note_ids_with_status, find_current_versions,
    find_initial_created_by_kind, find_latest_versions, find_version_of_note,
};
use chrono::{DateTime, FixedOffset};
use core_application::strategy_scope::StrategyScope;
use core_domain::note_reference::{
    ALLOWED_REF_KINDS, BodyTokenPolicy, collect_note_refs_with_policy, format_note_token_errors,
};
use gateway_postgres::entities::{note, note_ref, note_version};
use rmcp::ErrorData as McpError;
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait,
};

use super::super::dto::{ListNotesParams, ListNotesResult, NoteDto, NoteLinkDto, ReadNoteParams};
use super::super::{
    StrategyServer, clamp_limit, db_error, fetch_note_owned_by, internal_error, invalid_params,
};

/// note_version.status の CHECK 制約と一致させる。
const ALLOWED_NOTE_STATUS: [&str; 3] = ["approved", "unread", "rejected"];
const PENDING_LIST_SCAN_PAGE_SIZE: u64 = super::super::MAX_LIST_LIMIT;

#[derive(Clone, Copy)]
struct PendingNoteCursor {
    updated_at: DateTime<FixedOffset>,
    note_id: uuid::Uuid,
}

struct PendingNotesPage {
    matching_rows: Vec<(note::Model, note_version::Model)>,
    cursor: PendingNoteCursor,
    has_more: bool,
}

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
    Ok((kind.to_string(), id.to_string()))
}

fn refs_in_version(version: &note_version::Model) -> Result<Vec<(String, String)>, McpError> {
    let graphs: Vec<core_domain::note_graph::GraphDef> =
        serde_json::from_value(version.graphs_json.clone()).map_err(|error| {
            internal_error(format!(
                "failed to deserialize note_version.graphs_json: {error}"
            ))
        })?;
    collect_note_refs_with_policy(
        &version.body_md,
        &graphs,
        BodyTokenPolicy::AllowLegacyBodyTokens,
    )
    .map_err(|errors| {
        internal_error(format!(
            "failed to collect references from a pending note version: {}",
            format_note_token_errors(&errors)
        ))
    })
}

fn note_ids_matching_ref(
    kind: &str,
    id: &str,
    restrict_to: Option<sea_orm::sea_query::SelectStatement>,
) -> sea_orm::sea_query::SelectStatement {
    let mut query = note_ref::Entity::find()
        .select_only()
        .column(note_ref::Column::NoteId)
        .filter(note_ref::Column::RefKind.eq(kind))
        .filter(note_ref::Column::RefId.eq(id));
    if let Some(restrict_to) = restrict_to {
        query = query.filter(note_ref::Column::NoteId.in_subquery(restrict_to));
    }
    query.into_query()
}

/// `m.strategy_id` は呼び出し元が `session_strategy_id` で絞り込んだ行から来るため
/// 必ず `Some` になるはずだが、不変条件が壊れた場合に別 strategy の id を誤って
/// 返さないよう fail-loud にする。
fn note_to_dto(
    m: note::Model,
    version: note_version::Model,
    created_by_kind: String,
    include_body: bool,
) -> Result<NoteDto, McpError> {
    let strategy_id = m.strategy_id.ok_or_else(|| {
        internal_error(format!(
            "note {} has no strategy_id despite session scoping",
            m.id
        ))
    })?;
    let graphs: Vec<GraphDef> = serde_json::from_value(version.graphs_json).map_err(|e| {
        internal_error(format!(
            "failed to deserialize note_version.graphs_json: {e}"
        ))
    })?;
    let frontmatter_json = version
        .frontmatter_json
        .as_object()
        .cloned()
        .ok_or_else(|| internal_error("note_version.frontmatter_json is not a JSON object"))?;
    Ok(NoteDto {
        note_id: m.id,
        strategy_id,
        version_id: version.id,
        version_no: version.version_no,
        title: version.title,
        body_md: include_body.then_some(version.body_md),
        frontmatter_json,
        kind: m.kind,
        status: version.status,
        created_by_kind,
        created_at: m.created_at,
        updated_at: m.updated_at,
        graphs,
        links: None,
    })
}
impl StrategyServer {
    pub(crate) async fn read_note_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: ReadNoteParams,
    ) -> Result<NoteDto, McpError> {
        let session_strategy_id = scope.into().id();
        let row = fetch_note_owned_by(&self.db, params.note_id, session_strategy_id).await?;
        let version = match params.version_id {
            Some(version_id) => find_version_of_note(&self.db, params.note_id, Some(version_id))
                .await
                .map_err(db_error)?
                .ok_or_else(|| {
                    invalid_params(format!(
                        "version_id {version_id} does not belong to note {}",
                        params.note_id
                    ))
                })?,
            None => note_versions::find_current_or_latest_version(&self.db, params.note_id)
                .await
                .map_err(db_error)?
                .ok_or_else(|| internal_error(format!("note {} has no version", params.note_id)))?,
        };
        let created_by_kind = find_initial_created_by_kind(&self.db, &[params.note_id])
            .await
            .map_err(db_error)?
            .remove(&params.note_id)
            .ok_or_else(|| {
                internal_error(format!("note {} has no initial version", params.note_id))
            })?;
        let links = find_links_from_version(&self.db, version.id)
            .await
            .map_err(db_error)?
            .into_iter()
            .map(|link| NoteLinkDto {
                to_note_id: link.to_note_id,
                to_version_id: link.to_version_id,
            })
            .collect();
        let mut dto = note_to_dto(row, version, created_by_kind, true)?;
        dto.links = Some(links);
        Ok(dto)
    }

    async fn fetch_pending_notes_page(
        &self,
        session_strategy_id: uuid::Uuid,
        params: &ListNotesParams,
        reference: Option<&(String, String)>,
        cursor: Option<PendingNoteCursor>,
        page_size: u64,
    ) -> Result<Option<PendingNotesPage>, McpError> {
        let mut query =
            note::Entity::find().filter(note::Column::StrategyId.eq(session_strategy_id));
        if let Some(kind) = params.kind.as_deref() {
            query = query.filter(note::Column::Kind.eq(kind));
        }
        if let Some(updated_after) = params.updated_after {
            query = query.filter(note::Column::UpdatedAt.gte(updated_after));
        }
        if let Some(cursor) = cursor {
            query = query.filter(
                Condition::any()
                    .add(note::Column::UpdatedAt.lt(cursor.updated_at))
                    .add(
                        Condition::all()
                            .add(note::Column::UpdatedAt.eq(cursor.updated_at))
                            .add(note::Column::Id.lt(cursor.note_id)),
                    ),
            );
        }

        let mut current_candidate_ids = match params.status.as_deref() {
            Some(status) => current_note_ids_with_status(status),
            None => current_note_ids(),
        };
        if let Some((kind, id)) = reference {
            current_candidate_ids = note_ids_matching_ref(kind, id, Some(current_candidate_ids));
        }
        if params.status.is_some() || reference.is_some() {
            query = query.filter(
                Condition::any()
                    .add(note::Column::Id.in_subquery(current_candidate_ids))
                    .add(note::Column::Id.not_in_subquery(current_note_ids())),
            );
        }

        let rows = query
            .order_by_desc(note::Column::UpdatedAt)
            .order_by_desc(note::Column::Id)
            .limit(page_size)
            .all(&self.db)
            .await
            .map_err(db_error)?;
        if rows.is_empty() {
            return Ok(None);
        }
        let has_more = rows.len() as u64 == page_size;
        let last = rows
            .last()
            .ok_or_else(|| internal_error("pending note page is empty"))?;
        let cursor = PendingNoteCursor {
            updated_at: last.updated_at,
            note_id: last.id,
        };

        let note_ids: Vec<_> = rows.iter().map(|row| row.id).collect();
        let mut versions = find_current_versions(&self.db, &note_ids)
            .await
            .map_err(db_error)?;
        let pending_note_ids: Vec<_> = note_ids
            .iter()
            .filter(|note_id| !versions.contains_key(note_id))
            .copied()
            .collect();
        versions.extend(
            find_latest_versions(&self.db, &pending_note_ids)
                .await
                .map_err(db_error)?,
        );

        let mut matching_rows = Vec::new();
        for row in rows {
            let version = versions.remove(&row.id).ok_or_else(|| {
                internal_error(format!("note {} has no current or latest version", row.id))
            })?;
            if params
                .status
                .as_deref()
                .is_some_and(|status| version.status != status)
            {
                continue;
            }
            if let Some((kind, id)) = reference {
                let matches = if version.is_current {
                    true
                } else {
                    match refs_in_version(&version) {
                        Ok(refs) => refs
                            .iter()
                            .any(|reference| reference.0 == *kind && reference.1 == *id),
                        Err(error) => {
                            tracing::warn!(
                                note_id = %row.id,
                                version_id = %version.id,
                                %error,
                                "skipping pending note version with unparsable references"
                            );
                            false
                        }
                    }
                };
                if !matches {
                    continue;
                }
            }
            matching_rows.push((row, version));
        }

        Ok(Some(PendingNotesPage {
            matching_rows,
            cursor,
            has_more,
        }))
    }

    async fn list_notes_including_pending(
        &self,
        session_strategy_id: uuid::Uuid,
        params: ListNotesParams,
        reference: Option<(String, String)>,
        page_size: u64,
    ) -> Result<ListNotesResult, McpError> {
        let limit = clamp_limit(params.limit) as usize;
        let include_body = params.include_body.unwrap_or(true);
        let mut cursor = None;
        let mut notes = Vec::with_capacity(limit);

        loop {
            let Some(page) = self
                .fetch_pending_notes_page(
                    session_strategy_id,
                    &params,
                    reference.as_ref(),
                    cursor,
                    page_size,
                )
                .await?
            else {
                break;
            };
            let matching_ids: Vec<_> = page.matching_rows.iter().map(|(row, _)| row.id).collect();
            let creators = find_initial_created_by_kind(&self.db, &matching_ids)
                .await
                .map_err(db_error)?;
            for (row, version) in page.matching_rows {
                let created_by_kind = creators.get(&row.id).cloned().ok_or_else(|| {
                    internal_error(format!("note {} has no initial version", row.id))
                })?;
                notes.push(note_to_dto(row, version, created_by_kind, include_body)?);
                if notes.len() == limit {
                    return Ok(ListNotesResult { notes });
                }
            }
            if !page.has_more {
                break;
            }
            cursor = Some(page.cursor);
        }

        Ok(ListNotesResult { notes })
    }

    pub(crate) async fn list_notes_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: ListNotesParams,
    ) -> Result<ListNotesResult, McpError> {
        self.list_notes_inner_with_pending_page_size(
            scope.into().id(),
            params,
            PENDING_LIST_SCAN_PAGE_SIZE,
        )
        .await
    }

    pub(super) async fn list_notes_inner_with_pending_page_size(
        &self,
        session_strategy_id: uuid::Uuid,
        params: ListNotesParams,
        pending_page_size: u64,
    ) -> Result<ListNotesResult, McpError> {
        if let Some(status) = params.status.as_deref()
            && !ALLOWED_NOTE_STATUS.contains(&status)
        {
            return Err(invalid_params(format!(
                "invalid status: {status} (expected one of {ALLOWED_NOTE_STATUS:?})"
            )));
        }
        let include_body = params.include_body.unwrap_or(true);
        let reference = params.r#ref.as_deref().map(parse_note_ref).transpose()?;

        if params.include_pending.unwrap_or(false) {
            return self
                .list_notes_including_pending(
                    session_strategy_id,
                    params,
                    reference,
                    pending_page_size,
                )
                .await;
        }

        let mut query = note::Entity::find()
            .filter(note::Column::StrategyId.eq(session_strategy_id))
            .filter(note::Column::Id.in_subquery(current_note_ids()));
        if let Some(kind) = params.kind {
            query = query.filter(note::Column::Kind.eq(kind));
        }
        if let Some((kind, id)) = reference {
            query =
                query.filter(note::Column::Id.in_subquery(note_ids_matching_ref(&kind, &id, None)));
        }
        if let Some(status) = params.status {
            query =
                query.filter(note::Column::Id.in_subquery(current_note_ids_with_status(&status)));
        }
        if let Some(updated_after) = params.updated_after {
            query = query.filter(note::Column::UpdatedAt.gte(updated_after));
        }
        let rows = query
            .order_by_desc(note::Column::UpdatedAt)
            .limit(clamp_limit(params.limit))
            .all(&self.db)
            .await
            .map_err(db_error)?;
        let versions =
            find_current_versions(&self.db, &rows.iter().map(|row| row.id).collect::<Vec<_>>())
                .await
                .map_err(db_error)?;
        let creators = find_initial_created_by_kind(
            &self.db,
            &rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        )
        .await
        .map_err(db_error)?;
        let notes = rows
            .into_iter()
            .map(|row| {
                let version = versions.get(&row.id).cloned().ok_or_else(|| {
                    internal_error(format!("note {} has no current version", row.id))
                })?;
                let created_by_kind = creators.get(&row.id).cloned().ok_or_else(|| {
                    internal_error(format!("note {} has no initial version", row.id))
                })?;
                note_to_dto(row, version, created_by_kind, include_body)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ListNotesResult { notes })
    }
}
