//! 仮説 (hypothesis) の値域チェックを集約する。
//! HTTP handler と MCP tool で共通利用する。

use crate::error::AppError;

pub const STATUSES: [&str; 4] = ["unverified", "supported", "refuted", "obsolete"];
pub const DEFAULT_STATUS: &str = "unverified";

pub fn ensure_status(value: &str) -> Result<(), AppError> {
    if STATUSES.contains(&value) {
        Ok(())
    } else {
        Err(AppError::Validation(format!("invalid status: {value}")))
    }
}
