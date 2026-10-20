use std::sync::Arc;

use core_application::markdown_lint::MarkdownLintService;
use core_application::note::{NoteReadUseCases, NoteUseCases};
use gateway_postgres::{PostgresNoteReadQuery, PostgresNoteRepository};

use super::UseCases;

impl UseCases {
    pub fn note_reads(&self) -> NoteReadUseCases {
        NoteReadUseCases::with_markdown_lint(
            Arc::new(PostgresNoteReadQuery::new(self.db.clone())),
            MarkdownLintService::new(self.refs()),
        )
    }

    pub fn notes(&self) -> NoteUseCases {
        NoteUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresNoteRepository::new()),
            self.change_history.clone(),
            self.bars(),
            self.valuations(),
            self.indicator_observations(),
            self.custom_indicator_evaluations(),
            MarkdownLintService::new(self.refs()),
        )
    }
}
