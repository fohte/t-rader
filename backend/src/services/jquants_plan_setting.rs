//! J-Quants の契約プラン設定 (`jquants_plan_setting`)。行は常に 1 行のみ (id=1 固定)。
//! 行が存在しない間は「未設定 (自動検出を使う)」を表し、初回保存時に作成する。

use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, EntityTrait};

use crate::entities::jquants_plan_setting;
use crate::error::AppError;

const SINGLETON_ID: i16 = 1;

pub async fn find_current(
    db: &DatabaseConnection,
) -> Result<Option<jquants_plan_setting::Model>, AppError> {
    let row = jquants_plan_setting::Entity::find_by_id(SINGLETON_ID)
        .one(db)
        .await?;
    Ok(row)
}

pub async fn save(
    db: &DatabaseConnection,
    plan_setting: serde_json::Value,
) -> Result<jquants_plan_setting::Model, AppError> {
    let prev = find_current(db).await?;
    let model = jquants_plan_setting::ActiveModel {
        id: Set(SINGLETON_ID),
        plan_setting: Set(plan_setting),
        updated_at: Set(chrono::Utc::now().fixed_offset()),
    };
    let saved = jquants_plan_setting::Entity::insert(model)
        .on_conflict(
            OnConflict::column(jquants_plan_setting::Column::Id)
                .update_columns([
                    jquants_plan_setting::Column::PlanSetting,
                    jquants_plan_setting::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec_with_returning(db)
        .await?;
    // jquants_plan_setting は change_history::TargetKind に対応する種別が無いため、
    // 変更前後の値をログにのみ残す。
    tracing::info!(
        from = ?prev.map(|m| m.plan_setting),
        to = ?saved.plan_setting,
        "updated jquants_plan_setting",
    );
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;
    use crate::testing::create_test_db;

    #[sqlx::test(migrations = false)]
    async fn find_current_returns_none_when_row_missing(pool: PgPool) {
        let db = create_test_db(pool).await;

        let current = find_current(&db).await.expect("query");
        assert_eq!(current, None);
    }

    #[sqlx::test(migrations = false)]
    async fn save_creates_row_when_missing(pool: PgPool) {
        let db = create_test_db(pool).await;

        let saved = save(&db, serde_json::json!({ "plan": "standard" }))
            .await
            .expect("save");
        assert_eq!(saved.id, 1);
        assert_eq!(
            saved.plan_setting,
            serde_json::json!({ "plan": "standard" })
        );

        let current = find_current(&db).await.expect("query").expect("row exists");
        assert_eq!(current.id, 1);
        assert_eq!(
            current.plan_setting,
            serde_json::json!({ "plan": "standard" })
        );
    }

    #[sqlx::test(migrations = false)]
    async fn save_twice_keeps_single_row_and_updates_value(pool: PgPool) {
        let db = create_test_db(pool).await;

        save(&db, serde_json::json!({ "plan": "standard" }))
            .await
            .expect("save first");
        save(&db, serde_json::json!({ "plan": "premium" }))
            .await
            .expect("save second");

        let rows = jquants_plan_setting::Entity::find()
            .all(&db)
            .await
            .expect("list");
        assert_eq!(rows.len(), 1, "must stay a single row (id=1)");
        assert_eq!(rows[0].id, 1);
        assert_eq!(
            rows[0].plan_setting,
            serde_json::json!({ "plan": "premium" })
        );
    }
}
