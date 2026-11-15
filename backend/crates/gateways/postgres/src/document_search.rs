use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use core_application::document_search::{
    DocumentSearchRepository, DocumentSearchRepositoryError, DocumentSearchResult,
};
use core_domain::document_search::PreparedDocumentSearch;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::persistence::persistence_error;

const SEARCH_DOCUMENTS_SQL: &str = r#"
SELECT
    document.id AS document_id,
    document.source AS document_source,
    document.source_document_id AS document_source_document_id,
    document.stock_id AS document_stock_id,
    document.source_issuer_code AS document_source_issuer_code,
    document.title AS document_title,
    document.published_at AS document_published_at,
    version.id AS version_id,
    version.document_id AS version_document_id,
    version.content_hash AS version_content_hash,
    version.retrieved_at AS version_retrieved_at,
    version.body AS version_body,
    chunk.id AS chunk_id,
    chunk.version_id AS chunk_version_id,
    chunk.sequence AS chunk_sequence,
    chunk.start_offset AS chunk_start_offset,
    chunk.end_offset AS chunk_end_offset,
    chunk.body AS chunk_body
FROM document
JOIN LATERAL (
    SELECT document_version.*
    FROM document_version
    WHERE document_version.document_id = document.id
      AND ($3::timestamptz IS NULL OR document_version.retrieved_at <= $3)
    ORDER BY document_version.retrieved_at DESC,
             document_version.created_at DESC,
             document_version.id DESC
    LIMIT 1
) AS version ON TRUE
JOIN document_chunk AS chunk ON chunk.version_id = version.id
WHERE chunk.search_terms @> $1::text[]
  AND strpos(chunk.search_text, $2::text) > 0
  AND ($3::timestamptz IS NULL OR document.published_at <= $3)
  AND ($4::text IS NULL OR document.stock_id = $4)
ORDER BY document.published_at DESC,
         document.source,
         document.source_document_id,
         chunk.sequence
LIMIT $5
"#;

#[derive(Debug, FromQueryResult)]
struct DocumentSearchRow {
    document_id: Uuid,
    document_source: String,
    document_source_document_id: String,
    document_stock_id: Option<String>,
    document_source_issuer_code: String,
    document_title: String,
    document_published_at: DateTime<FixedOffset>,
    version_id: Uuid,
    version_document_id: Uuid,
    version_content_hash: String,
    version_retrieved_at: DateTime<FixedOffset>,
    version_body: String,
    chunk_id: Uuid,
    chunk_version_id: Uuid,
    chunk_sequence: i32,
    chunk_start_offset: i32,
    chunk_end_offset: i32,
    chunk_body: String,
}

#[derive(Clone)]
pub struct PostgresDocumentSearchRepository {
    db: DatabaseHandle,
}

impl PostgresDocumentSearchRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }

    async fn search_prepared(
        &self,
        query: PreparedDocumentSearch,
        as_of: Option<DateTime<FixedOffset>>,
        stock_id: Option<String>,
        limit: u32,
    ) -> Result<Vec<DocumentSearchResult>, DocumentSearchRepositoryError> {
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                SEARCH_DOCUMENTS_SQL,
                [
                    query.terms.into(),
                    query.normalized_text.into(),
                    as_of.into(),
                    stock_id.into(),
                    i64::from(limit).into(),
                ],
            ))
            .await
            .map_err(repository_error)?;
        rows.iter()
            .map(|row| {
                DocumentSearchRow::from_query_result(row, "")
                    .map(Into::into)
                    .map_err(repository_error)
            })
            .collect()
    }
}

#[async_trait]
impl DocumentSearchRepository for PostgresDocumentSearchRepository {
    async fn search(
        &self,
        query: PreparedDocumentSearch,
        as_of: Option<DateTime<FixedOffset>>,
        stock_id: Option<String>,
        limit: u32,
    ) -> Result<Vec<DocumentSearchResult>, DocumentSearchRepositoryError> {
        self.search_prepared(query, as_of, stock_id, limit).await
    }
}

