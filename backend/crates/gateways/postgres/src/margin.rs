use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::margin::{
    MarginQuery, MarginReadResult, MarginRepository, MarginRepositoryError,
};
use core_application::persistence::PersistenceError;
use core_domain::margin::{MarginAlertRecord, MarginInterestRecord, PubReason};
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};

use crate::DatabaseHandle;
use crate::persistence::persistence_error;
use crate::repositories::{margin_alert, margin_interest};

#[derive(Clone)]
pub struct PostgresMarginRepository {
    db: DatabaseHandle,
}

impl PostgresMarginRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MarginRepository for PostgresMarginRepository {
    async fn upsert_margin_interest(
        &self,
        records: Vec<MarginInterestRecord>,
    ) -> Result<(), MarginRepositoryError> {
        margin_interest::upsert_margin_interest(&self.db, records)
            .await
            .map_err(repository_error)
    }

    async fn find_latest_margin_interest_date(
        &self,
    ) -> Result<Option<NaiveDate>, MarginRepositoryError> {
        margin_interest::find_latest_margin_interest_date(&self.db)
            .await
            .map_err(repository_error)
    }

    async fn upsert_margin_alert(
        &self,
        records: Vec<MarginAlertRecord>,
    ) -> Result<(), MarginRepositoryError> {
        margin_alert::upsert_margin_alert(&self.db, records)
            .await
            .map_err(repository_error)
    }

    async fn find_latest_margin_alert_pub_date(
        &self,
    ) -> Result<Option<NaiveDate>, MarginRepositoryError> {
        margin_alert::find_latest_margin_alert_pub_date(&self.db)
            .await
            .map_err(repository_error)
    }

    async fn read(&self, query: MarginQuery) -> Result<MarginReadResult, MarginRepositoryError> {
        let values = [
            query.symbol.into(),
            query.from.into(),
            query.to.into(),
            (query.limit.min(i64::MAX as u64) as i64).into(),
        ];
        let interest_rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                READ_MARGIN_INTEREST_SQL,
                values.clone(),
            ))
            .await
            .map_err(repository_error)?;
        let interest = interest_rows
            .iter()
            .map(|row| MarginInterestRow::from_query_result(row, ""))
            .collect::<Result<Vec<_>, _>>()
            .map_err(repository_error)?
            .into_iter()
            .map(MarginInterestRow::into_record)
            .collect();

        let alert_rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                READ_MARGIN_ALERT_SQL,
                values,
            ))
            .await
            .map_err(repository_error)?;
        let alerts = alert_rows
            .iter()
            .map(|row| MarginAlertRow::from_query_result(row, ""))
            .collect::<Result<Vec<_>, _>>()
            .map_err(repository_error)?
            .into_iter()
            .map(MarginAlertRow::into_record)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(MarginReadResult { interest, alerts })
    }
}

const READ_MARGIN_INTEREST_SQL: &str = r#"
    SELECT date, code, iss_type, shrt_vol, long_vol, shrt_neg_vol, long_neg_vol,
        shrt_std_vol, long_std_vol, shrt_val, long_val, shrt_neg_val, long_neg_val,
        shrt_std_val, long_std_val
    FROM margin_interest
    WHERE LEFT(code, 4) = $1
        AND ($2::date IS NULL OR date >= $2::date)
        AND ($3::date IS NULL OR date <= $3::date)
    ORDER BY date DESC, code, iss_type
    LIMIT $4
"#;

const READ_MARGIN_ALERT_SQL: &str = r#"
    WITH deduped AS (
        SELECT DISTINCT ON (app_date, code)
            pub_date, code, app_date, pub_reason, shrt_out, long_out, shrt_out_chg,
            long_out_chg, shrt_out_ratio, long_out_ratio, sl_ratio, shrt_neg_out,
            shrt_std_out, long_neg_out, long_std_out, tse_mrgn_reg_cls
        FROM margin_alert
        WHERE LEFT(code, 4) = $1
            AND ($2::date IS NULL OR app_date >= $2::date)
            AND ($3::date IS NULL OR app_date <= $3::date)
        ORDER BY app_date, code, pub_date DESC
    )
    SELECT * FROM deduped
    ORDER BY app_date DESC, code
    LIMIT $4
"#;

