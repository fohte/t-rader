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
        let trade = self.require_trade(&transaction, trade_id).await?;
        let note_strategy_id = match self.note_read_use_cases.get_note_strategy_id(note_id).await {
            Ok(strategy_id) => strategy_id,
            Err(NoteReadUseCaseError::NotFound(_)) => None,
            Err(error) => return Err(error.into()),
        };
        if note_strategy_id != Some(trade.strategy_id) {
            return Err(TradeUseCaseError::Validation(
                "note_id must belong to the same strategy as the trade".into(),
            ));
        }
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
