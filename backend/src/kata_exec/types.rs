use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use super::error::KataExecError;

/// Python コードを 1 回実行するためのリクエスト
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecRequest {
    /// 実行する Python コード本体 (utf-8)
    pub code: String,
    /// 実行中に Python の sys.stdin に流す入力 (省略可)
    pub stdin: Option<String>,
    /// Pod 全体の wall-clock 上限 (backend 側で時計を見る)。
    /// 同時に Pod 側の `activeDeadlineSeconds` も同じ値を入れる。
    pub timeout: Option<Duration>,
    /// stdout + stderr の合計バイト数の上限。超えた場合 `OutputTooLarge` を返す。
    pub max_output_bytes: Option<usize>,
}

impl ExecRequest {
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            stdin: None,
            timeout: None,
            max_output_bytes: None,
        }
    }
}

/// Pod の実行結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[async_trait]
pub trait KataExecutor: Send + Sync {
    async fn run(&self, request: ExecRequest) -> Result<ExecResult, KataExecError>;
}

pub type SharedKataExecutor = Arc<dyn KataExecutor + Send + Sync>;

/// 「無効化された」executor。すべての操作が `NotConfigured` を返す。
pub struct DisabledKataExecutor;

#[async_trait]
impl KataExecutor for DisabledKataExecutor {
    async fn run(&self, _request: ExecRequest) -> Result<ExecResult, KataExecError> {
        Err(KataExecError::NotConfigured)
    }
}
