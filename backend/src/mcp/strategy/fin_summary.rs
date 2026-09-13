//! 戦略実行 MCP の `read_fin_summary` tool。
//!
//! `jquants_fin_summary.raw` (J-Quants `/fins/summary` のレスポンス 1 件をそのまま
//! 格納した JSONB) を意味の分かるフィールド名に変換して返す。財務情報は会社単位の
//! 開示であり戦略に属さないマスタデータのため `search_refs` / `search_news` 同様
//! `x-strategy-id` を検索条件に使わない。
//!
//! `jquants_fin_summary.code` は J-Quants の 5 桁コードだが、MCP 引数の `symbol` は
//! 既存 tool と揃えて 4 桁で受け取る。5 桁 → 4 桁の変換仕様は J-Quants 側に無いため、
//! 先頭 4 文字の一致で突き合わせる。
//!
//! 同じ (開示書類種別, 当会計期間) の開示が複数あるとき (訂正、あるいは同一期間内での
//! 業績予想修正の再修正) は、開示番号 (`DiscNo`) が最大の 1 件だけを返す。

use chrono::NaiveDate;
use rmcp::ErrorData as McpError;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use uuid::Uuid;

use super::dto::{FinSummaryDto, ReadFinSummaryParams, ReadFinSummaryResult};
use super::{StrategyServer, clamp_limit, db_error};

const READ_FIN_SUMMARY_SQL: &str = indoc::indoc! {"
    WITH deduped AS (
        SELECT DISTINCT ON (raw->>'DocType', raw->>'CurPerSt', raw->>'CurPerEn')
            disc_date, raw, (raw->>'DiscNo')::bigint AS disc_no_num
        FROM jquants_fin_summary
        WHERE LEFT(code, 4) = $1
        ORDER BY raw->>'DocType', raw->>'CurPerSt', raw->>'CurPerEn', disc_no_num DESC
    )
    SELECT disc_date, raw FROM deduped
    ORDER BY disc_date DESC, disc_no_num DESC
    LIMIT $2
"};

#[derive(Debug, FromQueryResult)]
struct FinSummaryRow {
    disc_date: NaiveDate,
    raw: serde_json::Value,
}

impl StrategyServer {
    pub(crate) async fn read_fin_summary_inner(
        &self,
        _session_strategy_id: Uuid,
        params: ReadFinSummaryParams,
    ) -> Result<ReadFinSummaryResult, McpError> {
        let limit = clamp_limit(params.limit) as i64;

        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                READ_FIN_SUMMARY_SQL,
                [params.symbol.into(), limit.into()],
            ))
            .await
            .map_err(db_error)?;

        let items = rows
            .iter()
            .map(|row| FinSummaryRow::from_query_result(row, ""))
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_error)?
            .into_iter()
            .map(|row| fin_summary_dto_from_row(row.disc_date, &row.raw))
            .collect();

        Ok(ReadFinSummaryResult { items })
    }
}

