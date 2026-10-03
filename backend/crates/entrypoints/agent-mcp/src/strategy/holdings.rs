//! 戦略実行 MCP の `read_shareholding_structure` tool。
//!
//! 保有構造データの 3 テーブルを読む。保存する銘柄コードは 5 桁で、
//! 4 桁の銘柄コードと先頭 4 文字が一致する行を対象銘柄の書類として扱う。
//! 戦略に属さない市場データのため `search_refs` / `search_news`
//! 同様、`x-strategy-id` を検索条件には使わない。

use core_application::shareholding_structure::{
    ShareholdingStructureRepositoryError, ShareholdingStructureUseCaseError,
};
use core_application::strategy_scope::StrategyScope;
use core_domain::holdings::{
    CrossShareholding as DomainCrossShareholding,
    CrossShareholdingCategory as DomainCrossShareholdingCategory, LargeVolumeReportType,
    MajorShareholderReportType, MutualHolding as DomainMutualHolding,
};
use rmcp::ErrorData as McpError;

use super::dto::{
    CrossShareholdingCategory, CrossShareholdingDto, CrossShareholdingsReportDto,
    LargeVolumeDocumentType, LargeVolumeHolderDto, LargeVolumeReportDto, MajorShareholderDto,
    MajorShareholdersDocumentType, MajorShareholdersReportDto, MutualHolding,
    ReadShareholdingStructureParams, ReadShareholdingStructureResult,
};
use super::{StrategyServer, clamp_limit, internal_error, validate_symbol};

fn large_volume_document_type(report_type: LargeVolumeReportType) -> LargeVolumeDocumentType {
    match report_type {
        LargeVolumeReportType::Report => LargeVolumeDocumentType::LargeVolumeReport,
        LargeVolumeReportType::Amendment => LargeVolumeDocumentType::Amendment,
        LargeVolumeReportType::AmendmentRapidTransfer => {
            LargeVolumeDocumentType::AmendmentRapidTransfer
        }
        LargeVolumeReportType::ReportSpecial => LargeVolumeDocumentType::LargeVolumeReportSpecial,
        LargeVolumeReportType::AmendmentSpecial => LargeVolumeDocumentType::AmendmentSpecial,
        LargeVolumeReportType::Unknown => LargeVolumeDocumentType::Unknown,
    }
}

fn major_shareholders_document_type(
    report_type: MajorShareholderReportType,
) -> MajorShareholdersDocumentType {
    match report_type {
        MajorShareholderReportType::Annual => MajorShareholdersDocumentType::AnnualReport,
        MajorShareholderReportType::Quarterly => MajorShareholdersDocumentType::QuarterlyReport,
        MajorShareholderReportType::SemiAnnual => MajorShareholdersDocumentType::SemiAnnualReport,
        MajorShareholderReportType::Unknown => MajorShareholdersDocumentType::Unknown,
    }
}

fn cross_shareholding_dto(entry: DomainCrossShareholding) -> CrossShareholdingDto {
    CrossShareholdingDto {
        issuer_name: entry.issuer_name,
        issuer_code: entry.issuer_stock_code,
        category: match entry.category {
            DomainCrossShareholdingCategory::Specified => CrossShareholdingCategory::Specified,
            DomainCrossShareholdingCategory::Deemed => CrossShareholdingCategory::Deemed,
        },
        current_shares: entry.current_shares,
        previous_shares: entry.previous_shares,
        current_book_value: entry.current_book_value,
        previous_book_value: entry.previous_book_value,
        mutual_holding: match entry.mutual_holding {
            DomainMutualHolding::Held => MutualHolding::Held,
            DomainMutualHolding::NotHeld => MutualHolding::NotHeld,
            DomainMutualHolding::Unknown => MutualHolding::Unknown,
        },
    }
}

