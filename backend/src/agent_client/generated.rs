//! `agent/openapi.json` (t-rader-agent internal API) から progenitor が生成した client。
//! `backend/build.rs` がビルドごとに再生成するため、手動編集禁止。
//!
//! `clippy::all` はデフォルトの correctness/suspicious/complexity/perf/style
//! グループのみを covers し、このリポジトリが個別に有効化している
//! restriction カテゴリの lint (unwrap_used 等) はカバーしないため、
//! 生成コードに現れるものを個別に allow している。
#![allow(
    clippy::all,
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "progenitor が生成したコードをそのまま取り込むため、手動での lint 対応はしない"
)]

include!(concat!(env!("OUT_DIR"), "/agent_internal_api_client.rs"));
