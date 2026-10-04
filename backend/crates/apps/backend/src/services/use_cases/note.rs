use std::sync::Arc;

use core_application::note::{NoteReadUseCases, NoteUseCases};
use gateway_postgres::{
    PostgresNoteReadQuery, PostgresNoteRepository, PostgresStrategyTaskStepEvidenceRepository,
};

use super::UseCases;

impl UseCases {
    pub fn note_reads(&self) -> NoteReadUseCases {
        NoteReadUseCases::new(Arc::new(PostgresNoteReadQuery::new(self.db.clone())))
    }

    pub fn notes(&self) -> NoteUseCases {
        NoteUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresNoteRepository::new()),
            self.change_history.clone(),
            Arc::new(PostgresStrategyTaskStepEvidenceRepository::new(
                self.db.clone(),
            )),
        )
    }
}
