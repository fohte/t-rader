//! 戦略実行 MCP の `read_margin` tool。信用取引週末残高/信用取引残高 (`margin_interest`)
//! と日々公表信用取引残高 (`margin_alert`) を 4 桁銘柄コード + 期間で読み出す。

use chrono::NaiveDate;
use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use uuid::Uuid;

use crate::models::PubReason;

use super::dto::{MarginAlertDto, MarginInterestDto, ReadMarginParams, ReadMarginResult};
use super::{StrategyServer, clamp_limit, db_error, decimal_to_f64, internal_error};

const READ_MARGIN_INTEREST_SQL: &str = indoc::indoc! {"
    -- code は J-Quants の5桁コード。4桁 symbol への変換仕様が無いため先頭4文字一致で突き合わせる
    SELECT date, code, iss_type, shrt_vol, long_vol, shrt_neg_vol, long_neg_vol,
        shrt_std_vol, long_std_vol, shrt_val, long_val, shrt_neg_val, long_neg_val,
        shrt_std_val, long_std_val
    FROM margin_interest
    WHERE LEFT(code, 4) = $1
        AND ($2::date IS NULL OR date >= $2::date)
        AND ($3::date IS NULL OR date <= $3::date)
    ORDER BY date DESC, code, iss_type
    LIMIT $4
"};

const READ_MARGIN_ALERT_SQL: &str = indoc::indoc! {"
    -- 同一 app_date の訂正は pub_date が新しい行として追加されるため、app_date + code ごとに
    -- pub_date 最大の 1 件のみ残す
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
"};

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

impl From<MarginInterestRow> for MarginInterestDto {
    fn from(r: MarginInterestRow) -> Self {
        MarginInterestDto {
            date: r.date,
            code: r.code,
            iss_type: r.iss_type,
            shrt_vol: r.shrt_vol,
            long_vol: r.long_vol,
            shrt_neg_vol: r.shrt_neg_vol,
            long_neg_vol: r.long_neg_vol,
            shrt_std_vol: r.shrt_std_vol,
            long_std_vol: r.long_std_vol,
            shrt_val: r.shrt_val,
            long_val: r.long_val,
            shrt_neg_val: r.shrt_neg_val,
            long_neg_val: r.long_neg_val,
            shrt_std_val: r.shrt_std_val,
            long_std_val: r.long_std_val,
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

fn margin_alert_dto_from_row(r: MarginAlertRow) -> Result<MarginAlertDto, McpError> {
    let pub_reason: PubReason = serde_json::from_value(r.pub_reason)
        .map_err(|e| internal_error(format!("invalid margin_alert.pub_reason: {e}")))?;
    Ok(MarginAlertDto {
        app_date: r.app_date,
        pub_date: r.pub_date,
        code: r.code,
        pub_reason,
        shrt_out: r.shrt_out,
        long_out: r.long_out,
        shrt_out_chg: r.shrt_out_chg,
        long_out_chg: r.long_out_chg,
        shrt_out_ratio: r.shrt_out_ratio.map(decimal_to_f64),
        long_out_ratio: r.long_out_ratio.map(decimal_to_f64),
        sl_ratio: r.sl_ratio.map(decimal_to_f64),
        shrt_neg_out: r.shrt_neg_out,
        shrt_std_out: r.shrt_std_out,
        long_neg_out: r.long_neg_out,
        long_std_out: r.long_std_out,
        tse_mrgn_reg_cls: r.tse_mrgn_reg_cls,
    })
}

impl StrategyServer {
    pub(crate) async fn read_margin_inner(
        &self,
        // 信用残は銘柄単位の市場データであり戦略に属さないため検索条件に使わない
        _session_strategy_id: Uuid,
        params: ReadMarginParams,
    ) -> Result<ReadMarginResult, McpError> {
        let limit = clamp_limit(params.limit) as i64;
        let values = [
            params.symbol.into(),
            params.from.into(),
            params.to.into(),
            limit.into(),
        ];

        let interest_rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                READ_MARGIN_INTEREST_SQL,
                values.clone(),
            ))
            .await
            .map_err(db_error)?;
        let interest = interest_rows
            .iter()
            .map(|row| MarginInterestRow::from_query_result(row, ""))
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_error)?
            .into_iter()
            .map(MarginInterestDto::from)
            .collect();

        let alert_rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                READ_MARGIN_ALERT_SQL,
                values,
            ))
            .await
            .map_err(db_error)?;
        let alerts = alert_rows
            .iter()
            .map(|row| MarginAlertRow::from_query_result(row, ""))
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_error)?
            .into_iter()
            .map(margin_alert_dto_from_row)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ReadMarginResult { interest, alerts })
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sea_orm::DatabaseConnection;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::{margin_alert, margin_interest};
    use crate::models::PubReason;
    use crate::testing::create_test_db;

    use super::super::dto::{
        MarginAlertDto, MarginInterestDto, ReadMarginParams, ReadMarginResult,
    };
    use super::super::tests_common::build_server;

    fn ymd(y: i32, m: u32, d: u32) -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    fn no_pub_reason() -> PubReason {
        PubReason {
            restricted: false,
            daily_publication: false,
            monitoring: false,
            restricted_by_jsf: false,
            precaution_by_jsf: false,
            unclear_or_sec_on_alert: false,
        }
    }

    async fn seed_interest(
        db: &DatabaseConnection,
        date: chrono::NaiveDate,
        code: &str,
        iss_type: i16,
        shrt_vol: i64,
        long_vol: i64,
    ) {
        margin_interest::ActiveModel {
            date: Set(date),
            code: Set(code.to_string()),
            iss_type: Set(iss_type),
            shrt_vol: Set(shrt_vol),
            long_vol: Set(long_vol),
            shrt_neg_vol: Set(0),
            long_neg_vol: Set(0),
            shrt_std_vol: Set(0),
            long_std_vol: Set(0),
            shrt_val: Set(None),
            long_val: Set(None),
            shrt_neg_val: Set(None),
            long_neg_val: Set(None),
            shrt_std_val: Set(None),
            long_std_val: Set(None),
        }
        .insert(db)
        .await
        .expect("seed margin_interest");
    }

    async fn seed_alert(
        db: &DatabaseConnection,
        pub_date: chrono::NaiveDate,
        app_date: chrono::NaiveDate,
        code: &str,
        pub_reason: PubReason,
        shrt_out: i64,
        long_out: i64,
    ) {
        margin_alert::ActiveModel {
            pub_date: Set(pub_date),
            code: Set(code.to_string()),
            app_date: Set(app_date),
            pub_reason: Set(serde_json::to_value(pub_reason).expect("serialize pub_reason")),
            shrt_out: Set(shrt_out),
            long_out: Set(long_out),
            shrt_out_chg: Set(None),
            long_out_chg: Set(None),
            shrt_out_ratio: Set(None),
            long_out_ratio: Set(None),
            sl_ratio: Set(None),
            shrt_neg_out: Set(0),
            shrt_std_out: Set(0),
            long_neg_out: Set(0),
            long_std_out: Set(0),
            tse_mrgn_reg_cls: Set("001".to_string()),
        }
        .insert(db)
        .await
        .expect("seed margin_alert");
    }

    #[sqlx::test(migrations = false)]
    async fn read_margin_returns_interest_newest_first_matching_5_digit_code_by_prefix(
        pool: PgPool,
    ) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());

        seed_interest(&db, ymd(2026, 9, 1), "72030", 1, 100, 200).await;
        seed_interest(&db, ymd(2026, 9, 8), "72030", 1, 110, 210).await;
        seed_interest(&db, ymd(2026, 9, 8), "99840", 1, 999, 999).await;

        let result = server
            .read_margin_inner(
                Uuid::new_v4(),
                ReadMarginParams {
                    symbol: "7203".to_string(),
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect("read_margin");

        assert_eq!(
            result,
            ReadMarginResult {
                interest: vec![
                    MarginInterestDto {
                        date: ymd(2026, 9, 8),
                        code: "72030".to_string(),
                        iss_type: 1,
                        shrt_vol: 110,
                        long_vol: 210,
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
                    },
                    MarginInterestDto {
                        date: ymd(2026, 9, 1),
                        code: "72030".to_string(),
                        iss_type: 1,
                        shrt_vol: 100,
                        long_vol: 200,
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
                    },
                ],
                alerts: vec![],
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn read_margin_includes_distinct_iss_type_rows_for_the_same_date(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());

        seed_interest(&db, ymd(2026, 9, 8), "72030", 1, 100, 200).await;
        seed_interest(&db, ymd(2026, 9, 8), "72030", 2, 50, 60).await;

        let result = server
            .read_margin_inner(
                Uuid::new_v4(),
                ReadMarginParams {
                    symbol: "7203".to_string(),
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect("read_margin");

        let iss_types: Vec<i16> = result.interest.iter().map(|i| i.iss_type).collect();
        assert_eq!(iss_types, vec![1, 2]);
    }

    #[sqlx::test(migrations = false)]
    async fn read_margin_filters_interest_by_date_range_inclusive(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());

        seed_interest(&db, ymd(2026, 9, 1), "72030", 1, 1, 1).await;
        seed_interest(&db, ymd(2026, 9, 8), "72030", 1, 2, 2).await;
        seed_interest(&db, ymd(2026, 9, 15), "72030", 1, 3, 3).await;

        let result = server
            .read_margin_inner(
                Uuid::new_v4(),
                ReadMarginParams {
                    symbol: "7203".to_string(),
                    from: Some(ymd(2026, 9, 8)),
                    to: Some(ymd(2026, 9, 8)),
                    limit: None,
                },
            )
            .await
            .expect("read_margin");

        let dates: Vec<chrono::NaiveDate> = result.interest.iter().map(|i| i.date).collect();
        assert_eq!(dates, vec![ymd(2026, 9, 8)]);
    }

    #[sqlx::test(migrations = false)]
    async fn read_margin_orders_interest_newest_first_and_respects_limit(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());

        for (i, day) in [1u32, 8, 15].into_iter().enumerate() {
            seed_interest(&db, ymd(2026, 9, day), "72030", 1, i as i64, i as i64).await;
        }

        let result = server
            .read_margin_inner(
                Uuid::new_v4(),
                ReadMarginParams {
                    symbol: "7203".to_string(),
                    from: None,
                    to: None,
                    limit: Some(2),
                },
            )
            .await
            .expect("read_margin");

        let dates: Vec<chrono::NaiveDate> = result.interest.iter().map(|i| i.date).collect();
        assert_eq!(dates, vec![ymd(2026, 9, 15), ymd(2026, 9, 8)]);
    }

    #[sqlx::test(migrations = false)]
    async fn read_margin_keeps_only_latest_pub_date_per_app_date_for_alerts(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());

        seed_alert(
            &db,
            ymd(2026, 9, 2),
            ymd(2026, 9, 1),
            "72030",
            no_pub_reason(),
            1000,
            2000,
        )
        .await;
        seed_alert(
            &db,
            ymd(2026, 9, 3),
            ymd(2026, 9, 1),
            "72030",
            PubReason {
                restricted: true,
                ..no_pub_reason()
            },
            1100,
            2100,
        )
        .await;

        let result = server
            .read_margin_inner(
                Uuid::new_v4(),
                ReadMarginParams {
                    symbol: "7203".to_string(),
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect("read_margin");

        assert_eq!(
            result,
            ReadMarginResult {
                interest: vec![],
                alerts: vec![MarginAlertDto {
                    app_date: ymd(2026, 9, 1),
                    pub_date: ymd(2026, 9, 3),
                    code: "72030".to_string(),
                    pub_reason: PubReason {
                        restricted: true,
                        ..no_pub_reason()
                    },
                    shrt_out: 1100,
                    long_out: 2100,
                    shrt_out_chg: None,
                    long_out_chg: None,
                    shrt_out_ratio: None,
                    long_out_ratio: None,
                    sl_ratio: None,
                    shrt_neg_out: 0,
                    shrt_std_out: 0,
                    long_neg_out: 0,
                    long_std_out: 0,
                    tse_mrgn_reg_cls: "001".to_string(),
                }],
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn read_margin_orders_alerts_newest_first_and_respects_limit(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());

        for (i, day) in [1u32, 8, 15].into_iter().enumerate() {
            seed_alert(
                &db,
                ymd(2026, 9, day),
                ymd(2026, 9, day),
                "72030",
                no_pub_reason(),
                i as i64,
                i as i64,
            )
            .await;
        }

        let result = server
            .read_margin_inner(
                Uuid::new_v4(),
                ReadMarginParams {
                    symbol: "7203".to_string(),
                    from: None,
                    to: None,
                    limit: Some(2),
                },
            )
            .await
            .expect("read_margin");

        let app_dates: Vec<chrono::NaiveDate> = result.alerts.iter().map(|a| a.app_date).collect();
        assert_eq!(app_dates, vec![ymd(2026, 9, 15), ymd(2026, 9, 8)]);
    }
}
