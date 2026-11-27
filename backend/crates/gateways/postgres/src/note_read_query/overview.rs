use std::collections::HashMap;

use core_application::note::{
    NoteOverview, NoteOverviewQuery, NoteOverviewView, NoteReadQueryError, NoteSnapshot,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, FromQueryResult, QueryFilter,
    Statement,
};
use uuid::Uuid;

use crate::entities::{note, note_version};

use super::list::version_matches_references;
use super::{PostgresNoteReadQuery, query_error, to_note, to_version};

const LIST_NOTE_OVERVIEWS_SQL: &str = r#"
    SELECT note.id AS note_id,
           representative.id AS representative_version_id,
           current_version.id AS current_version_id,
           latest_unread.id AS latest_unread_version_id,
           unread.unread_version_count
    FROM note
    JOIN LATERAL (
        SELECT id, frontmatter_json
        FROM note_version
        WHERE note_id = note.id
          AND ($3::text <> 'pending' OR status = 'unread')
        ORDER BY CASE WHEN $3::text = 'pending' THEN false ELSE is_current END DESC,
                 version_no DESC
        LIMIT 1
    ) AS representative ON TRUE
    LEFT JOIN LATERAL (
        SELECT id
        FROM note_version
        WHERE note_id = note.id AND is_current
        ORDER BY version_no DESC
        LIMIT 1
    ) AS current_version ON TRUE
    LEFT JOIN LATERAL (
        SELECT count(*) AS unread_version_count,
               max(version_no) AS latest_unread_version_no
        FROM note_version
        WHERE note_id = note.id AND status = 'unread'
    ) AS unread ON TRUE
    LEFT JOIN note_version AS latest_unread
        ON latest_unread.note_id = note.id
       AND latest_unread.version_no = unread.latest_unread_version_no
    WHERE ($1::text IS NULL OR note.kind = $1)
      AND ($2::text IS NULL OR representative.frontmatter_json @> jsonb_build_object('tags', jsonb_build_array($2)))
      AND ($3::text <> 'pending' OR latest_unread.id IS NOT NULL)
      AND (
          $4::text IS NULL
          OR current_version.id IS NULL
          OR EXISTS (
              SELECT 1
              FROM note_ref
              WHERE note_ref.note_id = note.id
                AND note_ref.ref_kind = $4
                AND note_ref.ref_id = $5
          )
      )
    ORDER BY note.updated_at DESC, note.id
"#;

#[derive(Debug, FromQueryResult)]
struct NoteOverviewRow {
    note_id: Uuid,
    representative_version_id: Uuid,
    current_version_id: Option<Uuid>,
    latest_unread_version_id: Option<Uuid>,
    unread_version_count: i64,
}

pub(super) async fn list_note_overviews(
    query_source: &PostgresNoteReadQuery,
    query: NoteOverviewQuery,
) -> Result<Vec<NoteOverview>, NoteReadQueryError> {
    let view = match query.view {
        NoteOverviewView::All => "all",
        NoteOverviewView::Pending => "pending",
    };
    let rows = query_source
        .db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            LIST_NOTE_OVERVIEWS_SQL,
            [
                query.kind.into(),
                query.tag.into(),
                view.into(),
                query
                    .reference
                    .as_ref()
                    .map(|(kind, _)| kind.clone())
                    .into(),
                query.reference.as_ref().map(|(_, id)| id.clone()).into(),
            ],
        ))
        .await
        .map_err(query_error)?;
    let rows = rows
        .iter()
        .map(|row| NoteOverviewRow::from_query_result(row, "").map_err(query_error))
        .collect::<Result<Vec<_>, _>>()?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let note_ids = rows.iter().map(|row| row.note_id).collect::<Vec<_>>();
    let version_ids = rows
        .iter()
        .flat_map(|row| {
            [
                Some(row.representative_version_id),
                row.current_version_id,
                row.latest_unread_version_id,
            ]
            .into_iter()
            .flatten()
        })
        .collect::<Vec<_>>();
    let note_models = note::Entity::find()
        .filter(note::Column::Id.is_in(note_ids.iter().copied()))
        .all(&query_source.db)
        .await
        .map_err(query_error)?;
    let version_models = note_version::Entity::find()
        .filter(note_version::Column::Id.is_in(version_ids))
        .all(&query_source.db)
        .await
        .map_err(query_error)?;
    let notes = note_models
        .into_iter()
        .map(|model| (model.id, model))
        .collect::<HashMap<_, _>>();
    let versions = version_models
        .into_iter()
        .map(|model| (model.id, model))
        .collect::<HashMap<_, _>>();
    let creators = super::find_initial_created_by_kind(&query_source.db, &note_ids).await?;

    let overviews = rows
        .into_iter()
        .map(|row| -> Result<Option<NoteOverview>, NoteReadQueryError> {
            let note_model = notes.get(&row.note_id).cloned().ok_or_else(|| {
                NoteReadQueryError::InvalidData(format!("note {} not found", row.note_id))
            })?;
            let created_by_kind = creators.get(&row.note_id).cloned().ok_or_else(|| {
                NoteReadQueryError::InvalidData(format!(
                    "note {} has no initial version",
                    row.note_id
                ))
            })?;
            let representative = required_version(&versions, row.representative_version_id)?;
            let current = row
                .current_version_id
                .map(|version_id| required_version(&versions, version_id))
                .transpose()?;
            if current.is_none()
                && let Some(references) = query.reference.as_ref()
            {
                let version_model =
                    versions
                        .get(&row.representative_version_id)
                        .ok_or_else(|| {
                            NoteReadQueryError::InvalidData(format!(
                                "note version {} not found",
                                row.representative_version_id
                            ))
                        })?;
                match version_matches_references(version_model, std::slice::from_ref(references)) {
                    Ok(true) => {}
                    Ok(false) => return Ok(None),
                    Err(error) => {
                        tracing::warn!(
                            note_id = %row.note_id,
                            version_id = %row.representative_version_id,
                            %error,
                            "skipping pending note version with unparsable references"
                        );
                        return Ok(None);
                    }
                }
            }
            let latest_unread = row
                .latest_unread_version_id
                .map(|version_id| required_version(&versions, version_id))
                .transpose()?;

            Ok(Some(NoteOverview {
                representative: NoteSnapshot {
                    note: to_note(note_model),
                    version: representative,
                    created_by_kind,
                },
                current,
                latest_unread,
                unread_version_count: row.unread_version_count,
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(overviews.into_iter().flatten().collect())
}

fn required_version(
    versions: &HashMap<Uuid, note_version::Model>,
    version_id: Uuid,
) -> Result<core_application::note::NoteVersion, NoteReadQueryError> {
    let version = versions.get(&version_id).cloned().ok_or_else(|| {
        NoteReadQueryError::InvalidData(format!("note version {version_id} not found"))
    })?;
    to_version(version)
}