impl StrategyServer {
    pub(crate) async fn read_shareholding_structure_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: ReadShareholdingStructureParams,
    ) -> Result<ReadShareholdingStructureResult, McpError> {
        let scope = scope.into();
        validate_symbol(&params.symbol)?;
        let limit = clamp_limit(params.limit);
        let shareholding = self
            .dependencies
            .shareholding_structures
            .find_for_symbol(scope, &params.symbol, limit)
            .await
            .map_err(|error| {
                tracing::error!(%error, "strategy mcp shareholding structure query failed");
                match error {
                    ShareholdingStructureUseCaseError::Repository(
                        ShareholdingStructureRepositoryError::MalformedDocument { .. },
                    ) => internal_error(error.to_string()),
                    _ => internal_error(format!("database error: {error}")),
                }
            })?;

        let large_volume_reports = shareholding
            .large_volume_reports
            .into_iter()
            .filter_map(|row| {
                let doc = row.content?;
                Some(LargeVolumeReportDto {
                    doc_id: row.metadata.document_id,
                    submitted_on: row.metadata.submitted_on,
                    document_type: large_volume_document_type(doc.report_type),
                    change_reason: doc.change_reason,
                    total_shares_ratio: doc.total_shares_ratio,
                    total_shares_ratio_last: doc.previous_total_shares_ratio,
                    holders: doc
                        .holders
                        .into_iter()
                        .map(|holder| LargeVolumeHolderDto {
                            holder_name: holder.name,
                            holding_purpose: holder.holding_purpose,
                            shares_held: holder.shares_held,
                            shares_ratio: holder.shares_ratio,
                            shares_ratio_last: holder.previous_shares_ratio,
                        })
                        .collect(),
                })
            })
            .collect();
        let major_shareholders = shareholding
            .major_shareholders
            .and_then(|row| row.content.map(|doc| (row.metadata, doc)))
            .map(|(metadata, doc)| MajorShareholdersReportDto {
                doc_id: metadata.document_id,
                submitted_on: metadata.submitted_on,
                period_end: doc.period_end,
                document_type: major_shareholders_document_type(doc.report_type),
                holders: doc
                    .holders
                    .into_iter()
                    .map(|holder| MajorShareholderDto {
                        rank: holder.rank,
                        holder_name: holder.name,
                        shares_held: holder.shares_held,
                        shares_ratio: holder.shares_ratio,
                    })
                    .collect(),
            });
        let cross_shareholdings = shareholding
            .cross_shareholdings
            .and_then(|row| row.content.map(|doc| (row.metadata, doc)))
            .map(|(metadata, doc)| CrossShareholdingsReportDto {
                doc_id: metadata.document_id,
                submitted_on: metadata.submitted_on,
                period_end: doc.period_end,
                holdings: doc
                    .holdings
                    .into_iter()
                    .map(cross_shareholding_dto)
                    .collect(),
            });

        Ok(ReadShareholdingStructureResult {
            symbol: params.symbol,
            large_volume_reports,
            major_shareholders,
            cross_shareholdings,
        })
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use core_domain::holdings::{
        LargeVolumeReportType as DomainLargeVolumeReportType,
        MajorShareholderReportType as DomainMajorShareholderReportType,
    };

    use super::super::dto::{LargeVolumeDocumentType, MajorShareholdersDocumentType};
    use super::{large_volume_document_type, major_shareholders_document_type, validate_symbol};

    #[rstest]
    #[case::valid("9999", true)]
    #[case::too_short("720", false)]
    #[case::too_long("99990", false)]
    #[case::non_digit("72a3", false)]
    #[case::empty("", false)]
    fn validate_symbol_cases(#[case] symbol: &str, #[case] expected_ok: bool) {
        assert_eq!(validate_symbol(symbol).is_ok(), expected_ok);
    }

    #[test]
    fn validate_symbol_explains_that_foreign_stocks_are_unsupported() {
        let error = validate_symbol("KR:QZ9012").expect_err("foreign stock is unsupported");

        assert_eq!(
            error,
            rmcp::ErrorData::invalid_params(
                "only Japanese stocks are supported by this tool",
                None,
            ),
        );
    }

    #[rstest]
    #[case::report(
        DomainLargeVolumeReportType::Report,
        LargeVolumeDocumentType::LargeVolumeReport
    )]
    #[case::amendment(
        DomainLargeVolumeReportType::Amendment,
        LargeVolumeDocumentType::Amendment
    )]
    #[case::amendment_rapid_transfer(
        DomainLargeVolumeReportType::AmendmentRapidTransfer,
        LargeVolumeDocumentType::AmendmentRapidTransfer
    )]
    #[case::report_special(
        DomainLargeVolumeReportType::ReportSpecial,
        LargeVolumeDocumentType::LargeVolumeReportSpecial
    )]
    #[case::amendment_special(
        DomainLargeVolumeReportType::AmendmentSpecial,
        LargeVolumeDocumentType::AmendmentSpecial
    )]
    #[case::unknown(DomainLargeVolumeReportType::Unknown, LargeVolumeDocumentType::Unknown)]
    fn large_volume_document_type_cases(
        #[case] report_type: DomainLargeVolumeReportType,
        #[case] expected: LargeVolumeDocumentType,
    ) {
        assert_eq!(large_volume_document_type(report_type), expected);
    }

    #[rstest]
    #[case::annual(
        DomainMajorShareholderReportType::Annual,
        MajorShareholdersDocumentType::AnnualReport
    )]
    #[case::quarterly(
        DomainMajorShareholderReportType::Quarterly,
        MajorShareholdersDocumentType::QuarterlyReport
    )]
    #[case::semi_annual(
        DomainMajorShareholderReportType::SemiAnnual,
        MajorShareholdersDocumentType::SemiAnnualReport
    )]
    #[case::unknown(
        DomainMajorShareholderReportType::Unknown,
        MajorShareholdersDocumentType::Unknown
    )]
    fn major_shareholders_document_type_cases(
        #[case] report_type: DomainMajorShareholderReportType,
        #[case] expected: MajorShareholdersDocumentType,
    ) {
        assert_eq!(major_shareholders_document_type(report_type), expected);
    }
}
