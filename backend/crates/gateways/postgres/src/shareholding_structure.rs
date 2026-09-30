use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use core_application::shareholding_structure::{
    ShareholdingStructureBySymbol, ShareholdingStructureRepository,
    ShareholdingStructureRepositoryError,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use core_domain::holdings::{
    CrossShareholdingDocument, LargeVolumeShareholdingContent, LargeVolumeShareholdingDocument,
    MajorShareholderContent, MajorShareholderDocument, ShareholdingDocumentMetadata,
};
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::DatabaseHandle;
use crate::entities::{
    cross_shareholding_documents, large_volume_shareholding_documents, major_shareholder_documents,
};
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

#[derive(Clone)]
pub struct PostgresShareholdingStructureRepository {
    db: DatabaseHandle,
}

impl PostgresShareholdingStructureRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ShareholdingStructureRepository for PostgresShareholdingStructureRepository {
    async fn latest_large_volume_submitted_on(
        &self,
    ) -> Result<Option<NaiveDate>, ShareholdingStructureRepositoryError> {
        latest_submitted_on::<large_volume_shareholding_documents::Entity>(
            &self.db,
            large_volume_shareholding_documents::Column::SubmittedOn,
            |model| model.submitted_on,
        )
        .await
        .map_err(repository_error)
    }

    async fn upsert_large_volume(
        &self,
        transaction: &UnitOfWorkTransaction,
        documents: Vec<LargeVolumeShareholdingDocument>,
    ) -> Result<usize, ShareholdingStructureRepositoryError> {
        upsert_documents::<large_volume_shareholding_documents::Entity, _, _>(
            transaction,
            documents,
            |document| {
                Ok(large_volume_shareholding_documents::ActiveModel {
                    document_id: Set(document.metadata.document_id),
                    stock_code: Set(document.metadata.stock_code),
                    filer_code: Set(document.metadata.filer_code),
                    submitted_on: Set(document.metadata.submitted_on),
                    details: Set(serialize_details(document.content)?),
                    created_at: NotSet,
                    updated_at: Set(Utc::now().fixed_offset()),
                })
            },
            OnConflict::column(large_volume_shareholding_documents::Column::DocumentId)
                .update_columns([
                    large_volume_shareholding_documents::Column::StockCode,
                    large_volume_shareholding_documents::Column::FilerCode,
                    large_volume_shareholding_documents::Column::SubmittedOn,
                    large_volume_shareholding_documents::Column::Details,
                    large_volume_shareholding_documents::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .await
    }

    async fn latest_major_shareholder_submitted_on(
        &self,
    ) -> Result<Option<NaiveDate>, ShareholdingStructureRepositoryError> {
        latest_submitted_on::<major_shareholder_documents::Entity>(
            &self.db,
            major_shareholder_documents::Column::SubmittedOn,
            |model| model.submitted_on,
        )
        .await
        .map_err(repository_error)
    }

    async fn upsert_major_shareholders(
        &self,
        transaction: &UnitOfWorkTransaction,
        documents: Vec<MajorShareholderDocument>,
    ) -> Result<usize, ShareholdingStructureRepositoryError> {
        upsert_documents::<major_shareholder_documents::Entity, _, _>(
            transaction,
            documents,
            |document| {
                Ok(major_shareholder_documents::ActiveModel {
                    document_id: Set(document.metadata.document_id),
                    stock_code: Set(document.metadata.stock_code),
                    filer_code: Set(document.metadata.filer_code),
                    submitted_on: Set(document.metadata.submitted_on),
                    details: Set(serialize_details(document.content)?),
                    created_at: NotSet,
                    updated_at: Set(Utc::now().fixed_offset()),
                })
            },
            OnConflict::column(major_shareholder_documents::Column::DocumentId)
                .update_columns([
                    major_shareholder_documents::Column::StockCode,
                    major_shareholder_documents::Column::FilerCode,
                    major_shareholder_documents::Column::SubmittedOn,
                    major_shareholder_documents::Column::Details,
                    major_shareholder_documents::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .await
    }

    async fn latest_cross_shareholding_submitted_on(
        &self,
    ) -> Result<Option<NaiveDate>, ShareholdingStructureRepositoryError> {
        latest_submitted_on::<cross_shareholding_documents::Entity>(
            &self.db,
            cross_shareholding_documents::Column::SubmittedOn,
            |model| model.submitted_on,
        )
        .await
        .map_err(repository_error)
    }

    async fn upsert_cross_shareholdings(
        &self,
        transaction: &UnitOfWorkTransaction,
        documents: Vec<CrossShareholdingDocument>,
    ) -> Result<usize, ShareholdingStructureRepositoryError> {
        upsert_documents::<cross_shareholding_documents::Entity, _, _>(
            transaction,
            documents,
            |document| {
                Ok(cross_shareholding_documents::ActiveModel {
                    document_id: Set(document.metadata.document_id),
                    stock_code: Set(document.metadata.stock_code),
                    filer_code: Set(document.metadata.filer_code),
                    submitted_on: Set(document.metadata.submitted_on),
                    details: Set(serialize_details(document.content)?),
                    created_at: NotSet,
                    updated_at: Set(Utc::now().fixed_offset()),
                })
            },
            OnConflict::column(cross_shareholding_documents::Column::DocumentId)
                .update_columns([
                    cross_shareholding_documents::Column::StockCode,
                    cross_shareholding_documents::Column::FilerCode,
                    cross_shareholding_documents::Column::SubmittedOn,
                    cross_shareholding_documents::Column::Details,
                    cross_shareholding_documents::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .await
    }

    async fn find_for_symbol(
        &self,
        symbol: &str,
        limit: u64,
    ) -> Result<ShareholdingStructureBySymbol, ShareholdingStructureRepositoryError> {
        let lower = format!("{symbol}0");
        let upper = format!("{symbol}9");
        let large_volume_rows = large_volume_shareholding_documents::Entity::find()
            .filter(
                large_volume_shareholding_documents::Column::StockCode
                    .between(lower.clone(), upper.clone()),
            )
            .order_by_desc(large_volume_shareholding_documents::Column::SubmittedOn)
            .order_by_desc(large_volume_shareholding_documents::Column::DocumentId)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(repository_error)?;

        let mut large_volume_reports = Vec::with_capacity(large_volume_rows.len());
        for row in large_volume_rows {
            let document_id = row.document_id;
            let content = match deserialize_details::<LargeVolumeShareholdingContent>(
                row.details,
                "large volume shareholding",
                &document_id,
            ) {
                Ok(content) => content,
                Err(error) => {
                    tracing::warn!(doc_id = document_id, %error, "malformed large volume shareholding details, skipping");
                    continue;
                }
            };
            large_volume_reports.push(LargeVolumeShareholdingDocument {
                metadata: ShareholdingDocumentMetadata {
                    document_id,
                    stock_code: row.stock_code,
                    filer_code: row.filer_code,
                    submitted_on: row.submitted_on,
                },
                content: Some(content),
            });
        }

        let major_shareholder_row =
            latest_matching_document::<major_shareholder_documents::Entity>(
                &self.db,
                major_shareholder_documents::Column::StockCode,
                major_shareholder_documents::Column::SubmittedOn,
                major_shareholder_documents::Column::DocumentId,
                major_shareholder_documents::Column::Details,
                &lower,
                &upper,
            )
            .await
            .map_err(repository_error)?;
        let major_shareholders = match major_shareholder_row {
            Some(row) => {
                let content = deserialize_details::<MajorShareholderContent>(
                    row.details,
                    "major shareholder",
                    &row.document_id,
                )?;
                Some(MajorShareholderDocument {
                    metadata: ShareholdingDocumentMetadata {
                        document_id: row.document_id,
                        stock_code: row.stock_code,
                        filer_code: row.filer_code,
                        submitted_on: row.submitted_on,
                    },
                    content: Some(content),
                })
            }
            None => None,
        };

        let cross_shareholding_row =
            latest_matching_document::<cross_shareholding_documents::Entity>(
                &self.db,
                cross_shareholding_documents::Column::StockCode,
                cross_shareholding_documents::Column::SubmittedOn,
                cross_shareholding_documents::Column::DocumentId,
                cross_shareholding_documents::Column::Details,
                &lower,
                &upper,
            )
            .await
            .map_err(repository_error)?;
        let cross_shareholdings = match cross_shareholding_row {
            Some(row) => {
                let content = deserialize_details::<core_domain::holdings::CrossShareholdingContent>(
                    row.details,
                    "cross shareholding",
                    &row.document_id,
                )?;
                Some(CrossShareholdingDocument {
                    metadata: ShareholdingDocumentMetadata {
                        document_id: row.document_id,
                        stock_code: row.stock_code,
                        filer_code: row.filer_code,
                        submitted_on: row.submitted_on,
                    },
                    content: Some(content),
                })
            }
            None => None,
        };

        Ok(ShareholdingStructureBySymbol {
            large_volume_reports,
            major_shareholders,
            cross_shareholdings,
        })
    }
}

async fn latest_submitted_on<E>(
    db: &impl ConnectionTrait,
    submitted_on_column: E::Column,
    submitted_on: impl Fn(&E::Model) -> NaiveDate,
) -> Result<Option<NaiveDate>, sea_orm::DbErr>
where
    E: EntityTrait,
{
    E::find()
        .order_by_desc(submitted_on_column)
        .one(db)
        .await
        .map(|row| row.as_ref().map(submitted_on))
}

async fn latest_matching_document<E>(
    db: &impl ConnectionTrait,
    code_column: E::Column,
    submitted_on_column: E::Column,
    document_id_column: E::Column,
    details_column: E::Column,
    lower: &str,
    upper: &str,
) -> Result<Option<E::Model>, sea_orm::DbErr>
where
    E: EntityTrait,
{
    E::find()
        .filter(code_column.between(lower.to_owned(), upper.to_owned()))
        .filter(details_column.ne(Value::Null))
        .order_by_desc(submitted_on_column)
        .order_by_desc(document_id_column)
        .one(db)
        .await
}

async fn upsert_documents<E, T, F>(
    transaction: &UnitOfWorkTransaction,
    documents: Vec<T>,
    build: F,
    conflict: OnConflict,
) -> Result<usize, ShareholdingStructureRepositoryError>
where
    E: EntityTrait,
    E::ActiveModel: Send,
    T: Send,
    F: Fn(T) -> Result<E::ActiveModel, sea_orm::DbErr> + Send,
{
    let transaction = postgres_transaction_ref(transaction)
        .ok_or(ShareholdingStructureRepositoryError::InvalidTransaction)?;
    let count = documents.len();
    if count == 0 {
        return Ok(0);
    }
    let models = documents
        .into_iter()
        .map(build)
        .collect::<Result<Vec<_>, _>>()
        .map_err(repository_error)?;
    E::insert_many(models)
        .on_conflict(conflict)
        .exec_without_returning(transaction)
        .await
        .map_err(repository_error)?;
    Ok(count)
}

fn serialize_details<T: Serialize>(details: T) -> Result<Value, sea_orm::DbErr> {
    serde_json::to_value(details).map_err(|error| sea_orm::DbErr::Custom(error.to_string()))
}

fn deserialize_details<T: DeserializeOwned>(
    details: Value,
    document_kind: &'static str,
    document_id: &str,
) -> Result<T, ShareholdingStructureRepositoryError> {
    serde_json::from_value(details).map_err(|error| {
        ShareholdingStructureRepositoryError::MalformedDocument {
            document_kind,
            document_id: document_id.to_owned(),
            message: error.to_string(),
        }
    })
}

fn repository_error(error: sea_orm::DbErr) -> ShareholdingStructureRepositoryError {
    ShareholdingStructureRepositoryError::Database(persistence_error(error))
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::shareholding_structure::{
        ShareholdingStructureBySymbol, ShareholdingStructureRepository,
    };
    use core_application::unit_of_work::UnitOfWork;
    use core_domain::holdings::{
        CrossShareholding, CrossShareholdingCategory, CrossShareholdingContent,
        CrossShareholdingDocument, LargeVolumeHolder, LargeVolumeReportType,
        LargeVolumeShareholdingContent, LargeVolumeShareholdingDocument, MajorShareholder,
        MajorShareholderContent, MajorShareholderDocument, MajorShareholderReportType,
        MutualHolding, ShareholdingDocumentMetadata,
    };

    use crate::{DatabaseHandle, PostgresUnitOfWork};

    use super::PostgresShareholdingStructureRepository;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn metadata(document_id: &str, submitted_on: NaiveDate) -> ShareholdingDocumentMetadata {
        ShareholdingDocumentMetadata {
            document_id: document_id.into(),
            stock_code: Some("ZZ990".into()),
            filer_code: "E99999".into(),
            submitted_on,
        }
    }

    #[backend_test_macros::database_test]
    async fn upserts_and_reads_each_document_kind(db: DatabaseHandle) {
        let repository = PostgresShareholdingStructureRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db);
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        let old_date = date(2040, 1, 1);
        let latest_date = date(2040, 2, 1);
        let large_volume_old = LargeVolumeShareholdingDocument {
            metadata: metadata("SAMPLE-LARGE-OLD", old_date),
            content: Some(LargeVolumeShareholdingContent {
                report_type: LargeVolumeReportType::Report,
                change_reason: None,
                total_shares_ratio: Some(0.1),
                previous_total_shares_ratio: None,
                holders: vec![LargeVolumeHolder {
                    name: "架空保有者".into(),
                    holding_purpose: Some("投資".into()),
                    shares_held: Some(100),
                    shares_ratio: Some(0.1),
                    previous_shares_ratio: None,
                }],
            }),
        };
        let large_volume_correction = LargeVolumeShareholdingDocument {
            metadata: metadata("SAMPLE-LARGE-OLD", old_date),
            content: Some(LargeVolumeShareholdingContent {
                report_type: LargeVolumeReportType::Amendment,
                change_reason: Some("架空の訂正".into()),
                total_shares_ratio: Some(0.15),
                previous_total_shares_ratio: Some(0.1),
                holders: vec![],
            }),
        };
        let large_volume_latest = LargeVolumeShareholdingDocument {
            metadata: metadata("SAMPLE-LARGE-NEW", latest_date),
            content: Some(LargeVolumeShareholdingContent {
                report_type: LargeVolumeReportType::Amendment,
                change_reason: Some("架空の変更".into()),
                total_shares_ratio: Some(0.2),
                previous_total_shares_ratio: Some(0.1),
                holders: vec![],
            }),
        };
        let major_shareholders = MajorShareholderDocument {
            metadata: metadata("SAMPLE-MAJOR", latest_date),
            content: Some(MajorShareholderContent {
                period_end: Some(old_date),
                report_type: MajorShareholderReportType::Annual,
                holders: vec![MajorShareholder {
                    rank: Some(1),
                    name: "架空株主".into(),
                    shares_held: Some(500),
                    shares_ratio: Some(0.25),
                }],
            }),
        };
        let cross_shareholdings = CrossShareholdingDocument {
            metadata: metadata("SAMPLE-CROSS", latest_date),
            content: Some(CrossShareholdingContent {
                period_end: Some(old_date),
                holdings: vec![CrossShareholding {
                    issuer_name: "架空発行体".into(),
                    issuer_stock_code: Some("YY880".into()),
                    category: CrossShareholdingCategory::Specified,
                    current_shares: Some(200),
                    previous_shares: Some(100),
                    current_book_value: Some(300),
                    previous_book_value: Some(150),
                    mutual_holding: MutualHolding::Held,
                }],
            }),
        };

        let saved = (
            repository
                .upsert_large_volume(
                    &transaction,
                    vec![large_volume_old.clone(), large_volume_latest.clone()],
                )
                .await
                .expect("upsert large volume documents"),
            repository
                .upsert_major_shareholders(&transaction, vec![major_shareholders.clone()])
                .await
                .expect("upsert major shareholder documents"),
            repository
                .upsert_cross_shareholdings(&transaction, vec![cross_shareholdings.clone()])
                .await
                .expect("upsert cross shareholding documents"),
        );
        let corrected_count = repository
            .upsert_large_volume(&transaction, vec![large_volume_correction.clone()])
            .await
            .expect("upsert corrected large volume document");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");

        let latest_dates = (
            repository
                .latest_large_volume_submitted_on()
                .await
                .expect("query latest large volume date"),
            repository
                .latest_major_shareholder_submitted_on()
                .await
                .expect("query latest major shareholder date"),
            repository
                .latest_cross_shareholding_submitted_on()
                .await
                .expect("query latest cross shareholding date"),
        );
        let read = repository
            .find_for_symbol("ZZ99", 2)
            .await
            .expect("read shareholding structure");

        assert_eq!(
            (saved, corrected_count, latest_dates, read),
            (
                (2, 1, 1),
                1,
                (Some(latest_date), Some(latest_date), Some(latest_date)),
                ShareholdingStructureBySymbol {
                    large_volume_reports: vec![large_volume_latest, large_volume_correction],
                    major_shareholders: Some(major_shareholders),
                    cross_shareholdings: Some(cross_shareholdings),
                },
            ),
        );
    }
}
