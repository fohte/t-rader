use chrono::NaiveDate;
use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, EntityTrait, QueryOrder};

use crate::entities::margin_alert;
use crate::error::AppError;
use crate::models::margin::MarginAlertRecord;

/// 日々公表信用取引残高を一括 upsert する。複合 PK (pub_date, code) で重複排除する。
/// 訂正は同じ AppDate で PubDate が新しい行として追加されるため通常は新規 insert のみが
/// 発生するが、再取得時の冪等性のため ON CONFLICT DO UPDATE にしておく。
pub async fn upsert_margin_alert(
    db: &DatabaseConnection,
    records: Vec<MarginAlertRecord>,
) -> Result<(), AppError> {
    if records.is_empty() {
        return Ok(());
    }

    let active_models: Vec<margin_alert::ActiveModel> =
        records.into_iter().map(Into::into).collect();

    margin_alert::Entity::insert_many(active_models)
        .on_conflict(
            OnConflict::columns([margin_alert::Column::PubDate, margin_alert::Column::Code])
                .update_columns([
                    margin_alert::Column::AppDate,
                    margin_alert::Column::PubReason,
                    margin_alert::Column::ShrtOut,
                    margin_alert::Column::LongOut,
                    margin_alert::Column::ShrtOutChg,
                    margin_alert::Column::LongOutChg,
                    margin_alert::Column::ShrtOutRatio,
                    margin_alert::Column::LongOutRatio,
                    margin_alert::Column::SlRatio,
                    margin_alert::Column::ShrtNegOut,
                    margin_alert::Column::ShrtStdOut,
                    margin_alert::Column::LongNegOut,
                    margin_alert::Column::LongStdOut,
                    margin_alert::Column::TseMrgnRegCls,
                ])
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;

    Ok(())
}

/// テーブル中の最新公表日 (PubDate) を返す。1 行も無ければ None。
pub async fn find_latest_margin_alert_pub_date(
    db: &DatabaseConnection,
) -> Result<Option<NaiveDate>, AppError> {
    let latest = margin_alert::Entity::find()
        .order_by_desc(margin_alert::Column::PubDate)
        .one(db)
        .await?;
    Ok(latest.map(|m| m.pub_date))
}

#[cfg(test)]
mod tests {
    use sea_orm::{DatabaseConnection, EntityTrait};
    use sqlx::PgPool;

    use super::*;
    use crate::models::margin::PubReason;
    use crate::testing::create_test_db;

    fn make_record(pub_date: NaiveDate, code: &str, app_date: NaiveDate) -> MarginAlertRecord {
        MarginAlertRecord {
            pub_date,
            code: code.to_string(),
            app_date,
            pub_reason: PubReason {
                restricted: false,
                daily_publication: true,
                monitoring: false,
                restricted_by_jsf: false,
                precaution_by_jsf: false,
                unclear_or_sec_on_alert: false,
            },
            shrt_out: 100,
            long_out: 200,
            shrt_out_chg: None,
            long_out_chg: Some(10),
            shrt_out_ratio: None,
            long_out_ratio: None,
            sl_ratio: None,
            shrt_neg_out: 10,
            shrt_std_out: 90,
            long_neg_out: 20,
            long_std_out: 180,
            tse_mrgn_reg_cls: "001".to_string(),
        }
    }

    #[sqlx::test(migrations = false)]
    async fn upsert_keeps_correction_rows_with_different_pub_date(pool: PgPool) {
        let db: DatabaseConnection = create_test_db(pool).await;
        let app_date = NaiveDate::from_ymd_opt(2024, 2, 7).expect("date");
        let original_pub_date = NaiveDate::from_ymd_opt(2024, 2, 8).expect("date");
        let correction_pub_date = NaiveDate::from_ymd_opt(2024, 2, 9).expect("date");

        upsert_margin_alert(&db, vec![make_record(original_pub_date, "27800", app_date)])
            .await
            .expect("insert original");
        upsert_margin_alert(
            &db,
            vec![make_record(correction_pub_date, "27800", app_date)],
        )
        .await
        .expect("insert correction");

        let latest = find_latest_margin_alert_pub_date(&db)
            .await
            .expect("query ok");
        assert_eq!(latest, Some(correction_pub_date));

        // 訂正前の行が上書きで消えていないことを確認する
        let all = margin_alert::Entity::find()
            .all(&db)
            .await
            .expect("query all");
        assert_eq!(all.len(), 2);
    }

    #[sqlx::test(migrations = false)]
    async fn upsert_with_empty_vec_is_noop(pool: PgPool) {
        let db = create_test_db(pool).await;
        let result = upsert_margin_alert(&db, vec![]).await;
        assert!(result.is_ok());
    }

    #[sqlx::test(migrations = false)]
    async fn find_latest_returns_none_when_empty(pool: PgPool) {
        let db = create_test_db(pool).await;
        let latest = find_latest_margin_alert_pub_date(&db)
            .await
            .expect("query ok");
        assert_eq!(latest, None);
    }
}
