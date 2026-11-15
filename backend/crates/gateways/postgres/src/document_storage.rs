use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use core_application::document_storage::{
    Document, DocumentChunk, DocumentChunkContext, DocumentChunkInput, DocumentMetadata,
    DocumentSnapshot, DocumentStorageRepository, DocumentStorageRepositoryError, DocumentVersion,
    DocumentVersionReference,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use core_domain::document_search::prepare_document_search;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, FromQueryResult, QueryFilter,
    QueryOrder, Statement,
};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::{document, document_chunk, document_version};
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresDocumentStorageRepository {
    db: DatabaseHandle,
}

impl PostgresDocumentStorageRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DocumentStorageRepository for PostgresDocumentStorageRepository {
    async fn save(
        &self,
        transaction: &UnitOfWorkTransaction,
        metadata: DocumentMetadata,
        content_hash: String,
        retrieved_at: DateTime<FixedOffset>,
        body: String,
        chunks: Vec<DocumentChunkInput>,
    ) -> Result<DocumentSnapshot, DocumentStorageRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let document_id = Uuid::new_v4();
        let document_row = document::Entity::insert(document::ActiveModel {
            id: Set(document_id),
            source: Set(metadata.source.clone()),
            source_document_id: Set(metadata.source_document_id.clone()),
            stock_id: metadata
                .stock_id
                .clone()
                .map_or(Set(None), |id| Set(Some(id))),
            source_issuer_code: Set(metadata.source_issuer_code.clone()),
            title: Set(metadata.title.clone()),
            published_at: Set(metadata.published_at),
            created_at: NotSet,
            updated_at: NotSet,
        })
        .on_conflict(
            OnConflict::columns([document::Column::Source, document::Column::SourceDocumentId])
                .update_columns([
                    document::Column::StockId,
                    document::Column::SourceIssuerCode,
                    document::Column::Title,
                    document::Column::PublishedAt,
                    document::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec_with_returning(transaction)
        .await
        .map_err(repository_error)?;

        let latest_version = document_version::Entity::find()
            .filter(document_version::Column::DocumentId.eq(document_row.id))
            .order_by_desc(document_version::Column::RetrievedAt)
            .order_by_desc(document_version::Column::CreatedAt)
            .order_by_desc(document_version::Column::Id)
            .one(transaction)
            .await
            .map_err(repository_error)?;
        let (version_row, is_new_version) = match latest_version {
            Some(version_row) if version_row.content_hash == content_hash => (version_row, false),
            _ => {
                let version_row = document_version::Entity::insert(document_version::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    document_id: Set(document_row.id),
                    content_hash: Set(content_hash),
                    retrieved_at: Set(retrieved_at),
                    body: Set(body),
                    created_at: NotSet,
                })
                .exec_with_returning(transaction)
                .await
                .map_err(repository_error)?;
                (version_row, true)
            }
        };

        if is_new_version && !chunks.is_empty() {
            document_chunk::Entity::insert_many(chunks.into_iter().map(|chunk| {
                let search = prepare_document_search(&chunk.body);
                document_chunk::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    version_id: Set(version_row.id),
                    sequence: Set(chunk.sequence),
                    start_offset: Set(chunk.start_offset),
                    end_offset: Set(chunk.end_offset),
                    body: Set(chunk.body),
                    search_text: Set(search.normalized_text),
                    search_terms: Set(search.terms),
                }
            }))
            .exec_without_returning(transaction)
            .await
            .map_err(repository_error)?;
        }
        let chunk_rows = document_chunk::Entity::find()
            .filter(document_chunk::Column::VersionId.eq(version_row.id))
            .order_by_asc(document_chunk::Column::Sequence)
            .all(transaction)
            .await
            .map_err(repository_error)?;
        Ok(DocumentSnapshot {
            document: to_document(document_row),
            version: to_version(version_row),
            chunks: chunk_rows.into_iter().map(to_chunk).collect(),
        })
    }

