use std::collections::HashMap;

use uuid::Uuid;

use crate::note::NoteSnapshot;
use crate::note::SharedNoteRepository;
use crate::unit_of_work::SharedUnitOfWork;

use super::error::TradeUseCaseError;
use super::repository::SharedTradeRepository;
use super::types::NewTradeNoteLink;

#[derive(Clone)]
pub struct TradeNoteUseCases {
    unit_of_work: SharedUnitOfWork,
    trade_repository: SharedTradeRepository,
    note_repository: SharedNoteRepository,
}

impl TradeNoteUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        trade_repository: SharedTradeRepository,
        note_repository: SharedNoteRepository,
    ) -> Self {
        Self {
            unit_of_work,
            trade_repository,
            note_repository,
        }
    }

    pub async fn list(&self, trade_id: Uuid) -> Result<Vec<NoteSnapshot>, TradeUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        self.require_trade(&transaction, trade_id).await?;
        let links = self
            .trade_repository
            .list_note_links(&transaction, trade_id)
            .await?;
        if links.is_empty() {
            self.unit_of_work.commit(transaction).await?;
            return Ok(Vec::new());
        }

        let note_ids: Vec<Uuid> = links.iter().map(|link| link.note_id).collect();
        let version_ids: Vec<Uuid> = links.iter().map(|link| link.note_version_id).collect();
        let notes = self
            .note_repository
            .find_notes_by_ids(&transaction, &note_ids)
            .await?
            .into_iter()
            .map(|note| (note.id, note))
            .collect::<HashMap<_, _>>();
        let versions = self
            .note_repository
            .find_versions_by_ids(&transaction, &version_ids)
            .await?
            .into_iter()
            .map(|version| (version.id, version))
            .collect::<HashMap<_, _>>();
        let creators = self
            .note_repository
            .find_initial_created_by_kind_by_note_ids(&transaction, &note_ids)
            .await?;

        let snapshots = links
            .into_iter()
            .map(|link| {
                let note = notes.get(&link.note_id).cloned().ok_or_else(|| {
                    TradeUseCaseError::ResourceNotFound(format!("note {} not found", link.note_id))
                })?;
                let version = versions
                    .get(&link.note_version_id)
                    .cloned()
                    .filter(|version| version.note_id == link.note_id)
                    .ok_or_else(|| {
                        TradeUseCaseError::ResourceNotFound(format!(
                            "version {} for note {} not found",
                            link.note_version_id, link.note_id
                        ))
                    })?;
                let created_by_kind = creators.get(&link.note_id).cloned().ok_or_else(|| {
                    TradeUseCaseError::ResourceNotFound(format!(
                        "initial version for note {} not found",
                        link.note_id
                    ))
                })?;
                Ok(NoteSnapshot {
                    note,
                    version,
                    created_by_kind,
                })
            })
            .collect::<Result<Vec<_>, TradeUseCaseError>>()?;

        self.unit_of_work.commit(transaction).await?;
        Ok(snapshots)
    }

    pub async fn create(
        &self,
        trade_id: Uuid,
        note_id: Uuid,
    ) -> Result<super::types::TradeNoteLink, TradeUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let trade = self.require_trade(&transaction, trade_id).await?;
        let note = self
            .note_repository
            .find_note(&transaction, note_id)
            .await?;
        match note {
            Some(note) if note.strategy_id == Some(trade.strategy_id) => {}
            _ => {
                return Err(TradeUseCaseError::Validation(
                    "note_id must belong to the same strategy as the trade".into(),
                ));
            }
        }
        let version = self
            .note_repository
            .find_current_version(&transaction, note_id)
            .await?
            .ok_or_else(|| {
                TradeUseCaseError::ResourceNotFound(format!(
                    "current version for note {note_id} not found"
                ))
            })?;
        let link = self
            .trade_repository
            .insert_note_link(
                &transaction,
                NewTradeNoteLink {
                    trade_id,
                    note_id,
                    note_version_id: version.id,
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
