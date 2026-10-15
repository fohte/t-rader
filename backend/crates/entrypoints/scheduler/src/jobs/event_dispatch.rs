use core_application::event_dispatch_job::EVENT_DISPATCH_JOB_IDENTIFIER;
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, run_with_state};

#[derive(Debug, Deserialize, Serialize)]
#[serde(from = "Option<EmptyObject>")]
pub struct EventDispatch;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyObject {}

impl From<Option<EmptyObject>> for EventDispatch {
    fn from(_payload: Option<EmptyObject>) -> Self {
        Self
    }
}

impl TaskHandler for EventDispatch {
    const IDENTIFIER: &'static str = EVENT_DISPATCH_JOB_IDENTIFIER;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let (dispatched, deleted) = state
                    .dependencies
                    .event_dispatch
                    .dispatch_and_cleanup(state.dependencies.agent_task_client.as_ref())
                    .await
                    .map_err(|error| error.to_string())?;
                if dispatched > 0 {
                    tracing::info!(dispatched, "pending events dispatched");
                }
                if deleted > 0 {
                    tracing::info!(deleted, "expired events deleted");
                }
                Ok(())
            },
        )
        .await
    }
}
