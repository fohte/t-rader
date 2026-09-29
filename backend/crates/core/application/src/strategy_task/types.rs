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
}

impl TaskSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MgmtMcp => "mgmt-mcp",
            Self::Frontend => "frontend",
            Self::Cron => "cron",
            Self::Hook => "hook",
            Self::Review => "review",
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
    pub deadline_at: DateTime<FixedOffset>,
    pub purpose: Option<String>,
    pub as_of: Option<DateTime<FixedOffset>>,
    pub auto_resumed_at: Option<DateTime<FixedOffset>>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone)]
pub struct StrategyTaskStep {
    pub execution_step_id: Uuid,
    pub task_id: Uuid,
    pub phase_key: String,
    pub label: String,
    pub model: String,
    pub status: StrategyTaskStepStatus,
    pub item: Option<serde_json::Value>,
    pub item_label: Option<String>,
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
}

#[derive(Debug, Clone, Default)]
pub struct TaskListQuery {
    pub strategy_id: Option<Uuid>,
    pub purpose: Option<String>,
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
