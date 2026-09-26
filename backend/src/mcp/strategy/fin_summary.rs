//! 戦略実行 MCP の `read_fin_summary` tool。財務情報テーブルの型付き列を返却 DTO に変換する。

use chrono::NaiveDate;
use rmcp::ErrorData as McpError;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use uuid::Uuid;

use super::dto::{FinSummaryDto, ReadFinSummaryParams, ReadFinSummaryResult};
use super::{StrategyServer, clamp_limit, db_error};

const READ_FIN_SUMMARY_SQL: &str = indoc::indoc! {"
    -- 同じ書類種別・会計期間の開示が複数あれば開示番号が最大の 1 件のみ残す
    -- (訂正、あるいは業績予想修正の再修正)
    WITH deduped AS (
        SELECT DISTINCT ON (report_group_key) financial_summary.*,
            disclosure_no::bigint AS disclosure_no_num
        FROM financial_summary
        -- 4 桁銘柄コードは、登録済みコードの先頭 4 文字と突き合わせる
        WHERE LEFT(code, 4) = $1
        ORDER BY report_group_key, disclosure_no_num DESC
    )
    SELECT *
    FROM deduped
    ORDER BY disclosure_date DESC, disclosure_no_num DESC
    LIMIT $2
"};

#[derive(Debug, FromQueryResult)]
struct FinSummaryRow {
    disclosure_date: NaiveDate,
    document_type: Option<String>,
    current_period_type: Option<String>,
    current_period_start: Option<NaiveDate>,
    current_period_end: Option<NaiveDate>,
    current_fiscal_year_start: Option<NaiveDate>,
    current_fiscal_year_end: Option<NaiveDate>,
    sales: Option<f64>,
    operating_profit: Option<f64>,
    ordinary_profit: Option<f64>,
    net_profit: Option<f64>,
    eps: Option<f64>,
    bps: Option<f64>,
    total_assets: Option<f64>,
    equity: Option<f64>,
    equity_to_asset_ratio: Option<f64>,
    roe: Option<f64>,
    cash_flow_operating: Option<f64>,
    cash_flow_investing: Option<f64>,
    cash_flow_financing: Option<f64>,
    cash_and_equivalents: Option<f64>,
    dividend_annual: Option<f64>,
    dividend_annual_forecast: Option<f64>,
    dividend_annual_forecast_next: Option<f64>,
    forecast_sales: Option<f64>,
    forecast_operating_profit: Option<f64>,
    forecast_ordinary_profit: Option<f64>,
    forecast_net_profit: Option<f64>,
    forecast_eps: Option<f64>,
    next_forecast_sales: Option<f64>,
    next_forecast_operating_profit: Option<f64>,
    next_forecast_ordinary_profit: Option<f64>,
    next_forecast_net_profit: Option<f64>,
    next_forecast_eps: Option<f64>,
}

impl StrategyServer {
    pub(crate) async fn read_fin_summary_inner(
        &self,
        // 財務情報は会社単位の開示であり戦略に属さないマスタデータのため検索条件に使わない
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
            .map(fin_summary_dto_from_row)
            .collect();

        Ok(ReadFinSummaryResult { items })
    }
}

fn fin_summary_dto_from_row(row: FinSummaryRow) -> FinSummaryDto {
    let is_quarterly_financial_statement = is_quarterly_financial_statement(
        row.document_type.as_deref(),
        row.current_period_type.as_deref(),
    );

    FinSummaryDto {
        disc_date: row.disclosure_date,
        doc_type: row.document_type,
        current_period_type: row.current_period_type,
        current_period_start: row.current_period_start,
        current_period_end: row.current_period_end,
        current_fiscal_year_start: row.current_fiscal_year_start,
        current_fiscal_year_end: row.current_fiscal_year_end,
        sales: row.sales,
        operating_profit: row.operating_profit,
        ordinary_profit: row.ordinary_profit,
        net_profit: row.net_profit,
        sales_progress_rate: progress_rate(
            is_quarterly_financial_statement,
            row.sales,
            row.forecast_sales,
        ),
        operating_profit_progress_rate: progress_rate(
            is_quarterly_financial_statement,
            row.operating_profit,
            row.forecast_operating_profit,
        ),
        ordinary_profit_progress_rate: progress_rate(
            is_quarterly_financial_statement,
            row.ordinary_profit,
            row.forecast_ordinary_profit,
        ),
        net_profit_progress_rate: progress_rate(
            is_quarterly_financial_statement,
            row.net_profit,
            row.forecast_net_profit,
        ),
        eps: row.eps,
        bps: row.bps,
        total_assets: row.total_assets,
        equity: row.equity,
        equity_to_asset_ratio: row.equity_to_asset_ratio,
        roe: row.roe,
        cf_operating: row.cash_flow_operating,
        cf_investing: row.cash_flow_investing,
        cf_financing: row.cash_flow_financing,
        cash_and_equivalents: row.cash_and_equivalents,
        dividend_annual: row.dividend_annual,
        dividend_annual_forecast: row.dividend_annual_forecast,
        dividend_annual_forecast_next: row.dividend_annual_forecast_next,
        forecast_sales: row.forecast_sales,
        forecast_operating_profit: row.forecast_operating_profit,
        forecast_ordinary_profit: row.forecast_ordinary_profit,
        forecast_net_profit: row.forecast_net_profit,
        forecast_eps: row.forecast_eps,
        next_forecast_sales: row.next_forecast_sales,
        next_forecast_operating_profit: row.next_forecast_operating_profit,
        next_forecast_ordinary_profit: row.next_forecast_ordinary_profit,
        next_forecast_net_profit: row.next_forecast_net_profit,
        next_forecast_eps: row.next_forecast_eps,
    }
}

fn is_quarterly_financial_statement(doc_type: Option<&str>, period_type: Option<&str>) -> bool {
    let prefix = match period_type {
        Some("1Q") => "1QFinancialStatements_",
        Some("2Q") => "2QFinancialStatements_",
        Some("3Q") => "3QFinancialStatements_",
        _ => return false,
    };

    doc_type.is_some_and(|doc_type| doc_type.starts_with(prefix))
}

fn progress_rate(
    is_quarterly_financial_statement: bool,
    actual: Option<f64>,
    forecast: Option<f64>,
) -> Option<f64> {
    if !is_quarterly_financial_statement {
        return None;
    }

    let actual = actual.filter(|value| value.is_finite())?;
    let forecast = forecast.filter(|value| value.is_finite() && *value > 0.0)?;
    let rate = actual / forecast;

    rate.is_finite().then_some(rate)
}

#[cfg(test)]
mod tests {
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sea_orm::ConnectionTrait;
    use uuid::Uuid;

    use super::super::tests_common::build_server;
    use super::{FinSummaryDto, ReadFinSummaryParams, ReadFinSummaryResult};
    use crate::entities::financial_summary;

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
            sales_progress_rate: None,
            operating_profit_progress_rate: None,
            ordinary_profit_progress_rate: None,
            net_profit_progress_rate: None,
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

    async fn seed(
        db: &impl ConnectionTrait,
        code: &str,
        disclosure_no: &str,
        disclosure_date: chrono::NaiveDate,
        configure: impl FnOnce(&mut financial_summary::ActiveModel),
    ) {
        let mut summary = financial_summary::ActiveModel {
            code: Set(code.to_string()),
            disclosure_no: Set(disclosure_no.to_string()),
            disclosure_date: Set(disclosure_date),
            ..Default::default()
        };
        configure(&mut summary);
        summary.report_group_key = Set(format!(
            "{:?}|{:?}|{:?}",
            summary.document_type, summary.current_period_start, summary.current_period_end
        ));
        summary.insert(db).await.expect("seed fin summary");
    }

    async fn seed_with_report_group_key(
        db: &impl ConnectionTrait,
        code: &str,
        disclosure_no: &str,
        disclosure_date: chrono::NaiveDate,
        report_group_key: &str,
    ) {
        financial_summary::ActiveModel {
            code: Set(code.to_string()),
            disclosure_no: Set(disclosure_no.to_string()),
            disclosure_date: Set(disclosure_date),
            report_group_key: Set(report_group_key.to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("seed fin summary");
    }

    #[backend_test_macros::database_test]
    async fn read_fin_summary_returns_typed_fields_and_nulls_missing_values(
        db: crate::database::DatabaseHandle,
    ) {
        let server = build_server(db.clone());

        seed(&db, "99990", "1", ymd(2026, 5, 1), |summary| {
            summary.document_type = Set(Some("FYFinancialStatements_TestFixture".to_string()));
            summary.current_period_type = Set(Some("FY".to_string()));
            summary.current_period_start = Set(Some(ymd(2025, 4, 1)));
            summary.current_period_end = Set(Some(ymd(2026, 3, 31)));
            summary.current_fiscal_year_start = Set(Some(ymd(2025, 4, 1)));
            summary.current_fiscal_year_end = Set(Some(ymd(2026, 3, 31)));
            summary.sales = Set(Some(3_000_000.0));
            summary.operating_profit = Set(Some(200_000.0));
            summary.net_profit = Set(Some(150_000.0));
            summary.eps = Set(Some(120.5));
            summary.bps = Set(Some(1_500.0));
            summary.total_assets = Set(Some(5_000_000.0));
            summary.equity = Set(Some(2_000_000.0));
            summary.equity_to_asset_ratio = Set(Some(0.4));
            summary.roe = Set(Some(0.08));
            summary.cash_flow_operating = Set(Some(100_000.0));
            summary.cash_flow_investing = Set(Some(-50_000.0));
            summary.cash_flow_financing = Set(Some(-20_000.0));
            summary.cash_and_equivalents = Set(Some(300_000.0));
            summary.dividend_annual = Set(Some(30.0));
            summary.dividend_annual_forecast = Set(Some(32.0));
            summary.forecast_sales = Set(Some(3_100_000.0));
            summary.forecast_operating_profit = Set(Some(210_000.0));
            summary.forecast_net_profit = Set(Some(160_000.0));
            summary.forecast_eps = Set(Some(128.0));
        })
        .await;

        let result = server
            .read_fin_summary_inner(
                Uuid::new_v4(),
                ReadFinSummaryParams {
                    symbol: "9999".to_string(),
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
                    doc_type: Some("FYFinancialStatements_TestFixture".to_string()),
                    current_period_type: Some("FY".to_string()),
                    current_period_start: Some(ymd(2025, 4, 1)),
                    current_period_end: Some(ymd(2026, 3, 31)),
                    current_fiscal_year_start: Some(ymd(2025, 4, 1)),
                    current_fiscal_year_end: Some(ymd(2026, 3, 31)),
                    sales: Some(3000000.0),
                    operating_profit: Some(200000.0),
                    ordinary_profit: None,
                    net_profit: Some(150000.0),
                    sales_progress_rate: None,
                    operating_profit_progress_rate: None,
                    ordinary_profit_progress_rate: None,
                    net_profit_progress_rate: None,
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

    #[backend_test_macros::database_test]
    async fn read_fin_summary_computes_progress_rates_only_for_quarterly_statements(
        db: crate::database::DatabaseHandle,
    ) {
        let server = build_server(db.clone());

        seed(&db, "ABCD0", "1", ymd(2001, 5, 1), |summary| {
            summary.document_type = Set(Some("1QFinancialStatements_TestFixture".to_string()));
            summary.current_period_type = Set(Some("1Q".to_string()));
            summary.sales = Set(Some(50.0));
            summary.forecast_sales = Set(Some(100.0));
            summary.operating_profit = Set(Some(20.0));
            summary.forecast_operating_profit = Set(Some(40.0));
            summary.ordinary_profit = Set(Some(15.0));
            summary.forecast_ordinary_profit = Set(Some(30.0));
            summary.net_profit = Set(Some(5.0));
            summary.forecast_net_profit = Set(Some(10.0));
        })
        .await;
        seed(&db, "ABCD0", "2", ymd(2001, 5, 2), |summary| {
            summary.document_type = Set(Some("2QFinancialStatements_TestFixture".to_string()));
            summary.current_period_type = Set(Some("2Q".to_string()));
            summary.sales = Set(Some(50.0));
            summary.operating_profit = Set(Some(20.0));
            summary.forecast_operating_profit = Set(Some(0.0));
            summary.ordinary_profit = Set(Some(15.0));
            summary.forecast_ordinary_profit = Set(Some(-30.0));
            summary.net_profit = Set(Some(5.0));
            summary.forecast_net_profit = Set(Some(10.0));
        })
        .await;
        seed(&db, "ABCD0", "3", ymd(2001, 5, 3), |summary| {
            summary.document_type = Set(Some("FYFinancialStatements_TestFixture".to_string()));
            summary.current_period_type = Set(Some("FY".to_string()));
            summary.sales = Set(Some(50.0));
            summary.forecast_sales = Set(Some(100.0));
            summary.operating_profit = Set(Some(20.0));
            summary.forecast_operating_profit = Set(Some(40.0));
            summary.ordinary_profit = Set(Some(15.0));
            summary.forecast_ordinary_profit = Set(Some(30.0));
            summary.net_profit = Set(Some(5.0));
            summary.forecast_net_profit = Set(Some(10.0));
        })
        .await;
        seed(&db, "ABCD0", "4", ymd(2001, 5, 4), |summary| {
            summary.document_type = Set(Some("ForecastUpdate_Fictional".to_string()));
            summary.current_period_type = Set(Some("2Q".to_string()));
            summary.sales = Set(Some(50.0));
            summary.forecast_sales = Set(Some(100.0));
            summary.operating_profit = Set(Some(20.0));
            summary.forecast_operating_profit = Set(Some(40.0));
            summary.ordinary_profit = Set(Some(15.0));
            summary.forecast_ordinary_profit = Set(Some(30.0));
            summary.net_profit = Set(Some(5.0));
            summary.forecast_net_profit = Set(Some(10.0));
        })
        .await;

        let result = server
            .read_fin_summary_inner(
                Uuid::new_v4(),
                ReadFinSummaryParams {
                    symbol: "ABCD".to_string(),
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
                        doc_type: Some("ForecastUpdate_Fictional".to_string()),
                        current_period_type: Some("2Q".to_string()),
                        sales: Some(50.0),
                        operating_profit: Some(20.0),
                        ordinary_profit: Some(15.0),
                        net_profit: Some(5.0),
                        forecast_sales: Some(100.0),
                        forecast_operating_profit: Some(40.0),
                        forecast_ordinary_profit: Some(30.0),
                        forecast_net_profit: Some(10.0),
                        ..blank_dto(ymd(2001, 5, 4))
                    },
                    FinSummaryDto {
                        doc_type: Some("FYFinancialStatements_TestFixture".to_string()),
                        current_period_type: Some("FY".to_string()),
                        sales: Some(50.0),
                        operating_profit: Some(20.0),
                        ordinary_profit: Some(15.0),
                        net_profit: Some(5.0),
                        forecast_sales: Some(100.0),
                        forecast_operating_profit: Some(40.0),
                        forecast_ordinary_profit: Some(30.0),
                        forecast_net_profit: Some(10.0),
                        ..blank_dto(ymd(2001, 5, 3))
                    },
                    FinSummaryDto {
                        doc_type: Some("2QFinancialStatements_TestFixture".to_string()),
                        current_period_type: Some("2Q".to_string()),
                        sales: Some(50.0),
                        operating_profit: Some(20.0),
                        ordinary_profit: Some(15.0),
                        net_profit: Some(5.0),
                        operating_profit_progress_rate: None,
                        ordinary_profit_progress_rate: None,
                        net_profit_progress_rate: Some(0.5),
                        forecast_operating_profit: Some(0.0),
                        forecast_ordinary_profit: Some(-30.0),
                        forecast_net_profit: Some(10.0),
                        ..blank_dto(ymd(2001, 5, 2))
                    },
                    FinSummaryDto {
                        doc_type: Some("1QFinancialStatements_TestFixture".to_string()),
                        current_period_type: Some("1Q".to_string()),
                        sales: Some(50.0),
                        operating_profit: Some(20.0),
                        ordinary_profit: Some(15.0),
                        net_profit: Some(5.0),
                        sales_progress_rate: Some(0.5),
                        operating_profit_progress_rate: Some(0.5),
                        ordinary_profit_progress_rate: Some(0.5),
                        net_profit_progress_rate: Some(0.5),
                        forecast_sales: Some(100.0),
                        forecast_operating_profit: Some(40.0),
                        forecast_ordinary_profit: Some(30.0),
                        forecast_net_profit: Some(10.0),
                        ..blank_dto(ymd(2001, 5, 1))
                    },
                ],
            }
        );
    }

    #[backend_test_macros::database_test]
    async fn read_fin_summary_matches_5_digit_code_by_leading_4_chars(
        db: crate::database::DatabaseHandle,
    ) {
        let server = build_server(db.clone());

        seed(&db, "99990", "1", ymd(2026, 5, 1), |_| {}).await;
        seed(&db, "88880", "1", ymd(2026, 5, 1), |_| {}).await;

        let result = server
            .read_fin_summary_inner(
                Uuid::new_v4(),
                ReadFinSummaryParams {
                    symbol: "9999".to_string(),
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

    #[backend_test_macros::database_test]
    async fn read_fin_summary_keeps_only_the_highest_disc_no_per_period_and_doc_type(
        db: crate::database::DatabaseHandle,
    ) {
        let server = build_server(db.clone());

        seed(&db, "99990", "1", ymd(2026, 5, 1), |summary| {
            summary.document_type = Set(Some("FYFinancialStatements_TestFixture".to_string()));
            summary.current_period_start = Set(Some(ymd(2025, 4, 1)));
            summary.current_period_end = Set(Some(ymd(2026, 3, 31)));
            summary.sales = Set(Some(3_000_000.0));
        })
        .await;
        seed(&db, "99990", "2", ymd(2026, 5, 10), |summary| {
            summary.document_type = Set(Some("FYFinancialStatements_TestFixture".to_string()));
            summary.current_period_start = Set(Some(ymd(2025, 4, 1)));
            summary.current_period_end = Set(Some(ymd(2026, 3, 31)));
            summary.sales = Set(Some(3_050_000.0));
        })
        .await;
        seed(&db, "99990", "3", ymd(2026, 6, 1), |summary| {
            summary.document_type = Set(Some("ForecastUpdate_Fictional".to_string()));
            summary.current_period_start = Set(Some(ymd(2025, 4, 1)));
            summary.current_period_end = Set(Some(ymd(2026, 3, 31)));
        })
        .await;

        let result = server
            .read_fin_summary_inner(
                Uuid::new_v4(),
                ReadFinSummaryParams {
                    symbol: "9999".to_string(),
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
                        doc_type: Some("ForecastUpdate_Fictional".to_string()),
                        current_period_start: Some(ymd(2025, 4, 1)),
                        current_period_end: Some(ymd(2026, 3, 31)),
                        ..blank_dto(ymd(2026, 6, 1))
                    },
                    FinSummaryDto {
                        doc_type: Some("FYFinancialStatements_TestFixture".to_string()),
                        current_period_start: Some(ymd(2025, 4, 1)),
                        current_period_end: Some(ymd(2026, 3, 31)),
                        sales: Some(3050000.0),
                        ..blank_dto(ymd(2026, 5, 10))
                    },
                ],
            }
        );
    }

    #[backend_test_macros::database_test]
    async fn read_fin_summary_keeps_missing_and_blank_group_values_separate(
        db: crate::database::DatabaseHandle,
    ) {
        let server = build_server(db.clone());

        seed_with_report_group_key(&db, "99990", "1", ymd(2026, 5, 1), "N;N;N;").await;
        seed_with_report_group_key(&db, "99990", "2", ymd(2026, 5, 2), "V0:;V0:;V0:;").await;

        let result = server
            .read_fin_summary_inner(
                Uuid::new_v4(),
                ReadFinSummaryParams {
                    symbol: "9999".to_string(),
                    limit: None,
                },
            )
            .await
            .expect("read_fin_summary");

        assert_eq!(
            result,
            ReadFinSummaryResult {
                items: vec![blank_dto(ymd(2026, 5, 2)), blank_dto(ymd(2026, 5, 1))],
            }
        );
    }

    #[backend_test_macros::database_test]
    async fn read_fin_summary_orders_newest_first_and_respects_limit(
        db: crate::database::DatabaseHandle,
    ) {
        let server = build_server(db.clone());

        for (disc_no, date) in [
            ("1", ymd(2026, 1, 1)),
            ("2", ymd(2026, 2, 1)),
            ("3", ymd(2026, 3, 1)),
        ] {
            seed(&db, "99990", disc_no, date, |summary| {
                summary.document_type = Set(Some(format!("doc-{disc_no}")));
            })
            .await;
        }

        let result = server
            .read_fin_summary_inner(
                Uuid::new_v4(),
                ReadFinSummaryParams {
                    symbol: "9999".to_string(),
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
