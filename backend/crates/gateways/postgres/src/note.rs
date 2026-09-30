use async_trait::async_trait;
use core_application::note::{
    NewNote, NewNoteLink, NewNoteVersion, Note, NoteLinkTarget, NoteMetadataUpdate, NoteRepository,
    NoteRepositoryError, NoteVersion, NoteVersionUpdate,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{NotSet, Set, Unchanged};
use sea_orm::sea_query::{Expr, ExprTrait, OnConflict};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::entities::{note, note_kind, note_link, note_ref, note_version};
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref;

#[derive(Clone, Copy, Default)]
pub struct PostgresNoteRepository;

impl PostgresNoteRepository {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NoteRepository for PostgresNoteRepository {
    async fn find_note(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<Option<Note>, NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        note::Entity::find_by_id(note_id)
            .one(transaction)
            .await
            .map(|row| row.map(to_note))
            .map_err(repository_error)
    }

    async fn find_note_by_execution_id(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
        execution_id: &str,
    ) -> Result<Option<Note>, NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        note::Entity::find()
            .filter(note::Column::StrategyId.eq(strategy_id))
            .filter(note::Column::ExecutionId.eq(execution_id))
            .one(transaction)
            .await
            .map(|row| row.map(to_note))
            .map_err(repository_error)
    }

    async fn find_note_kind_requires_approval(
        &self,
        transaction: &UnitOfWorkTransaction,
        kind: &str,
    ) -> Result<Option<bool>, NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        note_kind::Entity::find_by_id(kind.to_string())
            .one(transaction)
            .await
            .map(|row| row.map(|row| row.requires_approval))
            .map_err(repository_error)
    }

    async fn find_current_version(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<Option<NoteVersion>, NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        note_version::Entity::find()
            .filter(note_version::Column::NoteId.eq(note_id))
            .filter(note_version::Column::IsCurrent.eq(true))
            .one(transaction)
            .await
            .map(|row| row.map(to_version))
            .map_err(repository_error)
    }

    async fn find_latest_version(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<Option<NoteVersion>, NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        note_version::Entity::find()
            .filter(note_version::Column::NoteId.eq(note_id))
            .order_by_desc(note_version::Column::VersionNo)
            .one(transaction)
            .await
            .map(|row| row.map(to_version))
            .map_err(repository_error)
    }

    async fn find_latest_pending_versions_by_kind(
        &self,
        transaction: &UnitOfWorkTransaction,
        kind: &str,
    ) -> Result<Vec<NoteVersion>, NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        let note_ids = note::Entity::find()
            .select_only()
            .column(note::Column::Id)
            .filter(note::Column::Kind.eq(kind))
            .into_tuple::<Uuid>()
            .all(transaction)
            .await
            .map_err(repository_error)?;
        if note_ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = note_version::Entity::find()
            .filter(note_version::Column::NoteId.is_in(note_ids))
            .filter(note_version::Column::Status.eq("unread"))
            .order_by_asc(note_version::Column::NoteId)
            .order_by_desc(note_version::Column::VersionNo)
            .all(transaction)
            .await
            .map_err(repository_error)?;
        let mut seen_note_ids = std::collections::HashSet::new();
        Ok(rows
            .into_iter()
            .filter(|row| seen_note_ids.insert(row.note_id))
            .map(to_version)
            .collect())
    }

    async fn find_version_by_number(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
        version_no: i32,
    ) -> Result<Option<NoteVersion>, NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        note_version::Entity::find()
            .filter(note_version::Column::NoteId.eq(note_id))
            .filter(note_version::Column::VersionNo.eq(version_no))
            .one(transaction)
            .await
            .map(|row| row.map(to_version))
            .map_err(repository_error)
    }

    async fn find_initial_created_by_kind(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<Option<String>, NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        note_version::Entity::find()
            .filter(note_version::Column::NoteId.eq(note_id))
            .order_by_asc(note_version::Column::VersionNo)
            .one(transaction)
            .await
            .map(|row| row.map(|row| row.created_by_kind))
            .map_err(repository_error)
    }

    async fn insert_note(
        &self,
        transaction: &UnitOfWorkTransaction,
        note: NewNote,
    ) -> Result<Option<Note>, NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        let has_execution_id = note.execution_id.is_some();
        let model = note::ActiveModel {
            id: Set(note.id),
            strategy_id: Set(note.strategy_id),
            kind: Set(note.kind),
            trigger: Set(note.trigger),
            trigger_label: Set(note.trigger_label),
            created_at: NotSet,
            updated_at: NotSet,
            execution_id: Set(note.execution_id),
        };
        let result = if has_execution_id {
            // 部分 unique index の predicate と一致しないと、Postgres が conflict target に選べない。
            note::Entity::insert(model)
                .on_conflict(
                    OnConflict::columns([note::Column::StrategyId, note::Column::ExecutionId])
                        .target_and_where(Expr::col(note::Column::ExecutionId).is_not_null())
                        .do_nothing()
                        .to_owned(),
                )
                .exec_with_returning(transaction)
                .await
        } else {
            note::Entity::insert(model)
                .exec_with_returning(transaction)
                .await
        };
        match result {
            Ok(row) => Ok(Some(to_note(row))),
            // SeaORM 2.0 は DO NOTHING 後の空 RETURNING をこの 2 種類で返す。
            Err(sea_orm::DbErr::RecordNotInserted | sea_orm::DbErr::RecordNotFound(_))
                if has_execution_id =>
            {
                Ok(None)
            }
            Err(error) => Err(repository_error(error)),
        }
    }

    async fn update_note(
        &self,
        transaction: &UnitOfWorkTransaction,
        update: NoteMetadataUpdate,
    ) -> Result<Note, NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        let updated = note::ActiveModel {
            id: Unchanged(update.id),
            kind: active_value(update.kind),
            trigger: active_value(update.trigger),
            trigger_label: active_value(update.trigger_label),
            updated_at: update.updated_at.map_or(NotSet, Set),
            ..Default::default()
        }
        .update(transaction)
        .await
        .map_err(repository_error)?;
        Ok(to_note(updated))
    }

    async fn delete_note(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<bool, NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        note::Entity::delete_by_id(note_id)
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }

    async fn insert_version(
        &self,
        transaction: &UnitOfWorkTransaction,
        version: NewNoteVersion,
    ) -> Result<NoteVersion, NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        note_version::Entity::insert(note_version::ActiveModel {
            id: Set(version.id),
            note_id: Set(version.note_id),
            version_no: Set(version.version_no),
            title: Set(version.title),
            body_md: Set(version.body_md),
            frontmatter_json: Set(version.frontmatter_json),
            graphs_json: Set(version.graphs_json),
            status: Set(version.status),
            is_current: Set(version.is_current),
            change_reason: Set(version.change_reason),
            created_by_kind: Set(version.created_by_kind),
            execution_id: Set(version.execution_id),
            created_at: NotSet,
            reviewed_at: Set(None),
        })
        .exec_with_returning(transaction)
        .await
        .map(to_version)
        .map_err(repository_error)
    }

    async fn update_version(
        &self,
        transaction: &UnitOfWorkTransaction,
        update: NoteVersionUpdate,
    ) -> Result<NoteVersion, NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        let updated = note_version::ActiveModel {
            id: Unchanged(update.id),
            is_current: update.is_current.map_or(NotSet, Set),
            status: update.status.map_or(NotSet, Set),
            reviewed_at: update
                .reviewed_at
                .map(|value| Set(Some(value)))
                .unwrap_or(NotSet),
            ..Default::default()
        }
        .update(transaction)
        .await
        .map_err(repository_error)?;
        Ok(to_version(updated))
    }

    async fn update_note_timestamp(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
        updated_at: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<(), NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        note::ActiveModel {
            id: Unchanged(note_id),
            updated_at: Set(updated_at),
            ..Default::default()
        }
        .update(transaction)
        .await
        .map(|_| ())
        .map_err(repository_error)
    }

    async fn replace_references(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
        references: Vec<(String, String)>,
    ) -> Result<(), NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        note_ref::Entity::delete_many()
            .filter(note_ref::Column::NoteId.eq(note_id))
            .exec(transaction)
            .await
            .map_err(repository_error)?;
        if references.is_empty() {
            return Ok(());
        }
        let models = references
            .into_iter()
            .map(|(kind, id)| note_ref::ActiveModel {
                note_id: Set(note_id),
                ref_kind: Set(kind),
                ref_id: Set(id),
            });
        note_ref::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([
                    note_ref::Column::NoteId,
                    note_ref::Column::RefKind,
                    note_ref::Column::RefId,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec_without_returning(transaction)
            .await
            .map_err(repository_error)?;
        Ok(())
    }

    async fn find_note_link_targets(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_ids: &[Uuid],
    ) -> Result<Vec<NoteLinkTarget>, NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        if note_ids.is_empty() {
            return Ok(Vec::new());
        }
        let notes = note::Entity::find()
            .filter(note::Column::Id.is_in(note_ids.iter().copied()))
            .all(transaction)
            .await
            .map_err(repository_error)?;
        let versions = note_version::Entity::find()
            .filter(note_version::Column::NoteId.is_in(note_ids.iter().copied()))
            .filter(note_version::Column::IsCurrent.eq(true))
            .all(transaction)
            .await
            .map_err(repository_error)?;
        let current_versions = versions
            .into_iter()
            .map(|version| (version.note_id, version.id))
            .collect::<std::collections::HashMap<_, _>>();
        Ok(notes
            .into_iter()
            .map(|note| NoteLinkTarget {
                id: note.id,
                strategy_id: note.strategy_id,
                current_version_id: current_versions.get(&note.id).copied(),
            })
            .collect())
    }

    async fn insert_note_links(
        &self,
        transaction: &UnitOfWorkTransaction,
        links: Vec<NewNoteLink>,
    ) -> Result<(), NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        if links.is_empty() {
            return Ok(());
        }
        note_link::Entity::insert_many(links.into_iter().map(|link| note_link::ActiveModel {
            from_version_id: Set(link.from_version_id),
            to_note_id: Set(link.to_note_id),
            to_version_id: Set(link.to_version_id),
        }))
        .exec_without_returning(transaction)
        .await
        .map_err(repository_error)?;
        Ok(())
    }

    async fn copy_note_links(
        &self,
        transaction: &UnitOfWorkTransaction,
        from_version_id: Uuid,
        to_version_id: Uuid,
    ) -> Result<(), NoteRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteRepositoryError::InvalidTransaction)?;
        let links = note_link::Entity::find()
            .filter(note_link::Column::FromVersionId.eq(from_version_id))
            .all(transaction)
            .await
            .map_err(repository_error)?;
        if links.is_empty() {
            return Ok(());
        }
        note_link::Entity::insert_many(links.into_iter().map(|link| note_link::ActiveModel {
            from_version_id: Set(to_version_id),
            to_note_id: Set(link.to_note_id),
            to_version_id: Set(link.to_version_id),
        }))
        .exec_without_returning(transaction)
        .await
        .map_err(repository_error)?;
        Ok(())
    }
}

