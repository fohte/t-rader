use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use uuid::Uuid;

use crate::document_storage::DocumentChunk;

use super::super::{
    DocumentMention, DocumentMentionRepository, DocumentMentionRepositoryError, RegisteredStock,
};
use super::DocumentMentionUseCases;

#[derive(Default)]
struct RecordingRepository {
    stocks: Vec<RegisteredStock>,
    saved_mentions: Mutex<Vec<DocumentMention>>,
}

#[async_trait]
impl DocumentMentionRepository for RecordingRepository {
    async fn find_registered_stocks(
        &self,
    ) -> Result<Vec<RegisteredStock>, DocumentMentionRepositoryError> {
        Ok(self.stocks.clone())
    }

    async fn save_mentions(
        &self,
        mentions: Vec<DocumentMention>,
    ) -> Result<(), DocumentMentionRepositoryError> {
        self.saved_mentions
            .lock()
            .expect("recording repository lock is available")
            .extend(mentions);
        Ok(())
    }
}

#[tokio::test]
async fn record_matches_registered_names_and_keeps_original_utf8_offsets() {
    let repository = Arc::new(RecordingRepository {
        stocks: vec![RegisteredStock {
            id: "DEMO-1".into(),
            name: "DemoCo".into(),
        }],
        ..RecordingRepository::default()
    });
    let use_cases = DocumentMentionUseCases::new(repository.clone());
    let chunk = DocumentChunk {
        id: Uuid::nil(),
        version_id: Uuid::from_u128(1),
        sequence: 0,
        start_offset: 5,
        end_offset: 45,
        body: "xＤｅｍｏＣｏ|DemoCo and UnknownCo".into(),
    };

    let mentions = use_cases
        .record(&[chunk])
        .await
        .expect("document mentions are recorded");

    assert_eq!(
        (
            mentions,
            repository.saved_mentions.lock().expect("lock").clone()
        ),
        (
            vec![
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
            ],
            vec![
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
            ],
        ),
    );
}