fn fin_summary_dto_from_row(disc_date: NaiveDate, raw: &serde_json::Value) -> FinSummaryDto {
    FinSummaryDto {
        disc_date,
        doc_type: str_field(raw, "DocType"),
        current_period_type: str_field(raw, "CurPerType"),
        current_period_start: date_field(raw, "CurPerSt"),
        current_period_end: date_field(raw, "CurPerEn"),
        current_fiscal_year_start: date_field(raw, "CurFYSt"),
        current_fiscal_year_end: date_field(raw, "CurFYEn"),
        sales: f64_field(raw, "Sales"),
        operating_profit: f64_field(raw, "OP"),
        ordinary_profit: f64_field(raw, "OdP"),
        net_profit: f64_field(raw, "NP"),
        eps: f64_field(raw, "EPS"),
        bps: f64_field(raw, "BPS"),
        total_assets: f64_field(raw, "TA"),
        equity: f64_field(raw, "Eq"),
        equity_to_asset_ratio: f64_field(raw, "EqAR"),
        roe: f64_field(raw, "ROE"),
        cf_operating: f64_field(raw, "CFO"),
        cf_investing: f64_field(raw, "CFI"),
        cf_financing: f64_field(raw, "CFF"),
        cash_and_equivalents: f64_field(raw, "CashEq"),
        dividend_annual: f64_field(raw, "DivAnn"),
        dividend_annual_forecast: f64_field(raw, "FDivAnn"),
        dividend_annual_forecast_next: f64_field(raw, "NxFDivAnn"),
        forecast_sales: f64_field(raw, "FSales"),
        forecast_operating_profit: f64_field(raw, "FOP"),
        forecast_ordinary_profit: f64_field(raw, "FOdP"),
        forecast_net_profit: f64_field(raw, "FNP"),
        forecast_eps: f64_field(raw, "FEPS"),
        next_forecast_sales: f64_field(raw, "NxFSales"),
        next_forecast_operating_profit: f64_field(raw, "NxFOP"),
        next_forecast_ordinary_profit: f64_field(raw, "NxFOdP"),
        // J-Quants 仕様上このキーのみ NxFNp (p が小文字)。他の翌期予想キー (NxFOP 等) との
        // 表記ゆれで typo ではない。
        next_forecast_net_profit: f64_field(raw, "NxFNp"),
        next_forecast_eps: f64_field(raw, "NxFEPS"),
    }
}

