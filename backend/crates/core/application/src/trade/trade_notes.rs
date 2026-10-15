use uuid::Uuid;

use crate::note::{NoteReadUseCaseError, NoteReadUseCases, NoteSnapshot};
use crate::unit_of_work::SharedUnitOfWork;

use super::error::TradeUseCaseError;
use super::repository::SharedTradeRepository;
use super::types::NewTradeNoteLink;

#[derive(Clone)]
pub struct TradeNoteUseCases {
    unit_of_work: SharedUnitOfWork,
    trade_repository: SharedTradeRepository,
    note_read_use_cases: NoteReadUseCases,
}

impl TradeNoteUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        trade_repository: SharedTradeRepository,
        note_read_use_cases: NoteReadUseCases,
    ) -> Self {
        Self {
            unit_of_work,
            trade_repository,
            note_read_use_cases,
        }
    }

    pub async fn list(&self, trade_id: Uuid) -> Result<Vec<NoteSnapshot>, TradeUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        self.require_trade(&transaction, trade_id).await?;
        let links = self
            .trade_repository
            .list_note_links(&transaction, trade_id)
            .await?;
        self.unit_of_work.commit(transaction).await?;

        let mut snapshots = Vec::with_capacity(links.len());
        for link in links {
            snapshots.push(
                self.note_read_use_cases
                    .get_note(link.note_id, Some(link.note_version_id), false)
                    .await
                    .map_err(map_note_read_error)?,
            );
        }
        Ok(snapshots)
    }

    pub async fn list_trades_for_note(
        &self,
        note_id: Uuid,
    ) -> Result<Vec<super::types::Trade>, TradeUseCaseError> {
        self.note_read_use_cases
            .require_note(note_id)
            .await
            .map_err(map_note_read_error)?;
        self.trade_repository
            .list_trades_for_note(note_id)
            .await
            .map_err(Into::into)
    }

    pub async fn create(
        &self,
        trade_id: Uuid,
        note_id: Uuid,
    ) -> Result<super::types::TradeNoteLink, TradeUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        self.require_trade(&transaction, trade_id).await?;
        let snapshot = self
            .note_read_use_cases
            .get_note(note_id, None, false)
            .await
            .map_err(map_note_read_error)?;
        let link = self
            .trade_repository
            .insert_note_link(
                &transaction,
                NewTradeNoteLink {
                    trade_id,
                    note_id,
                    note_version_id: snapshot.version.id,
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(link)
    }

    pub async fn delete(&self, trade_id: Uuid, note_id: Uuid) -> Result<(), TradeUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        if !self
            .trade_repository
            .delete_note_link(&transaction, trade_id, note_id)
            .await?
        {
            return Err(TradeUseCaseError::ResourceNotFound(format!(
                "trade_note ({trade_id}, {note_id}) not found"
            )));
        }
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }

    async fn require_trade(
        &self,
        transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        trade_id: Uuid,
    ) -> Result<super::types::Trade, TradeUseCaseError> {
        self.trade_repository
            .find_by_id_in_transaction(transaction, trade_id)
            .await?
            .ok_or(TradeUseCaseError::NotFound(trade_id))
    }
}

