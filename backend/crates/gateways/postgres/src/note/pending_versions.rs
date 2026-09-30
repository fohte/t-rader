use core_application::note::{NoteRepositoryError, NoteVersion};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use uuid::Uuid;

use crate::entities::{note, note_version};
use crate::transaction::transaction_ref;

use super::{repository_error, to_version};

pub async fn supersede_pending_versions_before(
    transaction: &impl ConnectionTrait,
    note_id: Uuid,
    version_no: i32,
) -> Result<Vec<Uuid>, DbErr> {
    let mut superseded_ids = note_version::Entity::update_many()
        .col_expr(note_version::Column::Status, Expr::value("superseded"))
        .filter(note_version::Column::NoteId.eq(note_id))
        .filter(note_version::Column::VersionNo.lt(version_no))
        .filter(note_version::Column::Status.eq("unread"))
        .exec_with_returning(transaction)
        .await?
        .into_iter()
        .map(|version| version.id)
        .collect::<Vec<_>>();
    superseded_ids.sort_unstable();
    Ok(superseded_ids)
}

pub(super) async fn find_latest_pending_versions_by_kind(
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
