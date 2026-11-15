#![allow(
    clippy::expect_used,
    reason = "use case test harness setup failures should report their cause"
)]

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rstest::{fixture, rstest};
use uuid::Uuid;

use crate::document_storage::DocumentChunk;
use crate::unit_of_work::{
    FakeTransaction, FakeUnitOfWork, SharedUnitOfWork, UnitOfWorkTransaction,
};

use super::super::{
    DocumentMention, DocumentMentionRepository, DocumentMentionRepositoryError, RegisteredStock,
};
use super::DocumentMentionUseCases;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RepositoryOperation {
    LockChunksForUpdate,
    FindRegisteredStocks,
    ReplaceMentions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepositoryCall {
    operation: RepositoryOperation,
    transaction_id: Uuid,
    chunk_ids: Vec<Uuid>,
    mentions: Vec<DocumentMention>,
}

#[derive(Debug, PartialEq, Eq)]
struct ExpectedRepositoryCall {
    operation: RepositoryOperation,
    transaction_index: usize,
    chunk_ids: Vec<Uuid>,
    mentions: Vec<DocumentMention>,
}

#[derive(Debug, PartialEq, Eq)]
struct RepositoryFlow {
    calls: Vec<ExpectedRepositoryCall>,
    begun_transactions: Vec<usize>,
    committed_transactions: Vec<usize>,
}

#[derive(Default)]
struct RecordingRepository {
    stocks: Mutex<Vec<RegisteredStock>>,
    calls: Mutex<Vec<RepositoryCall>>,
}

impl RecordingRepository {
    fn set_stocks(&self, stocks: Vec<RegisteredStock>) {
        *self.stocks.lock().expect("stock lock is available") = stocks;
    }

    fn record_call(
        &self,
        operation: RepositoryOperation,
        transaction: &UnitOfWorkTransaction,
        chunk_ids: Vec<Uuid>,
        mentions: Vec<DocumentMention>,
    ) {
        let transaction_id = transaction
            .downcast_ref::<FakeTransaction>()
            .expect("recording repository receives the fake transaction")
            .id;
        self.calls
            .lock()
            .expect("repository call lock is available")
            .push(RepositoryCall {
                operation,
                transaction_id,
                chunk_ids,
                mentions,
            });
    }
}

#[async_trait]
impl DocumentMentionRepository for RecordingRepository {
    async fn find_mentions_for_chunks(
        &self,
        _chunk_ids: Vec<Uuid>,
    ) -> Result<Vec<DocumentMention>, DocumentMentionRepositoryError> {
        Ok(Vec::new())
    }

    async fn lock_chunks_for_update(
        &self,
        transaction: &UnitOfWorkTransaction,
        chunk_ids: Vec<Uuid>,
    ) -> Result<(), DocumentMentionRepositoryError> {
        self.record_call(
            RepositoryOperation::LockChunksForUpdate,
            transaction,
            chunk_ids,
            Vec::new(),
        );
        Ok(())
    }

    async fn find_registered_stocks(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<Vec<RegisteredStock>, DocumentMentionRepositoryError> {
        self.record_call(
            RepositoryOperation::FindRegisteredStocks,
            transaction,
            Vec::new(),
            Vec::new(),
        );
        Ok(self.stocks.lock().expect("stock lock is available").clone())
    }

    async fn replace_mentions(
        &self,
        transaction: &UnitOfWorkTransaction,
        chunk_ids: Vec<Uuid>,
        mentions: Vec<DocumentMention>,
    ) -> Result<(), DocumentMentionRepositoryError> {
        self.record_call(
            RepositoryOperation::ReplaceMentions,
            transaction,
            chunk_ids,
            mentions,
        );
        Ok(())
    }
}

struct Harness {
    repository: Arc<RecordingRepository>,
    unit_of_work: Arc<FakeUnitOfWork>,
    use_cases: DocumentMentionUseCases,
}

#[fixture]
fn harness() -> Harness {
    let repository = Arc::new(RecordingRepository::default());
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let shared_unit_of_work: SharedUnitOfWork = unit_of_work.clone();
    let use_cases = DocumentMentionUseCases::new(shared_unit_of_work, repository.clone());
    Harness {
        repository,
        unit_of_work,
        use_cases,
    }
}

fn document_chunk(id: Uuid, start_offset: i32, body: &str) -> DocumentChunk {
    DocumentChunk {
        id,
        version_id: Uuid::from_u128(1),
        sequence: 0,
        start_offset,
        end_offset: start_offset + i32::try_from(body.len()).expect("body length fits in i32"),
        body: body.into(),
    }
}

async fn repository_flow(
    repository: &RecordingRepository,
    unit_of_work: &FakeUnitOfWork,
) -> RepositoryFlow {
    let calls = repository
        .calls
        .lock()
        .expect("repository call lock is available")
        .clone();
    let begun = unit_of_work.begun.lock().await.clone();
    let committed = unit_of_work.committed.lock().await.clone();
    RepositoryFlow {
        calls: calls
            .into_iter()
            .map(|call| ExpectedRepositoryCall {
                operation: call.operation,
                transaction_index: transaction_index(call.transaction_id, &begun),
                chunk_ids: call.chunk_ids,
                mentions: call.mentions,
            })
            .collect(),
        begun_transactions: normalize_transaction_ids(&begun, &begun),
        committed_transactions: normalize_transaction_ids(&committed, &begun),
    }
}

fn transaction_index(transaction_id: Uuid, transactions: &[Uuid]) -> usize {
    transactions
        .iter()
        .position(|started| *started == transaction_id)
        .expect("recorded transaction was begun")
}

fn normalize_transaction_ids(transaction_ids: &[Uuid], transactions: &[Uuid]) -> Vec<usize> {
    transaction_ids
        .iter()
        .map(|transaction_id| transaction_index(*transaction_id, transactions))
        .collect()
}

#[rstest]
#[tokio::test]
async fn record_matches_registered_names_and_keeps_original_utf8_offsets(harness: Harness) {
    harness.repository.set_stocks(vec![
        RegisteredStock {
            id: "DEMO-1".into(),
            name: "DemoCo".into(),
        },
        RegisteredStock {
            id: "DEMO-2".into(),
            name: "à\u{0315}".into(),
        },
    ]);
    let chunk = document_chunk(
        Uuid::nil(),
        5,
        "!ＤｅｍｏＣｏ|DemoCo and UnknownCo|a\u{0315}\u{0300}",
    );

    let mentions = harness
        .use_cases
        .record(&[chunk])
        .await
        .expect("document mentions are recorded");

    let expected = vec![
        DocumentMention {
            chunk_id: Uuid::nil(),
            stock_id: "DEMO-1".into(),
            start_offset: 6,
            end_offset: 24,
        },
        DocumentMention {
            chunk_id: Uuid::nil(),
            stock_id: "DEMO-1".into(),
            start_offset: 25,
            end_offset: 31,
        },
        DocumentMention {
            chunk_id: Uuid::nil(),
            stock_id: "DEMO-2".into(),
            start_offset: 46,
            end_offset: 51,
        },
    ];
    assert_eq!(
        (
            mentions,
            repository_flow(&harness.repository, &harness.unit_of_work).await
        ),
        (
            expected.clone(),
            RepositoryFlow {
                calls: vec![
                    ExpectedRepositoryCall {
                        operation: RepositoryOperation::LockChunksForUpdate,
                        transaction_index: 0,
                        chunk_ids: vec![Uuid::nil()],
                        mentions: Vec::new(),
                    },
                    ExpectedRepositoryCall {
                        operation: RepositoryOperation::FindRegisteredStocks,
                        transaction_index: 0,
                        chunk_ids: Vec::new(),
                        mentions: Vec::new(),
                    },
                    ExpectedRepositoryCall {
                        operation: RepositoryOperation::ReplaceMentions,
                        transaction_index: 0,
                        chunk_ids: vec![Uuid::nil()],
                        mentions: expected,
                    },
                ],
                begun_transactions: vec![0],
                committed_transactions: vec![0],
            },
        ),
    );
}

#[rstest]
#[case::precomposed_dakuten("デモガス")]
#[case::halfwidth_dakuten("デモｶﾞス")]
#[tokio::test]
async fn record_rejects_matches_that_end_before_a_combining_mark(
    harness: Harness,
    #[case] body: &str,
) {
    harness.repository.set_stocks(vec![RegisteredStock {
        id: "DEMO-1".into(),
        name: "デモカ".into(),
    }]);

    let mentions = harness
        .use_cases
        .record(&[document_chunk(Uuid::nil(), 0, body)])
        .await
        .expect("document mentions are recorded");
    assert_eq!(mentions, Vec::new());
}

#[rstest]
#[case::match_ends_inside_decomposed_source("デモ株式", "デモ㍿")]
#[case::match_starts_inside_decomposed_source("会社", "㍿")]
#[tokio::test]
async fn record_rejects_matches_that_start_or_end_inside_a_decomposed_character(
    harness: Harness,
    #[case] stock_name: &str,
    #[case] body: &str,
) {
    harness.repository.set_stocks(vec![RegisteredStock {
        id: "DEMO-1".into(),
        name: stock_name.into(),
    }]);

    let mentions = harness
        .use_cases
        .record(&[document_chunk(Uuid::nil(), 0, body)])
        .await
        .expect("document mentions are recorded");

    assert_eq!(mentions, Vec::new());
}

#[rstest]
#[case::ascii_prefix_inside_word("xDemoCo", "DemoCo", None)]
#[case::fullwidth_prefix_inside_word("ｘDemoCo", "DemoCo", None)]
#[case::ascii_suffix_inside_word("DemoCo7", "DemoCo", None)]
#[case::fullwidth_suffix_inside_word("DemoCo９", "DemoCo", None)]
#[case::fullwidth_name_after_ascii_prefix("xＤｅｍｏＣｏ", "ＤｅｍｏＣｏ", None)]
#[case::ascii_name_before_japanese("DemoCo工業", "DemoCo", Some((0, 6)))]
#[case::japanese_name_before_japanese("デモ工業が", "デモ工業", Some((0, 12)))]
#[tokio::test]
async fn record_applies_ascii_boundaries_without_blocking_japanese_adjacency(
    harness: Harness,
    #[case] body: &str,
    #[case] stock_name: &str,
    #[case] expected_range: Option<(i32, i32)>,
) {
    harness.repository.set_stocks(vec![RegisteredStock {
        id: "DEMO-1".into(),
        name: stock_name.into(),
    }]);

    let mentions = harness
        .use_cases
        .record(&[document_chunk(Uuid::nil(), 0, body)])
        .await
        .expect("document mentions are recorded");
    let expected = expected_range
        .map(|(start_offset, end_offset)| DocumentMention {
            chunk_id: Uuid::nil(),
            stock_id: "DEMO-1".into(),
            start_offset,
            end_offset,
        })
        .into_iter()
        .collect::<Vec<_>>();

    assert_eq!(mentions, expected);
}

#[rstest]
#[case::more_characters_wins(
    vec![
        RegisteredStock { id: "DEMO-SHORT".into(), name: "デモ".into() },
        RegisteredStock { id: "DEMO-LONG".into(), name: "デモ不動産".into() },
    ],
    "デモ不動産は",
    vec![DocumentMention { chunk_id: Uuid::nil(), stock_id: "DEMO-LONG".into(), start_offset: 0, end_offset: 15 }],
)]
#[case::source_character_count_breaks_normalized_tie(
    vec![
        RegisteredStock { id: "DEMO-GAS".into(), name: "ガス".into() },
        RegisteredStock { id: "DEMO-FRUIT".into(), name: "スイカ".into() },
    ],
    "ガスイカ",
    vec![DocumentMention { chunk_id: Uuid::nil(), stock_id: "DEMO-FRUIT".into(), start_offset: 3, end_offset: 12 }],
)]
#[tokio::test]
async fn record_keeps_only_the_longest_overlapping_name(
    harness: Harness,
    #[case] stocks: Vec<RegisteredStock>,
    #[case] body: &str,
    #[case] expected: Vec<DocumentMention>,
) {
    harness.repository.set_stocks(stocks);
    let mentions = harness
        .use_cases
        .record(&[document_chunk(Uuid::nil(), 0, body)])
        .await
        .expect("document mentions are recorded");

    assert_eq!(mentions, expected);
}

