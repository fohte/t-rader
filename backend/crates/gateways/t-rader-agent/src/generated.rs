//! `agent/openapi.json` (t-rader-agent internal API) から progenitor が生成した client。

#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::unwrap_used,
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    reason = "progenitor が生成するコードは手書きコードと異なる lint 基準にする"
)]

include!(concat!(env!("OUT_DIR"), "/agent_internal_api_client.rs"));
