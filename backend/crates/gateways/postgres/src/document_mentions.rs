use async_trait::async_trait;
use core_application::document_mentions::{
    DocumentMention, DocumentMentionRepository, DocumentMentionRepositoryError, RegisteredStock,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::LockType;
use sea_orm::{
    ColumnTrait, DatabaseTransaction, EntityTrait, Iterable, QueryFilter, QueryOrder, QuerySelect,
};
use uuid::Uuid;

use crate::entities::{document_chunk, document_chunk_mention, stock};
use crate::persistence::persistence_error;

#[derive(Clone, Default)]
pub struct PostgresDocumentMentionRepository;

impl PostgresDocumentMentionRepository {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl DocumentMentionRepository for PostgresDocumentMentionRepository {
    async fn lock_chunks_for_update(
        &self,
        transaction: &UnitOfWorkTransaction,
        chunk_ids: Vec<Uuid>,
    ) -> Result<(), DocumentMentionRepositoryError> {
        if chunk_ids.is_empty() {
            return Ok(());
        }
        let transaction = transaction_ref(transaction)?;
        document_chunk::Entity::find()
            .filter(document_chunk::Column::Id.is_in(chunk_ids))
            .select_only()
            .column(document_chunk::Column::Id)
            .order_by_asc(document_chunk::Column::Id)
            .lock(LockType::Update)
            .into_tuple::<Uuid>()
            .all(transaction)
            .await
            .map(|_| ())
            .map_err(repository_error)
    }

    async fn find_registered_stocks(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<Vec<RegisteredStock>, DocumentMentionRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        stock::Entity::find()
            .select_only()
            .columns([stock::Column::Id, stock::Column::Name])
            .order_by_asc(stock::Column::Id)
            .into_tuple::<(String, String)>()
            .all(transaction)
            .await
            .map(|stocks| {
                stocks
                    .into_iter()
                    .map(|(id, name)| RegisteredStock { id, name })
                    .collect()
            })
            .map_err(repository_error)
    }

    async fn replace_mentions(
        &self,
        transaction: &UnitOfWorkTransaction,
        chunk_ids: Vec<Uuid>,
        mentions: Vec<DocumentMention>,
    ) -> Result<(), DocumentMentionRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        if !chunk_ids.is_empty() {
            document_chunk_mention::Entity::delete_many()
                .filter(document_chunk_mention::Column::ChunkId.is_in(chunk_ids))
                .exec(transaction)
                .await
                .map_err(repository_error)?;
        }
        if mentions.is_empty() {
            return Ok(());
        }

        let active_models = mentions
            .into_iter()
            .map(|mention| document_chunk_mention::ActiveModel {
                id: Set(Uuid::new_v4()),
                chunk_id: Set(mention.chunk_id),
                stock_id: Set(mention.stock_id),
                start_offset: Set(mention.start_offset),
                end_offset: Set(mention.end_offset),
            })
            .collect::<Vec<_>>();
        let columns_per_row = document_chunk_mention::Column::iter().count();
        let chunk_size = crate::repositories::max_rows_per_statement(columns_per_row);
        for chunk in active_models.chunks(chunk_size) {
            document_chunk_mention::Entity::insert_many(chunk.to_vec())
                .exec_without_returning(transaction)
                .await
                .map_err(repository_error)?;
        }
        Ok(())
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&DatabaseTransaction, DocumentMentionRepositoryError> {
    crate::transaction::transaction_ref(transaction)
        .ok_or(DocumentMentionRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> DocumentMentionRepositoryError {
    DocumentMentionRepositoryError::Persistence(persistence_error(error))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "DB テストの準備が失敗した場合に原因を表示するため"
    )]

    use chrono::{DateTime, FixedOffset, Utc};
    use core_application::document_mentions::{
        DocumentMention, DocumentMentionRepository, RegisteredStock,
    };
    use core_application::unit_of_work::UnitOfWork;
    use sea_orm::ActiveValue::Set;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
    use uuid::Uuid;

    use crate::entities::{
        document, document_chunk, document_chunk_mention, document_version, stock,
    };
    use crate::{DatabaseHandle, unit_of_work::PostgresUnitOfWork};

    use super::PostgresDocumentMentionRepository;

    fn timestamp() -> DateTime<FixedOffset> {
        DateTime::<Utc>::UNIX_EPOCH.fixed_offset()
    }

    async fn mentions_for_chunk(db: &DatabaseHandle, chunk_id: Uuid) -> Vec<(String, i32, i32)> {
        document_chunk_mention::Entity::find()
            .filter(document_chunk_mention::Column::ChunkId.eq(chunk_id))
            .order_by_asc(document_chunk_mention::Column::StartOffset)
            .select_only()
            .columns([
                document_chunk_mention::Column::StockId,
                document_chunk_mention::Column::StartOffset,
                document_chunk_mention::Column::EndOffset,
            ])
            .into_tuple::<(String, i32, i32)>()
            .all(db)
            .await
            .expect("mention query succeeds")
    }

    #[backend_test_macros::database_test]
    async fn repository_replaces_mentions_separate_from_issuer_stock(db: DatabaseHandle) {
        let issuer_id = format!("TEST-ISSUER-{}", Uuid::new_v4());
        let mentioned_id = format!("TEST-MENTIONED-{}", Uuid::new_v4());
        let document_id = Uuid::new_v4();
        let version_id = Uuid::new_v4();
        let chunk_id = Uuid::new_v4();
        stock::Entity::insert_many([
            stock::ActiveModel {
                id: Set(issuer_id.clone()),
                name: Set("Synthetic Issuer".into()),
                market: Set(None),
                product_category: Set(None),
                cik: Set(None),
                created_at: Default::default(),
                updated_at: Default::default(),
            },
            stock::ActiveModel {
                id: Set(mentioned_id.clone()),
                name: Set("Synthetic Mentioned Company".into()),
                market: Set(None),
                product_category: Set(None),
                cik: Set(None),
                created_at: Default::default(),
                updated_at: Default::default(),
            },
        ])
        .exec_without_returning(&db)
        .await
        .expect("insert test stocks");
        document::Entity::insert(document::ActiveModel {
            id: Set(document_id),
            source: Set("archive-test".into()),
            source_document_id: Set(Uuid::new_v4().to_string()),
            stock_id: Set(Some(issuer_id.clone())),
            source_issuer_code: Set("ISSUER-TEST".into()),
            title: Set("Synthetic document".into()),
            published_at: Set(timestamp()),
            created_at: Default::default(),
            updated_at: Default::default(),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert test document");
        document_version::Entity::insert(document_version::ActiveModel {
            id: Set(version_id),
            document_id: Set(document_id),
            content_hash: Set("0".repeat(64)),
            retrieved_at: Set(timestamp()),
            body: Set("Synthetic Mentioned Company".into()),
            created_at: Default::default(),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert test document version");
        document_chunk::Entity::insert(document_chunk::ActiveModel {
            id: Set(chunk_id),
            version_id: Set(version_id),
            sequence: Set(0),
            start_offset: Set(0),
            end_offset: Set(27),
            body: Set("Synthetic Mentioned Company".into()),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert test document chunk");

        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let repository = PostgresDocumentMentionRepository::new();
        let first_mention = DocumentMention {
            chunk_id,
            stock_id: mentioned_id.clone(),
            start_offset: 0,
            end_offset: 27,
        };
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        repository
            .lock_chunks_for_update(&transaction, vec![chunk_id])
            .await
            .expect("chunk lock succeeds");
        let registered_stocks = repository
            .find_registered_stocks(&transaction)
            .await
            .expect("registered stocks query succeeds");
        repository
            .replace_mentions(&transaction, vec![chunk_id], vec![first_mention])
            .await
            .expect("first mention replacement succeeds");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");

        let updated_mention = DocumentMention {
            chunk_id,
            stock_id: mentioned_id.clone(),
            start_offset: 1,
            end_offset: 27,
        };
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        repository
            .replace_mentions(&transaction, vec![chunk_id], vec![updated_mention])
            .await
            .expect("updated mention replacement succeeds");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");
        let updated_mentions = mentions_for_chunk(&db, chunk_id).await;

        let transaction = unit_of_work.begin().await.expect("transaction begins");
        repository
            .replace_mentions(&transaction, vec![chunk_id], Vec::new())
            .await
            .expect("empty mention replacement succeeds");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");

        let issuer_stock_id = document::Entity::find_by_id(document_id)
            .one(&db)
            .await
            .expect("document query succeeds")
            .expect("document exists")
            .stock_id;
        let mentions = mentions_for_chunk(&db, chunk_id).await;

        assert_eq!(
            (
                registered_stocks,
                issuer_stock_id,
                updated_mentions,
                mentions
            ),
            (
                vec![
                    RegisteredStock {
                        id: issuer_id.clone(),
                        name: "Synthetic Issuer".into(),
                    },
                    RegisteredStock {
                        id: mentioned_id.clone(),
                        name: "Synthetic Mentioned Company".into(),
                    },
                ],
                Some(issuer_id),
                vec![(mentioned_id, 1, 27)],
                Vec::<(String, i32, i32)>::new(),
            ),
        );
    }
}
