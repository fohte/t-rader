//! 関心 (strategy_interest) の値域チェックを集約する。
//! HTTP handler と MCP tool の両方で同じ判定を使う。

use crate::error::AppError;

pub const REF_KINDS: [&str; 4] = ["stock", "indicator", "sector", "theme"];
pub const ROLES: [&str; 2] = ["seed", "derived"];
pub const ORIGINS: [&str; 2] = ["human", "llm"];

pub const DEFAULT_ROLE: &str = "seed";
pub const DEFAULT_ORIGIN: &str = "human";

pub fn ensure_ref_kind(value: &str) -> Result<(), AppError> {
    if REF_KINDS.contains(&value) {
        Ok(())
    } else {
        Err(AppError::Validation(format!("invalid ref_kind: {value}")))
    }
}

pub fn ensure_role(value: &str) -> Result<(), AppError> {
    if ROLES.contains(&value) {
        Ok(())
    } else {
        Err(AppError::Validation(format!("invalid role: {value}")))
    }
}

pub fn ensure_origin(value: &str) -> Result<(), AppError> {
    if ORIGINS.contains(&value) {
        Ok(())
    } else {
        Err(AppError::Validation(format!("invalid origin: {value}")))
    }
}
