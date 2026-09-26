//! 口座全体のリスク上限 (`account_risk_policy`)。行は常に 1 行のみ (id=1 固定)。
//! 行が存在しない間は「未設定」を表し、初回保存時に作成する。

use sea_orm::ActiveValue::Set;
use sea_orm::EntityTrait;
use sea_orm::sea_query::OnConflict;

use crate::entities::account_risk_policy;
use crate::error::AppError;

const SINGLETON_ID: i16 = 1;

pub async fn find_current(
    db: &impl sea_orm::ConnectionTrait,
) -> Result<Option<account_risk_policy::Model>, AppError> {
    let row = account_risk_policy::Entity::find_by_id(SINGLETON_ID)
        .one(db)
        .await?;
    Ok(row)
}

pub async fn save(
    db: &impl sea_orm::ConnectionTrait,
    risk_policy: serde_json::Value,
) -> Result<account_risk_policy::Model, AppError> {
    let prev = find_current(db).await?;
    let model = account_risk_policy::ActiveModel {
        id: Set(SINGLETON_ID),
        risk_policy: Set(risk_policy),
        updated_at: Set(chrono::Utc::now().fixed_offset()),
    };
    let saved = account_risk_policy::Entity::insert(model)
        .on_conflict(
            OnConflict::column(account_risk_policy::Column::Id)
                .update_columns([
                    account_risk_policy::Column::RiskPolicy,
                    account_risk_policy::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec_with_returning(db)
        .await?;
    // account_risk_policy は change_history::TargetKind に対応する種別が無いため、
    // 変更前後の値をログにのみ残す。
    tracing::info!(
        from = ?prev.map(|m| m.risk_policy),
        to = ?saved.risk_policy,
        "updated account_risk_policy",
    );
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[backend_test_macros::database_test]
    async fn find_current_returns_none_when_row_missing(db: crate::database::DatabaseHandle) {
        let current = find_current(&db).await.expect("query");
        assert_eq!(current, None);
    }

    #[backend_test_macros::database_test]
    async fn save_creates_row_when_missing(db: crate::database::DatabaseHandle) {
        let saved = save(&db, serde_json::json!({ "max_sector_ratio": "0.3" }))
            .await
            .expect("save");
        assert_eq!(saved.id, 1);
        assert_eq!(
            saved.risk_policy,
            serde_json::json!({ "max_sector_ratio": "0.3" })
        );

        let current = find_current(&db).await.expect("query").expect("row exists");
        assert_eq!(current.id, 1);
        assert_eq!(
            current.risk_policy,
            serde_json::json!({ "max_sector_ratio": "0.3" })
        );
    }

    #[backend_test_macros::database_test]
    async fn save_twice_keeps_single_row_and_updates_value(db: crate::database::DatabaseHandle) {
        save(&db, serde_json::json!({ "max_sector_ratio": "0.3" }))
            .await
            .expect("save first");
        save(&db, serde_json::json!({ "max_sector_ratio": "0.4" }))
            .await
            .expect("save second");

        let rows = account_risk_policy::Entity::find()
            .all(&db)
            .await
            .expect("list");
        assert_eq!(rows.len(), 1, "must stay a single row (id=1)");
        assert_eq!(rows[0].id, 1);
        assert_eq!(
            rows[0].risk_policy,
            serde_json::json!({ "max_sector_ratio": "0.4" })
        );
    }
}
