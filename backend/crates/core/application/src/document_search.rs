mod repository;
mod types;
mod use_cases;

pub use repository::{
    DocumentSearchRepository, DocumentSearchRepositoryError, SharedDocumentSearchRepository,
};
pub use types::{DocumentSearchCriteria, DocumentSearchResult};
pub use use_cases::DocumentSearchUseCases;
