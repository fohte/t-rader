use chrono::NaiveDate;
use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, EntityTrait, QueryOrder};

use crate::entities::margin_interest;
use crate::error::AppError;
use crate::models::margin::MarginInterestRecord;

/// 信用取引週末残高を一括 upsert する。複合 PK (date, code, iss_type) で重複排除し、
/// 既存行は数量・金額カラムを更新する (J-Quants の訂正は上書きで反映されるため)。
pub async fn upsert_margin_interest(
    db: &DatabaseConnection,
    records: Vec<MarginInterestRecord>,
) -> Result<(), AppError> {
    if records.is_empty() {
        return Ok(());
    }

    let active_models: Vec<margin_interest::ActiveModel> =
        records.into_iter().map(Into::into).collect();

    margin_interest::Entity::insert_many(active_models)
        .on_conflict(
            OnConflict::columns([
                margin_interest::Column::Date,
                margin_interest::Column::Code,
                margin_interest::Column::IssType,
            ])
            .update_columns([
                margin_interest::Column::ShrtVol,
                margin_interest::Column::LongVol,
                margin_interest::Column::ShrtNegVol,
                margin_interest::Column::LongNegVol,
                margin_interest::Column::ShrtStdVol,
                margin_interest::Column::LongStdVol,
                margin_interest::Column::ShrtVal,
                margin_interest::Column::LongVal,
                margin_interest::Column::ShrtNegVal,
                margin_interest::Column::LongNegVal,
                margin_interest::Column::ShrtStdVal,
                margin_interest::Column::LongStdVal,
            ])
            .to_owned(),
        )
        .exec_without_returning(db)
        .await?;

    Ok(())
}

/// テーブル中の最新日付を返す。1 行も無ければ None。
pub async fn find_latest_margin_interest_date(
    db: &DatabaseConnection,
) -> Result<Option<NaiveDate>, AppError> {
    let latest = margin_interest::Entity::find()
        .order_by_desc(margin_interest::Column::Date)
        .one(db)
        .await?;
    Ok(latest.map(|m| m.date))
}

#[cfg(test)]
mod tests {
    use sea_orm::DatabaseConnection;
    use sqlx::PgPool;

    use super::*;
    use crate::testing::create_test_db;

    fn make_record(date: NaiveDate, code: &str, shrt_vol: i64) -> MarginInterestRecord {
        MarginInterestRecord {
            date,
            code: code.to_string(),
            iss_type: 1,
            shrt_vol,
            long_vol: 0,
            shrt_neg_vol: 0,
            long_neg_vol: 0,
            shrt_std_vol: 0,
            long_std_vol: 0,
            shrt_val: None,
            long_val: None,
            shrt_neg_val: None,
            long_neg_val: None,
            shrt_std_val: None,
            long_std_val: None,
        }
    }

    #[sqlx::test(migrations = false)]
    async fn upsert_inserts_and_updates_on_conflict(pool: PgPool) {
        let db: DatabaseConnection = create_test_db(pool).await;
        let date = NaiveDate::from_ymd_opt(2024, 1, 5).expect("date");

        upsert_margin_interest(&db, vec![make_record(date, "7203", 100)])
            .await
            .expect("insert");
        upsert_margin_interest(&db, vec![make_record(date, "7203", 200)])
            .await
            .expect("update via conflict");

        let latest = find_latest_margin_interest_date(&db)
            .await
            .expect("query ok");
        assert_eq!(latest, Some(date));
    }

    #[sqlx::test(migrations = false)]
    async fn upsert_with_empty_vec_is_noop(pool: PgPool) {
        let db = create_test_db(pool).await;
        let result = upsert_margin_interest(&db, vec![]).await;
        assert!(result.is_ok());
    }

    #[sqlx::test(migrations = false)]
    async fn find_latest_returns_none_when_empty(pool: PgPool) {
        let db = create_test_db(pool).await;
        let latest = find_latest_margin_interest_date(&db)
            .await
            .expect("query ok");
        assert_eq!(latest, None);
    }

    #[sqlx::test(migrations = false)]
    async fn find_latest_returns_max_date(pool: PgPool) {
        let db = create_test_db(pool).await;
        let older = NaiveDate::from_ymd_opt(2024, 1, 5).expect("date");
        let newer = NaiveDate::from_ymd_opt(2024, 1, 12).expect("date");
        upsert_margin_interest(
            &db,
            vec![make_record(older, "7203", 1), make_record(newer, "7203", 2)],
        )
        .await
        .expect("insert");

        let latest = find_latest_margin_interest_date(&db)
            .await
            .expect("query ok");
        assert_eq!(latest, Some(newer));
    }
}
