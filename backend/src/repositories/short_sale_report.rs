use chrono::NaiveDate;
use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, EntityTrait, QueryOrder};

use crate::entities::short_sale_report;
use crate::error::AppError;
use crate::models::ShortSaleReport;

/// 空売り残高報告を一括 upsert する
///
/// 複合 PK (disc_date, code, ss_name, ss_addr, dic_name, dic_addr, fund_name) で
/// 重複排除し、既存行は残高・比率等のカラムを更新する (訂正の反映)。
pub async fn upsert_short_sale_reports(
    db: &DatabaseConnection,
    reports: Vec<ShortSaleReport>,
) -> Result<(), AppError> {
    if reports.is_empty() {
        return Ok(());
    }

    let active_models: Vec<short_sale_report::ActiveModel> =
        reports.into_iter().map(Into::into).collect();

    short_sale_report::Entity::insert_many(active_models)
        .on_conflict(
            OnConflict::columns([
                short_sale_report::Column::DiscDate,
                short_sale_report::Column::Code,
                short_sale_report::Column::SsName,
                short_sale_report::Column::SsAddr,
                short_sale_report::Column::DicName,
                short_sale_report::Column::DicAddr,
                short_sale_report::Column::FundName,
            ])
            .update_columns([
                short_sale_report::Column::CalcDate,
                short_sale_report::Column::ShortPositionRatio,
                short_sale_report::Column::ShortPositionShares,
                short_sale_report::Column::ShortPositionUnits,
                short_sale_report::Column::PrevReportDate,
                short_sale_report::Column::PrevReportRatio,
                short_sale_report::Column::Notes,
            ])
            .to_owned(),
        )
        .exec_without_returning(db)
        .await?;

    Ok(())
}

/// DB 上の最新の公表日を返す。1 件も無ければ `None`。
pub async fn find_latest_disc_date(db: &DatabaseConnection) -> Result<Option<NaiveDate>, AppError> {
    let result = short_sale_report::Entity::find()
        .order_by_desc(short_sale_report::Column::DiscDate)
        .one(db)
        .await?;
    Ok(result.map(|m| m.disc_date))
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use sqlx::PgPool;

    use super::*;
    use crate::testing::create_test_db;

    /// テスト用の空売り残高報告を生成する
    fn make_report(disc_date: NaiveDate, code: &str, ss_name: &str, ratio: f64) -> ShortSaleReport {
        ShortSaleReport {
            disc_date,
            calc_date: disc_date,
            code: code.to_string(),
            ss_name: ss_name.to_string(),
            ss_addr: "東京都".to_string(),
            dic_name: "テスト委託者".to_string(),
            dic_addr: "東京都".to_string(),
            fund_name: String::new(),
            short_position_ratio: Decimal::try_from(ratio).expect("decimal"),
            short_position_shares: 1000,
            short_position_units: 10,
            prev_report_date: None,
            prev_report_ratio: None,
            notes: String::new(),
        }
    }

    #[sqlx::test(migrations = false)]
    async fn upsert_inserts_new_records(pool: PgPool) {
        let db = create_test_db(pool).await;
        let date = NaiveDate::from_ymd_opt(2025, 1, 6).expect("date");

        let reports = vec![
            make_report(date, "7203", "報告者A", 0.05),
            make_report(date, "7203", "報告者B", 0.03),
        ];
        upsert_short_sale_reports(&db, reports)
            .await
            .expect("upsert failed");

        let rows = short_sale_report::Entity::find()
            .all(&db)
            .await
            .expect("find failed");
        assert_eq!(rows.len(), 2);
    }

    #[sqlx::test(migrations = false)]
    async fn upsert_updates_existing_record_on_correction(pool: PgPool) {
        let db = create_test_db(pool).await;
        let date = NaiveDate::from_ymd_opt(2025, 1, 6).expect("date");

        upsert_short_sale_reports(&db, vec![make_report(date, "7203", "報告者A", 0.05)])
            .await
            .expect("first upsert failed");

        // 同じ自然キーで割合を訂正
        upsert_short_sale_reports(&db, vec![make_report(date, "7203", "報告者A", 0.08)])
            .await
            .expect("second upsert failed");

        let rows = short_sale_report::Entity::find()
            .all(&db)
            .await
            .expect("find failed");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].short_position_ratio,
            Decimal::try_from(0.08).expect("decimal")
        );
    }

    #[sqlx::test(migrations = false)]
    async fn upsert_with_empty_vec_is_noop(pool: PgPool) {
        let db = create_test_db(pool).await;

        let result = upsert_short_sale_reports(&db, vec![]).await;
        assert!(result.is_ok());
    }

    #[sqlx::test(migrations = false)]
    async fn find_latest_disc_date_returns_most_recent(pool: PgPool) {
        let db = create_test_db(pool).await;
        let d1 = NaiveDate::from_ymd_opt(2025, 1, 6).expect("date");
        let d2 = NaiveDate::from_ymd_opt(2025, 1, 8).expect("date");
        let d3 = NaiveDate::from_ymd_opt(2025, 1, 7).expect("date");

        upsert_short_sale_reports(
            &db,
            vec![
                make_report(d1, "7203", "報告者A", 0.05),
                make_report(d2, "7203", "報告者A", 0.05),
                make_report(d3, "7203", "報告者A", 0.05),
            ],
        )
        .await
        .expect("upsert failed");

        let result = find_latest_disc_date(&db).await.expect("find failed");
        assert_eq!(result, Some(d2));
    }

    #[sqlx::test(migrations = false)]
    async fn find_latest_disc_date_returns_none_when_empty(pool: PgPool) {
        let db = create_test_db(pool).await;

        let result = find_latest_disc_date(&db).await.expect("find failed");
        assert_eq!(result, None);
    }
}
