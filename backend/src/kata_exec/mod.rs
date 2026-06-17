//! 純粋関数評価のための Kata Containers 実行ランタイム
//!
//! exec Pod は「値を受け取って値を返す」純粋関数評価以外の操作を物理的に
//! 不可能にする。MVP では LLM の `eval_python` 呼び出し、post-MVP では DB に
//! 保存されたインジケータスクリプトの実行に同じランタイムを使う。
//!
//! exec Pod spec の不変条件 (本モジュールが build_pod_manifest で固定する):
//!
//! - `runtimeClassName: kata` (microVM)
//! - `automountServiceAccountToken: false`
//! - `enableServiceLinks: false`
//! - `restartPolicy: Never`
//! - `activeDeadlineSeconds` (実行時間上限)
//! - `resources.limits` (CPU / memory / ephemeral-storage)
//! - 1 Pod 1 container (sidecar なし)
//! - `runAsNonRoot` + `readOnlyRootFilesystem` + `allowPrivilegeEscalation: false`
//!   + `capabilities.drop: ["ALL"]` + `seccompProfile: RuntimeDefault`
//! - 永続マウントなし。`/tmp` は in-memory な `emptyDir` で sizeLimit 付き
//! - secret 注入なし。コードと入力データのみ env 経由で渡す
//!
//! # infra 側で必要になる設定
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
//!
//! # 後続スコープ
//!
//! - 入力データの transport を env から kube attach API 経由の stdin に切り替える
//!   (env は Pod の `/proc/{pid}/environ` に露出するため、`stdin` の方が原則に合う)
//! - subprocess / fork / clone3 を OS レベルで deny する localhost custom seccomp
//!   profile (`seccompProfile.type: Localhost`) を infra 側でノードに配布する
//! - 実行 image に同梱するライブラリのホワイトリスト管理 (numpy / pandas のみ)

pub mod client;

pub use client::{
    DisabledKataExecutor, ExecRequest, ExecResult, HttpKataExecutor, KataExecError, KataExecutor,
    KataExecutorConfig, PodResourceLimits, SharedKataExecutor,
};

#[cfg(test)]
pub use client::FakeKataExecutor;
