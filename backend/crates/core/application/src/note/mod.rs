mod create_update;
mod error;
mod repository;
mod strategy_write;
mod types;
mod use_cases;
mod version_review;
mod version_write;

pub use error::NoteUseCaseError;
pub use repository::{NoteRepository, NoteRepositoryError, SharedNoteRepository};
pub use types::{
    NewNote, NewNoteLink, NewNoteVersion, Note, NoteLinkTarget, NoteMetadataUpdate, NoteSnapshot,
    NoteVersion, NoteVersionUpdate, NoteWriteCommand, NoteWriteResult, UpdateNoteCommand,
};
pub use use_cases::NoteUseCases;
