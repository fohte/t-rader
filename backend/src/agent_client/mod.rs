//! t-rader-agent (LangGraph + A2A server) の内部 API と通信するクライアント
//!
//! 戦略タスクの投入 (`POST /internal/tasks`) と状態照会 (`GET /internal/tasks/:task_id`) を
//! 提供する。backend は A2A wire format に直接触れず、この薄い facade 経由でのみ
//! t-rader-agent とやりとりする。

pub mod client;

pub use client::*;
