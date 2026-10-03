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
                    .get_note(link.note_id, Some(link.note_version_id), false, None)
                    .await
                    .map_err(map_note_read_error)?,
            );
        }
        Ok(snapshots)
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
            .get_note(note_id, None, false, None)
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

    use async_trait::async_trait;
    use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
    use rust_decimal::Decimal;
    use serde_json::json;

    use super::*;
    use crate::note::{
        Note, NoteLink, NoteLinks, NoteListPage, NoteListQuery, NoteReadQuery, NoteReadQueryError,
        NoteVersion,
    };
    use crate::trade::{FakeTradeRepository, Trade};
    use crate::unit_of_work::FakeUnitOfWork;

    const TRADE_ID: Uuid = Uuid::from_u128(1);
    const NOTE_ID: Uuid = Uuid::from_u128(2);
    const VERSION_ID: Uuid = Uuid::from_u128(3);

    struct UnassignedNoteQuery;

    #[async_trait]
    impl NoteReadQuery for UnassignedNoteQuery {
        async fn find_note(&self, note_id: Uuid) -> Result<Option<Note>, NoteReadQueryError> {
            Ok((note_id == NOTE_ID).then(note))
        }

        async fn find_note_for_version(
            &self,
            version_id: Uuid,
        ) -> Result<Option<Note>, NoteReadQueryError> {
            Ok((version_id == VERSION_ID).then(note))
        }

        async fn find_note_version(
            &self,
            note_id: Uuid,
            version_id: Option<Uuid>,
            _use_latest_if_no_current: bool,
        ) -> Result<Option<NoteVersion>, NoteReadQueryError> {
            Ok((note_id == NOTE_ID && version_id.is_none()).then(version))
        }

        async fn find_initial_created_by_kind(
            &self,
            note_id: Uuid,
        ) -> Result<Option<String>, NoteReadQueryError> {
            Ok((note_id == NOTE_ID).then(|| "human".into()))
        }

        async fn list_note_versions(
            &self,
            note_id: Uuid,
        ) -> Result<Vec<NoteVersion>, NoteReadQueryError> {
            Ok((note_id == NOTE_ID).then(version).into_iter().collect())
        }

        async fn find_note_version_by_number(
            &self,
            note_id: Uuid,
            version_no: i32,
        ) -> Result<Option<NoteVersion>, NoteReadQueryError> {
            Ok((note_id == NOTE_ID && version_no == 1).then(version))
        }

        async fn list_pending_note_versions(&self) -> Result<Vec<NoteVersion>, NoteReadQueryError> {
            Ok(Vec::new())
        }

        async fn list_notes(
            &self,
            _query: NoteListQuery,
        ) -> Result<NoteListPage, NoteReadQueryError> {
            Ok(NoteListPage {
                notes: Vec::new(),
                cursor: None,
                has_more: false,
            })
        }

        async fn find_links_from_version(
            &self,
            _version_id: Uuid,
        ) -> Result<Vec<NoteLink>, NoteReadQueryError> {
            Ok(Vec::new())
        }

        async fn list_note_links(
            &self,
            _note_id: Uuid,
            _source_version_id: Uuid,
        ) -> Result<NoteLinks, NoteReadQueryError> {
            Ok(NoteLinks {
                outgoing: Vec::new(),
                incoming: Vec::new(),
            })
        }
    }

    fn note() -> Note {
        Note {
            id: NOTE_ID,
            strategy_id: None,
            kind: None,
            trigger: None,
            trigger_label: None,
            created_at: timestamp(),
            updated_at: timestamp(),
            execution_id: None,
        }
    }

    fn version() -> NoteVersion {
        NoteVersion {
            id: VERSION_ID,
            note_id: NOTE_ID,
            version_no: 1,
            title: "Example note".into(),
            body_md: "body".into(),
            frontmatter_json: json!({}),
            graphs_json: json!([]),
            status: "approved".into(),
            is_current: true,
            change_reason: None,
            created_by_kind: "human".into(),
            execution_id: None,
            created_at: timestamp(),
            reviewed_at: None,
        }
    }

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

    #[tokio::test]
    async fn create_links_an_unassigned_note_to_a_strategy_trade() {
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let trade_repository = Arc::new(FakeTradeRepository::new());
        trade_repository.insert_trade(trade()).await;
        let note_reads = NoteReadUseCases::new(Arc::new(UnassignedNoteQuery));
        let use_cases = TradeNoteUseCases::new(unit_of_work, trade_repository.clone(), note_reads);

        let link = use_cases
            .create(TRADE_ID, NOTE_ID)
            .await
            .expect("an unassigned note can be linked to the trade");
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
}
