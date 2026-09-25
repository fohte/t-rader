use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum AgentTaskError {
    #[error("agent task client is not configured")]
    NotConfigured,

    #[error("agent task not found: {0}")]
    NotFound(String),

    #[error("agent task api error (status {status}): {message}")]
    Api { status: u16, message: String },

    #[error("network error: {0}")]
    Network(String),

    #[error("failed to parse response: {0}")]
    Parse(String),

    #[error("client initialization error: {0}")]
    Init(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentTaskState {
    Submitted,
    Working,
    InputRequired,
    Completed,
    Canceled,
    Failed,
    Rejected,
}

#[derive(Debug, Clone)]
pub struct SubmitAgentTask {
    pub strategy_id: Uuid,
    pub prompt: String,
    /// タスク実行に使う `agent_config` テーブルの purpose キー。`services::strategy_tasks::submit_task`
    /// が常に解決済みの値を詰めるため、実質的に `None` にはならない。
    pub purpose: Option<String>,
    /// 再開対象タスクの全 strategy_task_step 行 (seq 昇順)。中身は解釈せず素通しする。
    /// 新規タスクの投入では `None`。
    pub resume_steps: Option<Vec<serde_json::Value>>,
    /// この投入の締切。agent 側で実行全体を打ち切るための signal に使われる。
    pub deadline_at: DateTime<FixedOffset>,
    /// 実行の論理的な基準時刻 (`strategy_task.as_of`)。agent がプロンプトに含めて LLM に伝える。
    /// resume でも初回投入時の値を渡す。
    pub as_of: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Clone)]
pub struct AgentTaskRef {
    pub task_id: String,
}

/// t-rader-agent の watchdog が heartbeat 断絶を検知して failed タスクに付与する error_kind。
/// `agent/src/a2a/postgres-task-store.ts` の `EXECUTION_LOST_ERROR_KIND` と対応する契約値。
pub const EXECUTION_LOST_ERROR_KIND: &str = "execution_lost";

#[derive(Debug, Clone)]
pub struct AgentTaskStatus {
    pub state: AgentTaskState,
    pub result_text: Option<String>,
    /// 失敗系 state で agent が組み立てた失敗理由の本文 (フェーズ名を含む)。
    /// `error_kind` は分類名、こちらは人間が読む本文。
    pub error_message: Option<String>,
    pub error_kind: Option<String>,
    /// フェーズ/分岐ごとの実行状況。中身は解釈せず素通しする。空または応答に無ければ `None`。
    pub steps: Option<serde_json::Value>,
}

#[async_trait]
pub trait AgentTaskClient: Send + Sync {
    async fn submit(&self, req: SubmitAgentTask) -> Result<AgentTaskRef, AgentTaskError>;

    async fn get(&self, task_id: &str) -> Result<AgentTaskStatus, AgentTaskError>;
}

/// 「無効化された」クライアント。すべての操作が `NotConfigured` を返す。
pub struct DisabledAgentTaskClient;

#[async_trait]
impl AgentTaskClient for DisabledAgentTaskClient {
    async fn submit(&self, _req: SubmitAgentTask) -> Result<AgentTaskRef, AgentTaskError> {
        Err(AgentTaskError::NotConfigured)
    }

    async fn get(&self, _task_id: &str) -> Result<AgentTaskStatus, AgentTaskError> {
        Err(AgentTaskError::NotConfigured)
    }
}

/// 共有用 alias。`Arc<dyn AgentTaskClient + Send + Sync>` を頻繁に書くのを避ける。
pub type SharedAgentTaskClient = Arc<dyn AgentTaskClient + Send + Sync>;

#[cfg(feature = "test-support")]
#[derive(Default)]
pub struct FakeAgentTaskClient {
    pub submitted: tokio::sync::Mutex<Vec<SubmitAgentTask>>,
    pub statuses: tokio::sync::Mutex<std::collections::HashMap<String, AgentTaskStatus>>,
    pub next_task_id: tokio::sync::Mutex<Option<String>>,
    pub submit_error: tokio::sync::Mutex<Option<AgentTaskError>>,
    pub get_error: tokio::sync::Mutex<Option<AgentTaskError>>,
}

#[cfg(feature = "test-support")]
impl FakeAgentTaskClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn set_status(&self, task_id: &str, status: AgentTaskStatus) {
        self.statuses
            .lock()
            .await
            .insert(task_id.to_string(), status);
    }

    pub async fn set_next_task_id(&self, task_id: &str) {
        *self.next_task_id.lock().await = Some(task_id.to_string());
    }

    pub async fn set_submit_error(&self, err: AgentTaskError) {
        *self.submit_error.lock().await = Some(err);
    }

    pub async fn set_get_error(&self, err: AgentTaskError) {
        *self.get_error.lock().await = Some(err);
    }
}

#[cfg(feature = "test-support")]
#[async_trait]
impl AgentTaskClient for FakeAgentTaskClient {
    async fn submit(&self, req: SubmitAgentTask) -> Result<AgentTaskRef, AgentTaskError> {
        if let Some(err) = self.submit_error.lock().await.take() {
            return Err(err);
        }
        let task_id = self
            .next_task_id
            .lock()
            .await
            .take()
            .unwrap_or_else(|| format!("fake-task-{}", Uuid::new_v4()));
        self.submitted.lock().await.push(req);
        Ok(AgentTaskRef { task_id })
    }

    async fn get(&self, task_id: &str) -> Result<AgentTaskStatus, AgentTaskError> {
        if let Some(err) = self.get_error.lock().await.take() {
            return Err(err);
        }
        self.statuses
            .lock()
            .await
            .get(task_id)
            .cloned()
            .ok_or_else(|| AgentTaskError::NotFound(task_id.to_string()))
    }
}
