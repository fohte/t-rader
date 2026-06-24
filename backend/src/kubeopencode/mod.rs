//! kubeopencode の Task / Agent カスタムリソースを操作するためのクライアント
//!
//! Task CR は管理 MCP の `submit_strategy_task` と watcher から、Agent CR とその下流
//! リソース (ServiceAccount / ConfigMap / ExternalSecret) は戦略 CRUD から使われる。
//! 通常運用では in-cluster ServiceAccount トークン経由で kube-apiserver に直接叩く。

pub mod client;
pub mod manifest;

pub use client::{
    DisabledKubeopencodeClient, HttpKubeopencodeClient, KubeopencodeClient, KubeopencodeConfig,
    KubeopencodeConfigError, KubeopencodeConfigSource, KubeopencodeError, SharedKubeopencodeClient,
    TaskCrSpec, TaskCrStatus, TaskPhase,
};
pub use manifest::{
    DEFAULT_AGENT_MODEL, DEFAULT_AGENT_SMALL_MODEL, DEFAULT_SSM_PARAMETER_TEMPLATE,
    StrategyAgentSettings, StrategyAgentSpec, agent_name_for,
};

#[cfg(test)]
pub use client::FakeKubeopencodeClient;