    async fn find_latest_as_of(
        &self,
        source: &str,
        source_document_id: &str,
        as_of: DateTime<FixedOffset>,
    ) -> Result<Option<DocumentSnapshot>, DocumentStorageRepositoryError> {
        let Some(document_row) = document::Entity::find()
            .filter(document::Column::Source.eq(source))
            .filter(document::Column::SourceDocumentId.eq(source_document_id))
            .one(&self.db)
            .await
            .map_err(repository_error)?
        else {
            return Ok(None);
        };
        let Some(version_row) = document_version::Entity::find()
            .filter(document_version::Column::DocumentId.eq(document_row.id))
            .filter(document_version::Column::RetrievedAt.lte(as_of))
            .order_by_desc(document_version::Column::RetrievedAt)
            .order_by_desc(document_version::Column::CreatedAt)
            .order_by_desc(document_version::Column::Id)
            .one(&self.db)
            .await
            .map_err(repository_error)?
        else {
            return Ok(None);
        };
        let chunk_rows = document_chunk::Entity::find()
            .filter(document_chunk::Column::VersionId.eq(version_row.id))
            .order_by_asc(document_chunk::Column::Sequence)
            .all(&self.db)
            .await
            .map_err(repository_error)?;
        Ok(Some(DocumentSnapshot {
            document: to_document(document_row),
            version: to_version(version_row),
            chunks: chunk_rows.into_iter().map(to_chunk).collect(),
        }))
    }

    async fn find_chunk_context(
        &self,
        chunk_id: Uuid,
        as_of: Option<DateTime<FixedOffset>>,
    ) -> Result<Option<DocumentChunkContext>, DocumentStorageRepositoryError> {
        let Some(target) = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
SELECT
    document.id AS document_id,
    document.source AS document_source,
    document.source_document_id AS document_source_document_id,
    document.stock_id AS document_stock_id,
    document.source_issuer_code AS document_source_issuer_code,
    document.title AS document_title,
    document.published_at AS document_published_at,
    version.id AS version_id,
    version.content_hash AS version_content_hash,
    version.retrieved_at AS version_retrieved_at,
    chunk.id AS chunk_id,
    chunk.version_id AS chunk_version_id,
    chunk.sequence AS chunk_sequence,
    chunk.start_offset AS chunk_start_offset,
    chunk.end_offset AS chunk_end_offset,
    chunk.body AS chunk_body
FROM document_chunk AS chunk
JOIN document_version AS version ON version.id = chunk.version_id
JOIN document ON document.id = version.document_id
WHERE chunk.id = $1
  AND ($2::timestamptz IS NULL OR version.retrieved_at <= $2)
  AND ($2::timestamptz IS NULL OR document.published_at <= $2)
"#,
                [chunk_id.into(), as_of.into()],
            ))
            .await
            .map_err(repository_error)?
        else {
            return Ok(None);
        };
        let target =
            DocumentChunkContextRow::from_query_result(&target, "").map_err(repository_error)?;
        let previous_chunk = document_chunk::Entity::find()
            .filter(document_chunk::Column::VersionId.eq(target.version_id))
            .filter(document_chunk::Column::Sequence.lt(target.chunk_sequence))
            .order_by_desc(document_chunk::Column::Sequence)
            .one(&self.db)
            .await
            .map_err(repository_error)?
            .map(to_chunk);
        let next_chunk = document_chunk::Entity::find()
            .filter(document_chunk::Column::VersionId.eq(target.version_id))
            .filter(document_chunk::Column::Sequence.gt(target.chunk_sequence))
            .order_by_asc(document_chunk::Column::Sequence)
            .one(&self.db)
            .await
            .map_err(repository_error)?
            .map(to_chunk);

        Ok(Some(DocumentChunkContext {
            document: Document {
                id: target.document_id,
                metadata: DocumentMetadata {
                    source: target.document_source,
                    source_document_id: target.document_source_document_id,
                    stock_id: target.document_stock_id,
                    source_issuer_code: target.document_source_issuer_code,
                    title: target.document_title,
                    published_at: target.document_published_at,
                },
            },
            version: DocumentVersionReference {
                id: target.version_id,
                content_hash: target.version_content_hash,
                retrieved_at: target.version_retrieved_at,
            },
            previous_chunk,
            chunk: DocumentChunk {
                id: target.chunk_id,
                version_id: target.chunk_version_id,
                sequence: target.chunk_sequence,
                start_offset: target.chunk_start_offset,
                end_offset: target.chunk_end_offset,
                body: target.chunk_body,
            },
            next_chunk,
        }))
    }
}

