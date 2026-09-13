//! 戦略実行 MCP の `read_shareholding_structure` tool。
//!
//! J-Quants EDINET 保有構造データ (`backend/src/services/edinet_holdings/`) の 3 テーブル
//! (`edinet_large_volume_shareholdings` / `edinet_major_shareholders` /
//! `edinet_cross_shareholdings`) を読む。`code` は J-Quants の 5 桁コードで、
//! 4 桁の銘柄コードとの対応関係は仕様に明記されていないため、先頭 4 文字が一致する行を
//! 対象銘柄の書類として扱う。戦略に属さない市場データのため `search_refs` / `search_news`
//! 同様、`x-strategy-id` を検索条件には使わない。

use chrono::NaiveDate;
use rmcp::ErrorData as McpError;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use uuid::Uuid;

use crate::entities::{
    edinet_cross_shareholdings, edinet_large_volume_shareholdings, edinet_major_shareholders,
};

use super::dto::{
    CrossShareholdingCategory, CrossShareholdingDto, CrossShareholdingsReportDto,
    LargeVolumeDocumentType, LargeVolumeHolderDto, LargeVolumeReportDto, MajorShareholderDto,
    MajorShareholdersDocumentType, MajorShareholdersReportDto, MutualHolding,
    ReadShareholdingStructureParams, ReadShareholdingStructureResult,
};
use super::{StrategyServer, clamp_limit, db_error, internal_error, invalid_params};

