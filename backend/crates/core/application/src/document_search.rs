mod repository;
mod types;
mod use_cases;

pub use repository::{
    DocumentSearchRepository, DocumentSearchRepositoryError, SharedDocumentSearchRepository,
};
pub use types::{DocumentSearchCriteria, DocumentSearchResult, DocumentSearchVersion};
pub use use_cases::{DocumentSearchUseCaseError, DocumentSearchUseCases};