fn active_value(value: Option<Option<String>>) -> sea_orm::ActiveValue<Option<String>> {
    value.map_or(NotSet, Set)
}

fn repository_error(error: sea_orm::DbErr) -> NoteRepositoryError {
    NoteRepositoryError::Database(persistence_error(error))
}

fn to_note(model: note::Model) -> Note {
    Note {
        id: model.id,
        strategy_id: model.strategy_id,
        kind: model.kind,
        trigger: model.trigger,
        trigger_label: model.trigger_label,
        created_at: model.created_at,
        updated_at: model.updated_at,
        execution_id: model.execution_id,
    }
}

fn to_version(model: note_version::Model) -> NoteVersion {
    NoteVersion {
        id: model.id,
        note_id: model.note_id,
        version_no: model.version_no,
        title: model.title,
        body_md: model.body_md,
        frontmatter_json: model.frontmatter_json,
        graphs_json: model.graphs_json,
        status: model.status,
        is_current: model.is_current,
        change_reason: model.change_reason,
        created_by_kind: model.created_by_kind,
        execution_id: model.execution_id,
        created_at: model.created_at,
        reviewed_at: model.reviewed_at,
    }
}

#[cfg(test)]
mod tests {
    use core_application::note::{NewNote, NewNoteVersion, NoteRepository, NoteVersion};
    use core_application::unit_of_work::{UnitOfWork, UnitOfWorkTransaction};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use uuid::Uuid;

