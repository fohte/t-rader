mod error;
mod ports;
mod use_cases;

pub use error::AnnotationUseCaseError;
pub use ports::{
    Annotation, AnnotationRepository, AnnotationRepositoryError, ChangeAnnotationStatusCommand,
    CreateAnnotationCommand, DeleteAnnotationCommand, NewAnnotation, SharedAnnotationRepository,
    UpdateAnnotationCommand,
};
pub use use_cases::AnnotationUseCases;

#[cfg(feature = "test-support")]
mod fake;

#[cfg(feature = "test-support")]
pub use fake::FakeAnnotationRepository;

#[cfg(all(test, feature = "test-support"))]
mod tests;
