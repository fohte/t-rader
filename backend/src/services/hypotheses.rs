//! 仮説 (hypothesis) の値域チェックを集約する。
//! handler と (将来追加される) MCP tool の両方で同じ判定を使う。

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
