//! 戦略実行 MCP の `read_margin` tool。信用取引週末残高/信用取引残高 (`margin_interest`)
//! と日々公表信用取引残高 (`margin_alert`) を 4 桁銘柄コード + 期間で読み出す。

use chrono::NaiveDate;
use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;
use schemars::JsonSchema;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::PubReason;

use super::{
    StrategyServer, clamp_limit, db_error, decimal_to_f64, internal_error, invalid_params,
};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadMarginParams {
    /// 対象銘柄コード (4桁、例: "7203")
    pub symbol: String,
    /// 取得開始日 (YYYY-MM-DD, inclusive)。省略時は下限なし
    pub from: Option<NaiveDate>,
    /// 取得終了日 (YYYY-MM-DD, inclusive)。省略時は上限なし
    pub to: Option<NaiveDate>,
    /// interest / alerts それぞれに独立に適用される件数上限
    pub limit: Option<u32>,
}

/// 信用取引週末残高 (2026-09-28 以降の切替後は日次の信用取引残高) 1 行分。
#[derive(Debug, Serialize, JsonSchema, PartialEq, FromQueryResult)]
pub struct MarginInterestDto {
    pub date: NaiveDate,
    /// J-Quants の5桁コード。同一銘柄でも普通株/優先株など株式の種類ごとに別コードで並び得る
    pub code: String,
    /// 銘柄区分 (1: 信用銘柄, 2: 貸借銘柄, 3: その他)
    pub iss_type: i16,
    pub shrt_vol: i64,
    pub long_vol: i64,
    pub shrt_neg_vol: i64,
    pub long_neg_vol: i64,
    pub shrt_std_vol: i64,
    pub long_std_vol: i64,
    /// 2026-09-25 申込分より前は金額データ自体が存在しないため null
    pub shrt_val: Option<i64>,
    pub long_val: Option<i64>,
    pub shrt_neg_val: Option<i64>,
    pub long_neg_val: Option<i64>,
    pub shrt_std_val: Option<i64>,
    pub long_std_val: Option<i64>,
}

/// 日々公表信用取引残高 1 行分。取引所が日々公表銘柄に指定した銘柄のみが対象であり、
/// この一覧に載っていないことは残高ゼロを意味しない。
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct MarginAlertDto {
    /// 申込日。同一 app_date に訂正が複数あれば公表日 (pub_date) が最新の 1 件のみ返る
    pub app_date: NaiveDate,
    pub pub_date: NaiveDate,
    pub code: String,
    pub pub_reason: MarginPubReasonDto,
    pub shrt_out: i64,
    pub long_out: i64,
    /// 前日に公表されていなければ null
    pub shrt_out_chg: Option<i64>,
    pub long_out_chg: Option<i64>,
    /// ETF 等では null
    pub shrt_out_ratio: Option<f64>,
    pub long_out_ratio: Option<f64>,
    pub sl_ratio: Option<f64>,
    pub shrt_neg_out: i64,
    pub shrt_std_out: i64,
    pub long_neg_out: i64,
    pub long_std_out: i64,
    /// 規制区分 (文字列。J-Quants 側の分類をそのまま保持)
    pub tse_mrgn_reg_cls: String,
}

/// 日々公表信用取引残高の公表理由フラグ。
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct MarginPubReasonDto {
    pub restricted: bool,
    pub daily_publication: bool,
    pub monitoring: bool,
    pub restricted_by_jsf: bool,
    pub precaution_by_jsf: bool,
    pub unclear_or_sec_on_alert: bool,
}

impl From<PubReason> for MarginPubReasonDto {
    fn from(r: PubReason) -> Self {
        MarginPubReasonDto {
            restricted: r.restricted,
            daily_publication: r.daily_publication,
            monitoring: r.monitoring,
            restricted_by_jsf: r.restricted_by_jsf,
            precaution_by_jsf: r.precaution_by_jsf,
            unclear_or_sec_on_alert: r.unclear_or_sec_on_alert,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadMarginResult {
    /// 信用取引週末残高 (日次切替後は信用取引残高)。新しい順
    pub interest: Vec<MarginInterestDto>,
    /// 日々公表信用取引残高。新しい順
    pub alerts: Vec<MarginAlertDto>,
}

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
        pub_reason: pub_reason.into(),
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
        if let (Some(from), Some(to)) = (params.from, params.to)
            && from > to
        {
            return Err(invalid_params("from must be on or before to"));
        }

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
            .map(|row| MarginInterestDto::from_query_result(row, ""))
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_error)?;

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

    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::{margin_alert, margin_interest};
    use crate::models::PubReason;
    use crate::testing::create_test_db;

    use super::super::tests_common::build_server;
    use super::{
        MarginAlertDto, MarginInterestDto, MarginPubReasonDto, ReadMarginParams, ReadMarginResult,
    };

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

    fn no_pub_reason_dto() -> MarginPubReasonDto {
        no_pub_reason().into()
    }

    async fn seed_interest(
        db: &impl sea_orm::ConnectionTrait,
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
        db: &impl sea_orm::ConnectionTrait,
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

    #[backend_test_macros::database_test]
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

    #[backend_test_macros::database_test]
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

    #[backend_test_macros::database_test]
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

    #[backend_test_macros::database_test]
    async fn read_margin_rejects_from_after_to(pool: PgPool) {
        let db = create_test_db(pool).await;

        let err = build_server(db)
            .read_margin_inner(
                Uuid::new_v4(),
                ReadMarginParams {
                    symbol: "7203".to_string(),
                    from: Some(ymd(2026, 9, 10)),
                    to: Some(ymd(2026, 9, 1)),
                    limit: None,
                },
            )
            .await
            .expect_err("from after to should be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
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

    #[backend_test_macros::database_test]
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
                    pub_reason: MarginPubReasonDto {
                        restricted: true,
                        ..no_pub_reason_dto()
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

    #[backend_test_macros::database_test]
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

    #[backend_test_macros::database_test]
    async fn read_margin_matches_alerts_5_digit_code_by_prefix(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());

        seed_alert(
            &db,
            ymd(2026, 9, 8),
            ymd(2026, 9, 8),
            "72030",
            no_pub_reason(),
            100,
            200,
        )
        .await;
        seed_alert(
            &db,
            ymd(2026, 9, 8),
            ymd(2026, 9, 8),
            "99840",
            no_pub_reason(),
            999,
            999,
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

        let codes: Vec<String> = result.alerts.iter().map(|a| a.code.clone()).collect();
        assert_eq!(codes, vec!["72030".to_string()]);
    }

    #[backend_test_macros::database_test]
    async fn read_margin_filters_alerts_by_date_range_inclusive(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());

        seed_alert(
            &db,
            ymd(2026, 9, 1),
            ymd(2026, 9, 1),
            "72030",
            no_pub_reason(),
            1,
            1,
        )
        .await;
        seed_alert(
            &db,
            ymd(2026, 9, 8),
            ymd(2026, 9, 8),
            "72030",
            no_pub_reason(),
            2,
            2,
        )
        .await;
        seed_alert(
            &db,
            ymd(2026, 9, 15),
            ymd(2026, 9, 15),
            "72030",
            no_pub_reason(),
            3,
            3,
        )
        .await;

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

        let app_dates: Vec<chrono::NaiveDate> = result.alerts.iter().map(|a| a.app_date).collect();
        assert_eq!(app_dates, vec![ymd(2026, 9, 8)]);
    }
}