#[derive(Debug, FromQueryResult)]
struct MarginInterestRow {
    date: NaiveDate,
    code: String,
    iss_type: i16,
    shrt_vol: i64,
    long_vol: i64,
    shrt_neg_vol: i64,
    long_neg_vol: i64,
    shrt_std_vol: i64,
    long_std_vol: i64,
    shrt_val: Option<i64>,
    long_val: Option<i64>,
    shrt_neg_val: Option<i64>,
    long_neg_val: Option<i64>,
    shrt_std_val: Option<i64>,
    long_std_val: Option<i64>,
}

impl MarginInterestRow {
    fn into_record(self) -> MarginInterestRecord {
        MarginInterestRecord {
            date: self.date,
            code: self.code,
            iss_type: self.iss_type,
            shrt_vol: self.shrt_vol,
            long_vol: self.long_vol,
            shrt_neg_vol: self.shrt_neg_vol,
            long_neg_vol: self.long_neg_vol,
            shrt_std_vol: self.shrt_std_vol,
            long_std_vol: self.long_std_vol,
            shrt_val: self.shrt_val,
            long_val: self.long_val,
            shrt_neg_val: self.shrt_neg_val,
            long_neg_val: self.long_neg_val,
            shrt_std_val: self.shrt_std_val,
            long_std_val: self.long_std_val,
        }
    }
}

#[derive(Debug, FromQueryResult)]
struct MarginAlertRow {
    pub_date: NaiveDate,
    code: String,
    app_date: NaiveDate,
    pub_reason: serde_json::Value,
    shrt_out: i64,
    long_out: i64,
    shrt_out_chg: Option<i64>,
    long_out_chg: Option<i64>,
    shrt_out_ratio: Option<Decimal>,
    long_out_ratio: Option<Decimal>,
    sl_ratio: Option<Decimal>,
    shrt_neg_out: i64,
    shrt_std_out: i64,
    long_neg_out: i64,
    long_std_out: i64,
    tse_mrgn_reg_cls: String,
}

impl MarginAlertRow {
    fn into_record(self) -> Result<MarginAlertRecord, MarginRepositoryError> {
        let pub_reason = serde_json::from_value::<PubReason>(self.pub_reason).map_err(|error| {
            MarginRepositoryError::Database(PersistenceError::Database(format!(
                "invalid margin_alert.pub_reason: {error}"
            )))
        })?;
        Ok(MarginAlertRecord {
            pub_date: self.pub_date,
            code: self.code,
            app_date: self.app_date,
            pub_reason,
            shrt_out: self.shrt_out,
            long_out: self.long_out,
            shrt_out_chg: self.shrt_out_chg,
            long_out_chg: self.long_out_chg,
            shrt_out_ratio: self.shrt_out_ratio,
            long_out_ratio: self.long_out_ratio,
            sl_ratio: self.sl_ratio,
            shrt_neg_out: self.shrt_neg_out,
            shrt_std_out: self.shrt_std_out,
            long_neg_out: self.long_neg_out,
            long_std_out: self.long_std_out,
            tse_mrgn_reg_cls: self.tse_mrgn_reg_cls,
        })
    }
}

