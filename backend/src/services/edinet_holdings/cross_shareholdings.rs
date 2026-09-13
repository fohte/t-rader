use chrono::NaiveDate;
use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, Set};
use serde_json::Value;

use crate::entities::edinet_cross_shareholdings::{ActiveModel, Column, Entity};

pub(crate) struct Endpoint;

impl super::EdinetEndpoint for Endpoint {
    const NAME: &'static str = "cross_shareholdings";
    const PATH: &'static str = "/edinet/cross-shareholdings";

    fn available_from() -> NaiveDate {
        NaiveDate::from_ymd_opt(2020, 3, 31).unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset, NaiveDate};
    use sea_orm::EntityTrait;
    use serde_json::json;
    use sqlx::PgPool;

    use super::Endpoint;
    use crate::data_provider::jquants::mock::JQuantsMockServer;
    use crate::entities::edinet_cross_shareholdings;
    use crate::services::edinet_holdings::{EdinetEndpoint, run_ingest_cycle};
    use crate::testing::create_test_db;

    #[test]
    fn endpoint_metadata_is_correct() {
        assert_eq!(Endpoint::NAME, "cross_shareholdings");
        assert_eq!(Endpoint::PATH, "/edinet/cross-shareholdings");
        assert_eq!(
            Endpoint::available_from(),
            NaiveDate::from_ymd_opt(2020, 3, 31).expect("date")
        );
    }

    #[sqlx::test(migrations = false)]
    async fn ingests_a_single_document(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let from = Endpoint::available_from();
        let doc = json!({
            "DocId": "S100ABCD",
            "Code": "72030",
            "EdinetCode": "E00001",
            "SubDate": from.format("%Y-%m-%d").to_string(),
        });

        mock.edinet_documents(Endpoint::PATH)
            .date(&from.format("%Y%m%d").to_string())
            .docs(vec![doc.clone()])
            .ok()
            .await;

        let client = mock.client().expect("client");
        client.set_detected_range((from, from));

        let stats = run_ingest_cycle::<Endpoint>(&db, &client)
            .await
            .expect("cycle ok");
        assert_eq!(stats.documents_saved, 1);

        let fixed: DateTime<FixedOffset> = DateTime::UNIX_EPOCH.fixed_offset();
        let rows: Vec<edinet_cross_shareholdings::Model> =
            edinet_cross_shareholdings::Entity::find()
                .all(&db)
                .await
                .expect("find all")
                .into_iter()
                .map(|mut m| {
                    m.created_at = fixed;
                    m.updated_at = fixed;
                    m
                })
                .collect();

        assert_eq!(
            rows,
            vec![edinet_cross_shareholdings::Model {
                doc_id: "S100ABCD".to_string(),
                code: Some("72030".to_string()),
                edinet_code: "E00001".to_string(),
                sub_date: from,
                document: doc,
                created_at: fixed,
                updated_at: fixed,
            }]
        );
    }
}
