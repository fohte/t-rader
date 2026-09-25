mod agent_task_client;
mod daily_bar_source;
mod equity_master_source;
mod financial_summary_source;
mod indicator_observation_source;
mod kata_exec;
mod llm_client;
mod margin_source;
mod market_daily_bar_source;
mod news_aggregator;
mod short_selling_source;

pub use agent_task_client::{
    AgentTaskClient, AgentTaskError, AgentTaskRef, AgentTaskState, AgentTaskStatus,
    DisabledAgentTaskClient, EXECUTION_LOST_ERROR_KIND, SharedAgentTaskClient, SubmitAgentTask,
};
pub use core_domain::IndicatorObservation;
pub use indicator_observation_source::{
    IndicatorObservationSource, IndicatorObservationSourceError, SharedIndicatorObservationSource,
};

pub use kata_exec::{ExecRequest, ExecResult, KataExecError, KataExecutor, SharedKataExecutor};
pub use llm_client::{
    ChatMessage, ContentPart, FilePart, LlmClient, LlmClientError, LlmModel, SharedLlmClient,
    WebSearchOutcome,
};

#[cfg(feature = "test-support")]
pub use kata_exec::FakeKataExecutor;

pub use daily_bar_source::{DailyBarSource, DailyBarSourceError, DateRange, SharedDailyBarSource};
pub use equity_master_source::{
    EquityMasterSource, EquityMasterSourceError, SharedEquityMasterSource,
};
pub use financial_summary_source::{
    FinancialSummarySource, FinancialSummarySourceError, SharedFinancialSummarySource,
};
pub use margin_source::{MarginSource, MarginSourceError, SharedMarginSource};
pub use market_daily_bar_source::{
    MarketDailyBarSource, MarketDailyBarSourceError, SharedMarketDailyBarSource,
};
pub use short_selling_source::{
    SharedShortSellingSource, ShortSellingSource, ShortSellingSourceError,
};

#[cfg(feature = "test-support")]
pub use agent_task_client::FakeAgentTaskClient;

pub use news_aggregator::{
    NewsAggregator, NewsAggregatorError, NewsFeed, NewsItem, SharedNewsAggregator,
};

#[cfg(feature = "test-support")]
pub use news_aggregator::FakeNewsAggregator;
