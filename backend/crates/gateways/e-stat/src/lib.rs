//! e-Stat の公表予定を取得する gateway。
//!
//! 出典: <https://www.e-stat.go.jp/release-calendar>

mod client;
mod macro_references;
mod month;
mod official_http;
mod parser;

pub use client::EStatCalendarEventSource;
pub use macro_references::EStatMacroReferenceSource;

pub(crate) const SOURCE_NAME: &str = "e_stat";
