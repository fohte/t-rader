use chrono::{DateTime, FixedOffset};
use sha2::{Digest, Sha256};

use crate::unit_of_work::SharedUnitOfWork;

use super::error::DocumentStorageUseCaseError;
use super::repository::SharedDocumentStorageRepository;
use super::types::{DocumentChunkContext, DocumentChunkInput, DocumentMetadata, DocumentSnapshot};

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

    pub async fn get_chunk_context(
        &self,
        chunk_id: uuid::Uuid,
        as_of: Option<DateTime<FixedOffset>>,
    ) -> Result<Option<DocumentChunkContext>, DocumentStorageUseCaseError> {
        self.repository
            .find_chunk_context(chunk_id, as_of)
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

    use chrono::{DateTime, FixedOffset, Utc};
    use rstest::{fixture, rstest};
    use uuid::Uuid;

    use crate::{
        document_storage::{
            Document, DocumentChunk, DocumentChunkContext, DocumentChunkInput, DocumentMetadata,
            DocumentSnapshot, DocumentStorageUseCases, DocumentVersion, DocumentVersionReference,
            FakeDocumentStorageRepository,
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

    fn expected_snapshot(
        body: &str,
        content_hash: &str,
        retrieved_at: DateTime<FixedOffset>,
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
    async fn hashes_body_when_saving(
        harness: (DocumentStorageUseCases, Arc<FakeDocumentStorageRepository>),
    ) {
        let (use_cases, _) = harness;
        let body = "alpha beta";
        let snapshot = use_cases
            .save(
                metadata(),
                timestamp(),
                body.into(),
                vec![chunk(0, body, 0, 10)],
            )
            .await
            .unwrap();

        assert_eq!(
            normalize(snapshot),
            expected_snapshot(
                body,
                "1a989ea86150171c687b0727f218eedbb94c4665a7da9b0add1bf5de607f2bf1",
                timestamp(),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn reads_chunk_with_neighbors_from_its_version(
        harness: (DocumentStorageUseCases, Arc<FakeDocumentStorageRepository>),
    ) {
        let (use_cases, _) = harness;
        let body = "alpha beta gamma";
        let snapshot = use_cases
            .save(
                metadata(),
                timestamp(),
                body.into(),
                vec![
                    chunk(0, body, 0, 5),
                    chunk(1, body, 6, 10),
                    chunk(2, body, 11, 16),
                ],
            )
            .await
            .unwrap();

        let mut context = use_cases
            .get_chunk_context(snapshot.chunks[1].id, Some(timestamp()))
            .await
            .unwrap()
            .unwrap();
        let normalize_chunk = |mut chunk: DocumentChunk| {
            chunk.id = Uuid::nil();
            chunk.version_id = Uuid::nil();
            chunk
        };
        let mut document = snapshot.document;
        document.id = Uuid::nil();
        let mut version = DocumentVersionReference {
            id: snapshot.version.id,
            content_hash: snapshot.version.content_hash.clone(),
            retrieved_at: snapshot.version.retrieved_at,
        };
        version.id = Uuid::nil();
        context.document.id = Uuid::nil();
        context.version.id = Uuid::nil();
        context.previous_chunk = context.previous_chunk.map(normalize_chunk);
        context.chunk = normalize_chunk(context.chunk);
        context.next_chunk = context.next_chunk.map(normalize_chunk);

        assert_eq!(
            context,
            DocumentChunkContext {
                document,
                version,
                previous_chunk: Some(normalize_chunk(snapshot.chunks[0].clone())),
                chunk: normalize_chunk(snapshot.chunks[1].clone()),
                next_chunk: Some(normalize_chunk(snapshot.chunks[2].clone())),
            },
        );
    }

    #[rstest]
    #[case::source("source")]
    #[case::source_document_id("source_document_id")]
    #[case::source_issuer_code("source_issuer_code")]
    #[case::title("title")]
    #[tokio::test]
    async fn rejects_blank_metadata_before_saving(
        #[case] field: &str,
        harness: (DocumentStorageUseCases, Arc<FakeDocumentStorageRepository>),
    ) {
        let (use_cases, repository) = harness;
        let mut metadata = metadata();
        match field {
            "source" => metadata.source = "   ".into(),
            "source_document_id" => metadata.source_document_id = "   ".into(),
            "source_issuer_code" => metadata.source_issuer_code = "   ".into(),
            "title" => metadata.title = "   ".into(),
            _ => {}
        }
        let result = use_cases
            .save(metadata, timestamp(), String::new(), Vec::new())
            .await;

        assert_eq!(
            (
                result.err().map(|error| error.to_string()),
                repository.version_count().await,
            ),
            (
                Some(format!(
                    "document metadata field '{field}' must not be empty"
                )),
                0,
            ),
        );
    }

    fn chunk_from_range(sequence: i32, start: usize, end: usize, body: &str) -> DocumentChunkInput {
        DocumentChunkInput {
            sequence,
            start_offset: i32::try_from(start).expect("test offset fits in an integer"),
            end_offset: i32::try_from(end).expect("test offset fits in an integer"),
            body: body[start..end].into(),
        }
    }

    #[rstest]
    #[case::negative_sequence(
        "negative sequence",
        "invalid document chunk at sequence -1: sequences must be non-negative and strictly increasing"
    )]
    #[case::repeated_sequence(
        "repeated sequence",
        "invalid document chunk at sequence 0: sequences must be non-negative and strictly increasing"
    )]
    #[case::overlapping_offsets(
        "overlapping offsets",
        "invalid document chunk at sequence 1: offsets must be non-negative, ordered, and non-overlapping"
    )]
    #[tokio::test]
    async fn rejects_invalid_chunk_order_and_offsets(
        #[case] invalid_case: &str,
        #[case] expected_error: &str,
        harness: (DocumentStorageUseCases, Arc<FakeDocumentStorageRepository>),
    ) {
        let (use_cases, repository) = harness;
        let body = "abcdef";
        let chunks = match invalid_case {
            "negative sequence" => vec![chunk_from_range(-1, 0, 3, body)],
            "repeated sequence" => vec![
                chunk_from_range(0, 0, 3, body),
                chunk_from_range(0, 3, 6, body),
            ],
            "overlapping offsets" => vec![
                chunk_from_range(0, 0, 4, body),
                chunk_from_range(1, 3, 5, body),
            ],
            _ => Vec::new(),
        };
        let result = use_cases
            .save(metadata(), timestamp(), body.into(), chunks)
            .await;

        assert_eq!(
            (
                result.err().map(|error| error.to_string()),
                repository.version_count().await,
            ),
            (Some(expected_error.into()), 0),
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
