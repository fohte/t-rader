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
        // Postgres のデフォルト照合順序では LIKE 'symbol%' が B-tree index を使えないため、範囲条件を使う。
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
        CrossShareholdingContent, CrossShareholdingDocument, LargeVolumeReportType,
        LargeVolumeShareholdingContent, LargeVolumeShareholdingDocument, MajorShareholderContent,
        MajorShareholderDocument, MajorShareholderReportType, ShareholdingDocumentMetadata,
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

    fn large_volume_document(
        document_id: &str,
        submitted_on: NaiveDate,
    ) -> LargeVolumeShareholdingDocument {
        LargeVolumeShareholdingDocument {
            metadata: metadata(document_id, submitted_on),
            content: Some(LargeVolumeShareholdingContent {
                report_type: LargeVolumeReportType::Report,
                change_reason: None,
                total_shares_ratio: None,
                previous_total_shares_ratio: None,
                holders: vec![],
            }),
        }
    }

    fn corrected_large_volume_document(
        document_id: &str,
        submitted_on: NaiveDate,
    ) -> LargeVolumeShareholdingDocument {
        LargeVolumeShareholdingDocument {
            metadata: metadata(document_id, submitted_on),
            content: Some(LargeVolumeShareholdingContent {
                report_type: LargeVolumeReportType::Amendment,
                change_reason: Some("架空の訂正".into()),
                total_shares_ratio: None,
                previous_total_shares_ratio: None,
                holders: vec![],
            }),
        }
    }

    fn major_shareholder_document(
        document_id: &str,
        submitted_on: NaiveDate,
    ) -> MajorShareholderDocument {
        MajorShareholderDocument {
            metadata: metadata(document_id, submitted_on),
            content: Some(MajorShareholderContent {
                period_end: None,
                report_type: MajorShareholderReportType::Annual,
                holders: vec![],
            }),
        }
    }

    fn cross_shareholding_document(
        document_id: &str,
        submitted_on: NaiveDate,
    ) -> CrossShareholdingDocument {
        CrossShareholdingDocument {
            metadata: metadata(document_id, submitted_on),
            content: Some(CrossShareholdingContent {
                period_end: None,
                holdings: vec![],
            }),
        }
    }

    async fn save_documents(
        repository: &PostgresShareholdingStructureRepository,
        unit_of_work: &PostgresUnitOfWork,
        large_volume: Vec<LargeVolumeShareholdingDocument>,
        major_shareholders: Vec<MajorShareholderDocument>,
        cross_shareholdings: Vec<CrossShareholdingDocument>,
    ) -> (usize, usize, usize) {
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        let saved = (
            repository
                .upsert_large_volume(&transaction, large_volume)
                .await
                .expect("upsert large volume documents"),
            repository
                .upsert_major_shareholders(&transaction, major_shareholders)
                .await
                .expect("upsert major shareholder documents"),
            repository
                .upsert_cross_shareholdings(&transaction, cross_shareholdings)
                .await
                .expect("upsert cross shareholding documents"),
        );
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");
        saved
    }

    #[backend_test_macros::database_test]
    async fn upserts_and_reads_each_document_kind(db: DatabaseHandle) {
        let repository = PostgresShareholdingStructureRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db);
        let submitted_on = date(2040, 2, 1);
        let large_volume = large_volume_document("SAMPLE-LARGE", submitted_on);
        let major_shareholder = major_shareholder_document("SAMPLE-MAJOR", submitted_on);
        let cross_shareholding = cross_shareholding_document("SAMPLE-CROSS", submitted_on);
        let saved = save_documents(
            &repository,
            &unit_of_work,
            vec![large_volume.clone()],
            vec![major_shareholder.clone()],
            vec![cross_shareholding.clone()],
        )
        .await;
        let read = repository
            .find_for_symbol("ZZ99", 1)
            .await
            .expect("read documents");

        assert_eq!(
            (saved, read),
            (
                (1, 1, 1),
                ShareholdingStructureBySymbol {
                    large_volume_reports: vec![large_volume],
                    major_shareholders: Some(major_shareholder),
                    cross_shareholdings: Some(cross_shareholding),
                },
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn upserts_large_volume_correction(db: DatabaseHandle) {
        let repository = PostgresShareholdingStructureRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db);
        let submitted_on = date(2040, 2, 1);
        let original = large_volume_document("SAMPLE-LARGE", submitted_on);
        let correction = corrected_large_volume_document("SAMPLE-LARGE", submitted_on);
        let first_saved =
            save_documents(&repository, &unit_of_work, vec![original], vec![], vec![]).await;
        let corrected_saved = save_documents(
            &repository,
            &unit_of_work,
            vec![correction.clone()],
            vec![],
            vec![],
        )
        .await;
        let read = repository
            .find_for_symbol("ZZ99", 1)
            .await
            .expect("read corrected document");

        assert_eq!(
            (first_saved, corrected_saved, read.large_volume_reports),
            ((1, 0, 0), (1, 0, 0), vec![correction]),
        );
    }

    #[backend_test_macros::database_test]
    async fn latest_submitted_on_returns_latest_date_for_each_document_kind(db: DatabaseHandle) {
        let repository = PostgresShareholdingStructureRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db);
        let old_date = date(2040, 1, 1);
        let latest_date = date(2040, 2, 1);
        let saved = save_documents(
            &repository,
            &unit_of_work,
            vec![
                large_volume_document("SAMPLE-LARGE-OLD", old_date),
                large_volume_document("SAMPLE-LARGE-NEW", latest_date),
            ],
            vec![
                major_shareholder_document("SAMPLE-MAJOR-OLD", old_date),
                major_shareholder_document("SAMPLE-MAJOR-NEW", latest_date),
            ],
            vec![
                cross_shareholding_document("SAMPLE-CROSS-OLD", old_date),
                cross_shareholding_document("SAMPLE-CROSS-NEW", latest_date),
            ],
        )
        .await;
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

        assert_eq!(
            (saved, latest_dates),
            (
                (2, 2, 2),
                (Some(latest_date), Some(latest_date), Some(latest_date)),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn find_for_symbol_orders_large_volume_reports_and_respects_limit(db: DatabaseHandle) {
        let repository = PostgresShareholdingStructureRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db);
        let old = large_volume_document("SAMPLE-LARGE-OLD", date(2040, 1, 1));
        let latest_b = large_volume_document("SAMPLE-LARGE-B", date(2040, 2, 1));
        let latest_c = large_volume_document("SAMPLE-LARGE-C", date(2040, 2, 1));
        let saved = save_documents(
            &repository,
            &unit_of_work,
            vec![old, latest_b.clone(), latest_c.clone()],
            vec![],
            vec![],
        )
        .await;
        let read = repository
            .find_for_symbol("ZZ99", 2)
            .await
            .expect("read documents");

        assert_eq!(
            (saved, read),
            (
                (3, 0, 0),
                ShareholdingStructureBySymbol {
                    large_volume_reports: vec![latest_c, latest_b],
                    major_shareholders: None,
                    cross_shareholdings: None,
                },
            ),
        );
    }
}
