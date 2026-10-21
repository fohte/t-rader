use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use core_application::kata_exec::{ExecRequest, ExecResult, KataExecError, KataExecutor};
use futures_util::try_join;

use super::config::{FileTransferLimits, KataExecutorConfig, PodResourceLimits};
use super::manifest::{
    PodPhase, RUNTIME_CLASS_NAME, assemble_result, build_pod_manifest, generate_pod_name,
};
use super::pod_api::{PodApi, PodAttach, ReqwestPodApi};
use super::stream;

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

    async fn wait_running(&self, poll_interval: Duration) -> Result<(), KataExecError> {
        loop {
            let info = self.api.get_status(&self.pod_name).await?;
            match info.phase {
                PodPhase::Running => return Ok(()),
                PodPhase::Failed | PodPhase::Succeeded => {
                    return Err(KataExecError::PodFailed(
                        info.terminated_reason
                            .or(info.message)
                            .unwrap_or_else(|| "exec pod stopped before attach".to_string()),
                    ));
                }
                PodPhase::Pending | PodPhase::Unknown => {
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }

    async fn transfer_files(
        &self,
        request: &ExecRequest,
        limits: FileTransferLimits,
        max_output_bytes: usize,
        poll_interval: Duration,
    ) -> Result<ExecResult, KataExecError> {
        self.wait_running(poll_interval).await?;
        let PodAttach {
            mut stdin,
            mut stdout,
            mut stderr,
            status,
            _keepalive,
        } = self.api.attach(&self.pod_name).await?;
        let _keepalive = _keepalive;
        let stderr_task = tokio::spawn(async move { stream::drain_stderr(&mut stderr).await });
        let transfer = stream::transfer(&mut stdin, &mut stdout, request, limits, max_output_bytes);
        let combined = try_join!(transfer, status);
        let result = match combined {
            Ok((result, ())) => result,
            Err(error) => {
                stderr_task.abort();
                return Err(error);
            }
        };
        stderr_task.await.map_err(|error| {
            KataExecError::FileTransfer(format!("attach stderr task failed: {error}"))
        })??;
        Ok(result)
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
    file_transfer_limits: FileTransferLimits,
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
            file_transfer_limits: config.file_transfer_limits,
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
        if let Some(files) = &request.input_files {
            stream::validate_input_files(files, self.file_transfer_limits)?;
        }
        let pod_name = generate_pod_name();
        let manifest = build_pod_manifest(
            &pod_name,
            &self.namespace,
            &self.image,
            &request,
            &self.resource_limits,
            wall_clock_timeout.as_secs().max(1) as i64,
            Some(RUNTIME_CLASS_NAME),
        );

        tracing::debug!(pod = %pod_name, "creating kata exec pod");
        if let Err(e) = self.api.create(&manifest).await {
            tracing::warn!(pod = %pod_name, error = %e, "failed to create kata exec pod");
            return Err(e);
        }

        let mut execution = PodExecution::new(self.api.clone(), pod_name.clone());

        let outcome = tokio::time::timeout(wall_clock_timeout, async {
            if request.input_files.is_some() {
                execution
                    .transfer_files(
                        &request,
                        self.file_transfer_limits,
                        max_output_bytes,
                        self.poll_interval,
                    )
                    .await
            } else {
                execution
                    .wait_terminal(max_output_bytes, self.poll_interval)
                    .await
            }
        })
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
    use crate::manifest::{ENVELOPE_MARKER, PodStatusInfo};
    use indoc::indoc;
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
        attach_hangs: bool,
        delete_count: AtomicUsize,
        spawn_delete_count: AtomicUsize,
    }

    #[tokio::test]
    #[ignore = "requires the disposable local kind cluster"]
    async fn local_cluster_attach_round_trip() {
        let context = std::env::var("KATA_EXEC_TEST_CONTEXT").unwrap();
        let api_base_url = std::env::var("KATA_EXEC_TEST_API_URL").unwrap();
        let parsed_api_url = reqwest::Url::parse(&api_base_url).unwrap();
        let namespace = std::env::var("KATA_EXEC_TEST_NAMESPACE").unwrap();
        let token_path = std::env::var("KATA_EXEC_TEST_TOKEN_FILE").unwrap();
        let token = std::fs::read_to_string(token_path).unwrap();
        let image = std::env::var("KATA_EXEC_TEST_IMAGE").unwrap();
        let config = KataExecutorConfig {
            api_base_url,
            namespace: namespace.clone(),
            image: image.clone(),
            bearer_token: Some(token.trim().to_string()),
            ca_cert_path: None,
            insecure_tls: true,
            default_timeout: Duration::from_secs(60),
            default_max_output_bytes: 1024 * 1024,
            file_transfer_limits: FileTransferLimits::default(),
            resource_limits: PodResourceLimits::default(),
            poll_interval: Duration::from_millis(100),
        };
        let api: Arc<dyn PodApi> = Arc::new(ReqwestPodApi::new(&config).unwrap());
        let request = ExecRequest::new(indoc! {"
            from pathlib import Path
            import os
            source = Path(os.environ['KATA_INPUT_DIR'], 'source.bin').read_bytes()
            Path(os.environ['KATA_OUTPUT_DIR'], 'result.bin').write_bytes(source[::-1])
            print('attached')
        "})
        .with_files(vec![core_application::kata_exec::ExecFile {
            path: "source.bin".to_string(),
            bytes: b"sample".to_vec(),
        }]);
        let pod_name = super::super::manifest::generate_pod_name();
        let manifest = build_pod_manifest(
            &pod_name,
            &namespace,
            &image,
            &request,
            &config.resource_limits,
            60,
            None,
        );
        api.create(&manifest).await.unwrap();
        let mut execution = PodExecution::new(api.clone(), pod_name.clone());
        let transfer_result = tokio::time::timeout(
            Duration::from_secs(60),
            execution.transfer_files(
                &request,
                config.file_transfer_limits,
                config.default_max_output_bytes,
                config.poll_interval,
            ),
        )
        .await
        .unwrap_or_else(|_| Err(KataExecError::Timeout(Duration::from_secs(60))));
        let pod_logs = if transfer_result.is_err() {
            api.fetch_log(&pod_name, config.default_max_output_bytes)
                .await
                .unwrap_or_default()
        } else {
            String::new()
        };
        execution.disarm();
        api.delete(&pod_name).await;

        assert_eq!(
            (
                context.as_str(),
                matches!(
                    parsed_api_url.host_str(),
                    Some("127.0.0.1" | "localhost" | "[::1]")
                ),
                namespace.as_str(),
                transfer_result,
            ),
            (
                "kind-kata-streamed-files",
                true,
                "kata-exec-test",
                Ok(ExecResult {
                    stdout: "attached\n".to_string(),
                    stderr: String::new(),
                    exit_code: 0,
                    files: vec![core_application::kata_exec::ExecFile {
                        path: "result.bin".to_string(),
                        bytes: b"elpmas".to_vec(),
                    }],
                }),
            ),
            "pod logs: {pod_logs}",
        );
    }

    impl MockPodApi {
        fn succeeded(log: impl Into<String>) -> Self {
            Self {
                phase: PodPhase::Succeeded,
                terminated_reason: None,
                log_reply: Mutex::new(Some(LogReply::Body(log.into()))),
                attach_hangs: false,
                delete_count: AtomicUsize::new(0),
                spawn_delete_count: AtomicUsize::new(0),
            }
        }

        fn running() -> Self {
            Self {
                phase: PodPhase::Running,
                terminated_reason: None,
                log_reply: Mutex::new(Some(LogReply::Body(String::new()))),
                attach_hangs: false,
                delete_count: AtomicUsize::new(0),
                spawn_delete_count: AtomicUsize::new(0),
            }
        }

        fn oversize() -> Self {
            Self {
                phase: PodPhase::Succeeded,
                terminated_reason: None,
                log_reply: Mutex::new(Some(LogReply::Oversized)),
                attach_hangs: false,
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

        async fn attach(&self, _name: &str) -> Result<PodAttach, KataExecError> {
            if !self.attach_hangs {
                return Err(KataExecError::FileTransfer(
                    "mock attach is not configured".to_string(),
                ));
            }
            let (stdin, remote_stdin) = tokio::io::duplex(1024);
            let (remote_stdout, stdout) = tokio::io::duplex(1024);
            let (remote_stderr, stderr) = tokio::io::duplex(1024);
            let status = Box::pin(async move {
                let (_remote_stdin, _remote_stdout, _remote_stderr) =
                    (remote_stdin, remote_stdout, remote_stderr);
                futures_util::future::pending::<Result<(), KataExecError>>().await
            });
            Ok(PodAttach {
                stdin: Box::pin(stdin),
                stdout: Box::pin(stdout),
                stderr: Box::pin(stderr),
                status,
                _keepalive: None,
            })
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
            file_transfer_limits: FileTransferLimits {
                max_files: 2,
                max_file_bytes: 8,
                max_total_bytes: 12,
                chunk_bytes: 3,
            },
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
            (
                exec.run(ExecRequest::new("print(1+1)")).await,
                api.delete_count.load(Ordering::SeqCst),
                api.spawn_delete_count.load(Ordering::SeqCst),
            ),
            (
                Ok(ExecResult {
                    stdout: "2\n".to_string(),
                    stderr: String::new(),
                    exit_code: 0,
                    files: Vec::new(),
                }),
                1,
                0,
            ),
        );
    }

    #[tokio::test]
    async fn file_transfer_timeout_deletes_the_pod() {
        let mut api = MockPodApi::running();
        api.attach_hangs = true;
        let api = Arc::new(api);
        let exec = executor_with(api.clone());
        let timeout = Duration::from_millis(20);
        let mut request = ExecRequest::new("pass").with_files(Vec::new());
        request.timeout = Some(timeout);

        assert_eq!(
            (
                exec.run(request).await,
                api.delete_count.load(Ordering::SeqCst),
                api.spawn_delete_count.load(Ordering::SeqCst),
            ),
            (Err(KataExecError::Timeout(timeout)), 1, 0),
        );
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
