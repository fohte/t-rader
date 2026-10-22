use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetResource {
    ModelCall,
    ToolCall,
    SubagentRun,
}

impl BudgetResource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ModelCall => "model_call",
            Self::ToolCall => "tool_call",
            Self::SubagentRun => "subagent_run",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "model_call" => Some(Self::ModelCall),
            "tool_call" => Some(Self::ToolCall),
            "subagent_run" => Some(Self::SubagentRun),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetReservationState {
    Reserved,
    Consumed,
    Released,
}

impl BudgetReservationState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reserved => "reserved",
            Self::Consumed => "consumed",
            Self::Released => "released",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "reserved" => Some(Self::Reserved),
            "consumed" => Some(Self::Consumed),
            "released" => Some(Self::Released),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetScopeLimit {
    pub resource_key: String,
    pub scope_key: String,
    pub amount: u64,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetReservationCommand {
    pub task_id: Uuid,
    pub call_id: String,
    pub resource: BudgetResource,
    pub charges: Vec<BudgetScopeLimit>,
    pub now: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetReservation {
    pub reservation_id: Uuid,
    pub task_id: Uuid,
    pub call_id: String,
    pub resource: BudgetResource,
    pub resource_key: String,
    pub scope_key: String,
    pub amount: u64,
    pub state: BudgetReservationState,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetReservationResult {
    pub reservations: Vec<BudgetReservation>,
}
