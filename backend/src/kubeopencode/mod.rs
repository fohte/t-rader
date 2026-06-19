//! kubeopencode の Task カスタムリソースを操作するためのクライアント
//!
//! 管理 MCP の `submit_strategy_task` と watcher から使われる。
//! 通常運用では in-cluster ServiceAccount トークン経由で kube-apiserver に直接叩く。

pub mod client;

pub use client::{
    DisabledKubeopencodeClient, HttpKubeopencodeClient, KubeopencodeClient, KubeopencodeConfig,
    KubeopencodeConfigError, KubeopencodeConfigSource, KubeopencodeError, SharedKubeopencodeClient,
    TaskCrSpec, TaskCrStatus, TaskPhase,
};

#[cfg(test)]
pub use client::FakeKubeopencodeClient;
