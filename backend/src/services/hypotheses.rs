//! 仮説 (hypothesis) の値域チェックとエンティティ取得を集約する。
//! HTTP handler と MCP tool で共通利用する。

use sea_orm::{ConnectionTrait, EntityTrait};
use uuid::Uuid;

use crate::entities::hypothesis;
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

/// 仮説を戦略の有無を問わず ID だけで検索する
pub async fn find_hypothesis_or_404<C: ConnectionTrait>(
    db: &C,
    hypothesis_id: Uuid,
) -> Result<hypothesis::Model, AppError> {
    hypothesis::Entity::find_by_id(hypothesis_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("hypothesis {hypothesis_id} not found")))
}
