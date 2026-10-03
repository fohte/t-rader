//! e-Stat の公表予定を取得する gateway。
//!
//! 出典: <https://www.e-stat.go.jp/release-calendar>

mod client;
mod parser;

pub use client::EStatCalendarEventSource;
