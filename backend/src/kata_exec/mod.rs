//! LLM 由来の Python コードを Kata Containers (microVM 隔離) で実行するための executor
//!
//! exec Pod spec の不変条件 (本モジュールが build_pod_manifest で固定する):
//!
//! - `runtimeClassName: kata`
//! - `automountServiceAccountToken: false`
//! - `restartPolicy: Never`
//! - `activeDeadlineSeconds` (実行時間上限)
//! - `resources.limits` (CPU / memory / ephemeral-storage)
//! - 1 Pod 1 container (sidecar なし)
//! - `runAsNonRoot` + `readOnlyRootFilesystem` + `capabilities.drop: ["ALL"]`
//!
//! exec Pod には secret を渡さない。必要なデータは backend が DB / API から取り出し、
//! 値として `EXEC_CODE_B64` / `EXEC_STDIN_B64` 環境変数で注入する。
//!
//! # インフラ側で必要になる設定
//!
//! このモジュールは backend Pod の ServiceAccount に以下の権限がある前提で動く。
//! 付与は infra 側の責務:
//!
//! ```yaml
//! apiVersion: rbac.authorization.k8s.io/v1
//! kind: Role
//! metadata:
//!   name: t-rader-kata-exec
//!   namespace: t-rader-exec
//! rules:
//!   - apiGroups: [""]
//!     resources: ["pods"]
//!     verbs: ["create", "get", "list", "watch", "delete"]
//!   - apiGroups: [""]
//!     resources: ["pods/log"]
//!     verbs: ["get"]
//! ```
//!
//! NetworkPolicy で `t-rader-exec` namespace の egress を全 deny、PSA は
//! `restricted` 相当を設定する。RuntimeClass `kata` は Talos の kata-containers
//! extension を有効化したノードで登録される。

pub mod client;

pub use client::{
    DisabledKataExecutor, ExecRequest, ExecResult, HttpKataExecutor, KataExecError, KataExecutor,
    KataExecutorConfig, PodResourceLimits, SharedKataExecutor,
};

#[cfg(test)]
pub use client::FakeKataExecutor;