#[derive(Debug, FromQueryResult)]
struct DocumentChunkContextRow {
    document_id: Uuid,
    document_source: String,
    document_source_document_id: String,
    document_stock_id: Option<String>,
    document_source_issuer_code: String,
    document_title: String,
    document_published_at: DateTime<FixedOffset>,
    version_id: Uuid,
    version_content_hash: String,
    version_retrieved_at: DateTime<FixedOffset>,
    chunk_id: Uuid,
    chunk_version_id: Uuid,
    chunk_sequence: i32,
    chunk_start_offset: i32,
    chunk_end_offset: i32,
    chunk_body: String,
}

fn to_document(row: document::Model) -> Document {
    Document {
        id: row.id,
        metadata: DocumentMetadata {
            source: row.source,
            source_document_id: row.source_document_id,
            stock_id: row.stock_id,
            source_issuer_code: row.source_issuer_code,
            title: row.title,
            published_at: row.published_at,
        },
    }
}

fn to_version(row: document_version::Model) -> DocumentVersion {
    DocumentVersion {
        id: row.id,
        document_id: row.document_id,
        content_hash: row.content_hash,
        retrieved_at: row.retrieved_at,
        body: row.body,
    }
}

fn to_chunk(row: document_chunk::Model) -> DocumentChunk {
    DocumentChunk {
        id: row.id,
        version_id: row.version_id,
        sequence: row.sequence,
        start_offset: row.start_offset,
        end_offset: row.end_offset,
        body: row.body,
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, DocumentStorageRepositoryError> {
    transaction
        .downcast_ref::<sea_orm::DatabaseTransaction>()
        .ok_or(DocumentStorageRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> DocumentStorageRepositoryError {
    DocumentStorageRepositoryError::Persistence(persistence_error(error))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "DB テストの準備が失敗した場合に原因を表示するため"
    )]

    use chrono::{DateTime, Duration, FixedOffset, Utc};
    use core_application::document_storage::{
        Document, DocumentChunk, DocumentChunkContext, DocumentChunkInput, DocumentMetadata,
        DocumentSnapshot, DocumentStorageRepository, DocumentVersion, DocumentVersionReference,
    };
    use core_application::unit_of_work::UnitOfWork;
    use uuid::Uuid;

    use crate::{DatabaseHandle, unit_of_work::PostgresUnitOfWork};

    use super::PostgresDocumentStorageRepository;

    fn timestamp() -> DateTime<FixedOffset> {
        DateTime::<Utc>::UNIX_EPOCH.fixed_offset()
    }

    fn metadata() -> DocumentMetadata {
        DocumentMetadata {
            source: "archive-example".into(),
            source_document_id: "DOC-42".into(),
            stock_id: None,
            source_issuer_code: "ISSUER-42".into(),
            title: "Synthetic notice".into(),
            published_at: timestamp(),
        }
    }

    fn chunk(
        sequence: i32,
        start_offset: usize,
        end_offset: usize,
        body: &str,
    ) -> DocumentChunkInput {
        DocumentChunkInput {
            sequence,
            start_offset: i32::try_from(start_offset).expect("chunk offset fits in an integer"),
            end_offset: i32::try_from(end_offset).expect("chunk offset fits in an integer"),
            body: body[start_offset..end_offset].to_owned(),
        }
    }

    fn normalize(mut snapshot: DocumentSnapshot) -> DocumentSnapshot {
        snapshot.document.id = Uuid::nil();
        snapshot.version.id = Uuid::nil();
        snapshot.version.document_id = Uuid::nil();
        for chunk in &mut snapshot.chunks {
            chunk.id = Uuid::nil();
            chunk.version_id = Uuid::nil();
        }
        snapshot
    }

    fn normalize_chunk(mut chunk: DocumentChunk) -> DocumentChunk {
        chunk.id = Uuid::nil();
        chunk.version_id = Uuid::nil();
        chunk
    }

    fn expected_snapshot(
        body: &str,
        content_hash: &str,
        retrieved_at: DateTime<FixedOffset>,
    ) -> DocumentSnapshot {
        expected_snapshot_with_chunks(
            body,
            content_hash,
            retrieved_at,
            vec![expected_chunk(
                0,
                0,
                i32::try_from(body.len()).expect("body length fits in an integer"),
                body,
            )],
        )
    }

    fn expected_chunk(
        sequence: i32,
        start_offset: i32,
        end_offset: i32,
        body: &str,
    ) -> DocumentChunk {
        DocumentChunk {
            id: Uuid::nil(),
            version_id: Uuid::nil(),
            sequence,
            start_offset,
            end_offset,
            body: body.to_owned(),
        }
    }

    fn expected_snapshot_with_chunks(
        body: &str,
        content_hash: &str,
        retrieved_at: DateTime<FixedOffset>,
        chunks: Vec<DocumentChunk>,
    ) -> DocumentSnapshot {
        DocumentSnapshot {
            document: Document {
                id: Uuid::nil(),
                metadata: metadata(),
            },
            version: DocumentVersion {
                id: Uuid::nil(),
                document_id: Uuid::nil(),
                content_hash: content_hash.into(),
                retrieved_at,
                body: body.into(),
            },
            chunks,
        }
    }

    async fn save(
        repository: &PostgresDocumentStorageRepository,
        unit_of_work: &PostgresUnitOfWork,
        metadata: DocumentMetadata,
        retrieved_at: DateTime<FixedOffset>,
        body: &str,
        content_hash: &str,
        chunks: Vec<DocumentChunkInput>,
    ) -> DocumentSnapshot {
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        let snapshot = repository
            .save(
                &transaction,
                metadata,
                content_hash.to_owned(),
                retrieved_at,
                body.to_owned(),
                chunks,
            )
            .await
            .expect("document snapshot saves");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");
        snapshot
    }

    #[backend_test_macros::database_test]
    async fn repository_deduplicates_unchanged_content(db: DatabaseHandle) {
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let repository = PostgresDocumentStorageRepository::new(db);
        let first_body = "alpha beta";
        let changed_body = "alpha gamma";
        let first_hash = "a".repeat(64);
        let changed_hash = "b".repeat(64);
        let first = save(
            &repository,
            &unit_of_work,
            metadata(),
            timestamp(),
            first_body,
            &first_hash,
            vec![chunk(0, 0, 5, first_body), chunk(1, 5, 10, first_body)],
        )
        .await;
        let repeated = save(
            &repository,
            &unit_of_work,
            metadata(),
            timestamp() + Duration::hours(2),
            first_body,
            &first_hash,
            vec![chunk(0, 0, 10, first_body)],
        )
        .await;
        let changed = save(
            &repository,
            &unit_of_work,
            metadata(),
            timestamp() + Duration::hours(3),
            changed_body,
            &changed_hash,
            vec![chunk(0, 0, 11, changed_body)],
        )
        .await;
        let repeated_content_reuses_id = first.version.id == repeated.version.id;
        let changed_content_has_new_id = first.version.id != changed.version.id;
        let repeated_content_keeps_chunk_ids = first
            .chunks
            .iter()
            .map(|chunk| chunk.id)
            .collect::<Vec<_>>()
            == repeated
                .chunks
                .iter()
                .map(|chunk| chunk.id)
                .collect::<Vec<_>>();

        assert_eq!(
            (
                normalize(first),
                normalize(repeated),
                normalize(changed),
                repeated_content_reuses_id,
                repeated_content_keeps_chunk_ids,
                changed_content_has_new_id,
            ),
            (
                expected_snapshot_with_chunks(
                    first_body,
                    &first_hash,
                    timestamp(),
                    vec![
                        expected_chunk(0, 0, 5, "alpha"),
                        expected_chunk(1, 5, 10, " beta"),
                    ],
                ),
                expected_snapshot_with_chunks(
                    first_body,
                    &first_hash,
                    timestamp(),
                    vec![
                        expected_chunk(0, 0, 5, "alpha"),
                        expected_chunk(1, 5, 10, " beta"),
                    ],
                ),
                expected_snapshot(
                    changed_body,
                    &changed_hash,
                    timestamp() + Duration::hours(3)
                ),
                true,
                true,
                true,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn repository_reads_each_content_as_of_its_latest_retrieval(db: DatabaseHandle) {
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let repository = PostgresDocumentStorageRepository::new(db);
        let first_body = "alpha beta";
        let changed_body = "alpha gamma";
        let first_hash = "a".repeat(64);
        let changed_hash = "b".repeat(64);
        let first = save(
            &repository,
            &unit_of_work,
            metadata(),
            timestamp(),
            first_body,
            &first_hash,
            vec![chunk(0, 0, 10, first_body)],
        )
        .await;
        save(
            &repository,
            &unit_of_work,
            metadata(),
            timestamp() + Duration::hours(2),
            changed_body,
            &changed_hash,
            vec![chunk(0, 0, 11, changed_body)],
        )
        .await;
        let reverted = save(
            &repository,
            &unit_of_work,
            metadata(),
            timestamp() + Duration::hours(4),
            first_body,
            &first_hash,
            vec![chunk(0, 0, 10, first_body)],
        )
        .await;
        let before_change = repository
            .find_latest_as_of(
                "archive-example",
                "DOC-42",
                timestamp() + Duration::hours(1),
            )
            .await
            .expect("version lookup succeeds");
        let after_change = repository
            .find_latest_as_of(
                "archive-example",
                "DOC-42",
                timestamp() + Duration::hours(3),
            )
            .await
            .expect("version lookup succeeds");
        let after_reversion = repository
            .find_latest_as_of(
                "archive-example",
                "DOC-42",
                timestamp() + Duration::hours(5),
            )
            .await
            .expect("version lookup succeeds");

        assert_eq!(
            (
                first.version.id == reverted.version.id,
                before_change.map(normalize),
                after_change.map(normalize),
                after_reversion.map(normalize),
            ),
            (
                false,
                Some(expected_snapshot(first_body, &first_hash, timestamp())),
                Some(expected_snapshot(
                    changed_body,
                    &changed_hash,
                    timestamp() + Duration::hours(2)
                )),
                Some(expected_snapshot(
                    first_body,
                    &first_hash,
                    timestamp() + Duration::hours(4)
                )),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn repository_reads_chunk_context_only_when_visible_as_of(db: DatabaseHandle) {
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let repository = PostgresDocumentStorageRepository::new(db);
        let first_body = "alpha beta gamma";
        let first = save(
            &repository,
            &unit_of_work,
            metadata(),
            timestamp(),
            first_body,
            &"a".repeat(64),
            vec![
                chunk(0, 0, 5, first_body),
                chunk(1, 6, 10, first_body),
                chunk(2, 11, 16, first_body),
            ],
        )
        .await;
        let later_body = "alpha delta";
        let later = save(
            &repository,
            &unit_of_work,
            metadata(),
            timestamp() + Duration::hours(2),
            later_body,
            &"b".repeat(64),
            vec![chunk(0, 0, 11, later_body)],
        )
        .await;
        let mut future_publication_metadata = metadata();
        future_publication_metadata.source_document_id = "DOC-FUTURE-42".into();
        future_publication_metadata.published_at = timestamp() + Duration::hours(2);
        let future_publication_body = "future document";
        let future_published = save(
            &repository,
            &unit_of_work,
            future_publication_metadata,
            timestamp(),
            future_publication_body,
            &"c".repeat(64),
            vec![chunk(
                0,
                0,
                future_publication_body.len(),
                future_publication_body,
            )],
        )
        .await;

        let visible = repository
            .find_chunk_context(first.chunks[1].id, Some(timestamp() + Duration::hours(1)))
            .await
            .expect("context query succeeds");
        let hidden = repository
            .find_chunk_context(later.chunks[0].id, Some(timestamp() + Duration::hours(1)))
            .await
            .expect("context query succeeds");
        let future_publication_hidden = repository
            .find_chunk_context(
                future_published.chunks[0].id,
                Some(timestamp() + Duration::hours(1)),
            )
            .await
            .expect("context query succeeds");
        let normalized = visible.map(|mut context| {
            context.document.id = Uuid::nil();
            context.version.id = Uuid::nil();
            context.chunk = normalize_chunk(context.chunk);
            context.previous_chunk = context.previous_chunk.map(normalize_chunk);
            context.next_chunk = context.next_chunk.map(normalize_chunk);
            context
        });
        let mut expected_document = first.document;
        expected_document.id = Uuid::nil();
        let mut expected_version = DocumentVersionReference {
            id: first.version.id,
            content_hash: first.version.content_hash.clone(),
            retrieved_at: first.version.retrieved_at,
        };
        expected_version.id = Uuid::nil();

        assert_eq!(
            (normalized, hidden, future_publication_hidden),
            (
                Some(DocumentChunkContext {
                    document: expected_document,
                    version: expected_version,
                    previous_chunk: Some(normalize_chunk(first.chunks[0].clone())),
                    chunk: normalize_chunk(first.chunks[1].clone()),
                    next_chunk: Some(normalize_chunk(first.chunks[2].clone())),
                }),
                None,
                None,
            ),
        );
    }
}
