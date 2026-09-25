mod agent_task_client;
mod kata_exec;

pub use agent_task_client::{
    AgentTaskClient, AgentTaskError, AgentTaskRef, AgentTaskState, AgentTaskStatus,
    DisabledAgentTaskClient, EXECUTION_LOST_ERROR_KIND, SharedAgentTaskClient, SubmitAgentTask,
};
pub use kata_exec::{ExecRequest, ExecResult, KataExecError, KataExecutor, SharedKataExecutor};

#[cfg(feature = "test-support")]
pub use kata_exec::FakeKataExecutor;

#[cfg(feature = "test-support")]
pub use agent_task_client::FakeAgentTaskClient;
