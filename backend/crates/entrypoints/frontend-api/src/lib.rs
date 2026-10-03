pub mod error;
pub mod extractors;
pub mod handlers;
pub mod models;
pub(crate) mod serde_helpers;
pub mod services;

pub use error::{AppError, ErrorResponse};
pub use handlers::state::AppState;
pub type FrontendApiState = AppState;

pub(crate) mod kata_exec {
    pub use core_application::kata_exec::{ExecRequest, KataExecError, SharedKataExecutor};
}
