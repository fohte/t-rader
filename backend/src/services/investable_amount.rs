//! 戦略ごとの投資可能額 (history テーブル)。行は追記のみで、更新・削除はしない。
//! 「現在の値」は `effective_at` が現在時刻以下の最新行として求める。

use chrono::{DateTime, FixedOffset, Utc};
use rust_decimal::Decimal;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::entities::strategy_investable_amount;
use crate::error::AppError;

/// `effective_at` が現在時刻以下の最新行を返す。1 行も無ければ `None`。
/// `effective_at` が同値の行が複数あった場合は `created_at` が新しい方を優先する。
pub async fn find_current(
    db: &DatabaseConnection,
    strategy_id: Uuid,
) -> Result<Option<strategy_investable_amount::Model>, AppError> {
    let now = Utc::now().fixed_offset();
    let row = strategy_investable_amount::Entity::find()
        .filter(strategy_investable_amount::Column::StrategyId.eq(strategy_id))
        .filter(strategy_investable_amount::Column::EffectiveAt.lte(now))
        .order_by_desc(strategy_investable_amount::Column::EffectiveAt)
        .order_by_desc(strategy_investable_amount::Column::CreatedAt)
        .one(db)
        .await?;
    Ok(row)
}

/// 新しい history 行を追記する。既存行は変更しない。
pub async fn record(
    db: &DatabaseConnection,
    strategy_id: Uuid,
    amount_jpy: Decimal,
    effective_at: DateTime<FixedOffset>,
) -> Result<strategy_investable_amount::Model, AppError> {
    let model = strategy_investable_amount::ActiveModel {
        id: Set(Uuid::new_v4()),
        strategy_id: Set(strategy_id),
        amount_jpy: Set(amount_jpy),
        effective_at: Set(effective_at),
        created_at: NotSet,
    };
    let created = strategy_investable_amount::Entity::insert(model)
        .exec_with_returning(db)
        .await?;
    Ok(created)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use chrono::Duration;

    use super::*;
    use crate::testing::{create_test_db, insert_test_strategy};

    #[sqlx::test(migrations = false)]
    async fn find_current_returns_none_when_no_history(pool: sqlx::PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "s").await;

        let current = find_current(&db, strategy_id).await.expect("query");
        assert_eq!(current, None);
    }

    #[sqlx::test(migrations = false)]
    async fn find_current_ignores_future_effective_at(pool: sqlx::PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let now = Utc::now().fixed_offset();

        record(
            &db,
            strategy_id,
            Decimal::from_str("1000000").unwrap(),
            now - Duration::days(1),
        )
        .await
        .expect("record past");
        record(
            &db,
            strategy_id,
            Decimal::from_str("9999999").unwrap(),
            now + Duration::days(1),
        )
        .await
        .expect("record future");

        let current = find_current(&db, strategy_id)
            .await
            .expect("query")
            .expect("row exists");
        assert_eq!(current.amount_jpy, Decimal::from_str("1000000").unwrap());
    }

    #[sqlx::test(migrations = false)]
    async fn find_current_returns_latest_of_multiple_past_rows(pool: sqlx::PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let now = Utc::now().fixed_offset();

        record(
            &db,
            strategy_id,
            Decimal::from_str("1000000").unwrap(),
            now - Duration::days(2),
        )
        .await
        .expect("record older");
        record(
            &db,
            strategy_id,
            Decimal::from_str("2000000").unwrap(),
            now - Duration::days(1),
        )
        .await
        .expect("record newer");

        let current = find_current(&db, strategy_id)
            .await
            .expect("query")
            .expect("row exists");
        assert_eq!(current.amount_jpy, Decimal::from_str("2000000").unwrap());

        let count = strategy_investable_amount::Entity::find()
            .filter(strategy_investable_amount::Column::StrategyId.eq(strategy_id))
            .all(&db)
            .await
            .expect("list")
            .len();
        assert_eq!(count, 2, "both history rows must remain (no overwrite)");
    }
}
