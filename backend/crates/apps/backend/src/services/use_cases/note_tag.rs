use std::sync::Arc;

use core_application::note_tag::NoteTagUseCases;
use gateway_postgres::PostgresNoteTagRepository;

use super::UseCases;

impl UseCases {
    pub fn note_tags(&self) -> NoteTagUseCases {
        NoteTagUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresNoteTagRepository::new(self.db.clone())),
            self.change_history.clone(),
        )
    }
}
