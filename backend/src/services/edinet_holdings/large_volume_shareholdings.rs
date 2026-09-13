use chrono::NaiveDate;
use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, Set};
use serde_json::Value;

use crate::entities::edinet_large_volume_shareholdings::{ActiveModel, Column, Entity};

pub(crate) struct Endpoint;

impl super::EdinetEndpoint for Endpoint {
    const NAME: &'static str = "large_volume_shareholdings";
    const PATH: &'static str = "/edinet/large-volume-shareholders";

    fn available_from() -> NaiveDate {
        NaiveDate::from_ymd_opt(2021, 7, 1).unwrap_or_default()
    }

    async fn latest_sub_date(db: &DatabaseConnection) -> Result<Option<NaiveDate>, sea_orm::DbErr> {
        super::latest_sub_date_of::<Entity, _>(db, Column::SubDate, |m| m.sub_date).await
    }

    async fn upsert(db: &DatabaseConnection, docs: Vec<Value>) -> Result<usize, sea_orm::DbErr> {
        super::upsert_documents::<Entity>(
            db,
            docs,
            |meta, doc| ActiveModel {
                doc_id: Set(meta.doc_id),
                code: Set(meta.code),
                edinet_code: Set(meta.edinet_code),
                sub_date: Set(meta.sub_date),
                document: Set(doc),
                created_at: sea_orm::ActiveValue::NotSet,
                updated_at: Set(chrono::Utc::now().fixed_offset()),
            },
            OnConflict::column(Column::DocId)
                .update_columns([
                    Column::Code,
                    Column::EdinetCode,
                    Column::SubDate,
                    Column::Document,
                    Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .await
    }
}
