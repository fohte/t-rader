use chrono::NaiveDate;
use core_application::{ShareholdingStructureSource, ShareholdingStructureSourceError};
use core_domain::holdings::LargeVolumeShareholdingDocument;
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::OnConflict;

use crate::entities::large_volume_shareholding_documents::{ActiveModel, Column, Entity};

pub(crate) struct Endpoint;

#[async_trait::async_trait]
impl super::EdinetEndpoint for Endpoint {
    type Document = LargeVolumeShareholdingDocument;

    const NAME: &'static str = "large_volume_shareholdings";

    fn available_from() -> NaiveDate {
        NaiveDate::from_ymd_opt(2021, 7, 1).unwrap_or_default()
    }

    async fn latest_submitted_on(
        db: &impl sea_orm::ConnectionTrait,
    ) -> Result<Option<NaiveDate>, sea_orm::DbErr> {
        super::latest_submitted_on_of::<Entity, _>(db, Column::SubmittedOn, |model| {
            model.submitted_on
        })
        .await
    }

    async fn fetch(
        source: &dyn ShareholdingStructureSource,
        date: NaiveDate,
    ) -> Result<Vec<Self::Document>, ShareholdingStructureSourceError> {
        source.fetch_large_volume_documents(date).await
    }

    async fn upsert(
        db: &impl sea_orm::ConnectionTrait,
        documents: Vec<Self::Document>,
    ) -> Result<usize, sea_orm::DbErr> {
        super::upsert_documents::<Entity, _>(
            db,
            documents,
            |document| {
                Ok(ActiveModel {
                    document_id: Set(document.metadata.document_id),
                    stock_code: Set(document.metadata.stock_code),
                    filer_code: Set(document.metadata.filer_code),
                    submitted_on: Set(document.metadata.submitted_on),
                    details: Set(super::serialize_details(document.content)?),
                    created_at: sea_orm::ActiveValue::NotSet,
                    updated_at: Set(chrono::Utc::now().fixed_offset()),
                })
            },
            OnConflict::column(Column::DocumentId)
                .update_columns([
                    Column::StockCode,
                    Column::FilerCode,
                    Column::SubmittedOn,
                    Column::Details,
                    Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .await
    }
}