/// raw の文字列項目を取り出す。キー欠落・非文字列・空文字はすべて「記載なし」として null。
fn str_field(raw: &serde_json::Value, key: &str) -> Option<String> {
    raw.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn date_field(raw: &serde_json::Value, key: &str) -> Option<NaiveDate> {
    str_field(raw, key).and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
}

fn f64_field(raw: &serde_json::Value, key: &str) -> Option<f64> {
    str_field(raw, key).and_then(|s| s.parse::<f64>().ok())
}

#[cfg(test)]
mod tests {
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sea_orm::DatabaseConnection;
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::jquants_fin_summary;
    use crate::testing::create_test_db;

    use super::super::tests_common::build_server;
    use super::{FinSummaryDto, ReadFinSummaryParams, ReadFinSummaryResult};

    fn ymd(y: i32, m: u32, d: u32) -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    fn blank_dto(disc_date: chrono::NaiveDate) -> FinSummaryDto {
        FinSummaryDto {
            disc_date,
            doc_type: None,
            current_period_type: None,
            current_period_start: None,
            current_period_end: None,
            current_fiscal_year_start: None,
            current_fiscal_year_end: None,
            sales: None,
            operating_profit: None,
            ordinary_profit: None,
            net_profit: None,
            eps: None,
            bps: None,
            total_assets: None,
            equity: None,
            equity_to_asset_ratio: None,
            roe: None,
            cf_operating: None,
            cf_investing: None,
            cf_financing: None,
            cash_and_equivalents: None,
            dividend_annual: None,
            dividend_annual_forecast: None,
            dividend_annual_forecast_next: None,
            forecast_sales: None,
            forecast_operating_profit: None,
            forecast_ordinary_profit: None,
            forecast_net_profit: None,
            forecast_eps: None,
            next_forecast_sales: None,
            next_forecast_operating_profit: None,
            next_forecast_ordinary_profit: None,
            next_forecast_net_profit: None,
            next_forecast_eps: None,
        }
    }

    async fn seed(db: &DatabaseConnection, code: &str, disc_no: &str, raw: serde_json::Value) {
        let disc_date = raw
            .get("DiscDate")
            .and_then(|v| v.as_str())
            .map(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("valid date"))
            .expect("DiscDate required in test fixture");
        jquants_fin_summary::ActiveModel {
            code: Set(code.to_string()),
            disc_no: Set(disc_no.to_string()),
            disc_date: Set(disc_date),
            raw: Set(raw),
        }
        .insert(db)
        .await
        .expect("seed fin summary");
    }

    #[sqlx::test(migrations = false)]
    async fn read_fin_summary_converts_abbreviations_to_named_fields_and_blanks_to_null(
        pool: PgPool,
    ) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());

        seed(
            &db,
            "72030",
            "1",
            json!({
                "DiscDate": "2026-05-01",
                "Code": "72030",
                "DiscNo": "1",
                "DocType": "FYFinancialStatements_Consolidated_JP",
                "CurPerType": "FY",
                "CurPerSt": "2025-04-01",
                "CurPerEn": "2026-03-31",
                "CurFYSt": "2025-04-01",
                "CurFYEn": "2026-03-31",
                "Sales": "3000000",
                "OP": "200000",
                "OdP": "",
                "NP": "150000",
                "EPS": "120.5",
                "BPS": "1500.0",
                "TA": "5000000",
                "Eq": "2000000",
                "EqAR": "0.4",
                "ROE": "0.08",
                "CFO": "100000",
                "CFI": "-50000",
                "CFF": "-20000",
                "CashEq": "300000",
                "DivAnn": "30",
                "FDivAnn": "32",
                "NxFDivAnn": "",
                "FSales": "3100000",
                "FOP": "210000",
                "FOdP": "",
                "FNP": "160000",
                "FEPS": "128.0",
                "NxFSales": "",
                "NxFOP": "",
                "NxFOdP": "",
                "NxFNp": "",
                "NxFEPS": "",
            }),
        )
        .await;

        let result = server
            .read_fin_summary_inner(
                Uuid::new_v4(),
                ReadFinSummaryParams {
                    symbol: "7203".to_string(),
                    limit: None,
                },
            )
            .await
            .expect("read_fin_summary");

        assert_eq!(
            result,
            ReadFinSummaryResult {
                items: vec![FinSummaryDto {
                    disc_date: ymd(2026, 5, 1),
                    doc_type: Some("FYFinancialStatements_Consolidated_JP".to_string()),
                    current_period_type: Some("FY".to_string()),
                    current_period_start: Some(ymd(2025, 4, 1)),
                    current_period_end: Some(ymd(2026, 3, 31)),
                    current_fiscal_year_start: Some(ymd(2025, 4, 1)),
                    current_fiscal_year_end: Some(ymd(2026, 3, 31)),
                    sales: Some(3000000.0),
                    operating_profit: Some(200000.0),
                    ordinary_profit: None,
                    net_profit: Some(150000.0),
                    eps: Some(120.5),
                    bps: Some(1500.0),
                    total_assets: Some(5000000.0),
                    equity: Some(2000000.0),
                    equity_to_asset_ratio: Some(0.4),
                    roe: Some(0.08),
                    cf_operating: Some(100000.0),
                    cf_investing: Some(-50000.0),
                    cf_financing: Some(-20000.0),
                    cash_and_equivalents: Some(300000.0),
                    dividend_annual: Some(30.0),
                    dividend_annual_forecast: Some(32.0),
                    dividend_annual_forecast_next: None,
                    forecast_sales: Some(3100000.0),
                    forecast_operating_profit: Some(210000.0),
                    forecast_ordinary_profit: None,
                    forecast_net_profit: Some(160000.0),
                    forecast_eps: Some(128.0),
                    next_forecast_sales: None,
                    next_forecast_operating_profit: None,
                    next_forecast_ordinary_profit: None,
                    next_forecast_net_profit: None,
                    next_forecast_eps: None,
                }],
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn read_fin_summary_matches_5_digit_code_by_leading_4_chars(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());

        seed(
            &db,
            "72030",
            "1",
            json!({ "DiscDate": "2026-05-01", "Code": "72030", "DiscNo": "1" }),
        )
        .await;
        seed(
            &db,
            "99840",
            "1",
            json!({ "DiscDate": "2026-05-01", "Code": "99840", "DiscNo": "1" }),
        )
        .await;

        let result = server
            .read_fin_summary_inner(
                Uuid::new_v4(),
                ReadFinSummaryParams {
                    symbol: "7203".to_string(),
                    limit: None,
                },
            )
            .await
            .expect("read_fin_summary");

        assert_eq!(
            result,
            ReadFinSummaryResult {
                items: vec![blank_dto(ymd(2026, 5, 1))],
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn read_fin_summary_keeps_only_the_highest_disc_no_per_period_and_doc_type(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());

        seed(
            &db,
            "72030",
            "1",
            json!({
                "DiscDate": "2026-05-01",
                "Code": "72030",
                "DiscNo": "1",
                "DocType": "FYFinancialStatements_Consolidated_JP",
                "CurPerSt": "2025-04-01",
                "CurPerEn": "2026-03-31",
                "Sales": "3000000",
            }),
        )
        .await;
        seed(
            &db,
            "72030",
            "2",
            json!({
                "DiscDate": "2026-05-10",
                "Code": "72030",
                "DiscNo": "2",
                "DocType": "FYFinancialStatements_Consolidated_JP",
                "CurPerSt": "2025-04-01",
                "CurPerEn": "2026-03-31",
                "Sales": "3050000",
            }),
        )
        .await;
        seed(
            &db,
            "72030",
            "3",
            json!({
                "DiscDate": "2026-06-01",
                "Code": "72030",
                "DiscNo": "3",
                "DocType": "EarnForecastRevision",
                "CurPerSt": "2025-04-01",
                "CurPerEn": "2026-03-31",
            }),
        )
        .await;

        let result = server
            .read_fin_summary_inner(
                Uuid::new_v4(),
                ReadFinSummaryParams {
                    symbol: "7203".to_string(),
                    limit: None,
                },
            )
            .await
            .expect("read_fin_summary");

        assert_eq!(
            result,
            ReadFinSummaryResult {
                items: vec![
                    FinSummaryDto {
                        doc_type: Some("EarnForecastRevision".to_string()),
                        current_period_start: Some(ymd(2025, 4, 1)),
                        current_period_end: Some(ymd(2026, 3, 31)),
                        ..blank_dto(ymd(2026, 6, 1))
                    },
                    FinSummaryDto {
                        doc_type: Some("FYFinancialStatements_Consolidated_JP".to_string()),
                        current_period_start: Some(ymd(2025, 4, 1)),
                        current_period_end: Some(ymd(2026, 3, 31)),
                        sales: Some(3050000.0),
                        ..blank_dto(ymd(2026, 5, 10))
                    },
                ],
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn read_fin_summary_orders_newest_first_and_respects_limit(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone());

        for (disc_no, date) in [
            ("1", "2026-01-01"),
            ("2", "2026-02-01"),
            ("3", "2026-03-01"),
        ] {
            seed(
                &db,
                "72030",
                disc_no,
                json!({
                    "DiscDate": date,
                    "Code": "72030",
                    "DiscNo": disc_no,
                    "DocType": format!("doc-{disc_no}"),
                }),
            )
            .await;
        }

        let result = server
            .read_fin_summary_inner(
                Uuid::new_v4(),
                ReadFinSummaryParams {
                    symbol: "7203".to_string(),
                    limit: Some(2),
                },
            )
            .await
            .expect("read_fin_summary");

        assert_eq!(
            result,
            ReadFinSummaryResult {
                items: vec![
                    FinSummaryDto {
                        doc_type: Some("doc-3".to_string()),
                        ..blank_dto(ymd(2026, 3, 1))
                    },
                    FinSummaryDto {
                        doc_type: Some("doc-2".to_string()),
                        ..blank_dto(ymd(2026, 2, 1))
                    },
                ],
            }
        );
    }
}