    use super::PostgresNoteRepository;

    #[backend_test_macros::database_test]
    async fn find_latest_pending_versions_by_kind_returns_latest_unread_per_note(
        db: crate::DatabaseHandle,
    ) {
        let strategy_id = Uuid::new_v4();
        crate::entities::strategy::ActiveModel {
            id: Set(strategy_id),
            name: Set("sample strategy".to_string()),
            description: Set(None),
            sort_order: Set(0),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(&db)
        .await
        .expect("insert test strategy");
        for key in ["sample-kind", "other-kind"] {
            crate::entities::note_kind::ActiveModel {
                key: Set(key.to_string()),
                display_name: Set("Sample Kind".to_string()),
                requires_approval: Set(true),
                description: Set(None),
                sort_order: Set(0),
            }
            .insert(&db)
            .await
            .expect("insert test note kind");
        }

        let repository = PostgresNoteRepository::new();
        let unit_of_work = crate::unit_of_work::PostgresUnitOfWork::new(db);
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        let first_note_id = Uuid::new_v4();
        let second_note_id = Uuid::new_v4();
        let other_note_id = Uuid::new_v4();
        for (id, kind) in [
            (first_note_id, "sample-kind"),
            (second_note_id, "sample-kind"),
            (other_note_id, "other-kind"),
        ] {
            insert_note(&repository, &transaction, id, strategy_id, kind).await;
        }

        insert_version(&repository, &transaction, first_note_id, 1, "unread", false).await;
        let first_note_pending =
            insert_version(&repository, &transaction, first_note_id, 2, "unread", false).await;
        insert_version(
            &repository,
            &transaction,
            first_note_id,
            3,
            "approved",
            true,
        )
        .await;
        insert_version(
            &repository,
            &transaction,
            second_note_id,
            1,
            "unread",
            false,
        )
        .await;
        let second_note_pending = insert_version(
            &repository,
            &transaction,
            second_note_id,
            2,
            "unread",
            false,
        )
        .await;
        insert_version(&repository, &transaction, other_note_id, 1, "unread", false).await;

        let actual = repository
            .find_latest_pending_versions_by_kind(&transaction, "sample-kind")
            .await
            .expect("find pending versions");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");

        let mut expected = vec![first_note_pending, second_note_pending];
        expected.sort_by_key(|version| version.note_id);
        assert_eq!(actual, expected);
    }

    #[backend_test_macros::database_test]
    async fn insert_note_returns_none_when_execution_id_conflicts(db: crate::DatabaseHandle) {
        let strategy_id = Uuid::new_v4();
        crate::entities::strategy::ActiveModel {
            id: Set(strategy_id),
            name: Set("sample strategy".to_string()),
            description: Set(None),
            sort_order: Set(0),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(&db)
        .await
        .expect("insert test strategy");

        let repository = PostgresNoteRepository::new();
        let unit_of_work = crate::unit_of_work::PostgresUnitOfWork::new(db);
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        let existing_note_id = Uuid::new_v4();
        let execution_id = "sample-execution";
        let first = repository
            .insert_note(
                &transaction,
                NewNote {
                    id: existing_note_id,
                    strategy_id: Some(strategy_id),
                    kind: None,
                    trigger: None,
                    trigger_label: None,
                    execution_id: Some(execution_id.into()),
                },
            )
            .await
            .expect("insert first note")
            .map(|note| note.id);
        let conflict = repository
            .insert_note(
                &transaction,
                NewNote {
                    id: Uuid::new_v4(),
                    strategy_id: Some(strategy_id),
                    kind: None,
                    trigger: None,
                    trigger_label: None,
                    execution_id: Some(execution_id.into()),
                },
            )
            .await
            .expect("execution id conflict is not a repository error")
            .map(|note| note.id);
        let existing = repository
            .find_note_by_execution_id(&transaction, strategy_id, execution_id)
            .await
            .expect("find first note")
            .map(|note| note.id);

        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");

        assert_eq!(
            (first, conflict, existing),
            (Some(existing_note_id), None, Some(existing_note_id),),
        );
    }

    async fn insert_note(
        repository: &PostgresNoteRepository,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
        strategy_id: Uuid,
        kind: &str,
    ) {
        repository
            .insert_note(
                transaction,
                NewNote {
                    id,
                    strategy_id: Some(strategy_id),
                    kind: Some(kind.to_string()),
                    trigger: None,
                    trigger_label: None,
                    execution_id: None,
                },
            )
            .await
            .expect("insert test note")
            .expect("test note inserted");
    }

    async fn insert_version(
        repository: &PostgresNoteRepository,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
        version_no: i32,
        status: &str,
        is_current: bool,
    ) -> NoteVersion {
        repository
            .insert_version(
                transaction,
                NewNoteVersion {
                    id: Uuid::new_v4(),
                    note_id,
                    version_no,
                    title: format!("Sample version {version_no}"),
                    body_md: "Sample body".to_string(),
                    frontmatter_json: serde_json::json!({}),
                    graphs_json: serde_json::json!([]),
                    status: status.to_string(),
                    is_current,
                    change_reason: None,
                    created_by_kind: "human".to_string(),
                    execution_id: None,
                },
            )
            .await
            .expect("insert test note version")
    }
}