#[derive(Debug, Deserialize)]
struct RawHolder {
    #[serde(rename = "HldrName")]
    hldr_name: String,
    #[serde(rename = "HldgPurp")]
    hldg_purp: Option<String>,
    #[serde(rename = "ShsHeld")]
    shs_held: Option<i64>,
    #[serde(rename = "ShsRatio")]
    shs_ratio: Option<f64>,
    #[serde(rename = "ShsRatioLast")]
    shs_ratio_last: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawLargeVolumeDocument {
    #[serde(rename = "LargeHldgTypeCode")]
    large_hldg_type_code: Option<String>,
    #[serde(rename = "ChgRsn")]
    chg_rsn: Option<String>,
    #[serde(rename = "TotalShsRatio")]
    total_shs_ratio: Option<f64>,
    #[serde(rename = "TotalShsRatioLast")]
    total_shs_ratio_last: Option<f64>,
    #[serde(rename = "Hldrs", default)]
    hldrs: Vec<RawHolder>,
}

fn large_volume_document_type(code: Option<&str>) -> LargeVolumeDocumentType {
    match code {
        Some("1") => LargeVolumeDocumentType::LargeVolumeReport,
        Some("2") => LargeVolumeDocumentType::Amendment,
        Some("3") => LargeVolumeDocumentType::AmendmentRapidTransfer,
        Some("4") => LargeVolumeDocumentType::LargeVolumeReportSpecial,
        Some("5") => LargeVolumeDocumentType::AmendmentSpecial,
        _ => LargeVolumeDocumentType::Unknown,
    }
}

#[derive(Debug, Deserialize)]
struct RawMajorShareholder {
    #[serde(rename = "Rank")]
    rank: Option<i32>,
    #[serde(rename = "HldrName")]
    hldr_name: String,
    #[serde(rename = "ShsHeld")]
    shs_held: Option<i64>,
    #[serde(rename = "ShsRatio")]
    shs_ratio: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawMajorShareholdersDocument {
    #[serde(rename = "PerEn")]
    per_en: Option<NaiveDate>,
    #[serde(rename = "DocTypeCode")]
    doc_type_code: Option<String>,
    #[serde(rename = "Hldrs", default)]
    hldrs: Vec<RawMajorShareholder>,
}

fn major_shareholders_document_type(code: Option<&str>) -> MajorShareholdersDocumentType {
    match code {
        Some("120") => MajorShareholdersDocumentType::AnnualReport,
        Some("140") => MajorShareholdersDocumentType::QuarterlyReport,
        Some("160") => MajorShareholdersDocumentType::SemiAnnualReport,
        _ => MajorShareholdersDocumentType::Unknown,
    }
}

#[derive(Debug, Deserialize)]
struct RawCrossShareholdingEntry {
    #[serde(rename = "IsrName")]
    isr_name: String,
    #[serde(rename = "IsrCode")]
    isr_code: Option<String>,
    #[serde(rename = "CurShs")]
    cur_shs: Option<i64>,
    #[serde(rename = "PriShs")]
    pri_shs: Option<i64>,
    #[serde(rename = "CurBookVal")]
    cur_book_val: Option<i64>,
    #[serde(rename = "PriBookVal")]
    pri_book_val: Option<i64>,
    #[serde(rename = "IsrHoldsCode")]
    isr_holds_code: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawCrossShareholdingReportBlock {
    #[serde(rename = "Spec", default)]
    spec: Vec<RawCrossShareholdingEntry>,
    #[serde(rename = "Deem", default)]
    deem: Vec<RawCrossShareholdingEntry>,
}

#[derive(Debug, Deserialize)]
struct RawCrossShareholdingsDocument {
    #[serde(rename = "PerEn")]
    per_en: Option<NaiveDate>,
    #[serde(rename = "Report", default)]
    report: RawCrossShareholdingReportBlock,
}

fn mutual_holding(code: Option<&str>) -> MutualHolding {
    match code {
        Some("1") => MutualHolding::Held,
        Some("0") => MutualHolding::NotHeld,
        _ => MutualHolding::Unknown,
    }
}

fn cross_shareholding_dto(
    entry: RawCrossShareholdingEntry,
    category: CrossShareholdingCategory,
) -> CrossShareholdingDto {
    CrossShareholdingDto {
        issuer_name: entry.isr_name,
        issuer_code: entry.isr_code,
        category,
        current_shares: entry.cur_shs,
        previous_shares: entry.pri_shs,
        current_book_value: entry.cur_book_val,
        previous_book_value: entry.pri_book_val,
        mutual_holding: mutual_holding(entry.isr_holds_code.as_deref()),
    }
}

fn validate_symbol(symbol: &str) -> Result<(), McpError> {
    if symbol.len() == 4 && symbol.bytes().all(|b| b.is_ascii_digit()) {
        Ok(())
    } else {
        Err(invalid_params(format!(
            "symbol must be a 4-digit stock code, got {symbol:?}"
        )))
    }
}

impl StrategyServer {
    pub(crate) async fn read_shareholding_structure_inner(
        &self,
        _session_strategy_id: Uuid,
        params: ReadShareholdingStructureParams,
    ) -> Result<ReadShareholdingStructureResult, McpError> {
        validate_symbol(&params.symbol)?;
        let limit = clamp_limit(params.limit);

        let large_volume_rows = edinet_large_volume_shareholdings::Entity::find()
            .filter(edinet_large_volume_shareholdings::Column::Code.starts_with(&params.symbol))
            .order_by_desc(edinet_large_volume_shareholdings::Column::SubDate)
            .order_by_desc(edinet_large_volume_shareholdings::Column::DocId)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(db_error)?;

        let mut large_volume_reports = Vec::with_capacity(large_volume_rows.len());
        for row in large_volume_rows {
            let doc_id = row.doc_id;
            let doc: RawLargeVolumeDocument =
                serde_json::from_value(row.document).map_err(|e| {
                    internal_error(format!(
                        "malformed edinet_large_volume_shareholdings document {doc_id}: {e}"
                    ))
                })?;
            large_volume_reports.push(LargeVolumeReportDto {
                doc_id,
                submitted_on: row.sub_date,
                document_type: large_volume_document_type(doc.large_hldg_type_code.as_deref()),
                change_reason: doc.chg_rsn,
                total_shares_ratio: doc.total_shs_ratio,
                total_shares_ratio_last: doc.total_shs_ratio_last,
                holders: doc
                    .hldrs
                    .into_iter()
                    .map(|h| LargeVolumeHolderDto {
                        holder_name: h.hldr_name,
                        holding_purpose: h.hldg_purp,
                        shares_held: h.shs_held,
                        shares_ratio: h.shs_ratio,
                        shares_ratio_last: h.shs_ratio_last,
                    })
                    .collect(),
            });
        }

        let major_shareholders_row = edinet_major_shareholders::Entity::find()
            .filter(edinet_major_shareholders::Column::Code.starts_with(&params.symbol))
            .order_by_desc(edinet_major_shareholders::Column::SubDate)
            .order_by_desc(edinet_major_shareholders::Column::DocId)
            .one(&self.db)
            .await
            .map_err(db_error)?;
        let major_shareholders = major_shareholders_row
            .map(|row| -> Result<_, McpError> {
                let doc_id = row.doc_id;
                let doc: RawMajorShareholdersDocument = serde_json::from_value(row.document)
                    .map_err(|e| {
                        internal_error(format!(
                            "malformed edinet_major_shareholders document {doc_id}: {e}"
                        ))
                    })?;
                Ok(MajorShareholdersReportDto {
                    doc_id,
                    submitted_on: row.sub_date,
                    period_end: doc.per_en,
                    document_type: major_shareholders_document_type(doc.doc_type_code.as_deref()),
                    holders: doc
                        .hldrs
                        .into_iter()
                        .map(|h| MajorShareholderDto {
                            rank: h.rank,
                            holder_name: h.hldr_name,
                            shares_held: h.shs_held,
                            shares_ratio: h.shs_ratio,
                        })
                        .collect(),
                })
            })
            .transpose()?;

        let cross_shareholdings_row = edinet_cross_shareholdings::Entity::find()
            .filter(edinet_cross_shareholdings::Column::Code.starts_with(&params.symbol))
            .order_by_desc(edinet_cross_shareholdings::Column::SubDate)
            .order_by_desc(edinet_cross_shareholdings::Column::DocId)
            .one(&self.db)
            .await
            .map_err(db_error)?;
        let cross_shareholdings = cross_shareholdings_row
            .map(|row| -> Result<_, McpError> {
                let doc_id = row.doc_id;
                let doc: RawCrossShareholdingsDocument = serde_json::from_value(row.document)
                    .map_err(|e| {
                        internal_error(format!(
                            "malformed edinet_cross_shareholdings document {doc_id}: {e}"
                        ))
                    })?;
                let mut holdings =
                    Vec::with_capacity(doc.report.spec.len() + doc.report.deem.len());
                holdings.extend(
                    doc.report
                        .spec
                        .into_iter()
                        .map(|e| cross_shareholding_dto(e, CrossShareholdingCategory::Specified)),
                );
                holdings.extend(
                    doc.report
                        .deem
                        .into_iter()
                        .map(|e| cross_shareholding_dto(e, CrossShareholdingCategory::Deemed)),
                );
                Ok(CrossShareholdingsReportDto {
                    doc_id,
                    submitted_on: row.sub_date,
                    period_end: doc.per_en,
                    holdings,
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
    use sea_orm::DatabaseConnection;
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::{
        edinet_cross_shareholdings, edinet_large_volume_shareholdings, edinet_major_shareholders,
    };
    use crate::testing::create_test_db;

    use super::super::dto::{
        CrossShareholdingCategory, LargeVolumeDocumentType, MajorShareholdersDocumentType,
        MutualHolding, ReadShareholdingStructureParams, ReadShareholdingStructureResult,
    };
    use super::super::tests_common::build_server;
    use super::{
        large_volume_document_type, major_shareholders_document_type, mutual_holding,
        validate_symbol,
    };

    #[rstest]
    #[case::valid("7203", true)]
    #[case::too_short("720", false)]
    #[case::too_long("72030", false)]
    #[case::non_digit("72a3", false)]
    #[case::empty("", false)]
    fn validate_symbol_cases(#[case] symbol: &str, #[case] expected_ok: bool) {
        assert_eq!(validate_symbol(symbol).is_ok(), expected_ok);
    }

    #[rstest]
    #[case::report(Some("1"), LargeVolumeDocumentType::LargeVolumeReport)]
    #[case::amendment(Some("2"), LargeVolumeDocumentType::Amendment)]
    #[case::amendment_rapid_transfer(Some("3"), LargeVolumeDocumentType::AmendmentRapidTransfer)]
    #[case::report_special(Some("4"), LargeVolumeDocumentType::LargeVolumeReportSpecial)]
    #[case::amendment_special(Some("5"), LargeVolumeDocumentType::AmendmentSpecial)]
    #[case::explicit_unknown(Some("0"), LargeVolumeDocumentType::Unknown)]
    #[case::missing(None, LargeVolumeDocumentType::Unknown)]
    fn large_volume_document_type_cases(
        #[case] code: Option<&str>,
        #[case] expected: LargeVolumeDocumentType,
    ) {
        assert_eq!(large_volume_document_type(code), expected);
    }

    #[rstest]
    #[case::annual(Some("120"), MajorShareholdersDocumentType::AnnualReport)]
    #[case::quarterly(Some("140"), MajorShareholdersDocumentType::QuarterlyReport)]
    #[case::semi_annual(Some("160"), MajorShareholdersDocumentType::SemiAnnualReport)]
    #[case::missing(None, MajorShareholdersDocumentType::Unknown)]
    fn major_shareholders_document_type_cases(
        #[case] code: Option<&str>,
        #[case] expected: MajorShareholdersDocumentType,
    ) {
        assert_eq!(major_shareholders_document_type(code), expected);
    }

    #[rstest]
    #[case::held(Some("1"), MutualHolding::Held)]
    #[case::not_held(Some("0"), MutualHolding::NotHeld)]
    #[case::undeterminable(Some("2"), MutualHolding::Unknown)]
    #[case::missing(None, MutualHolding::Unknown)]
    fn mutual_holding_cases(#[case] code: Option<&str>, #[case] expected: MutualHolding) {
        assert_eq!(mutual_holding(code), expected);
    }

    async fn insert_large_volume(
        db: &DatabaseConnection,
        doc_id: &str,
        code: &str,
        sub_date: NaiveDate,
        document: Value,
    ) {
        edinet_large_volume_shareholdings::ActiveModel {
            doc_id: Set(doc_id.to_string()),
            code: Set(Some(code.to_string())),
            edinet_code: Set("E00001".to_string()),
            sub_date: Set(sub_date),
            document: Set(document),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert large volume shareholding");
    }

    async fn insert_major_shareholders(
        db: &DatabaseConnection,
        doc_id: &str,
        code: &str,
        sub_date: NaiveDate,
        document: Value,
    ) {
        edinet_major_shareholders::ActiveModel {
            doc_id: Set(doc_id.to_string()),
            code: Set(Some(code.to_string())),
            edinet_code: Set("E00001".to_string()),
            sub_date: Set(sub_date),
            document: Set(document),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert major shareholders");
    }

    async fn insert_cross_shareholdings(
        db: &DatabaseConnection,
        doc_id: &str,
        code: &str,
        sub_date: NaiveDate,
        document: Value,
    ) {
        edinet_cross_shareholdings::ActiveModel {
            doc_id: Set(doc_id.to_string()),
            code: Set(Some(code.to_string())),
            edinet_code: Set("E00001".to_string()),
            sub_date: Set(sub_date),
            document: Set(document),
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

    async fn read(db: &DatabaseConnection, symbol: &str) -> ReadShareholdingStructureResult {
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

    #[sqlx::test(migrations = false)]
    async fn returns_empty_and_null_sections_when_nothing_ingested(pool: PgPool) {
        let db = create_test_db(pool).await;

        let result = read(&db, "7203").await;

        assert_eq!(
            result,
            ReadShareholdingStructureResult {
                symbol: "7203".to_string(),
                large_volume_reports: vec![],
                major_shareholders: None,
                cross_shareholdings: None,
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn rejects_non_4_digit_symbol(pool: PgPool) {
        let db = create_test_db(pool).await;

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

    #[sqlx::test(migrations = false)]
    async fn large_volume_reports_match_by_first_4_chars_newest_first_with_holder_details(
        pool: PgPool,
    ) {
        let db = create_test_db(pool).await;
        insert_large_volume(
            &db,
            "S100OLD",
            "72030",
            ymd(2026, 1, 10),
            json!({
                "LargeHldgTypeCode": "1",
                "TotalShsRatio": 0.0621,
                "Hldrs": [
                    {"HldrName": "Alpha Capital", "HldgPurp": "純投資", "ShsHeld": 1000000, "ShsRatio": 0.0621},
                ],
            }),
        )
        .await;
        insert_large_volume(
            &db,
            "S100NEW",
            "72031",
            ymd(2026, 3, 1),
            json!({
                "LargeHldgTypeCode": "2",
                "ChgRsn": "株式の追加取得",
                "TotalShsRatio": 0.0801,
                "TotalShsRatioLast": 0.0621,
                "Hldrs": [
                    {
                        "HldrName": "Alpha Capital",
                        "HldgPurp": "純投資",
                        "ShsHeld": 1300000,
                        "ShsRatio": 0.0801,
                        "ShsRatioLast": 0.0621,
                    },
                ],
            }),
        )
        .await;
        // 別銘柄 (先頭 4 文字が一致しない) は対象外
        insert_large_volume(
            &db,
            "S100OTHER",
            "99840",
            ymd(2026, 3, 2),
            json!({"LargeHldgTypeCode": "1", "Hldrs": []}),
        )
        .await;

        let result = read(&db, "7203").await;

        assert_eq!(
            result.large_volume_reports,
            vec![
                super::super::dto::LargeVolumeReportDto {
                    doc_id: "S100NEW".to_string(),
                    submitted_on: ymd(2026, 3, 1),
                    document_type: LargeVolumeDocumentType::Amendment,
                    change_reason: Some("株式の追加取得".to_string()),
                    total_shares_ratio: Some(0.0801),
                    total_shares_ratio_last: Some(0.0621),
                    holders: vec![super::super::dto::LargeVolumeHolderDto {
                        holder_name: "Alpha Capital".to_string(),
                        holding_purpose: Some("純投資".to_string()),
                        shares_held: Some(1300000),
                        shares_ratio: Some(0.0801),
                        shares_ratio_last: Some(0.0621),
                    }],
                },
                super::super::dto::LargeVolumeReportDto {
                    doc_id: "S100OLD".to_string(),
                    submitted_on: ymd(2026, 1, 10),
                    document_type: LargeVolumeDocumentType::LargeVolumeReport,
                    change_reason: None,
                    total_shares_ratio: Some(0.0621),
                    total_shares_ratio_last: None,
                    holders: vec![super::super::dto::LargeVolumeHolderDto {
                        holder_name: "Alpha Capital".to_string(),
                        holding_purpose: Some("純投資".to_string()),
                        shares_held: Some(1000000),
                        shares_ratio: Some(0.0621),
                        shares_ratio_last: None,
                    }],
                },
            ],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn large_volume_reports_respects_limit_after_ordering(pool: PgPool) {
        let db = create_test_db(pool).await;
        for (i, day) in [1u32, 2, 3].into_iter().enumerate() {
            insert_large_volume(
                &db,
                &format!("S100{i}"),
                "72030",
                ymd(2026, 1, day),
                json!({"LargeHldgTypeCode": "1", "Hldrs": []}),
            )
            .await;
        }

        let result = build_server(db.clone())
            .read_shareholding_structure_inner(
                Uuid::new_v4(),
                ReadShareholdingStructureParams {
                    symbol: "7203".to_string(),
                    limit: Some(2),
                },
            )
            .await
            .expect("read_shareholding_structure");

        assert_eq!(
            result
                .large_volume_reports
                .iter()
                .map(|r| r.doc_id.clone())
                .collect::<Vec<_>>(),
            vec!["S1002".to_string(), "S1001".to_string()],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn major_shareholders_returns_only_the_latest_filing(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_major_shareholders(
            &db,
            "S100OLD",
            "72030",
            ymd(2025, 6, 30),
            json!({
                "PerEn": "2025-03-31",
                "DocTypeCode": "120",
                "Hldrs": [{"Rank": 1, "HldrName": "旧筆頭株主", "ShsHeld": 5000000, "ShsRatio": 0.15}],
            }),
        )
        .await;
        insert_major_shareholders(
            &db,
            "S100NEW",
            "72030",
            ymd(2026, 6, 30),
            json!({
                "PerEn": "2026-03-31",
                "DocTypeCode": "120",
                "Hldrs": [
                    {"Rank": 1, "HldrName": "新筆頭株主", "ShsHeld": 6000000, "ShsRatio": 0.18},
                    {"Rank": 2, "HldrName": "第二位株主", "ShsHeld": 3000000, "ShsRatio": 0.09},
                ],
            }),
        )
        .await;

        let result = read(&db, "7203").await;

        assert_eq!(
            result.major_shareholders,
            Some(super::super::dto::MajorShareholdersReportDto {
                doc_id: "S100NEW".to_string(),
                submitted_on: ymd(2026, 6, 30),
                period_end: Some(ymd(2026, 3, 31)),
                document_type: MajorShareholdersDocumentType::AnnualReport,
                holders: vec![
                    super::super::dto::MajorShareholderDto {
                        rank: Some(1),
                        holder_name: "新筆頭株主".to_string(),
                        shares_held: Some(6000000),
                        shares_ratio: Some(0.18),
                    },
                    super::super::dto::MajorShareholderDto {
                        rank: Some(2),
                        holder_name: "第二位株主".to_string(),
                        shares_held: Some(3000000),
                        shares_ratio: Some(0.09),
                    },
                ],
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn cross_shareholdings_combines_spec_and_deem_with_mutual_holding(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_cross_shareholdings(
            &db,
            "S100XYZ",
            "72030",
            ymd(2026, 6, 30),
            json!({
                "PerEn": "2026-03-31",
                "Report": {
                    "Spec": [
                        {
                            "IsrName": "取引先商事",
                            "IsrCode": "13010",
                            "CurShs": 200000,
                            "PriShs": 180000,
                            "CurBookVal": 500000000,
                            "PriBookVal": 420000000,
                            "IsrHoldsCode": "1",
                        },
                    ],
                    "Deem": [
                        {
                            "IsrName": "年金信託先",
                            "IsrCode": null,
                            "CurShs": 50000,
                            "PriShs": null,
                            "CurBookVal": 90000000,
                            "PriBookVal": null,
                            "IsrHoldsCode": "2",
                        },
                    ],
                },
            }),
        )
        .await;

        let result = read(&db, "7203").await;

        assert_eq!(
            result.cross_shareholdings,
            Some(super::super::dto::CrossShareholdingsReportDto {
                doc_id: "S100XYZ".to_string(),
                submitted_on: ymd(2026, 6, 30),
                period_end: Some(ymd(2026, 3, 31)),
                holdings: vec![
                    super::super::dto::CrossShareholdingDto {
                        issuer_name: "取引先商事".to_string(),
                        issuer_code: Some("13010".to_string()),
                        category: CrossShareholdingCategory::Specified,
                        current_shares: Some(200000),
                        previous_shares: Some(180000),
                        current_book_value: Some(500000000),
                        previous_book_value: Some(420000000),
                        mutual_holding: MutualHolding::Held,
                    },
                    super::super::dto::CrossShareholdingDto {
                        issuer_name: "年金信託先".to_string(),
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
        );
    }
}
