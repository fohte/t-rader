use std::sync::Arc;

use core_application::note_kind::NoteKindUseCases;
use gateway_postgres::PostgresNoteKindRepository;

use super::UseCases;

impl UseCases {
    pub fn note_kinds(&self) -> NoteKindUseCases {
        NoteKindUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresNoteKindRepository::new(self.db.clone())),
            self.change_history.clone(),
            self.notes(),
        )
    }
}
