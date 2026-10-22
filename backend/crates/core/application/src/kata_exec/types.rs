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
    /// attach stream で渡すファイル。`None` は既存の JSON 評価経路を使う。
    pub input_files: Option<Vec<ExecFile>>,
}

impl ExecRequest {
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            stdin: None,
            timeout: None,
            max_output_bytes: None,
            input_files: None,
        }
    }

    pub fn with_files(mut self, files: Vec<ExecFile>) -> Self {
        self.input_files = Some(files);
        self
    }
}

/// attach stream で転送するファイル。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

/// Pod の実行結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub files: Vec<ExecFile>,
}

#[async_trait]
pub trait KataExecutor: Send + Sync {
    async fn run(&self, request: ExecRequest) -> Result<ExecResult, KataExecError>;
}

pub type SharedKataExecutor = Arc<dyn KataExecutor + Send + Sync>;
