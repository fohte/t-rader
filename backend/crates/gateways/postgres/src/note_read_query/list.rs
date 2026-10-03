use crate::entities::{note, note_ref, note_version};
use core_application::note::{NoteListCursor, NoteListPage, NoteListQuery, NoteReadQueryError};
use core_domain::note_graph::GraphDef;
use core_domain::note_reference::{
    BodyTokenPolicy, collect_note_refs_with_policy, format_note_token_errors,
};
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait,
};

use super::{PostgresNoteReadQuery, find_current_versions, find_latest_versions, query_error};

impl PostgresNoteReadQuery {
    pub(super) async fn list_query(
        &self,
        query: NoteListQuery,
    ) -> Result<NoteListPage, NoteReadQueryError> {
        if query.include_pending {
            self.list_including_pending(query).await
        } else {
            self.list_current_notes(query).await
        }
    }
    async fn list_current_notes(
        &self,
        query: NoteListQuery,
    ) -> Result<NoteListPage, NoteReadQueryError> {
        let mut select = note::Entity::find()
            .filter(note::Column::Id.in_subquery(current_note_ids()))
            .order_by_desc(note::Column::UpdatedAt);
        if let Some(strategy_id) = query.strategy_id {
            select = select.filter(note::Column::StrategyId.eq(strategy_id));
        }
        if let Some(status) = query.status.as_deref() {
            select =
                select.filter(note::Column::Id.in_subquery(current_note_ids_with_status(status)));
        }
        if let Some(kind) = query.kind.as_deref() {
            select = select.filter(note::Column::Kind.eq(kind));
        }
        if let Some((kind, id)) = query.reference.as_ref() {
            select =
                select.filter(note::Column::Id.in_subquery(note_ids_matching_ref(kind, id, None)));
        }
        if let Some(updated_after) = query.updated_after {
            select = select.filter(note::Column::UpdatedAt.gte(updated_after));
        }
        if let Some(limit) = query.limit {
            select = select.limit(limit);
        }
        let rows = select.all(&self.db).await.map_err(query_error)?;
        let ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
        let versions = find_current_versions(&self.db, &ids).await?;
        let notes = self.snapshots(rows, versions).await?;
        Ok(NoteListPage {
            notes,
            cursor: None,
            has_more: false,
        })
    }

    async fn list_including_pending(
        &self,
        query: NoteListQuery,
    ) -> Result<NoteListPage, NoteReadQueryError> {
        let mut select = note::Entity::find();
        if let Some(strategy_id) = query.strategy_id {
            select = select.filter(note::Column::StrategyId.eq(strategy_id));
        }
        if let Some(kind) = query.kind.as_deref() {
            select = select.filter(note::Column::Kind.eq(kind));
        }
        if let Some(updated_after) = query.updated_after {
            select = select.filter(note::Column::UpdatedAt.gte(updated_after));
        }
        if let Some(cursor) = query.cursor {
            select = select.filter(
                Condition::any()
                    .add(note::Column::UpdatedAt.lt(cursor.updated_at))
                    .add(
                        Condition::all()
                            .add(note::Column::UpdatedAt.eq(cursor.updated_at))
                            .add(note::Column::Id.lt(cursor.note_id)),
                    ),
            );
        }

        let mut current_candidate_ids = match query.status.as_deref() {
            Some(status) => current_note_ids_with_status(status),
            None => current_note_ids(),
        };
        if let Some((kind, id)) = query.reference.as_ref() {
            current_candidate_ids = note_ids_matching_ref(kind, id, Some(current_candidate_ids));
        }
        if query.status.is_some() || query.reference.is_some() {
            select = select.filter(
                Condition::any()
                    .add(note::Column::Id.in_subquery(current_candidate_ids))
                    .add(note::Column::Id.not_in_subquery(current_note_ids())),
            );
        }

        select = select
            .order_by_desc(note::Column::UpdatedAt)
            .order_by_desc(note::Column::Id);
        if let Some(limit) = query.limit {
            select = select.limit(limit);
        }
        let rows = select.all(&self.db).await.map_err(query_error)?;
        let Some(last) = rows.last() else {
            return Ok(NoteListPage {
                notes: Vec::new(),
                cursor: None,
                has_more: false,
            });
        };
        let cursor = NoteListCursor {
            updated_at: last.updated_at,
            note_id: last.id,
        };
        let has_more = query.limit.is_some_and(|limit| rows.len() as u64 == limit);
        let ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
        let mut versions = find_current_versions(&self.db, &ids).await?;
        let pending_ids = ids
            .iter()
            .filter(|id| !versions.contains_key(id))
            .copied()
            .collect::<Vec<_>>();
        versions.extend(find_latest_versions(&self.db, &pending_ids).await?);

        let mut matching_rows = Vec::new();
        for row in rows {
            let version = versions.remove(&row.id).ok_or_else(|| {
                NoteReadQueryError::InvalidData(format!(
                    "note {} has no current or latest version",
                    row.id
                ))
            })?;
            if query
                .status
                .as_deref()
                .is_some_and(|status| version.status != status)
            {
                continue;
            }
            if let Some((kind, id)) = query.reference.as_ref()
                && !version.is_current
            {
                match version_matches_reference(&version, kind, id) {
                    Ok(true) => {}
                    Ok(false) => continue,
                    Err(error) => {
                        tracing::warn!(
                            note_id = %row.id,
                            version_id = %version.id,
                            %error,
                            "skipping pending note version with unparsable references"
                        );
                        continue;
                    }
                }
            }
            matching_rows.push((row, version));
        }

        let selected_versions = matching_rows
            .iter()
            .map(|(row, version)| (row.id, version.clone()))
            .collect();
        let notes = self
            .snapshots(
                matching_rows.into_iter().map(|(row, _)| row).collect(),
                selected_versions,
            )
            .await?;
        Ok(NoteListPage {
            notes,
            cursor: Some(cursor),
            has_more,
        })
    }
}
fn current_note_ids() -> sea_orm::sea_query::SelectStatement {
    note_version::Entity::find()
        .select_only()
        .column(note_version::Column::NoteId)
        .filter(note_version::Column::IsCurrent.eq(true))
        .into_query()
}