fn map_note_read_error(error: NoteReadUseCaseError) -> TradeUseCaseError {
    match error {
        NoteReadUseCaseError::NotFound(message) => TradeUseCaseError::ResourceNotFound(message),
        NoteReadUseCaseError::VersionDoesNotBelong {
            note_id,
            version_id,
        } => TradeUseCaseError::ResourceNotFound(format!(
            "version {version_id} for note {note_id} not found"
        )),
        NoteReadUseCaseError::InitialVersionNotFound(note_id) => {
            TradeUseCaseError::ResourceNotFound(format!(
                "initial version for note {note_id} not found"
            ))
        }
        error => TradeUseCaseError::NoteRead(error),
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;

    use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
    use rstest::rstest;
    use rust_decimal::Decimal;

    use super::*;
    use crate::note::{FakeNoteReadQuery, NoteReadUseCases};
    use crate::trade::{FakeTradeRepository, Trade};
    use crate::unit_of_work::FakeUnitOfWork;

    const TRADE_ID: Uuid = Uuid::from_u128(1);
    const NOTE_ID: Uuid = Uuid::from_u128(2);
    const VERSION_ID: Uuid = Uuid::from_u128(3);

    fn trade() -> Trade {
        Trade {
            id: TRADE_ID,
            strategy_id: Uuid::from_u128(4),
            symbol: "FICTIONAL-ASSET".into(),
            side: "buy".into(),
            qty: Decimal::ONE,
            price: Decimal::ONE,
            fee: Decimal::ZERO,
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap_or(NaiveDate::MIN),
            source: "manual".into(),
            note: None,
            created_at: timestamp(),
            updated_at: timestamp(),
        }
    }

    fn timestamp() -> DateTime<FixedOffset> {
        DateTime::<Utc>::UNIX_EPOCH.fixed_offset()
    }

    fn build_use_cases(
        note_query: Arc<FakeNoteReadQuery>,
        trade_repository: Arc<FakeTradeRepository>,
    ) -> TradeNoteUseCases {
        TradeNoteUseCases::new(
            Arc::new(FakeUnitOfWork::new()),
            trade_repository,
            NoteReadUseCases::new(note_query),
        )
    }

    #[rstest]
    #[case::linked_trade(vec![trade()])]
    #[case::no_linked_trades(Vec::new())]
    #[tokio::test]
    async fn list_trades_for_note_returns_trades_without_a_current_version(
        #[case] expected_trades: Vec<Trade>,
    ) {
        let trade_repository = Arc::new(FakeTradeRepository::new());
        for linked_trade in &expected_trades {
            trade_repository.insert_trade(linked_trade.clone()).await;
            trade_repository
                .note_links
                .lock()
                .await
                .push(crate::trade::TradeNoteLink {
                    trade_id: linked_trade.id,
                    note_id: NOTE_ID,
                    note_version_id: VERSION_ID,
                    created_at: timestamp(),
                });
        }
        let use_cases = build_use_cases(
            Arc::new(FakeNoteReadQuery::new_unreviewed(NOTE_ID, VERSION_ID)),
            trade_repository,
        );

        assert_eq!(
            use_cases
                .list_trades_for_note(NOTE_ID)
                .await
                .expect("an existing note can list linked trades without a current version"),
            expected_trades,
        );
    }

    #[tokio::test]
    async fn list_trades_for_note_returns_not_found_for_an_unknown_note() {
        let unknown_note_id = Uuid::from_u128(99);
        let use_cases = build_use_cases(
            Arc::new(FakeNoteReadQuery::new(NOTE_ID, VERSION_ID)),
            Arc::new(FakeTradeRepository::new()),
        );

        assert_eq!(
            use_cases
                .list_trades_for_note(unknown_note_id)
                .await
                .map_err(|error| error.to_string()),
            Err(format!("note {unknown_note_id} not found")),
        );
    }

    #[tokio::test]
    async fn create_links_a_note_to_a_strategy_trade() {
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let trade_repository = Arc::new(FakeTradeRepository::new());
        trade_repository.insert_trade(trade()).await;
        let note_reads =
            NoteReadUseCases::new(Arc::new(FakeNoteReadQuery::new(NOTE_ID, VERSION_ID)));
        let use_cases = TradeNoteUseCases::new(unit_of_work, trade_repository.clone(), note_reads);

        let link = use_cases
            .create(TRADE_ID, NOTE_ID)
            .await
            .expect("the note can be linked to the trade");
        let mut normalized_link = link;
        normalized_link.created_at = timestamp();
        let mut stored_links = trade_repository.note_links.lock().await.clone();
        for stored_link in &mut stored_links {
            stored_link.created_at = timestamp();
        }

        assert_eq!(
            (normalized_link, stored_links),
            (
                crate::trade::TradeNoteLink {
                    trade_id: TRADE_ID,
                    note_id: NOTE_ID,
                    note_version_id: VERSION_ID,
                    created_at: timestamp(),
                },
                vec![crate::trade::TradeNoteLink {
                    trade_id: TRADE_ID,
                    note_id: NOTE_ID,
                    note_version_id: VERSION_ID,
                    created_at: timestamp(),
                }],
            ),
        );
    }

    #[tokio::test]
    async fn create_requires_a_current_note_version() {
        let trade_repository = Arc::new(FakeTradeRepository::new());
        trade_repository.insert_trade(trade()).await;
        let use_cases = build_use_cases(
            Arc::new(FakeNoteReadQuery::new_unreviewed(NOTE_ID, VERSION_ID)),
            trade_repository,
        );

        assert_eq!(
            use_cases
                .create(TRADE_ID, NOTE_ID)
                .await
                .map_err(|error| error.to_string()),
            Err(format!("current version for note {NOTE_ID} not found")),
        );
    }
}
