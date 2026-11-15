use chrono::{DateTime, FixedOffset};
use sha2::{Digest, Sha256};

use crate::unit_of_work::SharedUnitOfWork;

use super::error::DocumentStorageUseCaseError;
use super::repository::SharedDocumentStorageRepository;
use super::types::{DocumentChunkInput, DocumentMetadata, DocumentSnapshot};

#[derive(Clone)]
pub struct DocumentStorageUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedDocumentStorageRepository,
}

impl DocumentStorageUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedDocumentStorageRepository,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
        }
    }

    pub async fn save(
        &self,
        metadata: DocumentMetadata,
        retrieved_at: DateTime<FixedOffset>,
        body: String,
        chunks: Vec<DocumentChunkInput>,
    ) -> Result<DocumentSnapshot, DocumentStorageUseCaseError> {
        validate_metadata(&metadata)?;
        validate_chunks(&body, &chunks)?;
        let content_hash = hash_body(&body);
        let transaction = self.unit_of_work.begin().await?;
        let snapshot = self
            .repository
            .save(
                &transaction,
                metadata,
                content_hash,
                retrieved_at,
                body,
                chunks,
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(snapshot)
    }

    pub async fn get_latest_as_of(
        &self,
        source: &str,
        source_document_id: &str,
        as_of: DateTime<FixedOffset>,
    ) -> Result<Option<DocumentSnapshot>, DocumentStorageUseCaseError> {
        self.repository
            .find_latest_as_of(source, source_document_id, as_of)
            .await
            .map_err(Into::into)
    }
}