fn current_note_ids_with_status(status: &str) -> sea_orm::sea_query::SelectStatement {
    note_version::Entity::find()
        .select_only()
        .column(note_version::Column::NoteId)
        .filter(note_version::Column::IsCurrent.eq(true))
        .filter(note_version::Column::Status.eq(status))
        .into_query()
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

fn version_matches_reference(
    version: &note_version::Model,
    kind: &str,
    id: &str,
) -> Result<bool, String> {
    let graphs: Vec<GraphDef> = serde_json::from_value(version.graphs_json.clone())
        .map_err(|error| format!("failed to deserialize note_version.graphs_json: {error}"))?;
    collect_note_refs_with_policy(
        &version.body_md,
        &graphs,
        BodyTokenPolicy::AllowLegacyBodyTokens,
    )
    .map(|references| {
        references
            .iter()
            .any(|reference| reference.0 == kind && reference.1 == id)
    })
    .map_err(|errors| {
        format!(
            "failed to collect references from a pending note version: {}",
            format_note_token_errors(&errors)
        )
    })
}

#[cfg(test)]
mod tests {
    use core_application::note::{NoteListQuery, NoteReadQuery};
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{ActiveModelTrait, EntityTrait};
    use uuid::Uuid;

    use super::super::PostgresNoteReadQuery;
    use crate::DatabaseHandle;
    use crate::entities::{note, note_version, strategy};

    #[backend_test_macros::database_test]
    async fn list_without_strategy_filter_includes_other_strategies_and_unscoped_notes(
        db: DatabaseHandle,
    ) {
        let strategy_a = insert_strategy(&db, "strategy-a").await;
        let strategy_b = insert_strategy(&db, "strategy-b").await;
        let strategy_note_id = insert_note(&db, Some(strategy_a)).await;
        let other_strategy_note_id = insert_note(&db, Some(strategy_b)).await;
        let unscoped_note_id = insert_note(&db, None).await;
        let query = PostgresNoteReadQuery::new(db);

        let current = query
            .list_notes(NoteListQuery {
                strategy_id: None,
                ..Default::default()
            })
            .await
            .expect("list current notes without a strategy filter");
        let including_pending = query
            .list_notes(NoteListQuery {
                strategy_id: None,
                include_pending: true,
                ..Default::default()
            })
            .await
            .expect("list notes including pending without a strategy filter");

        let normalize = |notes: Vec<core_application::note::NoteSnapshot>| {
            let mut notes = notes
                .into_iter()
                .map(|snapshot| (snapshot.note.id, snapshot.note.strategy_id))
                .collect::<Vec<_>>();
            notes.sort_by_key(|(id, _)| *id);
            notes
        };
        let expected = {
            let mut notes = vec![
                (strategy_note_id, Some(strategy_a)),
                (other_strategy_note_id, Some(strategy_b)),
                (unscoped_note_id, None),
            ];
            notes.sort_by_key(|(id, _)| *id);
            notes
        };

        assert_eq!(
            (normalize(current.notes), normalize(including_pending.notes)),
            (expected.clone(), expected),
        );
    }

    async fn insert_strategy(db: &DatabaseHandle, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(id),
            name: Set(name.to_string()),
            description: Set(None),
            sort_order: Set(0),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert test strategy");
        id
    }

    async fn insert_note(db: &DatabaseHandle, strategy_id: Option<Uuid>) -> Uuid {
        let note_id = Uuid::new_v4();
        note::Entity::insert(note::ActiveModel {
            id: Set(note_id),
            strategy_id: Set(strategy_id),
            kind: Set(None),
            trigger: Set(None),
            trigger_label: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
            execution_id: Set(None),
        })
        .exec_without_returning(db)
        .await
        .expect("insert test note");
        note_version::Entity::insert(note_version::ActiveModel {
            id: Set(Uuid::new_v4()),
            note_id: Set(note_id),
            version_no: Set(1),
            title: Set("sample note".to_string()),
            body_md: Set("sample body".to_string()),
            frontmatter_json: Set(serde_json::json!({})),
            graphs_json: Set(serde_json::json!([])),
            status: Set("approved".to_string()),
            is_current: Set(true),
            change_reason: Set(None),
            created_by_kind: Set("human".to_string()),
            execution_id: Set(None),
            created_at: NotSet,
            reviewed_at: Set(None),
        })
        .exec_without_returning(db)
        .await
        .expect("insert test note version");
        note_id
    }
}
