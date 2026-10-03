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
            .dependencies
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
    use super::blank_to_none;
    use rstest::rstest;

    #[rstest]
    #[case::blank(String::new(), None)]
    #[case::non_blank("Synthetic value".to_string(), Some("Synthetic value".to_string()))]
    fn blank_to_none_cases(#[case] input: String, #[case] expected: Option<String>) {
        assert_eq!(blank_to_none(input), expected);
    }
}
