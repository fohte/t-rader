use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use super::config::KataExecutorConfig;
use super::config::PodResourceLimits;
use super::error::KataExecError;
use super::manifest::{PodPhase, assemble_result, build_pod_manifest, generate_pod_name};
use super::pod_api::{PodApi, ReqwestPodApi};
use super::types::{ExecRequest, ExecResult, KataExecutor};

/// Pod 削除を `Drop` で fire-and-forget 予約する RAII ガード。
/// 正常パスでは `disarm()` を呼んでから同期 `delete` する。disarm を忘れると
/// 同期 `delete` と Drop の `spawn_delete` が二重発火するので注意。
pub(crate) struct PodExecution {
    api: Arc<dyn PodApi>,
    pod_name: String,
    armed: bool,
}

impl PodExecution {
    fn new(api: Arc<dyn PodApi>, pod_name: String) -> Self {
        Self {
            api,
            pod_name,
            armed: true,
        }
    }

    async fn wait_terminal(
        &self,
        max_output_bytes: usize,
        poll_interval: Duration,
    ) -> Result<ExecResult, KataExecError> {
        loop {
            let info = self.api.get_status(&self.pod_name).await?;
            match info.phase {
                PodPhase::Succeeded | PodPhase::Failed => {
                    let logs = self.api.fetch_log(&self.pod_name, max_output_bytes).await?;
                    return assemble_result(&logs, info);
                }
                PodPhase::Pending | PodPhase::Running | PodPhase::Unknown => {
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PodExecution {
    fn drop(&mut self) {
        if self.armed {
            self.api.spawn_delete(&self.pod_name);
        }
    }
}

pub struct HttpKataExecutor {
    api: Arc<dyn PodApi>,
    namespace: String,
    image: String,
    default_timeout: Duration,
    default_max_output_bytes: usize,
    resource_limits: PodResourceLimits,
    poll_interval: Duration,
}

impl HttpKataExecutor {
    pub fn new(config: KataExecutorConfig) -> Result<Self, KataExecError> {
        let api: Arc<dyn PodApi> = Arc::new(ReqwestPodApi::new(&config)?);
        Ok(Self::from_parts(api, config))
    }

    fn from_parts(api: Arc<dyn PodApi>, config: KataExecutorConfig) -> Self {
        Self {
            api,
            namespace: config.namespace,
            image: config.image,
            default_timeout: config.default_timeout,
            default_max_output_bytes: config.default_max_output_bytes,
            resource_limits: config.resource_limits,
            poll_interval: config.poll_interval,
        }
    }
}

#[async_trait]
impl KataExecutor for HttpKataExecutor {
    async fn run(&self, request: ExecRequest) -> Result<ExecResult, KataExecError> {
        let wall_clock_timeout = request.timeout.unwrap_or(self.default_timeout);
        let max_output_bytes = request
            .max_output_bytes
            .unwrap_or(self.default_max_output_bytes);
        let pod_name = generate_pod_name();
        let manifest = build_pod_manifest(
            &pod_name,
            &self.namespace,
            &self.image,
            &request,
            &self.resource_limits,
            wall_clock_timeout.as_secs().max(1) as i64,
        );

        tracing::debug!(pod = %pod_name, "creating kata exec pod");
        if let Err(e) = self.api.create(&manifest).await {
            tracing::warn!(pod = %pod_name, error = %e, "failed to create kata exec pod");
            return Err(e);
        }

        let mut execution = PodExecution::new(self.api.clone(), pod_name.clone());

        let outcome = tokio::time::timeout(
            wall_clock_timeout,
            execution.wait_terminal(max_output_bytes, self.poll_interval),
        )
        .await;
        // 正常パスでは同期的に削除し、結果をログに残す。Drop 側の fire-and-forget は冗長になるので disarm。
        execution.disarm();
        self.api.delete(&pod_name).await;

        match outcome {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => {
                tracing::warn!(pod = %pod_name, error = %e, "kata exec pod failed");
                Err(e)
            }
            Err(_) => {
                tracing::warn!(
                    pod = %pod_name,
                    timeout_secs = wall_clock_timeout.as_secs(),
                    "kata exec pod hit wall-clock timeout",
                );
                Err(KataExecError::Timeout(wall_clock_timeout))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kata_exec::manifest::{ENVELOPE_MARKER, PodStatusInfo};
    use serde_json::json;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    enum LogReply {
        Body(String),
        Oversized,
    }

    struct MockPodApi {
        phase: PodPhase,
        terminated_reason: Option<String>,
        log_reply: Mutex<Option<LogReply>>,
        delete_count: AtomicUsize,
        spawn_delete_count: AtomicUsize,
    }

    impl MockPodApi {
        fn succeeded(log: impl Into<String>) -> Self {
            Self {
                phase: PodPhase::Succeeded,
                terminated_reason: None,
                log_reply: Mutex::new(Some(LogReply::Body(log.into()))),
                delete_count: AtomicUsize::new(0),
                spawn_delete_count: AtomicUsize::new(0),
            }
        }

        fn running() -> Self {
            Self {
                phase: PodPhase::Running,
                terminated_reason: None,
                log_reply: Mutex::new(Some(LogReply::Body(String::new()))),
                delete_count: AtomicUsize::new(0),
                spawn_delete_count: AtomicUsize::new(0),
            }
        }

        fn oversize() -> Self {
            Self {
                phase: PodPhase::Succeeded,
                terminated_reason: None,
                log_reply: Mutex::new(Some(LogReply::Oversized)),
                delete_count: AtomicUsize::new(0),
                spawn_delete_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl PodApi for MockPodApi {
        async fn create(&self, _manifest: &serde_json::Value) -> Result<(), KataExecError> {
            Ok(())
        }

        async fn get_status(&self, _name: &str) -> Result<PodStatusInfo, KataExecError> {
            Ok(PodStatusInfo {
                phase: self.phase,
                terminated_reason: self.terminated_reason.clone(),
                message: None,
            })
        }

        async fn fetch_log(&self, _name: &str, max_bytes: usize) -> Result<String, KataExecError> {
            // wait_terminal は terminal phase のとき 1 回しか呼ばないので take して使い切る。
            let reply = self
                .log_reply
                .lock()
                .expect("mock log_reply mutex poisoned")
                .take();
            match reply {
                Some(LogReply::Body(s)) => Ok(s),
                Some(LogReply::Oversized) => {
                    Err(KataExecError::OutputTooLarge { limit: max_bytes })
                }
                None => Ok(String::new()),
            }
        }

        async fn delete(&self, _name: &str) {
            self.delete_count.fetch_add(1, Ordering::SeqCst);
        }

        fn spawn_delete(&self, _name: &str) {
            self.spawn_delete_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn mock_config() -> KataExecutorConfig {
        KataExecutorConfig {
            api_base_url: "http://unused".into(),
            namespace: "t-rader-exec".into(),
            image: "ghcr.io/fohte/t-rader/python-exec:latest".into(),
            bearer_token: None,
            ca_cert_path: None,
            insecure_tls: true,
            default_timeout: Duration::from_millis(500),
            default_max_output_bytes: 1024,
            resource_limits: PodResourceLimits::default(),
            poll_interval: Duration::from_millis(10),
        }
    }

    fn envelope_log(stdout: &str, stderr: &str, exit_code: i32) -> String {
        let env = json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": exit_code,
        });
        format!("{ENVELOPE_MARKER}\n{env}")
    }

    fn executor_with(api: Arc<dyn PodApi>) -> HttpKataExecutor {
        HttpKataExecutor::from_parts(api, mock_config())
    }

    #[tokio::test]
    async fn run_returns_exec_result_with_logs_envelope() {
        let api = Arc::new(MockPodApi::succeeded(envelope_log("2\n", "", 0)));
        let exec = executor_with(api.clone());
        assert_eq!(
            exec.run(ExecRequest::new("print(1+1)")).await.expect("ok"),
            ExecResult {
                stdout: "2\n".to_string(),
                stderr: String::new(),
                exit_code: 0,
            },
        );
        // 正常パスでは同期 delete が 1 回、Drop による spawn_delete は 0 回。
        assert_eq!(api.delete_count.load(Ordering::SeqCst), 1);
        assert_eq!(api.spawn_delete_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn run_times_out_when_pod_stays_running() {
        let api = Arc::new(MockPodApi::running());
        let exec = executor_with(api);
        let mut req = ExecRequest::new("while True: pass");
        req.timeout = Some(Duration::from_millis(100));
        let err = exec.run(req).await.expect_err("expected timeout");
        assert_eq!(err, KataExecError::Timeout(Duration::from_millis(100)));
    }

    #[tokio::test]
    async fn run_rejects_oversized_output() {
        let api = Arc::new(MockPodApi::oversize());
        let exec = executor_with(api);
        let err = exec
            .run(ExecRequest::new("print('x' * 5000)"))
            .await
            .expect_err("expected OutputTooLarge");
        assert_eq!(err, KataExecError::OutputTooLarge { limit: 1024 });
    }

    #[tokio::test]
    async fn run_future_drop_triggers_pod_delete() {
        // Pod がずっと Running を返すので wait_terminal は終わらない。
        // 呼び出し側で future を短い timeout で打ち切り、PodExecution の Drop 経路で
        // spawn_delete が走ることを確認する。
        let api = Arc::new(MockPodApi::running());
        let exec = Arc::new(executor_with(api.clone()));
        let exec_for_task = exec.clone();
        let mut req = ExecRequest::new("while True: pass");
        req.timeout = Some(Duration::from_secs(10));

        let _ = tokio::time::timeout(Duration::from_millis(50), async move {
            exec_for_task.run(req).await
        })
        .await;

        assert_eq!(api.spawn_delete_count.load(Ordering::SeqCst), 1);
        assert_eq!(api.delete_count.load(Ordering::SeqCst), 0);
    }
}
