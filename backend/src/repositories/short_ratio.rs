use chrono::NaiveDate;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ConnectionTrait, EntityTrait, QueryOrder, Set};

use crate::entities::short_ratio;
use crate::error::AppError;
use crate::models::ShortRatio;

impl From<ShortRatio> for short_ratio::ActiveModel {
    fn from(ratio: ShortRatio) -> Self {
        short_ratio::ActiveModel {
            date: Set(ratio.date),
            sector33_code: Set(ratio.sector33_code),
            sell_excluding_short_value: Set(ratio.sell_excluding_short_value),
            short_with_restriction_value: Set(ratio.short_with_restriction_value),
            short_without_restriction_value: Set(ratio.short_without_restriction_value),
        }
    }
}

/// 業種別空売り比率を一括 upsert する
///
/// 複合 PK (date, sector33_code) で重複排除し、既存行は売買代金カラムを更新する
/// (訂正の反映)。
pub async fn upsert_short_ratios<C>(db: &C, ratios: Vec<ShortRatio>) -> Result<(), AppError>
where
    C: ConnectionTrait,
{
    if ratios.is_empty() {
        return Ok(());
    }

    let active_models: Vec<short_ratio::ActiveModel> = ratios.into_iter().map(Into::into).collect();

    short_ratio::Entity::insert_many(active_models)
        .on_conflict(
            OnConflict::columns([short_ratio::Column::Date, short_ratio::Column::Sector33Code])
                .update_columns([
                    short_ratio::Column::SellExcludingShortValue,
                    short_ratio::Column::ShortWithRestrictionValue,
                    short_ratio::Column::ShortWithoutRestrictionValue,
                ])
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;

    Ok(())
}

/// DB 上の最新の対象日を返す。1 件も無ければ `None`。
pub async fn find_latest_date<C>(db: &C) -> Result<Option<NaiveDate>, AppError>
where
    C: ConnectionTrait,
{
    let result = short_ratio::Entity::find()
        .order_by_desc(short_ratio::Column::Date)
        .one(db)
        .await?;
    Ok(result.map(|m| m.date))
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use sea_orm::{ConnectOptions, Database, DatabaseConnection, TransactionTrait};
    use sqlx::PgPool;

    use super::*;
    use crate::testing::create_test_db;

    /// テスト用の業種別空売り比率を生成する
    fn make_ratio(date: NaiveDate, sector33_code: &str, value: i64) -> ShortRatio {
        ShortRatio {
            date,
            sector33_code: sector33_code.to_string(),
            sell_excluding_short_value: Some(Decimal::new(value, 0)),
            short_with_restriction_value: Some(Decimal::new(value, 0)),
            short_without_restriction_value: Some(Decimal::new(value, 0)),
        }
    }

    async fn connect_with_application_name(application_name: &str) -> DatabaseConnection {
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let mut options = ConnectOptions::new(database_url);
        options.set_application_name(application_name);
        Database::connect(options)
            .await
            .expect("database connection")
    }

    #[sqlx::test(migrations = false)]
    async fn upsert_inserts_new_records(pool: PgPool) {
        let db = create_test_db(pool).await;
        let date = NaiveDate::from_ymd_opt(2025, 1, 6).expect("date");

        let ratios = vec![make_ratio(date, "0050", 100), make_ratio(date, "3050", 200)];
        upsert_short_ratios(&db, ratios)
            .await
            .expect("upsert failed");

        let rows = short_ratio::Entity::find()
            .all(&db)
            .await
            .expect("find failed");
        assert_eq!(rows.len(), 2);
    }

    #[tokio::test]
    async fn transaction_rollback_inserts_new_records() {
        let db = connect_with_application_name("h8-poc-primary").await;
        let txn = db.begin().await.expect("begin outer transaction");
        let date = NaiveDate::from_ymd_opt(2025, 1, 6).expect("date");

        let ratios = vec![make_ratio(date, "H801", 100), make_ratio(date, "H802", 200)];
        upsert_short_ratios(&txn, ratios)
            .await
            .expect("upsert failed");

        let rows = short_ratio::Entity::find()
            .all(&txn)
            .await
            .expect("find failed");
        assert_eq!(rows.len(), 2);

        txn.rollback().await.expect("rollback outer transaction");
        db.close().await.expect("close primary connection");

        let verifier = connect_with_application_name("h8-poc-verify").await;
        let remaining_rows = short_ratio::Entity::find()
            .all(&verifier)
            .await
            .expect("verify rollback");
        assert_eq!(remaining_rows, Vec::new());
        verifier.close().await.expect("close verifier connection");
    }

    #[sqlx::test(migrations = false)]
    async fn upsert_updates_existing_record_on_correction(pool: PgPool) {
        let db = create_test_db(pool).await;
        let date = NaiveDate::from_ymd_opt(2025, 1, 6).expect("date");

        upsert_short_ratios(&db, vec![make_ratio(date, "0050", 100)])
            .await
            .expect("first upsert failed");

        upsert_short_ratios(&db, vec![make_ratio(date, "0050", 999)])
            .await
            .expect("second upsert failed");

        let rows = short_ratio::Entity::find()
            .all(&db)
            .await
            .expect("find failed");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].sell_excluding_short_value,
            Some(Decimal::new(999, 0))
        );
    }

    #[sqlx::test(migrations = false)]
    async fn upsert_with_empty_vec_is_noop(pool: PgPool) {
        let db = create_test_db(pool).await;

        let result = upsert_short_ratios(&db, vec![]).await;
        assert!(result.is_ok());
    }

    #[sqlx::test(migrations = false)]
    async fn find_latest_date_returns_most_recent(pool: PgPool) {
        let db = create_test_db(pool).await;
        let d1 = NaiveDate::from_ymd_opt(2025, 1, 6).expect("date");
        let d2 = NaiveDate::from_ymd_opt(2025, 1, 8).expect("date");
        let d3 = NaiveDate::from_ymd_opt(2025, 1, 7).expect("date");

        upsert_short_ratios(
            &db,
            vec![
                make_ratio(d1, "0050", 100),
                make_ratio(d2, "0050", 100),
                make_ratio(d3, "0050", 100),
            ],
        )
        .await
        .expect("upsert failed");

        let result = find_latest_date(&db).await.expect("find failed");
        assert_eq!(result, Some(d2));
    }

    #[sqlx::test(migrations = false)]
    async fn find_latest_date_returns_none_when_empty(pool: PgPool) {
        let db = create_test_db(pool).await;

        let result = find_latest_date(&db).await.expect("find failed");
        assert_eq!(result, None);
    }
}
