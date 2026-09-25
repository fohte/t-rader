mod agent_task_client;
mod indicator_observation_source;
mod kata_exec;
mod news_aggregator;

#[cfg(feature = "test-support")]
pub use agent_task_client::FakeAgentTaskClient;
pub use agent_task_client::{
    AgentTaskClient, AgentTaskError, AgentTaskRef, AgentTaskState, AgentTaskStatus,
    DisabledAgentTaskClient, EXECUTION_LOST_ERROR_KIND, SharedAgentTaskClient, SubmitAgentTask,
};
pub use core_domain::IndicatorObservation;
pub use indicator_observation_source::{
    IndicatorObservationSource, IndicatorObservationSourceError, SharedIndicatorObservationSource,
};

pub use kata_exec::{ExecRequest, ExecResult, KataExecError, KataExecutor, SharedKataExecutor};

#[cfg(feature = "test-support")]
pub use kata_exec::FakeKataExecutor;

pub use news_aggregator::{
    NewsAggregator, NewsAggregatorError, NewsFeed, NewsItem, SharedNewsAggregator,
};

#[cfg(feature = "test-support")]
pub use news_aggregator::FakeNewsAggregator;
