//! 戦略実行 MCP の `read_short_sale_reports` tool。`short_sale_report` (J-Quants
//! `/markets/short-sale-report` の空売り残高報告) を銘柄・期間で読む。
//!
//! `code` は J-Quants の 5 桁コードで、4 桁の銘柄コードとの対応関係は仕様に明記されて
//! いないため、`read_shareholding_structure` と同様に先頭 4 文字が一致する行を対象銘柄
//! として扱う。空売り残高報告は会社単位の開示であり戦略に属さない市場データのため、
//! `search_refs` / `search_news` 同様 `x-strategy-id` を検索条件には使わない。

use rmcp::ErrorData as McpError;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::entities::short_sale_report;

use super::dto::{ReadShortSaleReportsParams, ReadShortSaleReportsResult, ShortSaleReportDto};
use super::{StrategyServer, clamp_limit, code_range, db_error, decimal_to_f64, validate_symbol};

/// 該当しない項目は空文字のまま入っているため、空文字は「記載なし」として null にする。
fn blank_to_none(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

fn short_sale_report_dto(row: short_sale_report::Model) -> ShortSaleReportDto {
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
        _session_strategy_id: Uuid,
        params: ReadShortSaleReportsParams,
    ) -> Result<ReadShortSaleReportsResult, McpError> {
        validate_symbol(&params.symbol)?;
        let limit = clamp_limit(params.limit);
        let (lower, upper) = code_range(&params.symbol);

        let mut query = short_sale_report::Entity::find()
            .filter(short_sale_report::Column::Code.between(lower, upper));
        if let Some(from) = params.from {
            query = query.filter(short_sale_report::Column::DiscDate.gte(from));
        }
        if let Some(to) = params.to {
            query = query.filter(short_sale_report::Column::DiscDate.lte(to));
        }

        let rows = query
            .order_by_desc(short_sale_report::Column::DiscDate)
            .order_by_asc(short_sale_report::Column::SsName)
            .order_by_asc(short_sale_report::Column::SsAddr)
            .order_by_asc(short_sale_report::Column::DicName)
            .order_by_asc(short_sale_report::Column::DicAddr)
            .order_by_asc(short_sale_report::Column::FundName)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(db_error)?;

        Ok(ReadShortSaleReportsResult {
            symbol: params.symbol,
            items: rows.into_iter().map(short_sale_report_dto).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use rstest::rstest;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;

    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::short_sale_report;
    use crate::testing::create_test_db;

    use super::super::dto::{ReadShortSaleReportsParams, ReadShortSaleReportsResult};
    use super::super::tests_common::build_server;
    use super::blank_to_none;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    /// `prev` は (直近計算年月日, 直近残高割合)。初回報告なら None
    async fn seed(
        db: &impl sea_orm::ConnectionTrait,
        code: &str,
        disc_date: NaiveDate,
        calc_date: NaiveDate,
        ss_name: &str,
        short_position_ratio: &str,
        prev: Option<(NaiveDate, &str)>,
    ) {
        short_sale_report::ActiveModel {
            disc_date: Set(disc_date),
            calc_date: Set(calc_date),
            code: Set(code.to_string()),
            ss_name: Set(ss_name.to_string()),
            ss_addr: Set(String::new()),
            dic_name: Set(String::new()),
            dic_addr: Set(String::new()),
            fund_name: Set(String::new()),
            short_position_ratio: Set(short_position_ratio.parse().expect("valid decimal")),
            short_position_shares: Set(1_000_000),
            short_position_units: Set(10_000),
            prev_report_date: Set(prev.map(|(date, _)| date)),
            prev_report_ratio: Set(prev.map(|(_, ratio)| ratio.parse().expect("valid decimal"))),
            notes: Set(String::new()),
        }
        .insert(db)
        .await
        .expect("seed short sale report");
    }

    #[rstest]
    #[case::blank(String::new(), None)]
    #[case::non_blank("foo".to_string(), Some("foo".to_string()))]
    fn blank_to_none_cases(#[case] input: String, #[case] expected: Option<String>) {
        assert_eq!(blank_to_none(input), expected);
    }

    #[backend_test_macros::database_test]
    async fn rejects_non_4_digit_symbol(pool: PgPool) {
        let db = create_test_db(pool).await;

        let err = build_server(db)
            .read_short_sale_reports_inner(
                Uuid::new_v4(),
                ReadShortSaleReportsParams {
                    symbol: "72a3".to_string(),
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect_err("invalid symbol should be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn matches_by_first_4_chars_newest_first_with_blank_fields_as_null(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed(
            &db,
            "72030",
            ymd(2026, 3, 1),
            ymd(2026, 2, 26),
            "Alpha Capital",
            "0.0153",
            Some((ymd(2026, 2, 19), "0.0102")),
        )
        .await;
        seed(
            &db,
            "72031",
            ymd(2026, 3, 2),
            ymd(2026, 2, 27),
            "Other Share Class Holder",
            "0.0089",
            None,
        )
        .await;
        seed(
            &db,
            "99840",
            ymd(2026, 3, 2),
            ymd(2026, 2, 27),
            "Unrelated Fund",
            "0.0201",
            None,
        )
        .await;

        let result = build_server(db)
            .read_short_sale_reports_inner(
                Uuid::new_v4(),
                ReadShortSaleReportsParams {
                    symbol: "7203".to_string(),
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect("read_short_sale_reports");

        assert_eq!(
            result,
            ReadShortSaleReportsResult {
                symbol: "7203".to_string(),
                items: vec![
                    super::super::dto::ShortSaleReportDto {
                        disc_date: ymd(2026, 3, 2),
                        calc_date: ymd(2026, 2, 27),
                        reporter_name: "Other Share Class Holder".to_string(),
                        reporter_address: None,
                        client_name: None,
                        client_address: None,
                        fund_name: None,
                        short_position_ratio: 0.0089,
                        short_position_shares: 1_000_000,
                        short_position_units: 10_000,
                        prev_report_date: None,
                        prev_report_ratio: None,
                        notes: None,
                    },
                    super::super::dto::ShortSaleReportDto {
                        disc_date: ymd(2026, 3, 1),
                        calc_date: ymd(2026, 2, 26),
                        reporter_name: "Alpha Capital".to_string(),
                        reporter_address: None,
                        client_name: None,
                        client_address: None,
                        fund_name: None,
                        short_position_ratio: 0.0153,
                        short_position_shares: 1_000_000,
                        short_position_units: 10_000,
                        prev_report_date: Some(ymd(2026, 2, 19)),
                        prev_report_ratio: Some(0.0102),
                        notes: None,
                    },
                ],
            }
        );
    }

    #[backend_test_macros::database_test]
    async fn filters_by_disc_date_range(pool: PgPool) {
        let db = create_test_db(pool).await;
        for (day, name) in [(1u32, "Jan"), (15, "Mid"), (28, "Late")] {
            seed(
                &db,
                "72030",
                ymd(2026, 1, day),
                ymd(2026, 1, day),
                name,
                "0.01",
                None,
            )
            .await;
        }

        let result = build_server(db)
            .read_short_sale_reports_inner(
                Uuid::new_v4(),
                ReadShortSaleReportsParams {
                    symbol: "7203".to_string(),
                    from: Some(ymd(2026, 1, 10)),
                    to: Some(ymd(2026, 1, 20)),
                    limit: None,
                },
            )
            .await
            .expect("read_short_sale_reports");

        assert_eq!(
            result
                .items
                .iter()
                .map(|i| i.reporter_name.clone())
                .collect::<Vec<_>>(),
            vec!["Mid".to_string()],
        );
    }

    #[backend_test_macros::database_test]
    async fn respects_limit_after_ordering(pool: PgPool) {
        let db = create_test_db(pool).await;
        for (day, name) in [(1u32, "A"), (2, "B"), (3, "C")] {
            seed(
                &db,
                "72030",
                ymd(2026, 1, day),
                ymd(2026, 1, day),
                name,
                "0.01",
                None,
            )
            .await;
        }

        let result = build_server(db)
            .read_short_sale_reports_inner(
                Uuid::new_v4(),
                ReadShortSaleReportsParams {
                    symbol: "7203".to_string(),
                    from: None,
                    to: None,
                    limit: Some(2),
                },
            )
            .await
            .expect("read_short_sale_reports");

        assert_eq!(
            result
                .items
                .iter()
                .map(|i| i.reporter_name.clone())
                .collect::<Vec<_>>(),
            vec!["C".to_string(), "B".to_string()],
        );
    }
}
