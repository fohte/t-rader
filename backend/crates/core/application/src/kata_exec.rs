mod error;
mod types;

#[cfg(feature = "test-support")]
mod fake;

pub use error::KataExecError;
pub use types::{ExecFile, ExecRequest, ExecResult, KataExecutor, SharedKataExecutor};

#[cfg(feature = "test-support")]
pub use fake::FakeKataExecutor;
