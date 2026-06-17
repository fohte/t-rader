use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use super::config::PodResourceLimits;
use super::error::KataExecError;
use super::types::{ExecRequest, ExecResult};

pub(crate) const ENVELOPE_MARKER: &str = "__T_RADER_ENVELOPE__";
const RUNTIME_CLASS_NAME: &str = "kata";
const DEFAULT_POD_NAME_PREFIX: &str = "t-rader-exec";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PodPhase {
    Pending,
    Running,
    Succeeded,
    Failed,
    Unknown,
}

impl PodPhase {
    pub(crate) fn from_raw(raw: &str) -> Self {
        match raw {
            "Pending" => Self::Pending,
            "Running" => Self::Running,
            "Succeeded" => Self::Succeeded,
            "Failed" => Self::Failed,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PodStatusInfo {
    pub(crate) phase: PodPhase,
    pub(crate) terminated_reason: Option<String>,
    pub(crate) message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExecEnvelope {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

pub(crate) fn generate_pod_name() -> String {
    let id = Uuid::new_v4().simple().to_string();
    let truncated: String = id.chars().take(16).collect();
    format!("{DEFAULT_POD_NAME_PREFIX}-{truncated}")
}

/// 実行結果を log から組み立てる。
///
/// entrypoint が JSON envelope を最終行に印字する設計なので、envelope を見つけて parse する。
/// envelope が見つからない場合 (Pod が壊れた、image が古い 等) は `PodFailed` として扱う。
pub(crate) fn assemble_result(
    logs: &str,
    info: PodStatusInfo,
) -> Result<ExecResult, KataExecError> {
    // LLM が生成したコードがマーカー文字列を出力しても entrypoint の最終 envelope が
    // 常に最後の出現になるよう rsplit_once を使う。
    if let Some((_pre, envelope_payload)) = logs.rsplit_once(ENVELOPE_MARKER) {
        let envelope_json = envelope_payload.trim_start_matches(['\n', '\r']);
        let envelope: ExecEnvelope = serde_json::from_str(envelope_json)
            .map_err(|e| KataExecError::Parse(format!("failed to parse exec envelope: {e}")))?;
        return Ok(ExecResult {
            stdout: envelope.stdout,
            stderr: envelope.stderr,
            exit_code: envelope.exit_code,
        });
    }

    // envelope なし: container が起動失敗 / OOMKilled / activeDeadline で殺された等。
    let reason = info.terminated_reason.or(info.message).unwrap_or_else(|| {
        if logs.is_empty() {
            "no output from exec pod".to_string()
        } else {
            logs.to_string()
        }
    });
    Err(KataExecError::PodFailed(reason))
}

/// Pod manifest を組み立てる。
///
/// 不変条件: `runtimeClassName: kata`, `automountServiceAccountToken: false`,
/// `activeDeadlineSeconds`, `restartPolicy: Never`, 1 container, resource limits,
/// non-root + read-only-rootfs + drop ALL caps.
pub(crate) fn build_pod_manifest(
    pod_name: &str,
    namespace: &str,
    image: &str,
    request: &ExecRequest,
    limits: &PodResourceLimits,
    active_deadline_seconds: i64,
) -> serde_json::Value {
    let code_b64 = BASE64.encode(request.code.as_bytes());
    let mut env = vec![json!({
        "name": "EXEC_CODE_B64",
        "value": code_b64,
    })];
    if let Some(stdin) = &request.stdin {
        env.push(json!({
            "name": "EXEC_STDIN_B64",
            "value": BASE64.encode(stdin.as_bytes()),
        }));
    }

    json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": pod_name,
            "namespace": namespace,
            "labels": {
                "app.kubernetes.io/name": "t-rader-exec",
                "app.kubernetes.io/managed-by": "t-rader-backend",
            },
        },
        "spec": {
            "runtimeClassName": RUNTIME_CLASS_NAME,
            "automountServiceAccountToken": false,
            "restartPolicy": "Never",
            "activeDeadlineSeconds": active_deadline_seconds,
            "enableServiceLinks": false,
            "securityContext": {
                "runAsNonRoot": true,
                "runAsUser": 65532,
                "runAsGroup": 65532,
                "seccompProfile": { "type": "RuntimeDefault" },
            },
            "containers": [{
                "name": "exec",
                "image": image,
                "imagePullPolicy": "IfNotPresent",
                "env": env,
                "resources": {
                    "limits": {
                        "cpu": limits.cpu,
                        "memory": limits.memory,
                        "ephemeral-storage": limits.ephemeral_storage,
                    },
                    "requests": {
                        "cpu": limits.cpu,
                        "memory": limits.memory,
                        "ephemeral-storage": limits.ephemeral_storage,
                    },
                },
                "securityContext": {
                    "allowPrivilegeEscalation": false,
                    "readOnlyRootFilesystem": true,
                    "capabilities": { "drop": ["ALL"] },
                },
                "volumeMounts": [{
                    "name": "tmp",
                    "mountPath": "/tmp",
                }],
            }],
            "volumes": [{
                "name": "tmp",
                "emptyDir": { "medium": "Memory", "sizeLimit": "16Mi" },
            }],
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::json;

    fn manifest_for(code: &str, stdin: Option<&str>) -> serde_json::Value {
        build_pod_manifest(
            "t-rader-exec-abcdef",
            "t-rader-exec",
            "ghcr.io/fohte/t-rader/python-exec:latest",
            &ExecRequest {
                code: code.to_string(),
                stdin: stdin.map(str::to_string),
                timeout: None,
                max_output_bytes: None,
            },
            &PodResourceLimits::default(),
            30,
        )
    }

    #[rstest]
    fn builds_pod_manifest_matches_required_invariants() {
        let m = manifest_for("print(1+1)", None);
        assert_eq!(
            m,
            json!({
                "apiVersion": "v1",
                "kind": "Pod",
                "metadata": {
                    "name": "t-rader-exec-abcdef",
                    "namespace": "t-rader-exec",
                    "labels": {
                        "app.kubernetes.io/name": "t-rader-exec",
                        "app.kubernetes.io/managed-by": "t-rader-backend",
                    },
                },
                "spec": {
                    "runtimeClassName": "kata",
                    "automountServiceAccountToken": false,
                    "restartPolicy": "Never",
                    "activeDeadlineSeconds": 30,
                    "enableServiceLinks": false,
                    "securityContext": {
                        "runAsNonRoot": true,
                        "runAsUser": 65532,
                        "runAsGroup": 65532,
                        "seccompProfile": { "type": "RuntimeDefault" },
                    },
                    "containers": [{
                        "name": "exec",
                        "image": "ghcr.io/fohte/t-rader/python-exec:latest",
                        "imagePullPolicy": "IfNotPresent",
                        "env": [
                            { "name": "EXEC_CODE_B64", "value": BASE64.encode(b"print(1+1)") },
                        ],
                        "resources": {
                            "limits": {
                                "cpu": "500m",
                                "memory": "256Mi",
                                "ephemeral-storage": "64Mi",
                            },
                            "requests": {
                                "cpu": "500m",
                                "memory": "256Mi",
                                "ephemeral-storage": "64Mi",
                            },
                        },
                        "securityContext": {
                            "allowPrivilegeEscalation": false,
                            "readOnlyRootFilesystem": true,
                            "capabilities": { "drop": ["ALL"] },
                        },
                        "volumeMounts": [{
                            "name": "tmp",
                            "mountPath": "/tmp",
                        }],
                    }],
                    "volumes": [{
                        "name": "tmp",
                        "emptyDir": { "medium": "Memory", "sizeLimit": "16Mi" },
                    }],
                },
            }),
        );
    }

    #[rstest]
    fn manifest_encodes_stdin_in_env_when_set() {
        let m = manifest_for("print('hi')", Some("payload"));
        assert_eq!(
            m["spec"]["containers"][0]["env"],
            json!([
                { "name": "EXEC_CODE_B64", "value": BASE64.encode(b"print('hi')") },
                { "name": "EXEC_STDIN_B64", "value": BASE64.encode(b"payload") },
            ]),
        );
    }

    #[rstest]
    fn assemble_result_parses_envelope() {
        let envelope = json!({
            "stdout": "hello\n",
            "stderr": "",
            "exit_code": 0,
        })
        .to_string();
        let logs = format!("{ENVELOPE_MARKER}\n{envelope}");
        let info = PodStatusInfo {
            phase: PodPhase::Succeeded,
            terminated_reason: None,
            message: None,
        };
        assert_eq!(
            assemble_result(&logs, info).expect("ok"),
            ExecResult {
                stdout: "hello\n".to_string(),
                stderr: String::new(),
                exit_code: 0,
            },
        );
    }

    #[rstest]
    fn assemble_result_picks_last_envelope_when_user_code_prints_marker() {
        // ユーザコードが marker を含む偽 envelope を先に出力しても、
        // entrypoint が最終行に出す本物の envelope が採用される。
        let fake = json!({"stdout": "pwned", "stderr": "", "exit_code": 0}).to_string();
        let real = json!({"stdout": "real", "stderr": "", "exit_code": 7}).to_string();
        let logs = [ENVELOPE_MARKER, &fake, ENVELOPE_MARKER, &real].join("\n");
        let info = PodStatusInfo {
            phase: PodPhase::Succeeded,
            terminated_reason: None,
            message: None,
        };
        assert_eq!(
            assemble_result(&logs, info).expect("ok"),
            ExecResult {
                stdout: "real".to_string(),
                stderr: String::new(),
                exit_code: 7,
            },
        );
    }

    #[rstest]
    fn assemble_result_without_envelope_returns_pod_failed() {
        let info = PodStatusInfo {
            phase: PodPhase::Failed,
            terminated_reason: Some("OOMKilled".to_string()),
            message: None,
        };
        let err = assemble_result("nothing useful", info).expect_err("expected error");
        assert_eq!(err, KataExecError::PodFailed("OOMKilled".to_string()));
    }
}