impl From<DocumentSearchRow> for DocumentSearchResult {
    fn from(row: DocumentSearchRow) -> Self {
        Self {
            document: core_application::document_storage::Document {
                id: row.document_id,
                metadata: core_application::document_storage::DocumentMetadata {
                    source: row.document_source,
                    source_document_id: row.document_source_document_id,
                    stock_id: row.document_stock_id,
                    source_issuer_code: row.document_source_issuer_code,
                    title: row.document_title,
                    published_at: row.document_published_at,
                },
            },
            version: core_application::document_storage::DocumentVersion {
                id: row.version_id,
                document_id: row.version_document_id,
                content_hash: row.version_content_hash,
                retrieved_at: row.version_retrieved_at,
                body: row.version_body,
            },
            chunk: core_application::document_storage::DocumentChunk {
                id: row.chunk_id,
                version_id: row.chunk_version_id,
                sequence: row.chunk_sequence,
                start_offset: row.chunk_start_offset,
                end_offset: row.chunk_end_offset,
                body: row.chunk_body,
            },
        }
    }
}

fn repository_error(error: sea_orm::DbErr) -> DocumentSearchRepositoryError {
    DocumentSearchRepositoryError::Persistence(persistence_error(error))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "固定した合成データによる DB test の準備失敗を明示するため"
    )]

    use chrono::{DateTime, Duration, FixedOffset, Utc};
    use core_application::{
        document_search::{DocumentSearchRepository, DocumentSearchResult},
        document_storage::{
            Document, DocumentChunk, DocumentChunkInput, DocumentMetadata,
            DocumentStorageRepository, DocumentVersion,
        },
        unit_of_work::UnitOfWork,
    };
    use sea_orm::ConnectionTrait;
    use uuid::Uuid;

    use crate::{DatabaseHandle, unit_of_work::PostgresUnitOfWork};

    use super::PostgresDocumentSearchRepository;
    use crate::document_storage::PostgresDocumentStorageRepository;

    fn timestamp() -> DateTime<FixedOffset> {
        DateTime::<Utc>::UNIX_EPOCH.fixed_offset()
    }

    fn metadata(
        source_document_id: &str,
        stock_id: Option<&str>,
        published_at: DateTime<FixedOffset>,
    ) -> DocumentMetadata {
        DocumentMetadata {
            source: "synthetic-source".into(),
            source_document_id: source_document_id.into(),
            stock_id: stock_id.map(str::to_owned),
            source_issuer_code: "synthetic-issuer".into(),
            title: "Synthetic document".into(),
            published_at,
        }
    }

    fn chunk(body: &str) -> DocumentChunkInput {
        DocumentChunkInput {
            sequence: 0,
            start_offset: 0,
            end_offset: i32::try_from(body.len()).expect("synthetic body fits in an integer"),
            body: body.into(),
        }
    }

    async fn save(
        storage: &PostgresDocumentStorageRepository,
        unit_of_work: &PostgresUnitOfWork,
        metadata: DocumentMetadata,
        retrieved_at: DateTime<FixedOffset>,
        body: &str,
        content_hash: &str,
    ) {
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        storage
            .save(
                &transaction,
                metadata,
                content_hash.into(),
                retrieved_at,
                body.into(),
                vec![chunk(body)],
            )
            .await
            .expect("document version saves");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");
    }

    fn normalize(mut results: Vec<DocumentSearchResult>) -> Vec<DocumentSearchResult> {
        for result in &mut results {
            result.document.id = Uuid::nil();
            result.version.id = Uuid::nil();
            result.version.document_id = Uuid::nil();
            result.chunk.id = Uuid::nil();
            result.chunk.version_id = Uuid::nil();
        }
        results
    }

    fn expected_result(
        source_document_id: &str,
        stock_id: Option<&str>,
        published_at: DateTime<FixedOffset>,
        retrieved_at: DateTime<FixedOffset>,
        body: &str,
        content_hash: &str,
    ) -> DocumentSearchResult {
        DocumentSearchResult {
            document: Document {
                id: Uuid::nil(),
                metadata: metadata(source_document_id, stock_id, published_at),
            },
            version: DocumentVersion {
                id: Uuid::nil(),
                document_id: Uuid::nil(),
                content_hash: content_hash.into(),
                retrieved_at,
                body: body.into(),
            },
            chunk: DocumentChunk {
                id: Uuid::nil(),
                version_id: Uuid::nil(),
                sequence: 0,
                start_offset: 0,
                end_offset: i32::try_from(body.len()).expect("synthetic body fits in an integer"),
                body: body.into(),
            },
        }
    }

    #[backend_test_macros::database_test]
    async fn repository_searches_normalized_phrases_and_filters_by_stock_id(db: DatabaseHandle) {
        db.execute_unprepared(
            "INSERT INTO stock (id, name, created_at, updated_at) VALUES ('demo-code', 'Synthetic organization', now(), now()), ('demo-other', 'Another synthetic organization', now(), now()) ON CONFLICT (id) DO NOTHING",
        )
        .await
        .expect("synthetic stock rows insert");
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let storage = PostgresDocumentStorageRepository::new(db.clone());
        let repository = PostgresDocumentSearchRepository::new(db);
        let matching_body = "xxＡＢＡＢyy";
        let other_matching_body = "abab";
        let ngram_collision_body = "baba";
        save(
            &storage,
            &unit_of_work,
            metadata("DOC-A", Some("demo-code"), timestamp()),
            timestamp(),
            matching_body,
            "hash-a",
        )
        .await;
        save(
            &storage,
            &unit_of_work,
            metadata("DOC-B", Some("demo-other"), timestamp()),
            timestamp(),
            other_matching_body,
            "hash-b",
        )
        .await;
        save(
            &storage,
            &unit_of_work,
            metadata("DOC-C", Some("demo-code"), timestamp()),
            timestamp(),
            ngram_collision_body,
            "hash-c",
        )
        .await;

        let results = repository
            .search(
                core_domain::document_search::prepare_document_search("ＡＢＡＢ"),
                None,
                Some("demo-code".into()),
                10,
            )
            .await
            .expect("document search succeeds");

        assert_eq!(
            normalize(results),
            vec![expected_result(
                "DOC-A",
                Some("demo-code"),
                timestamp(),
                timestamp(),
                matching_body,
                "hash-a",
            )],
        );
    }

    #[backend_test_macros::database_test]
    async fn repository_searches_only_published_latest_versions_as_of_time(db: DatabaseHandle) {
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let storage = PostgresDocumentStorageRepository::new(db.clone());
        let repository = PostgresDocumentSearchRepository::new(db);
        let published_at = timestamp();
        let later = timestamp() + Duration::hours(2);
        let query = core_domain::document_search::prepare_document_search("synthetic phrase");
        save(
            &storage,
            &unit_of_work,
            metadata("DOC-A", None, published_at),
            published_at,
            "synthetic phrase in an earlier version",
            "hash-a1",
        )
        .await;
        save(
            &storage,
            &unit_of_work,
            metadata("DOC-A", None, published_at),
            later,
            "replacement content",
            "hash-a2",
        )
        .await;
        save(
            &storage,
            &unit_of_work,
            metadata("DOC-B", None, later),
            later,
            "synthetic phrase in a newly published document",
            "hash-b",
        )
        .await;
        save(
            &storage,
            &unit_of_work,
            metadata("DOC-C", None, published_at),
            later,
            "synthetic phrase in a later retrieval",
            "hash-c",
        )
        .await;

        let before_later_retrieval = normalize(
            repository
                .search(
                    query.clone(),
                    Some(timestamp() + Duration::hours(1)),
                    None,
                    10,
                )
                .await
                .expect("as-of search succeeds"),
        );
        let after_later_retrieval = normalize(
            repository
                .search(query, Some(timestamp() + Duration::hours(3)), None, 10)
                .await
                .expect("as-of search succeeds"),
        );

        assert_eq!(
            (before_later_retrieval, after_later_retrieval),
            (
                vec![expected_result(
                    "DOC-A",
                    None,
                    published_at,
                    published_at,
                    "synthetic phrase in an earlier version",
                    "hash-a1",
                )],
                vec![
                    expected_result(
                        "DOC-B",
                        None,
                        later,
                        later,
                        "synthetic phrase in a newly published document",
                        "hash-b",
                    ),
                    expected_result(
                        "DOC-C",
                        None,
                        published_at,
                        later,
                        "synthetic phrase in a later retrieval",
                        "hash-c",
                    ),
                ],
            ),
        );
    }
}
