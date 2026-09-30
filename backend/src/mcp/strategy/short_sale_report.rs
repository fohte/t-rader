//! 戦略実行 MCP の `read_short_sale_reports` tool。`short_sale_report` (J-Quants
//! `/markets/short-sale-report` の空売り残高報告) を銘柄・期間で読む。
//!
//! `code` は J-Quants の 5 桁コードで、4 桁の銘柄コードとの対応関係は仕様に明記されて
//! いないため、`read_shareholding_structure` と同様に先頭 4 文字が一致する行を対象銘柄
//! として扱う。空売り残高報告は会社単位の開示であり戦略に属さない市場データのため、
//! `search_refs` / `search_news` 同様 `x-strategy-id` を検索条件には使わない。

use core_application::short_sale_report::ShortSaleReportQuery;
use core_application::strategy_scope::StrategyScope;
use core_domain::short_sale_report::ShortSaleReport;
use rmcp::ErrorData as McpError;

use super::dto::{ReadShortSaleReportsParams, ReadShortSaleReportsResult, ShortSaleReportDto};
use super::{
    StrategyServer, clamp_limit, code_range, decimal_to_f64, internal_error, validate_symbol,
};

/// 該当しない項目は空文字のまま入っているため、空文字は「記載なし」として null にする。
fn blank_to_none(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

fn short_sale_report_dto(row: ShortSaleReport) -> ShortSaleReportDto {
    ShortSaleReportDto {
        disc_date: row.disc_date,
        calc_date: row.calc_date,
        reporter_name: row.ss_name,
        reporter_address: blank_to_none(row.ss_addr),
        client_name: blank_to_none(row.dic_name),
        client_address: blank_to_none(row.dic_addr),
        fund_name: blank_to_none(row.fund_name),
        short_position_ratio: decimal_to_f64(row.short_position_ratio),
        short_position_shares: row.short_position_shares,
        short_position_units: row.short_position_units,
        prev_report_date: row.prev_report_date,
        prev_report_ratio: row.prev_report_ratio.map(decimal_to_f64),
        notes: blank_to_none(row.notes),
    }
}

impl StrategyServer {
    pub(crate) async fn read_short_sale_reports_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: ReadShortSaleReportsParams,
    ) -> Result<ReadShortSaleReportsResult, McpError> {
        let scope = scope.into();
        validate_symbol(&params.symbol)?;
        let limit = clamp_limit(params.limit);
        let (code_from, code_to) = code_range(&params.symbol);
        let rows = self
            .use_cases
            .short_sale_reports
            .read(
                scope,
                ShortSaleReportQuery {
                    code_from,
                    code_to,
                    from: params.from,
                    to: params.to,
                    limit,
                },
            )
            .await
            .map_err(|error| {
                tracing::error!(%error, "strategy mcp short sale report query failed");
                internal_error(format!("database error: {error}"))
            })?;

        Ok(ReadShortSaleReportsResult {
            symbol: params.symbol,
            items: rows.into_iter().map(short_sale_report_dto).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::NaiveDate;
    use core_application::short_sale_report::{
        FakeShortSaleReportRepository, ShortSaleReportQuery, ShortSaleReportUseCases,
    };
    use core_domain::short_sale_report::ShortSaleReport;
    use rstest::rstest;
    use sea_orm::{DatabaseBackend, MockDatabase};
    use uuid::Uuid;

    use super::super::dto::{
        ReadShortSaleReportsParams, ReadShortSaleReportsResult, ShortSaleReportDto,
    };
    use super::super::tests_common::build_server;
    use super::blank_to_none;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn report(
        disc_date: NaiveDate,
        calc_date: NaiveDate,
        code: &str,
        reporter: &str,
        ratio: &str,
        previous: Option<(NaiveDate, &str)>,
    ) -> ShortSaleReport {
        ShortSaleReport {
            disc_date,
            calc_date,
            code: code.into(),
            ss_name: reporter.into(),
            ss_addr: String::new(),
            dic_name: String::new(),
            dic_addr: String::new(),
            fund_name: String::new(),
            short_position_ratio: ratio.parse().unwrap(),
            short_position_shares: 1_000,
            short_position_units: 10,
            prev_report_date: previous.map(|(date, _)| date),
            prev_report_ratio: previous.map(|(_, ratio)| ratio.parse().unwrap()),
            notes: String::new(),
        }
    }

    fn server(repository: Arc<FakeShortSaleReportRepository>) -> super::super::StrategyServer {
        let db = gateway_postgres::DatabaseHandle::from(
            MockDatabase::new(DatabaseBackend::Postgres).into_connection(),
        );
        let mut server = build_server(db);
        server.use_cases.short_sale_reports = ShortSaleReportUseCases::new(repository);
        server
    }

    #[rstest]
    #[case::blank(String::new(), None)]
    #[case::non_blank("Synthetic value".to_string(), Some("Synthetic value".to_string()))]
    fn blank_to_none_cases(#[case] input: String, #[case] expected: Option<String>) {
        assert_eq!(blank_to_none(input), expected);
    }

    #[tokio::test]
    async fn rejects_non_four_digit_symbol() {
        let repository = Arc::new(FakeShortSaleReportRepository::new());
        let error = server(repository)
            .read_short_sale_reports_inner(
                Uuid::new_v4(),
                ReadShortSaleReportsParams {
                    symbol: "00A0".into(),
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect_err("non-numeric symbol should be rejected");

        assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn maps_matching_reports_and_preserves_query_range() {
        let repository = Arc::new(FakeShortSaleReportRepository::new());
        repository.rows.lock().unwrap().extend([
            report(
                ymd(2025, 3, 1),
                ymd(2025, 2, 26),
                "00001",
                "Synthetic A",
                "0.0153",
                Some((ymd(2025, 2, 19), "0.0102")),
            ),
            report(
                ymd(2025, 3, 2),
                ymd(2025, 2, 27),
                "00009",
                "Synthetic B",
                "0.0089",
                None,
            ),
            report(
                ymd(2025, 3, 2),
                ymd(2025, 2, 27),
                "99999",
                "Unrelated synthetic reporter",
                "0.0201",
                None,
            ),
        ]);

        let result = server(repository.clone())
            .read_short_sale_reports_inner(
                Uuid::new_v4(),
                ReadShortSaleReportsParams {
                    symbol: "0000".into(),
                    from: Some(ymd(2025, 3, 1)),
                    to: Some(ymd(2025, 3, 2)),
                    limit: Some(10),
                },
            )
            .await
            .expect("read short sale reports");

        assert_eq!(
            (result, repository.queries.lock().unwrap().clone(),),
            (
                ReadShortSaleReportsResult {
                    symbol: "0000".into(),
                    items: vec![
                        ShortSaleReportDto {
                            disc_date: ymd(2025, 3, 2),
                            calc_date: ymd(2025, 2, 27),
                            reporter_name: "Synthetic B".into(),
                            reporter_address: None,
                            client_name: None,
                            client_address: None,
                            fund_name: None,
                            short_position_ratio: 0.0089,
                            short_position_shares: 1_000,
                            short_position_units: 10,
                            prev_report_date: None,
                            prev_report_ratio: None,
                            notes: None,
                        },
                        ShortSaleReportDto {
                            disc_date: ymd(2025, 3, 1),
                            calc_date: ymd(2025, 2, 26),
                            reporter_name: "Synthetic A".into(),
                            reporter_address: None,
                            client_name: None,
                            client_address: None,
                            fund_name: None,
                            short_position_ratio: 0.0153,
                            short_position_shares: 1_000,
                            short_position_units: 10,
                            prev_report_date: Some(ymd(2025, 2, 19)),
                            prev_report_ratio: Some(0.0102),
                            notes: None,
                        },
                    ],
                },
                vec![ShortSaleReportQuery {
                    code_from: "00000".into(),
                    code_to: "00009".into(),
                    from: Some(ymd(2025, 3, 1)),
                    to: Some(ymd(2025, 3, 2)),
                    limit: 10,
                }],
            ),
        );
    }

    #[tokio::test]
    async fn respects_limit_after_report_ordering() {
        let repository = Arc::new(FakeShortSaleReportRepository::new());
        repository.rows.lock().unwrap().extend([
            report(
                ymd(2025, 1, 1),
                ymd(2025, 1, 1),
                "00001",
                "Synthetic A",
                "0.01",
                None,
            ),
            report(
                ymd(2025, 1, 2),
                ymd(2025, 1, 2),
                "00001",
                "Synthetic B",
                "0.01",
                None,
            ),
            report(
                ymd(2025, 1, 3),
                ymd(2025, 1, 3),
                "00001",
                "Synthetic C",
                "0.01",
                None,
            ),
        ]);

        let result = server(repository)
            .read_short_sale_reports_inner(
                Uuid::new_v4(),
                ReadShortSaleReportsParams {
                    symbol: "0000".into(),
                    from: None,
                    to: None,
                    limit: Some(2),
                },
            )
            .await
            .expect("read short sale reports");

        assert_eq!(
            result.items,
            vec![
                ShortSaleReportDto {
                    disc_date: ymd(2025, 1, 3),
                    calc_date: ymd(2025, 1, 3),
                    reporter_name: "Synthetic C".into(),
                    reporter_address: None,
                    client_name: None,
                    client_address: None,
                    fund_name: None,
                    short_position_ratio: 0.01,
                    short_position_shares: 1_000,
                    short_position_units: 10,
                    prev_report_date: None,
                    prev_report_ratio: None,
                    notes: None,
                },
                ShortSaleReportDto {
                    disc_date: ymd(2025, 1, 2),
                    calc_date: ymd(2025, 1, 2),
                    reporter_name: "Synthetic B".into(),
                    reporter_address: None,
                    client_name: None,
                    client_address: None,
                    fund_name: None,
                    short_position_ratio: 0.01,
                    short_position_shares: 1_000,
                    short_position_units: 10,
                    prev_report_date: None,
                    prev_report_ratio: None,
                    notes: None,
                },
            ],
        );
    }
}
