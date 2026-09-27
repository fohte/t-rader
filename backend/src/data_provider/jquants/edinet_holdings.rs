use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::holdings::{
    CrossShareholding, CrossShareholdingCategory, CrossShareholdingContent,
    CrossShareholdingDocument, LargeVolumeHolder, LargeVolumeReportType,
    LargeVolumeShareholdingContent, LargeVolumeShareholdingDocument, MajorShareholder,
    MajorShareholderContent, MajorShareholderDocument, MajorShareholderReportType, MutualHolding,
    ShareholdingDocumentMetadata,
};
use serde::Deserialize;
use serde_json::Value;

use super::JQuantsClient;
use crate::data_provider::{
    DateRange, ShareholdingStructureSource, ShareholdingStructureSourceError,
};

const LARGE_VOLUME_PATH: &str = "/edinet/large-volume-shareholders";
const CROSS_SHAREHOLDING_PATH: &str = "/edinet/cross-shareholdings";
const MAJOR_SHAREHOLDER_PATH: &str = "/edinet/major-shareholders";

#[async_trait]
impl ShareholdingStructureSource for JQuantsClient {
    async fn fetch_large_volume_documents(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<LargeVolumeShareholdingDocument>, ShareholdingStructureSourceError> {
        let docs = self.fetch_edinet_documents(LARGE_VOLUME_PATH, date).await?;
        Ok(docs.into_iter().filter_map(large_volume_document).collect())
    }

    async fn fetch_major_shareholder_documents(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<MajorShareholderDocument>, ShareholdingStructureSourceError> {
        let docs = self
            .fetch_edinet_documents(MAJOR_SHAREHOLDER_PATH, date)
            .await?;
        Ok(docs
            .into_iter()
            .filter_map(major_shareholder_document)
            .collect())
    }

    async fn fetch_cross_shareholding_documents(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<CrossShareholdingDocument>, ShareholdingStructureSourceError> {
        let docs = self
            .fetch_edinet_documents(CROSS_SHAREHOLDING_PATH, date)
            .await?;
        Ok(docs
            .into_iter()
            .filter_map(cross_shareholding_document)
            .collect())
    }

    fn fetchable_range(&self, today: NaiveDate) -> Option<DateRange> {
        Some(self.plan_date_range(today))
    }
}

#[derive(Debug, Deserialize)]
struct LargeVolumeDocumentRaw {
    #[serde(rename = "LargeHldgTypeCode")]
    report_type: Option<String>,
    #[serde(rename = "ChgRsn")]
    change_reason: Option<String>,
    #[serde(rename = "TotalShsRatio")]
    total_shares_ratio: Option<f64>,
    #[serde(rename = "TotalShsRatioLast")]
    previous_total_shares_ratio: Option<f64>,
    #[serde(rename = "Hldrs", default)]
    holders: Vec<LargeVolumeHolderRaw>,
}

#[derive(Debug, Deserialize)]
struct LargeVolumeHolderRaw {
    #[serde(rename = "HldrName")]
    name: String,
    #[serde(rename = "HldgPurp")]
    holding_purpose: Option<String>,
    #[serde(rename = "ShsHeld")]
    shares_held: Option<i64>,
    #[serde(rename = "ShsRatio")]
    shares_ratio: Option<f64>,
    #[serde(rename = "ShsRatioLast")]
    previous_shares_ratio: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct MajorShareholderDocumentRaw {
    #[serde(rename = "PerEn")]
    period_end: Option<NaiveDate>,
    #[serde(rename = "DocTypeCode")]
    report_type: Option<String>,
    #[serde(rename = "Hldrs", default)]
    holders: Vec<MajorShareholderRaw>,
}

#[derive(Debug, Deserialize)]
struct MajorShareholderRaw {
    #[serde(rename = "Rank")]
    rank: Option<i32>,
    #[serde(rename = "HldrName")]
    name: String,
    #[serde(rename = "ShsHeld")]
    shares_held: Option<i64>,
    #[serde(rename = "ShsRatio")]
    shares_ratio: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
struct CrossShareholdingReportRaw {
    #[serde(rename = "Spec", default)]
    specified: Vec<CrossShareholdingRaw>,
    #[serde(rename = "Deem", default)]
    deemed: Vec<CrossShareholdingRaw>,
}

#[derive(Debug, Deserialize)]
struct CrossShareholdingDocumentRaw {
    #[serde(rename = "PerEn")]
    period_end: Option<NaiveDate>,
    #[serde(rename = "Report", default)]
    report: CrossShareholdingReportRaw,
}

#[derive(Debug, Deserialize)]
struct CrossShareholdingRaw {
    #[serde(rename = "IsrName")]
    issuer_name: String,
    #[serde(rename = "IsrCode")]
    issuer_stock_code: Option<String>,
    #[serde(rename = "CurShs")]
    current_shares: Option<i64>,
    #[serde(rename = "PriShs")]
    previous_shares: Option<i64>,
    #[serde(rename = "CurBookVal")]
    current_book_value: Option<i64>,
    #[serde(rename = "PriBookVal")]
    previous_book_value: Option<i64>,
    #[serde(rename = "IsrHoldsCode")]
    mutual_holding_code: Option<String>,
}

fn document_metadata(doc: &Value, endpoint: &'static str) -> Option<ShareholdingDocumentMetadata> {
    let required_string = |key: &str| doc.get(key).and_then(Value::as_str);
    let document_id = required_string("DocId");
    let filer_code = required_string("EdinetCode");
    let raw_submitted_on = required_string("SubDate");
    let (Some(document_id), Some(filer_code), Some(raw_submitted_on)) =
        (document_id, filer_code, raw_submitted_on)
    else {
        tracing::warn!(
            endpoint,
            document_id = doc.get("DocId").and_then(|value| value.as_str()),
            has_filer_code = filer_code.is_some(),
            has_submitted_on = raw_submitted_on.is_some(),
            "書類の識別情報が不足しているためスキップします"
        );
        return None;
    };
    let submitted_on = match NaiveDate::parse_from_str(raw_submitted_on, "%Y-%m-%d") {
        Ok(date) => date,
        Err(error) => {
            tracing::warn!(
                endpoint,
                document_id,
                submitted_on = raw_submitted_on,
                %error,
                "提出日の形式が不正なためスキップします"
            );
            return None;
        }
    };

    Some(ShareholdingDocumentMetadata {
        document_id: document_id.to_string(),
        stock_code: doc.get("Code").and_then(Value::as_str).map(str::to_string),
        filer_code: filer_code.to_string(),
        submitted_on,
    })
}

fn parse_content<T: for<'de> Deserialize<'de>>(
    doc: Value,
    endpoint: &'static str,
    document_id: &str,
) -> Option<T> {
    serde_json::from_value(doc)
        .map_err(|error| {
            tracing::warn!(endpoint, document_id, %error, "書類本文を読み取れないため本文を保存できません");
        })
        .ok()
}

fn large_volume_document(doc: Value) -> Option<LargeVolumeShareholdingDocument> {
    let metadata = document_metadata(&doc, LARGE_VOLUME_PATH)?;
    let raw: Option<LargeVolumeDocumentRaw> =
        parse_content(doc, LARGE_VOLUME_PATH, &metadata.document_id);
    let content = raw.map(|raw| LargeVolumeShareholdingContent {
        report_type: large_volume_report_type(raw.report_type.as_deref()),
        change_reason: raw.change_reason,
        total_shares_ratio: raw.total_shares_ratio,
        previous_total_shares_ratio: raw.previous_total_shares_ratio,
        holders: raw
            .holders
            .into_iter()
            .map(|holder| LargeVolumeHolder {
                name: holder.name,
                holding_purpose: holder.holding_purpose,
                shares_held: holder.shares_held,
                shares_ratio: holder.shares_ratio,
                previous_shares_ratio: holder.previous_shares_ratio,
            })
            .collect(),
    });
    Some(LargeVolumeShareholdingDocument { metadata, content })
}

fn large_volume_report_type(code: Option<&str>) -> LargeVolumeReportType {
    match code {
        Some("1") => LargeVolumeReportType::Report,
        Some("2") => LargeVolumeReportType::Amendment,
        Some("3") => LargeVolumeReportType::AmendmentRapidTransfer,
        Some("4") => LargeVolumeReportType::ReportSpecial,
        Some("5") => LargeVolumeReportType::AmendmentSpecial,
        _ => LargeVolumeReportType::Unknown,
    }
}

fn major_shareholder_document(doc: Value) -> Option<MajorShareholderDocument> {
    let metadata = document_metadata(&doc, MAJOR_SHAREHOLDER_PATH)?;
    let raw: Option<MajorShareholderDocumentRaw> =
        parse_content(doc, MAJOR_SHAREHOLDER_PATH, &metadata.document_id);
    let content = raw.map(|raw| MajorShareholderContent {
        period_end: raw.period_end,
        report_type: major_shareholder_report_type(raw.report_type.as_deref()),
        holders: raw
            .holders
            .into_iter()
            .map(|holder| MajorShareholder {
                rank: holder.rank,
                name: holder.name,
                shares_held: holder.shares_held,
                shares_ratio: holder.shares_ratio,
            })
            .collect(),
    });
    Some(MajorShareholderDocument { metadata, content })
}

fn major_shareholder_report_type(code: Option<&str>) -> MajorShareholderReportType {
    match code {
        Some("120") => MajorShareholderReportType::Annual,
        Some("140") => MajorShareholderReportType::Quarterly,
        Some("160") => MajorShareholderReportType::SemiAnnual,
        _ => MajorShareholderReportType::Unknown,
    }
}

fn cross_shareholding_document(doc: Value) -> Option<CrossShareholdingDocument> {
    let metadata = document_metadata(&doc, CROSS_SHAREHOLDING_PATH)?;
    let raw: Option<CrossShareholdingDocumentRaw> =
        parse_content(doc, CROSS_SHAREHOLDING_PATH, &metadata.document_id);
    let content = raw.map(|raw| {
        let holdings = raw
            .report
            .specified
            .into_iter()
            .map(|holding| (holding, CrossShareholdingCategory::Specified))
            .chain(
                raw.report
                    .deemed
                    .into_iter()
                    .map(|holding| (holding, CrossShareholdingCategory::Deemed)),
            )
            .map(|(holding, category)| CrossShareholding {
                issuer_name: holding.issuer_name,
                issuer_stock_code: holding.issuer_stock_code,
                category,
                current_shares: holding.current_shares,
                previous_shares: holding.previous_shares,
                current_book_value: holding.current_book_value,
                previous_book_value: holding.previous_book_value,
                mutual_holding: mutual_holding(holding.mutual_holding_code.as_deref()),
            })
            .collect();
        CrossShareholdingContent {
            period_end: raw.period_end,
            holdings,
        }
    });

    Some(CrossShareholdingDocument { metadata, content })
}

fn mutual_holding(code: Option<&str>) -> MutualHolding {
    match code {
        Some("1") => MutualHolding::Held,
        Some("0") => MutualHolding::NotHeld,
        _ => MutualHolding::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_domain::holdings::{
        CrossShareholding, CrossShareholdingCategory, CrossShareholdingContent,
        CrossShareholdingDocument, LargeVolumeHolder, LargeVolumeReportType,
        LargeVolumeShareholdingContent, LargeVolumeShareholdingDocument, MajorShareholder,
        MajorShareholderContent, MajorShareholderDocument, MajorShareholderReportType,
        MutualHolding, ShareholdingDocumentMetadata,
    };
    use rstest::rstest;
    use serde_json::json;

    use crate::data_provider::{ShareholdingStructureSource, jquants::mock::JQuantsMockServer};
    use crate::models::jquants_plan::JQuantsPlan;

    use super::{
        CROSS_SHAREHOLDING_PATH, LARGE_VOLUME_PATH, MAJOR_SHAREHOLDER_PATH,
        cross_shareholding_document, document_metadata, large_volume_document,
        large_volume_report_type, major_shareholder_document, major_shareholder_report_type,
        mutual_holding,
    };

    fn submitted_document(fields: serde_json::Value) -> serde_json::Value {
        let mut document = json!({
            "DocId": "S100EXAMPLE",
            "Code": "99990",
            "EdinetCode": "E99999",
            "SubDate": "2025-04-01",
        });
        document
            .as_object_mut()
            .expect("document is an object")
            .extend(fields.as_object().expect("fields are an object").clone());
        document
    }

    fn expected_metadata() -> ShareholdingDocumentMetadata {
        ShareholdingDocumentMetadata {
            document_id: "SAMPLE-DOC".to_string(),
            stock_code: Some("99990".to_string()),
            filer_code: "E99999".to_string(),
            submitted_on: NaiveDate::from_ymd_opt(2025, 4, 1).expect("valid date"),
        }
    }

    #[test]
    fn converts_large_volume_document_into_domain_fields() {
        let document = submitted_document(json!({
            "DocId": "SAMPLE-DOC",
            "LargeHldgTypeCode": "3",
            "TotalShsRatio": 0.12,
            "Hldrs": [{
                "HldrName": "Example Holder",
                "HldgPurp": "investment",
                "ShsHeld": 1200,
                "ShsRatio": 0.12,
                "ShsRatioLast": 0.1
            }]
        }));
        let converted = large_volume_document(document);

        assert_eq!(
            converted,
            Some(LargeVolumeShareholdingDocument {
                metadata: expected_metadata(),
                content: Some(LargeVolumeShareholdingContent {
                    report_type: LargeVolumeReportType::AmendmentRapidTransfer,
                    change_reason: None,
                    total_shares_ratio: Some(0.12),
                    previous_total_shares_ratio: None,
                    holders: vec![LargeVolumeHolder {
                        name: "Example Holder".to_string(),
                        holding_purpose: Some("investment".to_string()),
                        shares_held: Some(1200),
                        shares_ratio: Some(0.12),
                        previous_shares_ratio: Some(0.1),
                    }],
                }),
            })
        );
    }

    #[test]
    fn converts_major_shareholder_document() {
        let document = submitted_document(json!({
            "DocId": "SAMPLE-DOC",
            "DocTypeCode": "140",
            "PerEn": "2025-03-31",
            "Hldrs": [{
                "Rank": 1,
                "HldrName": "Example Holder",
                "ShsHeld": 1200,
                "ShsRatio": 0.12
            }]
        }));

        assert_eq!(
            major_shareholder_document(document),
            Some(MajorShareholderDocument {
                metadata: expected_metadata(),
                content: Some(MajorShareholderContent {
                    period_end: Some(NaiveDate::from_ymd_opt(2025, 3, 31).expect("valid date")),
                    report_type: MajorShareholderReportType::Quarterly,
                    holders: vec![MajorShareholder {
                        rank: Some(1),
                        name: "Example Holder".to_string(),
                        shares_held: Some(1200),
                        shares_ratio: Some(0.12),
                    }],
                }),
            })
        );
    }

    #[test]
    fn cross_shareholding_document_preserves_category_and_holding_status() {
        let document = submitted_document(json!({
            "DocId": "SAMPLE-DOC",
            "PerEn": "2025-03-31",
            "Report": {
                "Spec": [{
                    "IsrName": "Example Issuer",
                    "IsrCode": "88880",
                    "IsrHoldsCode": "1"
                }],
                "Deem": [{
                    "IsrName": "Example Custodian",
                    "IsrHoldsCode": "0"
                }]
            }
        }));

        assert_eq!(
            cross_shareholding_document(document),
            Some(CrossShareholdingDocument {
                metadata: expected_metadata(),
                content: Some(CrossShareholdingContent {
                    period_end: Some(NaiveDate::from_ymd_opt(2025, 3, 31).expect("valid date")),
                    holdings: vec![
                        CrossShareholding {
                            issuer_name: "Example Issuer".to_string(),
                            issuer_stock_code: Some("88880".to_string()),
                            category: CrossShareholdingCategory::Specified,
                            current_shares: None,
                            previous_shares: None,
                            current_book_value: None,
                            previous_book_value: None,
                            mutual_holding: MutualHolding::Held,
                        },
                        CrossShareholding {
                            issuer_name: "Example Custodian".to_string(),
                            issuer_stock_code: None,
                            category: CrossShareholdingCategory::Deemed,
                            current_shares: None,
                            previous_shares: None,
                            current_book_value: None,
                            previous_book_value: None,
                            mutual_holding: MutualHolding::NotHeld,
                        },
                    ],
                }),
            })
        );
    }

    #[rstest]
    #[case::report(Some("1"), LargeVolumeReportType::Report)]
    #[case::amendment(Some("2"), LargeVolumeReportType::Amendment)]
    #[case::rapid_transfer(Some("3"), LargeVolumeReportType::AmendmentRapidTransfer)]
    #[case::special_report(Some("4"), LargeVolumeReportType::ReportSpecial)]
    #[case::special_amendment(Some("5"), LargeVolumeReportType::AmendmentSpecial)]
    #[case::unknown(Some("0"), LargeVolumeReportType::Unknown)]
    #[case::missing(None, LargeVolumeReportType::Unknown)]
    fn maps_large_volume_report_type(
        #[case] code: Option<&str>,
        #[case] expected: LargeVolumeReportType,
    ) {
        assert_eq!(large_volume_report_type(code), expected);
    }

    #[rstest]
    #[case::annual(Some("120"), MajorShareholderReportType::Annual)]
    #[case::quarterly(Some("140"), MajorShareholderReportType::Quarterly)]
    #[case::semi_annual(Some("160"), MajorShareholderReportType::SemiAnnual)]
    #[case::unknown(Some("0"), MajorShareholderReportType::Unknown)]
    #[case::missing(None, MajorShareholderReportType::Unknown)]
    fn maps_major_shareholder_report_type(
        #[case] code: Option<&str>,
        #[case] expected: MajorShareholderReportType,
    ) {
        assert_eq!(major_shareholder_report_type(code), expected);
    }

    #[rstest]
    #[case::held(Some("1"), MutualHolding::Held)]
    #[case::not_held(Some("0"), MutualHolding::NotHeld)]
    #[case::undeterminable(Some("2"), MutualHolding::Unknown)]
    #[case::missing(None, MutualHolding::Unknown)]
    fn maps_mutual_holding_code(#[case] code: Option<&str>, #[case] expected: MutualHolding) {
        assert_eq!(mutual_holding(code), expected);
    }

    #[rstest]
    #[case::missing_document_id(json!({"EdinetCode": "E99999", "SubDate": "2025-04-01"}), LARGE_VOLUME_PATH)]
    #[case::missing_filer_code(json!({"DocId": "SAMPLE-DOC", "SubDate": "2025-04-01"}), MAJOR_SHAREHOLDER_PATH)]
    #[case::missing_submitted_on(json!({"DocId": "SAMPLE-DOC", "EdinetCode": "E99999"}), CROSS_SHAREHOLDING_PATH)]
    #[case::invalid_submitted_on(json!({"DocId": "SAMPLE-DOC", "EdinetCode": "E99999", "SubDate": "20250401"}), LARGE_VOLUME_PATH)]
    fn skips_documents_without_valid_metadata(
        #[case] document: serde_json::Value,
        #[case] endpoint: &'static str,
    ) {
        assert_eq!(document_metadata(&document, endpoint), None);
    }

    #[test]
    fn preserves_metadata_when_content_cannot_be_parsed() {
        let document = submitted_document(json!({
            "DocId": "SAMPLE-DOC",
            "Hldrs": [{ "ShsHeld": 1200 }]
        }));

        assert_eq!(
            large_volume_document(document),
            Some(LargeVolumeShareholdingDocument {
                metadata: expected_metadata(),
                content: None,
            })
        );
    }

    #[test]
    fn missing_stock_code_is_kept_as_none() {
        let document = json!({
            "DocId": "SAMPLE-DOC",
            "EdinetCode": "E99999",
            "SubDate": "2025-04-01",
            "Hldrs": []
        });

        assert_eq!(
            large_volume_document(document),
            Some(LargeVolumeShareholdingDocument {
                metadata: ShareholdingDocumentMetadata {
                    stock_code: None,
                    ..expected_metadata()
                },
                content: Some(LargeVolumeShareholdingContent {
                    report_type: LargeVolumeReportType::Unknown,
                    change_reason: None,
                    total_shares_ratio: None,
                    previous_total_shares_ratio: None,
                    holders: vec![],
                }),
            })
        );
    }

    #[tokio::test]
    async fn fetchable_range_uses_configured_plan() {
        let mock = JQuantsMockServer::start().await;
        let client = mock
            .client_with_plan(JQuantsPlan::Standard)
            .expect("client");
        let today = NaiveDate::from_ymd_opt(2025, 4, 1).expect("valid date");
        let (from, to) = JQuantsPlan::Standard.range(today);

        assert_eq!(
            client.fetchable_range(today),
            Some(crate::data_provider::DateRange { from, to })
        );
    }
}
