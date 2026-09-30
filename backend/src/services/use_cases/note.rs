use std::sync::Arc;

use core_application::note::NoteUseCases;
use gateway_postgres::PostgresNoteRepository;

use super::UseCases;

impl UseCases {
    pub fn notes(&self) -> NoteUseCases {
        NoteUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresNoteRepository::new()),
            self.strategy_existence.clone(),
            self.change_history.clone(),
        )
    }
}
