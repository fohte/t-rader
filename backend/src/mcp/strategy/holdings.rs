//! 戦略実行 MCP の `read_shareholding_structure` tool。
//!
//! 保有構造データの 3 テーブルを読む。保存する銘柄コードは 5 桁で、
//! 4 桁の銘柄コードと先頭 4 文字が一致する行を対象銘柄の書類として扱う。
//! 戦略に属さない市場データのため `search_refs` / `search_news`
//! 同様、`x-strategy-id` を検索条件には使わない。

use core_domain::holdings::{
    CrossShareholding as DomainCrossShareholding,
    CrossShareholdingCategory as DomainCrossShareholdingCategory, CrossShareholdingContent,
    LargeVolumeReportType, LargeVolumeShareholdingContent, MajorShareholderContent,
    MajorShareholderReportType, MutualHolding as DomainMutualHolding,
};
use rmcp::ErrorData as McpError;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::from_value;
use uuid::Uuid;

use crate::entities::{
    cross_shareholding_documents, large_volume_shareholding_documents, major_shareholder_documents,
};

use super::dto::{
    CrossShareholdingCategory, CrossShareholdingDto, CrossShareholdingsReportDto,
    LargeVolumeDocumentType, LargeVolumeHolderDto, LargeVolumeReportDto, MajorShareholderDto,
    MajorShareholdersDocumentType, MajorShareholdersReportDto, MutualHolding,
    ReadShareholdingStructureParams, ReadShareholdingStructureResult,
};
use super::{StrategyServer, clamp_limit, code_range, db_error, internal_error, validate_symbol};

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

/// symbol の code range に一致する本文のある最新行 (提出日降順、書類 ID で tie-break) を 1 件取得する。
/// major_shareholders / cross_shareholdings は「直近の書類のみ返す」という同じクエリ形を
/// entity 違いで繰り返すため、ここに切り出す。
async fn latest_matching_document<E>(
    db: &impl sea_orm::ConnectionTrait,
    code_column: E::Column,
    sub_date_column: E::Column,
    doc_id_column: E::Column,
    details_column: E::Column,
    symbol: &str,
) -> Result<Option<E::Model>, sea_orm::DbErr>
where
    E: EntityTrait,
{
    let (lower, upper) = code_range(symbol);
    E::find()
        .filter(code_column.between(lower, upper))
        .filter(details_column.ne(serde_json::Value::Null))
        .order_by_desc(sub_date_column)
        .order_by_desc(doc_id_column)
        .one(db)
        .await
}

