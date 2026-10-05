use std::sync::Arc;

use core_application::note_status_change_aggregate::NoteStatusChangeAggregateUseCases;
use gateway_postgres::PostgresNoteStatusChangeAggregateQuery;

use super::UseCases;

impl UseCases {
    pub fn note_status_change_aggregate(&self) -> NoteStatusChangeAggregateUseCases {
        NoteStatusChangeAggregateUseCases::new(Arc::new(
            PostgresNoteStatusChangeAggregateQuery::new(self.db.clone()),
        ))
    }
}
