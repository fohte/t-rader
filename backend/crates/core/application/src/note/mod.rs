mod create_update;
mod error;
mod query;
mod read;
mod repository;
mod strategy_write;
mod types;
mod use_cases;
mod version_review;
mod version_write;

pub const INITIAL_NOTE_STATUS: &str = "unread";

pub use error::NoteUseCaseError;
pub use query::{NoteReadQuery, NoteReadQueryError, SharedNoteReadQuery};
pub use read::{NoteReadUseCaseError, NoteReadUseCases};
pub use repository::{NoteRepository, NoteRepositoryError, SharedNoteRepository};
pub use types::{
    NewNote, NewNoteLink, NewNoteVersion, Note, NoteLink, NoteLinkTarget, NoteLinkView, NoteLinks,
    NoteListCursor, NoteListPage, NoteListQuery, NoteMetadataUpdate, NoteSnapshot, NoteVersion,
    NoteVersionUpdate, NoteWriteCommand, NoteWriteResult, UpdateNoteCommand,
};
pub use use_cases::NoteUseCases;