impl StrategyServer {
    pub(crate) async fn read_shareholding_structure_inner(
        &self,
        _session_strategy_id: Uuid,
        params: ReadShareholdingStructureParams,
    ) -> Result<ReadShareholdingStructureResult, McpError> {
        validate_symbol(&params.symbol)?;
        let limit = clamp_limit(params.limit);
        let (lower, upper) = code_range(&params.symbol);

        let large_volume_rows = large_volume_shareholding_documents::Entity::find()
            .filter(large_volume_shareholding_documents::Column::StockCode.between(lower, upper))
            .order_by_desc(large_volume_shareholding_documents::Column::SubmittedOn)
            .order_by_desc(large_volume_shareholding_documents::Column::DocumentId)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(db_error)?;

        let mut large_volume_reports = Vec::with_capacity(large_volume_rows.len());
        for row in large_volume_rows {
            let doc_id = row.document_id;
            let doc: LargeVolumeShareholdingContent = match from_value(row.details) {
                Ok(doc) => doc,
                Err(e) => {
                    tracing::warn!(
                        doc_id,
                        error = %e,
                        "malformed large volume shareholding details, skipping"
                    );
                    continue;
                }
            };
            large_volume_reports.push(LargeVolumeReportDto {
                doc_id,
                submitted_on: row.submitted_on,
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
            });
        }

        let major_shareholders_row =
            latest_matching_document::<major_shareholder_documents::Entity>(
                &self.db,
                major_shareholder_documents::Column::StockCode,
                major_shareholder_documents::Column::SubmittedOn,
                major_shareholder_documents::Column::DocumentId,
                major_shareholder_documents::Column::Details,
                &params.symbol,
            )
            .await
            .map_err(db_error)?;
        let major_shareholders = major_shareholders_row
            .map(|row| -> Result<_, McpError> {
                let doc_id = row.document_id;
                let doc: MajorShareholderContent = from_value(row.details).map_err(|e| {
                    internal_error(format!("malformed major shareholder details {doc_id}: {e}"))
                })?;
                Ok(MajorShareholdersReportDto {
                    doc_id,
                    submitted_on: row.submitted_on,
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
                })
            })
            .transpose()?;

        let cross_shareholdings_row =
            latest_matching_document::<cross_shareholding_documents::Entity>(
                &self.db,
                cross_shareholding_documents::Column::StockCode,
                cross_shareholding_documents::Column::SubmittedOn,
                cross_shareholding_documents::Column::DocumentId,
                cross_shareholding_documents::Column::Details,
                &params.symbol,
            )
            .await
            .map_err(db_error)?;
        let cross_shareholdings = cross_shareholdings_row
            .map(|row| -> Result<_, McpError> {
                let doc_id = row.document_id;
                let doc: CrossShareholdingContent = from_value(row.details).map_err(|e| {
                    internal_error(format!(
                        "malformed cross shareholding details {doc_id}: {e}"
                    ))
                })?;
                Ok(CrossShareholdingsReportDto {
                    doc_id,
                    submitted_on: row.submitted_on,
                    period_end: doc.period_end,
                    holdings: doc
                        .holdings
                        .into_iter()
                        .map(cross_shareholding_dto)
                        .collect(),
                })
            })
            .transpose()?;

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
    use chrono::NaiveDate;
    use rstest::rstest;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use serde_json::{Value, json};
    use uuid::Uuid;

    use crate::entities::{
        cross_shareholding_documents, large_volume_shareholding_documents,
        major_shareholder_documents,
    };
    use core_domain::holdings::{
        LargeVolumeReportType as DomainLargeVolumeReportType,
        MajorShareholderReportType as DomainMajorShareholderReportType,
    };

    use super::super::dto::{
        CrossShareholdingCategory, LargeVolumeDocumentType, LargeVolumeReportDto,
        MajorShareholdersDocumentType, MutualHolding, ReadShareholdingStructureParams,
        ReadShareholdingStructureResult,
    };
    use super::super::tests_common::build_server;
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

    async fn insert_large_volume(
        db: &impl sea_orm::ConnectionTrait,
        document_id: &str,
        stock_code: &str,
        submitted_on: NaiveDate,
        details: Value,
    ) {
        large_volume_shareholding_documents::ActiveModel {
            document_id: Set(document_id.to_string()),
            stock_code: Set(Some(stock_code.to_string())),
            filer_code: Set("E99999".to_string()),
            submitted_on: Set(submitted_on),
            details: Set(details),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert large volume shareholding");
    }

    async fn insert_major_shareholders(
        db: &impl sea_orm::ConnectionTrait,
        document_id: &str,
        stock_code: &str,
        submitted_on: NaiveDate,
        details: Value,
    ) {
        major_shareholder_documents::ActiveModel {
            document_id: Set(document_id.to_string()),
            stock_code: Set(Some(stock_code.to_string())),
            filer_code: Set("E99999".to_string()),
            submitted_on: Set(submitted_on),
            details: Set(details),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert major shareholders");
    }

    async fn insert_cross_shareholdings(
        db: &impl sea_orm::ConnectionTrait,
        document_id: &str,
        stock_code: &str,
        submitted_on: NaiveDate,
        details: Value,
    ) {
        cross_shareholding_documents::ActiveModel {
            document_id: Set(document_id.to_string()),
            stock_code: Set(Some(stock_code.to_string())),
            filer_code: Set("E99999".to_string()),
            submitted_on: Set(submitted_on),
            details: Set(details),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert cross shareholdings");
    }

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    async fn read(
        db: &crate::database::DatabaseHandle,
        symbol: &str,
    ) -> ReadShareholdingStructureResult {
        build_server(db.clone())
            .read_shareholding_structure_inner(
                Uuid::new_v4(),
                ReadShareholdingStructureParams {
                    symbol: symbol.to_string(),
                    limit: None,
                },
            )
            .await
            .expect("read_shareholding_structure")
    }

    #[backend_test_macros::database_test]
    async fn returns_empty_and_null_sections_when_nothing_ingested(
        db: crate::database::DatabaseHandle,
    ) {
        let result = read(&db, "9999").await;

        assert_eq!(
            result,
            ReadShareholdingStructureResult {
                symbol: "9999".to_string(),
                large_volume_reports: vec![],
                major_shareholders: None,
                cross_shareholdings: None,
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn rejects_non_4_digit_symbol(db: crate::database::DatabaseHandle) {
        let err = build_server(db)
            .read_shareholding_structure_inner(
                Uuid::new_v4(),
                ReadShareholdingStructureParams {
                    symbol: "72a3".to_string(),
                    limit: None,
                },
            )
            .await
            .expect_err("invalid symbol should be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn large_volume_reports_match_by_first_4_chars_newest_first_with_holder_details(
        db: crate::database::DatabaseHandle,
    ) {
        insert_large_volume(
            &db,
            "EXAMPLE-OLD",
            "99990",
            ymd(2026, 1, 10),
            json!({
                "report_type": "report",
                "total_shares_ratio": 0.0621,
                "holders": [
                    {"name": "Example Holder", "holding_purpose": "investment", "shares_held": 1000000, "shares_ratio": 0.0621},
                ],
            }),
        )
        .await;
        insert_large_volume(
            &db,
            "EXAMPLE-NEW",
            "99991",
            ymd(2026, 3, 1),
            json!({
                "report_type": "amendment",
                "change_reason": "additional purchase",
                "total_shares_ratio": 0.0801,
                "previous_total_shares_ratio": 0.0621,
                "holders": [
                    {
                        "name": "Example Holder",
                        "holding_purpose": "investment",
                        "shares_held": 1300000,
                        "shares_ratio": 0.0801,
                        "previous_shares_ratio": 0.0621,
                    },
                ],
            }),
        )
        .await;
        insert_large_volume(
            &db,
            "EXAMPLE-OTHER",
            "88880",
            ymd(2026, 3, 2),
            json!({"report_type": "report", "holders": []}),
        )
        .await;

        let result = read(&db, "9999").await;

        assert_eq!(
            result,
            ReadShareholdingStructureResult {
                symbol: "9999".to_string(),
                large_volume_reports: vec![
                    super::super::dto::LargeVolumeReportDto {
                        doc_id: "EXAMPLE-NEW".to_string(),
                        submitted_on: ymd(2026, 3, 1),
                        document_type: LargeVolumeDocumentType::Amendment,
                        change_reason: Some("additional purchase".to_string()),
                        total_shares_ratio: Some(0.0801),
                        total_shares_ratio_last: Some(0.0621),
                        holders: vec![super::super::dto::LargeVolumeHolderDto {
                            holder_name: "Example Holder".to_string(),
                            holding_purpose: Some("investment".to_string()),
                            shares_held: Some(1300000),
                            shares_ratio: Some(0.0801),
                            shares_ratio_last: Some(0.0621),
                        }],
                    },
                    super::super::dto::LargeVolumeReportDto {
                        doc_id: "EXAMPLE-OLD".to_string(),
                        submitted_on: ymd(2026, 1, 10),
                        document_type: LargeVolumeDocumentType::LargeVolumeReport,
                        change_reason: None,
                        total_shares_ratio: Some(0.0621),
                        total_shares_ratio_last: None,
                        holders: vec![super::super::dto::LargeVolumeHolderDto {
                            holder_name: "Example Holder".to_string(),
                            holding_purpose: Some("investment".to_string()),
                            shares_held: Some(1000000),
                            shares_ratio: Some(0.0621),
                            shares_ratio_last: None,
                        }],
                    },
                ],
                major_shareholders: None,
                cross_shareholdings: None,
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn large_volume_reports_respects_limit_after_ordering(
        db: crate::database::DatabaseHandle,
    ) {
        for (i, day) in [1u32, 2, 3].into_iter().enumerate() {
            insert_large_volume(
                &db,
                &format!("EXAMPLE-{i}"),
                "99990",
                ymd(2026, 1, day),
                json!({"report_type": "report", "holders": []}),
            )
            .await;
        }

        let result = build_server(db.clone())
            .read_shareholding_structure_inner(
                Uuid::new_v4(),
                ReadShareholdingStructureParams {
                    symbol: "9999".to_string(),
                    limit: Some(2),
                },
            )
            .await
            .expect("read_shareholding_structure");

        assert_eq!(
            result,
            ReadShareholdingStructureResult {
                symbol: "9999".to_string(),
                large_volume_reports: vec![
                    LargeVolumeReportDto {
                        doc_id: "EXAMPLE-2".to_string(),
                        submitted_on: ymd(2026, 1, 3),
                        document_type: LargeVolumeDocumentType::LargeVolumeReport,
                        change_reason: None,
                        total_shares_ratio: None,
                        total_shares_ratio_last: None,
                        holders: vec![],
                    },
                    LargeVolumeReportDto {
                        doc_id: "EXAMPLE-1".to_string(),
                        submitted_on: ymd(2026, 1, 2),
                        document_type: LargeVolumeDocumentType::LargeVolumeReport,
                        change_reason: None,
                        total_shares_ratio: None,
                        total_shares_ratio_last: None,
                        holders: vec![],
                    },
                ],
                major_shareholders: None,
                cross_shareholdings: None,
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn major_shareholders_returns_only_the_latest_filing(
        db: crate::database::DatabaseHandle,
    ) {
        insert_major_shareholders(
            &db,
            "EXAMPLE-OLD",
            "99990",
            ymd(2025, 6, 30),
            json!({
                "period_end": "2025-03-31",
                "report_type": "annual",
                "holders": [{"rank": 1, "name": "Example Holder A", "shares_held": 5000000, "shares_ratio": 0.15}],
            }),
        )
        .await;
        insert_major_shareholders(
            &db,
            "EXAMPLE-NEW",
            "99990",
            ymd(2026, 6, 30),
            json!({
                "period_end": "2026-03-31",
                "report_type": "annual",
                "holders": [
                    {"rank": 1, "name": "Example Holder A", "shares_held": 6000000, "shares_ratio": 0.18},
                    {"rank": 2, "name": "Example Holder B", "shares_held": 3000000, "shares_ratio": 0.09},
                ],
            }),
        )
        .await;

        let result = read(&db, "9999").await;

        assert_eq!(
            result,
            ReadShareholdingStructureResult {
                symbol: "9999".to_string(),
                large_volume_reports: vec![],
                major_shareholders: Some(super::super::dto::MajorShareholdersReportDto {
                    doc_id: "EXAMPLE-NEW".to_string(),
                    submitted_on: ymd(2026, 6, 30),
                    period_end: Some(ymd(2026, 3, 31)),
                    document_type: MajorShareholdersDocumentType::AnnualReport,
                    holders: vec![
                        super::super::dto::MajorShareholderDto {
                            rank: Some(1),
                            holder_name: "Example Holder A".to_string(),
                            shares_held: Some(6000000),
                            shares_ratio: Some(0.18),
                        },
                        super::super::dto::MajorShareholderDto {
                            rank: Some(2),
                            holder_name: "Example Holder B".to_string(),
                            shares_held: Some(3000000),
                            shares_ratio: Some(0.09),
                        },
                    ],
                }),
                cross_shareholdings: None,
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn major_shareholders_skips_documents_without_decoded_content(
        db: crate::database::DatabaseHandle,
    ) {
        insert_major_shareholders(
            &db,
            "EXAMPLE-VALID",
            "99990",
            ymd(2025, 6, 30),
            json!({
                "period_end": "2025-03-31",
                "report_type": "annual",
                "holders": [{"rank": 1, "name": "Example Holder", "shares_held": 5000000, "shares_ratio": 0.15}],
            }),
        )
        .await;
        insert_major_shareholders(
            &db,
            "EXAMPLE-UNPARSEABLE",
            "99990",
            ymd(2026, 6, 30),
            Value::Null,
        )
        .await;

        assert_eq!(
            read(&db, "9999").await,
            ReadShareholdingStructureResult {
                symbol: "9999".to_string(),
                large_volume_reports: vec![],
                major_shareholders: Some(super::super::dto::MajorShareholdersReportDto {
                    doc_id: "EXAMPLE-VALID".to_string(),
                    submitted_on: ymd(2025, 6, 30),
                    period_end: Some(ymd(2025, 3, 31)),
                    document_type: MajorShareholdersDocumentType::AnnualReport,
                    holders: vec![super::super::dto::MajorShareholderDto {
                        rank: Some(1),
                        holder_name: "Example Holder".to_string(),
                        shares_held: Some(5000000),
                        shares_ratio: Some(0.15),
                    }],
                }),
                cross_shareholdings: None,
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn cross_shareholdings_combines_spec_and_deem_with_mutual_holding(
        db: crate::database::DatabaseHandle,
    ) {
        insert_cross_shareholdings(
            &db,
            "EXAMPLE-CROSS",
            "99990",
            ymd(2026, 6, 30),
            json!({
                "period_end": "2026-03-31",
                "holdings": [
                        {
                            "issuer_name": "Example Issuer",
                            "issuer_stock_code": "99991",
                            "category": "specified",
                            "current_shares": 200000,
                            "previous_shares": 180000,
                            "current_book_value": 500000000,
                            "previous_book_value": 420000000,
                            "mutual_holding": "held",
                        },
                        {
                            "issuer_name": "Example Custodian",
                            "issuer_stock_code": null,
                            "category": "deemed",
                            "current_shares": 50000,
                            "previous_shares": null,
                            "current_book_value": 90000000,
                            "previous_book_value": null,
                            "mutual_holding": "unknown",
                        },
                ],
            }),
        )
        .await;

        let result = read(&db, "9999").await;

        assert_eq!(
            result,
            ReadShareholdingStructureResult {
                symbol: "9999".to_string(),
                large_volume_reports: vec![],
                major_shareholders: None,
                cross_shareholdings: Some(super::super::dto::CrossShareholdingsReportDto {
                    doc_id: "EXAMPLE-CROSS".to_string(),
                    submitted_on: ymd(2026, 6, 30),
                    period_end: Some(ymd(2026, 3, 31)),
                    holdings: vec![
                        super::super::dto::CrossShareholdingDto {
                            issuer_name: "Example Issuer".to_string(),
                            issuer_code: Some("99991".to_string()),
                            category: CrossShareholdingCategory::Specified,
                            current_shares: Some(200000),
                            previous_shares: Some(180000),
                            current_book_value: Some(500000000),
                            previous_book_value: Some(420000000),
                            mutual_holding: MutualHolding::Held,
                        },
                        super::super::dto::CrossShareholdingDto {
                            issuer_name: "Example Custodian".to_string(),
                            issuer_code: None,
                            category: CrossShareholdingCategory::Deemed,
                            current_shares: Some(50000),
                            previous_shares: None,
                            current_book_value: Some(90000000),
                            previous_book_value: None,
                            mutual_holding: MutualHolding::Unknown,
                        },
                    ],
                }),
            },
        );
    }
}