fn hash_body(body: &str) -> String {
    Sha256::digest(body.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn validate_metadata(metadata: &DocumentMetadata) -> Result<(), DocumentStorageUseCaseError> {
    for (field, value) in [
        ("source", metadata.source.as_str()),
        ("source_document_id", metadata.source_document_id.as_str()),
        ("source_issuer_code", metadata.source_issuer_code.as_str()),
        ("title", metadata.title.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(DocumentStorageUseCaseError::EmptyMetadata(field));
        }
    }
    Ok(())
}

fn validate_chunks(
    body: &str,
    chunks: &[DocumentChunkInput],
) -> Result<(), DocumentStorageUseCaseError> {
    let mut previous_sequence = None;
    let mut previous_end = 0;
    for chunk in chunks {
        let invalid = |reason| DocumentStorageUseCaseError::InvalidChunk {
            sequence: chunk.sequence,
            reason,
        };
        if chunk.sequence < 0
            || previous_sequence.is_some_and(|previous| chunk.sequence <= previous)
        {
            return Err(invalid(
                "sequences must be non-negative and strictly increasing",
            ));
        }
        if chunk.start_offset < 0
            || chunk.end_offset < chunk.start_offset
            || (chunk.sequence > 0 && chunk.start_offset < previous_end)
        {
            return Err(invalid(
                "offsets must be non-negative, ordered, and non-overlapping",
            ));
        }
        let start =
            usize::try_from(chunk.start_offset).map_err(|_| invalid("start offset is invalid"))?;
        let end =
            usize::try_from(chunk.end_offset).map_err(|_| invalid("end offset is invalid"))?;
        if body.get(start..end) != Some(chunk.body.as_str()) {
            return Err(invalid(
                "body must match the UTF-8 byte range in the original body",
            ));
        }
        previous_sequence = Some(chunk.sequence);
        previous_end = chunk.end_offset;
    }
    Ok(())
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "固定した合成本文の byte offset をテスト入力に変換するため"
    )]

    use std::sync::Arc;

    use chrono::{DateTime, Duration, FixedOffset, Utc};
    use rstest::{fixture, rstest};
    use uuid::Uuid;

    use crate::{
        document_storage::{
            Document, DocumentChunk, DocumentChunkInput, DocumentMetadata, DocumentSnapshot,
            DocumentStorageUseCases, DocumentVersion, FakeDocumentStorageRepository,
        },
        unit_of_work::FakeUnitOfWork,
    };

    fn timestamp() -> DateTime<FixedOffset> {
        DateTime::<Utc>::UNIX_EPOCH.fixed_offset()
    }

    #[fixture]
    fn harness() -> (DocumentStorageUseCases, Arc<FakeDocumentStorageRepository>) {
        let repository = Arc::new(FakeDocumentStorageRepository::default());
        let use_cases =
            DocumentStorageUseCases::new(Arc::new(FakeUnitOfWork::new()), repository.clone());
        (use_cases, repository)
    }

    fn metadata() -> DocumentMetadata {
        DocumentMetadata {
            source: "archive-example".into(),
            source_document_id: "DOC-42".into(),
            stock_id: Some("demo-code".into()),
            source_issuer_code: "ISSUER-42".into(),
            title: "Synthetic notice".into(),
            published_at: timestamp(),
        }
    }

    fn chunk(sequence: i32, body: &str, start: usize, end: usize) -> DocumentChunkInput {
        DocumentChunkInput {
            sequence,
            start_offset: i32::try_from(start).expect("test offset fits in an integer"),
            end_offset: i32::try_from(end).expect("test offset fits in an integer"),
            body: body[start..end].into(),
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

    fn expected_snapshot(body: &str, retrieved_at: DateTime<FixedOffset>) -> DocumentSnapshot {
        DocumentSnapshot {
            document: Document {
                id: Uuid::nil(),
                metadata: metadata(),
            },
            version: DocumentVersion {
                id: Uuid::nil(),
                document_id: Uuid::nil(),
                content_hash: super::hash_body(body),
                retrieved_at,
                body: body.into(),
            },
            chunks: vec![DocumentChunk {
                id: Uuid::nil(),
                version_id: Uuid::nil(),
                sequence: 0,
                start_offset: 0,
                end_offset: i32::try_from(body.len()).expect("test body length fits in an integer"),
                body: body.into(),
            }],
        }
    }

    #[rstest]
    #[tokio::test]
    async fn reusing_body_keeps_one_version(
        harness: (DocumentStorageUseCases, Arc<FakeDocumentStorageRepository>),
    ) {
        let (use_cases, repository) = harness;
        let body = "alpha beta";
        let first = use_cases
            .save(
                metadata(),
                timestamp(),
                body.into(),
                vec![chunk(0, body, 0, 10)],
            )
            .await
            .unwrap();
        let second = use_cases
            .save(
                metadata(),
                timestamp() + Duration::hours(2),
                body.into(),
                vec![chunk(0, body, 0, 10)],
            )
            .await
            .unwrap();

        assert_eq!(
            (
                normalize(first),
                normalize(second),
                repository.version_count().await,
            ),
            (
                expected_snapshot(body, timestamp()),
                expected_snapshot(body, timestamp()),
                1,
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn reads_latest_version_and_chunks_available_by_cutoff(
        harness: (DocumentStorageUseCases, Arc<FakeDocumentStorageRepository>),
    ) {
        let (use_cases, _) = harness;
        let first_body = "alpha beta";
        let second_body = "alpha gamma";
        use_cases
            .save(
                metadata(),
                timestamp(),
                first_body.into(),
                vec![chunk(0, first_body, 0, 10)],
            )
            .await
            .unwrap();
        use_cases
            .save(
                metadata(),
                timestamp() + Duration::hours(2),
                second_body.into(),
                vec![chunk(0, second_body, 0, 11)],
            )
            .await
            .unwrap();

        let snapshot = use_cases
            .get_latest_as_of(
                "archive-example",
                "DOC-42",
                timestamp() + Duration::hours(1),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            normalize(snapshot),
            expected_snapshot(first_body, timestamp()),
        );
    }

    #[rstest]
    #[case::rejects_mismatched_chunk(
        "abcdef",
        0,
        3,
        "xyz",
        "body must match the UTF-8 byte range in the original body"
    )]
    #[case::rejects_mid_codepoint(
        "日本語",
        1,
        3,
        "本",
        "body must match the UTF-8 byte range in the original body"
    )]
    #[tokio::test]
    async fn rejects_chunk_that_does_not_reference_original_body(
        #[case] body: &str,
        #[case] start: i32,
        #[case] end: i32,
        #[case] chunk_body: &str,
        #[case] expected_reason: &str,
        harness: (DocumentStorageUseCases, Arc<FakeDocumentStorageRepository>),
    ) {
        let (use_cases, repository) = harness;
        let result = use_cases
            .save(
                metadata(),
                timestamp(),
                body.into(),
                vec![DocumentChunkInput {
                    sequence: 0,
                    start_offset: start,
                    end_offset: end,
                    body: chunk_body.into(),
                }],
            )
            .await;

        assert_eq!(
            (
                result.err().map(|error| error.to_string()),
                repository.version_count().await,
            ),
            (
                Some(format!(
                    "invalid document chunk at sequence 0: {expected_reason}"
                )),
                0,
            ),
        );
    }
}
