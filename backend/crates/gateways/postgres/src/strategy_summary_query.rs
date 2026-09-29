use std::collections::HashMap;

use async_trait::async_trait;
use core_application::strategy::{
    StrategySummary, StrategySummaryQuery, StrategySummaryQueryError,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::{annotation, note, note_version, strategy};
use crate::persistence::persistence_error;

const UNREAD_STATUS: &str = "unread";

#[derive(Clone)]
pub struct PostgresStrategySummaryQuery {
    db: DatabaseHandle,
}

impl PostgresStrategySummaryQuery {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl StrategySummaryQuery for PostgresStrategySummaryQuery {
    async fn list(&self) -> Result<Vec<StrategySummary>, StrategySummaryQueryError> {
        let strategies = strategy::Entity::find()
            .order_by_asc(strategy::Column::SortOrder)
            .order_by_asc(strategy::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(query_error)?;
        let note_counts = unread_note_counts_by_strategy(&self.db).await?;
        let annotation_counts = unread_annotation_counts_by_strategy(&self.db).await?;

        Ok(strategies
            .into_iter()
            .map(|row| StrategySummary {
                id: row.id,
                name: row.name,
                updated_at: row.updated_at,
                unread_card_count: note_counts.get(&row.id).copied().unwrap_or(0)
                    + annotation_counts.get(&row.id).copied().unwrap_or(0),
            })
            .collect())
    }
}

async fn unread_note_counts_by_strategy(
    db: &DatabaseHandle,
) -> Result<HashMap<Uuid, u64>, StrategySummaryQueryError> {
    let current_unread_note_ids = note_version::Entity::find()
        .select_only()
        .column(note_version::Column::NoteId)
        .filter(note_version::Column::IsCurrent.eq(true))
        .filter(note_version::Column::Status.eq(UNREAD_STATUS))
        .into_query();
    let rows: Vec<(Uuid, i64)> = note::Entity::find()
        .select_only()
        .column(note::Column::StrategyId)
        .column_as(note::Column::Id.count(), "unread_count")
        .filter(note::Column::Id.in_subquery(current_unread_note_ids))
        .filter(note::Column::StrategyId.is_not_null())
        .group_by(note::Column::StrategyId)
        .into_tuple()
        .all(db)
        .await
        .map_err(query_error)?;
    Ok(rows
        .into_iter()
        .map(|(id, count)| (id, count.max(0) as u64))
        .collect())
}

async fn unread_annotation_counts_by_strategy(
    db: &DatabaseHandle,
) -> Result<HashMap<Uuid, u64>, StrategySummaryQueryError> {
    let rows: Vec<(Uuid, i64)> = annotation::Entity::find()
        .select_only()
        .column(annotation::Column::StrategyId)
        .column_as(annotation::Column::Id.count(), "unread_count")
        .filter(annotation::Column::Status.eq(UNREAD_STATUS))
        .filter(annotation::Column::StrategyId.is_not_null())
        .group_by(annotation::Column::StrategyId)
        .into_tuple()
        .all(db)
        .await
        .map_err(query_error)?;
    Ok(rows
        .into_iter()
        .map(|(id, count)| (id, count.max(0) as u64))
        .collect())
}

fn query_error(error: sea_orm::DbErr) -> StrategySummaryQueryError {
    StrategySummaryQueryError::Database(persistence_error(error))
}
