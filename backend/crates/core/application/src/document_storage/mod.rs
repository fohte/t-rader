mod error;
#[cfg(feature = "test-support")]
mod fake;
mod repository;
mod types;
mod use_cases;

pub use error::DocumentStorageUseCaseError;
#[cfg(feature = "test-support")]
pub use fake::FakeDocumentStorageRepository;
pub use repository::{
    DocumentStorageRepository, DocumentStorageRepositoryError, SharedDocumentStorageRepository,
};
pub use types::{
    Document, DocumentChunk, DocumentChunkContext, DocumentChunkInput, DocumentMetadata,
    DocumentSnapshot, DocumentVersion,
};
pub use use_cases::DocumentStorageUseCases;
