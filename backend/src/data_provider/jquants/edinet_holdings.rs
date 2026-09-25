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

use super::{JQuantsClient, effective_range};
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
        let (from, to) = match self.manual_plan() {
            Some(plan) => plan.range(today),
            None => {
                let guard = self
                    .detected_range
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                effective_range(guard.as_ref(), today)?
            }
        };
        Some(DateRange { from, to })
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

fn document_metadata(doc: &Value) -> Option<ShareholdingDocumentMetadata> {
    let required_string = |key: &str| doc.get(key).and_then(Value::as_str);
    let (Some(document_id), Some(filer_code), Some(submitted_on)) = (
        required_string("DocId"),
        required_string("EdinetCode"),
        required_string("SubDate"),
    ) else {
        tracing::warn!("書類の識別情報が不足しているためスキップします");
        return None;
    };
    let submitted_on = match NaiveDate::parse_from_str(submitted_on, "%Y-%m-%d") {
        Ok(date) => date,
        Err(error) => {
            tracing::warn!(document_id, %submitted_on, %error, "提出日の形式が不正なためスキップします");
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

fn parse_content<T: for<'de> Deserialize<'de>>(doc: Value, document_id: &str) -> Option<T> {
    serde_json::from_value(doc)
        .map_err(|error| {
            tracing::warn!(document_id, %error, "書類本文を読み取れないためスキップします");
        })
        .ok()
}

fn large_volume_document(doc: Value) -> Option<LargeVolumeShareholdingDocument> {
    let metadata = document_metadata(&doc)?;
    let raw: LargeVolumeDocumentRaw = parse_content(doc, &metadata.document_id)?;
    Some(LargeVolumeShareholdingDocument {
        metadata,
        content: LargeVolumeShareholdingContent {
            report_type: match raw.report_type.as_deref() {
                Some("1") => LargeVolumeReportType::Report,
                Some("2") => LargeVolumeReportType::Amendment,
                Some("3") => LargeVolumeReportType::AmendmentRapidTransfer,
                Some("4") => LargeVolumeReportType::ReportSpecial,
                Some("5") => LargeVolumeReportType::AmendmentSpecial,
                _ => LargeVolumeReportType::Unknown,
            },
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
        },
    })
}

fn major_shareholder_document(doc: Value) -> Option<MajorShareholderDocument> {
    let metadata = document_metadata(&doc)?;
    let raw: MajorShareholderDocumentRaw = parse_content(doc, &metadata.document_id)?;
    Some(MajorShareholderDocument {
        metadata,
        content: MajorShareholderContent {
            period_end: raw.period_end,
            report_type: match raw.report_type.as_deref() {
                Some("120") => MajorShareholderReportType::Annual,
                Some("140") => MajorShareholderReportType::Quarterly,
                Some("160") => MajorShareholderReportType::SemiAnnual,
                _ => MajorShareholderReportType::Unknown,
            },
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
        },
    })
}

fn cross_shareholding_document(doc: Value) -> Option<CrossShareholdingDocument> {
    let metadata = document_metadata(&doc)?;
    let raw: CrossShareholdingDocumentRaw = parse_content(doc, &metadata.document_id)?;
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
            mutual_holding: match holding.mutual_holding_code.as_deref() {
                Some("1") => MutualHolding::Held,
                Some("0") => MutualHolding::NotHeld,
                _ => MutualHolding::Unknown,
            },
        })
        .collect();

    Some(CrossShareholdingDocument {
        metadata,
        content: CrossShareholdingContent {
            period_end: raw.period_end,
            holdings,
        },
    })
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use serde_json::json;

    use super::{cross_shareholding_document, large_volume_document, major_shareholder_document};
    use core_domain::holdings::{
        CrossShareholdingCategory, LargeVolumeReportType, MajorShareholderReportType, MutualHolding,
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

    #[test]
    fn converts_large_volume_document_into_domain_fields() {
        let converted = large_volume_document(submitted_document(json!({
            "LargeHldgTypeCode": "3",
            "TotalShsRatio": 0.12,
            "Hldrs": [{
                "HldrName": "Example Holder",
                "HldgPurp": "investment",
                "ShsHeld": 1200,
                "ShsRatio": 0.12,
                "ShsRatioLast": 0.1
            }]
        })));

        assert_eq!(
            converted,
            Some(core_domain::holdings::LargeVolumeShareholdingDocument {
                metadata: core_domain::holdings::ShareholdingDocumentMetadata {
                    document_id: "S100EXAMPLE".to_string(),
                    stock_code: Some("99990".to_string()),
                    filer_code: "E99999".to_string(),
                    submitted_on: NaiveDate::from_ymd_opt(2025, 4, 1).expect("valid date"),
                },
                content: core_domain::holdings::LargeVolumeShareholdingContent {
                    report_type: LargeVolumeReportType::AmendmentRapidTransfer,
                    change_reason: None,
                    total_shares_ratio: Some(0.12),
                    previous_total_shares_ratio: None,
                    holders: vec![core_domain::holdings::LargeVolumeHolder {
                        name: "Example Holder".to_string(),
                        holding_purpose: Some("investment".to_string()),
                        shares_held: Some(1200),
                        shares_ratio: Some(0.12),
                        previous_shares_ratio: Some(0.1),
                    }],
                },
            })
        );
    }

    #[test]
    fn converts_document_code_values_and_cross_shareholding_categories() {
        let major = major_shareholder_document(submitted_document(json!({
            "DocTypeCode": "140",
            "Hldrs": []
        })));
        let cross = cross_shareholding_document(submitted_document(json!({
            "Report": {
                "Spec": [{
                    "IsrName": "Example Issuer",
                    "IsrHoldsCode": "1"
                }],
                "Deem": [{
                    "IsrName": "Example Custodian",
                    "IsrHoldsCode": "0"
                }]
            }
        })));

        assert_eq!(
            (major.map(|doc| doc.content.report_type), cross),
            (
                Some(MajorShareholderReportType::Quarterly),
                Some(core_domain::holdings::CrossShareholdingDocument {
                    metadata: core_domain::holdings::ShareholdingDocumentMetadata {
                        document_id: "S100EXAMPLE".to_string(),
                        stock_code: Some("99990".to_string()),
                        filer_code: "E99999".to_string(),
                        submitted_on: NaiveDate::from_ymd_opt(2025, 4, 1).expect("valid date"),
                    },
                    content: core_domain::holdings::CrossShareholdingContent {
                        period_end: None,
                        holdings: vec![
                            core_domain::holdings::CrossShareholding {
                                issuer_name: "Example Issuer".to_string(),
                                issuer_stock_code: None,
                                category: CrossShareholdingCategory::Specified,
                                current_shares: None,
                                previous_shares: None,
                                current_book_value: None,
                                previous_book_value: None,
                                mutual_holding: MutualHolding::Held,
                            },
                            core_domain::holdings::CrossShareholding {
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
                    },
                }),
            )
        );
    }
}