#[rstest]
#[tokio::test]
async fn record_replaces_mentions_when_registered_names_change(harness: Harness) {
    harness.repository.set_stocks(vec![RegisteredStock {
        id: "DEMO-1".into(),
        name: "Synthetic Name".into(),
    }]);
    let chunk = document_chunk(Uuid::nil(), 0, "Synthetic Name");
    let first = harness
        .use_cases
        .record(std::slice::from_ref(&chunk))
        .await
        .expect("first mention recording succeeds");
    harness.repository.set_stocks(vec![RegisteredStock {
        id: "DEMO-1".into(),
        name: "Updated Name".into(),
    }]);
    let second = harness
        .use_cases
        .record(&[chunk])
        .await
        .expect("repeated mention recording succeeds");
    assert_eq!(
        (
            first,
            second,
            repository_flow(&harness.repository, &harness.unit_of_work).await,
        ),
        (
            vec![DocumentMention {
                chunk_id: Uuid::nil(),
                stock_id: "DEMO-1".into(),
                start_offset: 0,
                end_offset: 14,
            }],
            Vec::new(),
            RepositoryFlow {
                calls: vec![
                    ExpectedRepositoryCall {
                        operation: RepositoryOperation::LockChunksForUpdate,
                        transaction_index: 0,
                        chunk_ids: vec![Uuid::nil()],
                        mentions: Vec::new(),
                    },
                    ExpectedRepositoryCall {
                        operation: RepositoryOperation::FindRegisteredStocks,
                        transaction_index: 0,
                        chunk_ids: Vec::new(),
                        mentions: Vec::new(),
                    },
                    ExpectedRepositoryCall {
                        operation: RepositoryOperation::ReplaceMentions,
                        transaction_index: 0,
                        chunk_ids: vec![Uuid::nil()],
                        mentions: vec![DocumentMention {
                            chunk_id: Uuid::nil(),
                            stock_id: "DEMO-1".into(),
                            start_offset: 0,
                            end_offset: 14,
                        }],
                    },
                    ExpectedRepositoryCall {
                        operation: RepositoryOperation::LockChunksForUpdate,
                        transaction_index: 1,
                        chunk_ids: vec![Uuid::nil()],
                        mentions: Vec::new(),
                    },
                    ExpectedRepositoryCall {
                        operation: RepositoryOperation::FindRegisteredStocks,
                        transaction_index: 1,
                        chunk_ids: Vec::new(),
                        mentions: Vec::new(),
                    },
                    ExpectedRepositoryCall {
                        operation: RepositoryOperation::ReplaceMentions,
                        transaction_index: 1,
                        chunk_ids: vec![Uuid::nil()],
                        mentions: Vec::new(),
                    },
                ],
                begun_transactions: vec![0, 1],
                committed_transactions: vec![0, 1],
            },
        ),
    );
}
