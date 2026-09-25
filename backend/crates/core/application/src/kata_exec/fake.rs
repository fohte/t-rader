use async_trait::async_trait;

use super::error::KataExecError;
use super::types::{ExecRequest, ExecResult, KataExecutor};

/// テスト向け: 受け取った request を記録して事前設定の応答を返すフェイク
#[derive(Default)]
pub struct FakeKataExecutor {
    pub requests: tokio::sync::Mutex<Vec<ExecRequest>>,
    pub response: tokio::sync::Mutex<Option<Result<ExecResult, KataExecError>>>,
}

impl FakeKataExecutor {
    pub fn new() -> Self {
        Self::default()
    }
    pub async fn set_response(&self, response: Result<ExecResult, KataExecError>) {
        *self.response.lock().await = Some(response);
    }
}

#[async_trait]
impl KataExecutor for FakeKataExecutor {
    async fn run(&self, request: ExecRequest) -> Result<ExecResult, KataExecError> {
        self.requests.lock().await.push(request);
        match self.response.lock().await.take() {
            Some(r) => r,
            None => Ok(ExecResult {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 0,
            }),
        }
    }
}
