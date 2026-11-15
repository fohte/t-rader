#![allow(
    clippy::expect_used,
    reason = "use case test harness setup failures should report their cause"
)]

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rstest::{fixture, rstest};
use uuid::Uuid;

use crate::document_storage::DocumentChunk;
use crate::unit_of_work::{FakeTransaction, FakeUnitOfWork, SharedUnitOfWork};

use super::super::{
    DocumentMention, DocumentMentionRepository, DocumentMentionRepositoryError, RegisteredStock,
};
use super::DocumentMentionUseCases;

#[derive(Debug, PartialEq, Eq)]
struct Replacement {
    transaction_id: Option<Uuid>,
    chunk_ids: Vec<Uuid>,
    mentions: Vec<DocumentMention>,
}

#[derive(Default)]
struct RecordingRepository {
    stocks: Mutex<Vec<RegisteredStock>>,
    stock_query_transactions: Mutex<Vec<Option<Uuid>>>,
    replacements: Mutex<Vec<Replacement>>,
}

impl RecordingRepository {
    fn set_stocks(&self, stocks: Vec<RegisteredStock>) {
        *self.stocks.lock().expect("stock lock is available") = stocks;
    }
}

#[async_trait]
impl DocumentMentionRepository for RecordingRepository {
    async fn find_registered_stocks(
        &self,
        transaction: &crate::unit_of_work::UnitOfWorkTransaction,
    ) -> Result<Vec<RegisteredStock>, DocumentMentionRepositoryError> {
        self.stock_query_transactions
            .lock()
            .expect("query transaction lock is available")
            .push(
                transaction
                    .downcast_ref::<FakeTransaction>()
                    .map(|transaction| transaction.id),
            );
        Ok(self.stocks.lock().expect("stock lock is available").clone())
    }

    async fn replace_mentions(
        &self,
        transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        chunk_ids: Vec<Uuid>,
        mentions: Vec<DocumentMention>,
    ) -> Result<(), DocumentMentionRepositoryError> {
        self.replacements
            .lock()
            .expect("replacement lock is available")
            .push(Replacement {
                transaction_id: transaction
                    .downcast_ref::<FakeTransaction>()
                    .map(|transaction| transaction.id),
                chunk_ids,
                mentions,
            });
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

async fn transaction_flow_matches(
    repository: &RecordingRepository,
    unit_of_work: &FakeUnitOfWork,
) -> bool {
    let stock_queries = repository
        .stock_query_transactions
        .lock()
        .expect("query transaction lock is available")
        .clone();
    let replacements = repository
        .replacements
        .lock()
        .expect("replacement lock is available")
        .iter()
        .map(|replacement| replacement.transaction_id)
        .collect::<Vec<_>>();
    let begun = unit_of_work.begun.lock().await.clone();
    let committed = unit_of_work.committed.lock().await.clone();
    stock_queries == replacements
        && stock_queries.iter().all(Option::is_some)
        && begun == committed
        && begun.len() == stock_queries.len()
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
    let replacements = harness
        .repository
        .replacements
        .lock()
        .expect("replacement lock is available")
        .iter()
        .map(|replacement| (replacement.chunk_ids.clone(), replacement.mentions.clone()))
        .collect::<Vec<_>>();

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
            replacements,
            transaction_flow_matches(&harness.repository, &harness.unit_of_work).await,
        ),
        (expected.clone(), vec![(vec![Uuid::nil()], expected)], true),
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
    let replacement_mentions = harness.repository.replacements.lock().expect("lock")[0]
        .mentions
        .clone();

    assert_eq!((mentions, replacement_mentions), (Vec::new(), Vec::new()));
}

#[rstest]
#[tokio::test]
async fn record_rejects_matches_that_end_inside_a_decomposed_character(harness: Harness) {
    harness.repository.set_stocks(vec![RegisteredStock {
        id: "DEMO-1".into(),
        name: "f".into(),
    }]);

    let mentions = harness
        .use_cases
        .record(&[document_chunk(Uuid::nil(), 0, "ﬃ")])
        .await
        .expect("document mentions are recorded");

    assert_eq!(mentions, Vec::new());
}

#[rstest]
#[case::ascii_prefix_inside_word("xDemoCo", "DemoCo", None)]
#[case::ascii_suffix_inside_word("DemoCo7", "DemoCo", None)]
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
#[tokio::test]
async fn record_keeps_only_the_longest_overlapping_name(harness: Harness) {
    harness.repository.set_stocks(vec![
        RegisteredStock {
            id: "DEMO-SHORT".into(),
            name: "デモ".into(),
        },
        RegisteredStock {
            id: "DEMO-LONG".into(),
            name: "デモ不動産".into(),
        },
    ]);

    let mentions = harness
        .use_cases
        .record(&[document_chunk(Uuid::nil(), 0, "デモ不動産は")])
        .await
        .expect("document mentions are recorded");

    assert_eq!(
        mentions,
        vec![DocumentMention {
            chunk_id: Uuid::nil(),
            stock_id: "DEMO-LONG".into(),
            start_offset: 0,
            end_offset: 15,
        }],
    );
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
    let replacements = harness
        .repository
        .replacements
        .lock()
        .expect("replacement lock is available")
        .iter()
        .map(|replacement| (replacement.chunk_ids.clone(), replacement.mentions.clone()))
        .collect::<Vec<_>>();

    assert_eq!(
        (
            first,
            second,
            replacements,
            transaction_flow_matches(&harness.repository, &harness.unit_of_work).await,
        ),
        (
            vec![DocumentMention {
                chunk_id: Uuid::nil(),
                stock_id: "DEMO-1".into(),
                start_offset: 0,
                end_offset: 14,
            }],
            Vec::new(),
            vec![
                (
                    vec![Uuid::nil()],
                    vec![DocumentMention {
                        chunk_id: Uuid::nil(),
                        stock_id: "DEMO-1".into(),
                        start_offset: 0,
                        end_offset: 14,
                    }],
                ),
                (vec![Uuid::nil()], Vec::new()),
            ],
            true,
        ),
    );
}