fn repository_error(error: sea_orm::DbErr) -> MarginRepositoryError {
    MarginRepositoryError::Database(persistence_error(error))
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::margin::{MarginQuery, MarginReadResult, MarginRepository};
    use core_domain::margin::{MarginAlertRecord, MarginInterestRecord, PubReason};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;

    use crate::PostgresMarginRepository;
    use crate::entities::{margin_alert, margin_interest};

    fn ymd(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn interest(date: NaiveDate, code: &str, iss_type: i16, shrt_vol: i64) -> MarginInterestRecord {
        MarginInterestRecord {
            date,
            code: code.into(),
            iss_type,
            shrt_vol,
            long_vol: 2,
            shrt_neg_vol: 3,
            long_neg_vol: 4,
            shrt_std_vol: 5,
            long_std_vol: 6,
            shrt_val: Some(7),
            long_val: Some(8),
            shrt_neg_val: Some(9),
            long_neg_val: Some(10),
            shrt_std_val: Some(11),
            long_std_val: Some(12),
        }
    }

    fn alert(
        pub_date: NaiveDate,
        app_date: NaiveDate,
        code: &str,
        shrt_out: i64,
    ) -> MarginAlertRecord {
        MarginAlertRecord {
            pub_date,
            code: code.into(),
            app_date,
            pub_reason: PubReason {
                restricted: true,
                daily_publication: false,
                monitoring: true,
                restricted_by_jsf: false,
                precaution_by_jsf: false,
                unclear_or_sec_on_alert: false,
            },
            shrt_out,
            long_out: 20,
            shrt_out_chg: Some(1),
            long_out_chg: Some(2),
            shrt_out_ratio: None,
            long_out_ratio: None,
            sl_ratio: None,
            shrt_neg_out: 3,
            shrt_std_out: 4,
            long_neg_out: 5,
            long_std_out: 6,
            tse_mrgn_reg_cls: "001".into(),
        }
    }

    async fn insert_interest(db: &impl sea_orm::ConnectionTrait, record: MarginInterestRecord) {
        margin_interest::ActiveModel {
            date: Set(record.date),
            code: Set(record.code),
            iss_type: Set(record.iss_type),
            shrt_vol: Set(record.shrt_vol),
            long_vol: Set(record.long_vol),
            shrt_neg_vol: Set(record.shrt_neg_vol),
            long_neg_vol: Set(record.long_neg_vol),
            shrt_std_vol: Set(record.shrt_std_vol),
            long_std_vol: Set(record.long_std_vol),
            shrt_val: Set(record.shrt_val),
            long_val: Set(record.long_val),
            shrt_neg_val: Set(record.shrt_neg_val),
            long_neg_val: Set(record.long_neg_val),
            shrt_std_val: Set(record.shrt_std_val),
            long_std_val: Set(record.long_std_val),
        }
        .insert(db)
        .await
        .expect("insert margin interest");
    }

    async fn insert_alert(db: &impl sea_orm::ConnectionTrait, record: MarginAlertRecord) {
        margin_alert::ActiveModel {
            pub_date: Set(record.pub_date),
            code: Set(record.code),
            app_date: Set(record.app_date),
            pub_reason: Set(serde_json::to_value(record.pub_reason).expect("serialize reason")),
            shrt_out: Set(record.shrt_out),
            long_out: Set(record.long_out),
            shrt_out_chg: Set(record.shrt_out_chg),
            long_out_chg: Set(record.long_out_chg),
            shrt_out_ratio: Set(record.shrt_out_ratio),
            long_out_ratio: Set(record.long_out_ratio),
            sl_ratio: Set(record.sl_ratio),
            shrt_neg_out: Set(record.shrt_neg_out),
            shrt_std_out: Set(record.shrt_std_out),
            long_neg_out: Set(record.long_neg_out),
            long_std_out: Set(record.long_std_out),
            tse_mrgn_reg_cls: Set(record.tse_mrgn_reg_cls),
        }
        .insert(db)
        .await
        .expect("insert margin alert");
    }

    #[backend_test_macros::database_test]
    async fn read_preserves_interest_filters_order_and_alert_correction_selection(
        db: crate::DatabaseHandle,
    ) {
        let day = ymd(2024, 1, 2);
        insert_interest(&db, interest(ymd(2024, 1, 1), "12340", 1, 1)).await;
        insert_interest(&db, interest(day, "12341", 1, 2)).await;
        insert_interest(&db, interest(day, "12340", 2, 3)).await;
        insert_interest(&db, interest(day, "12340", 1, 4)).await;
        insert_interest(&db, interest(day, "99990", 1, 5)).await;

        insert_alert(&db, alert(ymd(2024, 1, 3), day, "12340", 10)).await;
        insert_alert(&db, alert(ymd(2024, 1, 4), day, "12340", 20)).await;
        insert_alert(&db, alert(ymd(2024, 1, 4), day, "12341", 30)).await;
        insert_alert(&db, alert(ymd(2024, 1, 5), ymd(2024, 1, 1), "12340", 40)).await;
        insert_alert(&db, alert(ymd(2024, 1, 4), day, "99990", 50)).await;

        let repository = PostgresMarginRepository::new(db);
        let actual = repository
            .read(MarginQuery {
                symbol: "1234".into(),
                from: Some(day),
                to: Some(day),
                limit: 3,
            })
            .await
            .expect("read margin");

        assert_eq!(
            actual,
            MarginReadResult {
                interest: vec![
                    interest(day, "12340", 1, 4),
                    interest(day, "12340", 2, 3),
                    interest(day, "12341", 1, 2),
                ],
                alerts: vec![
                    alert(ymd(2024, 1, 4), day, "12340", 20),
                    alert(ymd(2024, 1, 4), day, "12341", 30),
                ],
            },
        );
    }
}
