use async_trait::async_trait;
use core_application::note_status_change_aggregate::{
    NoteStatusChangeAggregateQuery, NoteStatusChangeAggregateQueryError, NoteStatusChangeCounts,
    NoteStatusChangePeriod,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

use crate::DatabaseHandle;
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresNoteStatusChangeAggregateQuery {
    db: DatabaseHandle,
}

impl PostgresNoteStatusChangeAggregateQuery {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl NoteStatusChangeAggregateQuery for PostgresNoteStatusChangeAggregateQuery {
    async fn count(
        &self,
        period: NoteStatusChangePeriod,
    ) -> Result<NoteStatusChangeCounts, NoteStatusChangeAggregateQueryError> {
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                NOTE_STATUS_CHANGE_COUNTS_SQL,
                [period.from.into(), period.to.into()],
            ))
            .await
            .map_err(query_error)?;
        let Some(row) = row else {
            return Ok(NoteStatusChangeCounts::default());
        };

        Ok(NoteStatusChangeCounts {
            approved: row.try_get("", "approved_count").map_err(query_error)?,
            rejected: row.try_get("", "rejected_count").map_err(query_error)?,
        })
    }
}

const NOTE_STATUS_CHANGE_COUNTS_SQL: &str = r#"
    SELECT
        COUNT(*) FILTER (WHERE diff_json ->> 'to' = 'approved') AS approved_count,
        COUNT(*) FILTER (WHERE diff_json ->> 'to' = 'rejected') AS rejected_count
    FROM public.change_history
    WHERE target_kind = 'note'
        AND op = 'status_change'
        AND created_at >= $1
        AND created_at < $2
"#;

fn query_error(error: sea_orm::DbErr) -> NoteStatusChangeAggregateQueryError {
    NoteStatusChangeAggregateQueryError::Database(persistence_error(error))
}
