use crate::agent_task_client::SubmitAgentTask;
use chrono::{DateTime, FixedOffset};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyTaskPhase {
    Pending,
    Running,
    Completed,
    Failed,
}

impl StrategyTaskPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyTaskStepStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StrategyTaskStepKind {
    #[default]
    Llm,
    Operation,
}

impl StrategyTaskStepKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Llm => "llm",
            Self::Operation => "operation",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "llm" => Some(Self::Llm),
            "operation" => Some(Self::Operation),
            _ => None,
        }
    }
}

impl StrategyTaskStepStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskSource {
    MgmtMcp,
    Frontend,
    Cron,
    Hook,
    Review,
    Event,
}

impl TaskSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MgmtMcp => "mgmt-mcp",
            Self::Frontend => "frontend",
            Self::Cron => "cron",
            Self::Hook => "hook",
            Self::Review => "review",
            Self::Event => "event",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StrategyTask {
    pub task_id: Uuid,
    pub strategy_id: Uuid,
    pub a2a_task_id: Option<String>,
    pub source: String,
    pub prompt: String,
    pub phase: StrategyTaskPhase,
    pub error_summary: Option<String>,
    pub result_text: Option<String>,
    /// task_execution_state には予算予約と claim の原子判定に使う同じ固定期限を保存する。
    pub deadline_at: DateTime<FixedOffset>,
    pub purpose: Option<String>,
    pub as_of: Option<DateTime<FixedOffset>>,
    pub context_kind: Option<String>,
    pub context_id: Option<String>,
    pub auto_resumed_at: Option<DateTime<FixedOffset>>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone)]
pub struct StrategyTaskStep {
    pub execution_step_id: Uuid,
    pub task_id: Uuid,
    pub phase_key: String,
    pub kind: StrategyTaskStepKind,
    pub label: String,
    pub model: Option<String>,
    pub status: StrategyTaskStepStatus,
    pub item: Option<serde_json::Value>,
    pub item_key: Option<serde_json::Value>,
    pub item_label: Option<String>,
    pub input_ref: Option<serde_json::Value>,
    pub output_ref: Option<serde_json::Value>,
    pub output: Option<serde_json::Value>,
    pub started_at: DateTime<FixedOffset>,
    pub finished_at: Option<DateTime<FixedOffset>>,
    pub trace_id: String,
    pub span_id: String,
    pub error: Option<String>,
    pub seq: i64,
}

#[derive(Debug, Clone)]
pub struct SubmittedTask {
    pub task_id: Uuid,
    pub a2a_task_id: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedStrategyTaskSubmission {
    pub(crate) task: StrategyTask,
    pub(crate) request: SubmitAgentTask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrategyTaskOrigin {
    pub strategy_id: Uuid,
    pub task_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct TaskStatusView {
    pub task_id: Uuid,
    pub strategy_id: Uuid,
    pub a2a_task_id: Option<String>,
    pub source: String,
    pub prompt: String,
    pub phase: StrategyTaskPhase,
    pub error_summary: Option<String>,
    pub result_text: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub steps: serde_json::Value,
    pub purpose: Option<String>,
    pub as_of: Option<DateTime<FixedOffset>>,
    pub context_kind: Option<String>,
    pub context_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TaskListQuery {
    pub strategy_id: Option<Uuid>,
    pub purpose: Option<String>,
    pub context: Option<StrategyTaskContext>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrategyTaskContext {
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct StrategyTaskUpdate {
    pub task_id: Uuid,
    pub a2a_task_id: Option<Option<String>>,
    pub phase: Option<StrategyTaskPhase>,
    pub error_summary: Option<Option<String>>,
    pub result_text: Option<Option<String>>,
    pub deadline_at: Option<DateTime<FixedOffset>>,
    pub auto_resumed_at: Option<Option<DateTime<FixedOffset>>>,
    pub updated_at: DateTime<FixedOffset>,
}
