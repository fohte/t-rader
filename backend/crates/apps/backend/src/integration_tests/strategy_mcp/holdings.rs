#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use serde_json::{Value, json};

    use gateway_postgres::entities::{
        cross_shareholding_documents, large_volume_shareholding_documents,
        major_shareholder_documents,
    };

    use super::super::dto::{
        CrossShareholdingCategory, LargeVolumeDocumentType, LargeVolumeReportDto,
        MajorShareholdersDocumentType, MutualHolding, ReadShareholdingStructureParams,
        ReadShareholdingStructureResult,
    };
    use super::super::tests_common::{build_server, insert_strategy};

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
        db: &gateway_postgres::DatabaseHandle,
        symbol: &str,
    ) -> super::super::ToolOutput<ReadShareholdingStructureResult> {
        let strategy_id = insert_strategy(db, "sample").await;
        build_server(db.clone())
            .read_shareholding_structure(
                strategy_id,
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
        db: gateway_postgres::DatabaseHandle,
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
    async fn rejects_non_4_digit_symbol(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "sample").await;
        let err = build_server(db)
            .read_shareholding_structure(
                strategy_id,
                ReadShareholdingStructureParams {
                    symbol: "72a3".to_string(),
                    limit: None,
                },
            )
            .await
            .expect_err("invalid symbol should be rejected");
        assert_eq!(
            err,
            rmcp::ErrorData::invalid_params(
                "symbol must be a 4-digit stock code, got \"72a3\"",
                None,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn large_volume_reports_match_by_first_4_chars_newest_first_with_holder_details(
        db: gateway_postgres::DatabaseHandle,
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
        db: gateway_postgres::DatabaseHandle,
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

        let strategy_id = insert_strategy(&db, "sample").await;
        let result = build_server(db.clone())
            .read_shareholding_structure(
                strategy_id,
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
        db: gateway_postgres::DatabaseHandle,
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
        db: gateway_postgres::DatabaseHandle,
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
        db: gateway_postgres::DatabaseHandle,
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
